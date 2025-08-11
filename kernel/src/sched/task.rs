// sched/task.rs
//
// NØNOS task core (kernel threads; ring0)
// - Strong TaskId allocator, priorities, CPU affinity (future SMP)
// - Guard-paged stacks + per-task canaries (deterministic from boot nonce)
// - Runtime stats (voluntary/involuntary switches, cpu time)
// - Safe states: New → Runnable ↔ Running ↔ {Sleeping,Blocked} → Dying → Dead
// - kspawn(entry,arg,prio,aff) creates a kernel thread; task_exit() finalizes
// - Proof audit on create/exit + stack map/unmap (no secrets, public commit)
//
// Zero-state: Nothing is persisted; TaskIds are monotonic per-boot only.

#![allow(dead_code)]

use core::sync::atomic::{AtomicU64, AtomicU32, AtomicUsize, AtomicU8, Ordering};
use core::ptr::NonNull;
use spin::{Mutex, Once};

use crate::sched::ctx::{Context, EntryFn, init_context};
use crate::memory::layout::{KSTACK_SIZE, GUARD_PAGES, PAGE_SIZE};
use crate::memory::virt::{self, VmFlags};
use crate::memory::proof::{self, CapTag};
use crate::memory::kaslr;
use crate::arch::x86_64::interrupt::apic;

// ───────────────────────────── IDs, priority, affinity ─────────────────────────

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct TaskId(u64);

#[derive(Clone, Copy, Debug)]
pub enum Priority {
    Realtime = 0, High = 1, Normal = 2, Low = 3, Idle = 4,
}

bitflags::bitflags! {
    pub struct Affinity: u64 {
        const ANY = u64::MAX;
        // When SMP lands, we’ll set bit=APIC ID; for BSP-only, ANY==BSP.
    }
}

// ───────────────────────────────── Task states ─────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    New       = 0,
    Runnable  = 1,
    Running   = 2,
    Sleeping  = 3,
    Blocked   = 4,
    Dying     = 5,
    Dead      = 6,
}

// ───────────────────────────── Task structure ──────────────────────────────────

pub struct Task {
    pub id: TaskId,
    pub prio: Priority,
    pub aff: Affinity,

    // Stack: [guard][ … KSTACK … ] (top grows down)
    pub stack_top: u64,
    pub stack_base: u64, // first byte of usable stack (above guard)
    pub canary: u64,     // placed near bottom of stack (checked on switch)

    pub ctx: Context,

    // Accounting
    pub switches_vol: AtomicU64, // voluntary
    pub switches_inv: AtomicU64, // involuntary (preempt)
    pub ns_exec: AtomicU64,      // total run time (ns)

    // State
    pub state: AtomicU8,         // State as u8
}

impl Task {
    fn new(id: TaskId, prio: Priority, aff: Affinity) -> Self {
        Self {
            id, prio, aff,
            stack_top: 0, stack_base: 0, canary: 0,
            ctx: Context::default(),
            switches_vol: AtomicU64::new(0),
            switches_inv: AtomicU64::new(0),
            ns_exec: AtomicU64::new(0),
            state: AtomicU8::new(State::New as u8),
        }
    }
    #[inline] pub fn state(&self) -> State { unsafe { core::mem::transmute(self.state.load(Ordering::Acquire)) } }
    #[inline] pub fn set_state(&self, s: State) { self.state.store(s as u8, Ordering::Release); }
}

// ─────────────────────────── Global registry (BSP) ─────────────────────────────

const MAX_TASKS: usize = 4096;
static TASKS: Mutex<heapless::FnvIndexMap<TaskId, NonNull<Task>, MAX_TASKS>> = Mutex::new(heapless::FnvIndexMap::new());
static NEXT_TID: AtomicU64 = AtomicU64::new(1);
static BOOT_CANARY_SALT: Once<u64> = Once::new();

fn alloc_tid() -> TaskId {
    let raw = NEXT_TID.fetch_add(1, Ordering::Relaxed);
    TaskId(((kaslr::boot_nonce() as u128 ^ (raw as u128)) & 0xffff_ffff_ffff_ffff) as u64)
}

fn boot_canary_salt() -> u64 {
    *BOOT_CANARY_SALT.call_once(|| {
        // Derive from boot nonce using a small PRF so canaries differ per boot (public).
        let mut out = [0u8; 8];
        crate::memory::kaslr::derive_subkey(b"STACK-CANARY", b"kernel", &mut out, &kaslr::init(Default::default()).transcript_hash);
        u64::from_le_bytes(out)
    })
}

// ───────────────────────────── Stack allocation ────────────────────────────────

struct Stack {
    base: u64, // VA of first usable byte (after guard page)
    top:  u64, // VA of top (aligned)
    pages: usize,
}

unsafe fn alloc_stack(pages: usize) -> Stack {
    // Map guard + stack pages RW|NX|GLOBAL; guard left **unmapped**.
    let guard = 1;
    let total = guard + pages;

    extern "Rust" { fn __nonos_alloc_kvm_va(pages: usize) -> u64; }
    let va = __nonos_alloc_kvm_va(total);
    let guard_va = va; // first page: guard
    let stack_va = va + (PAGE_SIZE as u64);

    // Map stack pages
    for i in 0..pages {
        let p = crate::memory::phys::PhysAllocator::alloc().expect("stack phys");
        virt::map4k_at(
            x86_64::VirtAddr::new(stack_va + (i as u64) * PAGE_SIZE as u64),
            x86_64::PhysAddr::new(p.0),
            VmFlags::RW | VmFlags::NX | VmFlags::GLOBAL,
        ).expect("stack map");
        proof::audit_map(stack_va + (i as u64) * PAGE_SIZE as u64, p.0, PAGE_SIZE as u64, (VmFlags::RW | VmFlags::NX | VmFlags::GLOBAL).bits(), CapTag::KERNEL);
    }

    let top = stack_va + (pages as u64) * PAGE_SIZE as u64;
    // Leave `guard_va` unmapped (faults on underflow)

    Stack { base: stack_va, top, pages }
}

unsafe fn free_stack(stk: &Stack) {
    for i in 0..stk.pages {
        let va = stk.base + (i as u64) * PAGE_SIZE as u64;
        if let Some(pa) = virt::unmap4k(x86_64::VirtAddr::new(va)) {
            crate::memory::phys::PhysAllocator::free(crate::memory::phys::Frame(pa.as_u64()));
            proof::audit_unmap(va, pa.as_u64(), PAGE_SIZE as u64, CapTag::KERNEL);
        }
    }
    // Guard page VA region remains reserved by VA allocator; optional reclaim later.
}

// ───────────────────────────── Task creation API ───────────────────────────────

pub fn kspawn(name: &'static str, entry: EntryFn, arg: usize, prio: Priority, aff: Affinity) -> TaskId {
    let id = alloc_tid();

    // Allocate control block
    let boxed = unsafe {
        // Permanent kernel allocation; zeroed for predictability.
        use core::alloc::Layout;
        let layout = Layout::new::<Task>();
        let p = crate::memory::alloc::kmem_alloc_zero(layout.size(), layout.align()).expect("task alloc");
        NonNull::new(p as *mut Task).expect("nn")
    };
    let t = unsafe { &mut *boxed.as_ptr() };
    *t = Task::new(id, prio, aff);

    // Stack (64 KiB default)
    let pages = (KSTACK_SIZE / PAGE_SIZE).max(2);
    let stk = unsafe { alloc_stack(pages) };
    t.stack_base = stk.base;
    t.stack_top  = stk.top;

    // Canary at lowest dword of usable stack
    let canary = kaslr::boot_nonce() ^ boot_canary_salt() ^ (id.0.rotate_left(13));
    t.canary = canary;
    unsafe {
        let ptr = t.stack_base as *mut u64;
        core::ptr::write_volatile(ptr, canary);
    }

    // Context init
    unsafe {
        init_context(
            &mut t.ctx,
            t.stack_top,
            entry,
            arg,
            task_exit_trampoline,
        );
    }

    // Register in task table
    {
        let mut tab = TASKS.lock();
        let _ = tab.insert(id, boxed);
    }

    // Push into runqueue
    crate::sched::runqueue::enqueue(id, prio);

    proof::audit_phys_alloc(0xTASK_NEW, ((id.0 as u64) << 8) | (prio as u64), CapTag::KERNEL);
    crate::log::logger::try_get_logger().map(|l| l.log(&format!("[TASK] spawn '{}' tid={:?} prio={:?}", name, id, prio)));

    id
}

/// Called when a task function returns, or explicitly via `task_exit()`.
#[no_mangle]
pub extern "C" fn task_exit_trampoline() -> ! {
    task_exit()
}

/// Terminate the current task; frees stack and removes from table.
/// Never returns.
pub fn task_exit() -> ! {
    let tid = current();
    let t = with_task(tid, |task| {
        // Check canary
        unsafe {
            let got = core::ptr::read_volatile(task.stack_base as *const u64);
            if got != task.canary {
                crate::panic::panic_with("[TASK] stack canary corrupt");
            }
        }
        task.set_state(State::Dying);
        (task.stack_base, task.stack_top, task.id)
    });

    // Unmap stack
    unsafe {
        let pages = (KSTACK_SIZE / PAGE_SIZE).max(2);
        free_stack(&Stack { base: t.0, top: t.1, pages });
    }

    // Remove from runqueue and table
    crate::sched::runqueue::dequeue(t.2);
    {
        let mut tab = TASKS.lock();
        if let Some((_, blk)) = tab.remove(&t.2) {
            // Free TCB memory
            use core::alloc::Layout;
            let layout = Layout::new::<Task>();
            unsafe { crate::memory::alloc::kmem_free(blk.as_ptr() as *mut u8, layout.size(), layout.align()); }
        }
    }

    proof::audit_phys_alloc(0xTASK_END, t.2 .0, CapTag::KERNEL);

    // Switch to idle
    crate::sched::switch_to_idle();
}

/// Helper: run closure with &mut Task from table.
fn with_task<R>(tid: TaskId, f: impl FnOnce(&mut Task) -> R) -> R {
    let mut tab = TASKS.lock();
    let blk = *tab.get(&tid).expect("task exists");
    let t = unsafe { &mut *blk.as_ptr() };
    f(t)
}

// —──────────────────────────── Scheduler touchpoints —──────────────────────────

/// Called by scheduler when it is about to run a task on a CPU.
pub fn on_run_start(tid: TaskId, now_ns: u64) {
    with_task(tid, |t| {
        t.set_state(State::Running);
        // store start time in rbp scratch (cheap trick) or extend Task with last_start_ns
        let _ = now_ns;
    });
}

/// Called by scheduler when a task yields or gets preempted.
pub fn on_run_end(tid: TaskId, ran_ns: u64, involuntary: bool) {
    with_task(tid, |t| {
        t.ns_exec.fetch_add(ran_ns, Ordering::Relaxed);
        if involuntary { t.switches_inv.fetch_add(1, Ordering::Relaxed); }
        else { t.switches_vol.fetch_add(1, Ordering::Relaxed); }
        t.set_state(State::Runnable);
    });
}

/// Change priority at runtime.
pub fn set_priority(tid: TaskId, prio: Priority) {
    with_task(tid, |t| t.prio = prio);
    crate::sched::runqueue::reprioritize(tid, prio);
}

/// Change CPU affinity at runtime.
pub fn set_affinity(tid: TaskId, aff: Affinity) {
    with_task(tid, |t| t.aff = aff);
    // runqueue may migrate later when SMP lands
}

// ───────────────────────────── Current task helpers ────────────────────────────

/// BSP-only for now; later read from PERCPU.
pub fn current() -> TaskId {
    crate::sched::runqueue::current_tid()
}

/// Direct access (debug/CLI).
pub fn get(tid: TaskId) -> Option<&'static Task> {
    let tab = TASKS.lock();
    tab.get(&tid).map(|p| unsafe { &*p.as_ptr() })
}

pub(crate) fn __nonos_task_table_get_mut(tid: TaskId) -> Option<core::ptr::NonNull<Task>> {
    let mut tab = TASKS.lock();
    tab.get(&tid).cloned()
}
