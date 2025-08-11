// sched/mod.rs
//
// NØNOS scheduler harness (BSP, preemptive via APIC timer)
// - Idle thread + main loop
// - tick() from timer IRQ: account, slice, pick-next, context switch
// - O(1) runqueue glue (see runqueue.rs)
// - Context switching via ctx::switch (non-preemptible switching window)
// - NEED_RESCHED flag for deferred preemption (if you want to switch outside IRQ)
// - Proof taps on major transitions
//
// Safety: tick() is called with IRQs disabled (from the timer handler).

#![allow(dead_code)]

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use spin::Mutex;

use crate::arch::x86_64::time::timer;
use crate::memory::proof::{self, CapTag};

use crate::sched::ctx::{self, Context, EntryFn};
use crate::sched::runqueue as rq;
use crate::sched::task::{self, TaskId, Priority, State};

static STARTED: AtomicBool = AtomicBool::new(false);

// “need reschedule” hint set by tick(); useful if you prefer to switch outside IRQ.
static NEED_RESCHED: AtomicBool = AtomicBool::new(false);

// Accounting for current task (BSP)
static CUR_TID: AtomicU64 = AtomicU64::new(0);      // mirrors rq::current_tid()
static CUR_SLICE_END_NS: AtomicU64 = AtomicU64::new(0);
static CUR_START_NS:     AtomicU64 = AtomicU64::new(0);

// Dedicated idle task context (its Task owns its own Context too; this one is only for safety).
static IDLECTX: UnsafeCell<Context> = UnsafeCell::new(Context::default());
static IDLE_TID: Mutex<Option<TaskId>> = Mutex::new(None);

#[inline] fn now_ns() -> u64 { timer::now_ns() }

// —————————————————— public API ——————————————————

pub fn init() {
    if STARTED.swap(true, Ordering::SeqCst) { return; }

    // Spawn idle first; it will HLT in a loop.
    let idle_tid = task::kspawn("idle", idle_entry, 0, Priority::Idle, task::Affinity::ANY);
    *IDLE_TID.lock() = Some(idle_tid);

    // Set current to idle.
    rq::set_current(Some(idle_tid));
    CUR_TID.store(idle_tid.0, Ordering::Relaxed);
    let t0 = now_ns();
    CUR_START_NS.store(t0, Ordering::Relaxed);
    CUR_SLICE_END_NS.store(t0, Ordering::Relaxed); // idle is cooperative (slice=0)

    proof::audit_phys_alloc(0xSCHED_INIT, idle_tid.0, CapTag::KERNEL);
}

/// Enter scheduling loop (never returns). In BSP-only demo we just let idle run.
/// Keep this for completeness; real work happens on IRQ ticks and kthreads.
pub fn enter() -> ! {
    loop {
        // idle thread will HLT; here we just yield the CPU to it regularly
        idle_yield_once();
    }
}

/// Timer IRQ hook (called by IDT handler for APIC timer).
/// Decide preemption and switch if a better runnable exists.
pub fn tick() {
    // 1) Update time and current task accounting.
    let now = now_ns();
    let cur_tid = rq::current_tid();
    let is_idle = Some(cur_tid) == IDLE_TID.lock().as_ref().copied();

    // Compute whether slice expired (idle has 0ms slice → no preempt unless runnable exists).
    let slice_expired = if is_idle {
        true
    } else {
        now >= CUR_SLICE_END_NS.load(Ordering::Relaxed)
    };

    // 2) If slice expired (or higher prio waiting), pick next.
    if slice_expired {
        // Account runtime of current
        let started = CUR_START_NS.swap(now, Ordering::Relaxed);
        let ran = now.saturating_sub(started);
        if cur_tid.0 != 0 {
            task::on_run_end(cur_tid, ran, /*involuntary=*/true);
        }

        // Put current back if not idle
        if !is_idle {
            // Query current prio (from task table)
            let prio = task_prio(cur_tid);
            rq::rotate_after_run(cur_tid, prio);
        }

        // Pick next runnable (or None → run idle)
        let next = rq::pick_next()
            .map(|(tid, prio)| (tid, prio))
            .or_else(|| IDLE_TID.lock().map(|t| (t, Priority::Idle)));

        if let Some((next_tid, next_prio)) = next {
            // Program new slice window
            let tnow = now_ns();
            CUR_START_NS.store(tnow, Ordering::Relaxed);
            let slice_ms = rq::timeslice_ms_for(next_prio);
            let end = if slice_ms == 0 { tnow } else { tnow + (slice_ms as u64) * 1_000_000 };
            CUR_SLICE_END_NS.store(end, Ordering::Relaxed);

            // Switch contexts: from current task -> next task.
            // Safety: timer IRQ runs with IF=0; ctx::switch is non-preemptible.
            unsafe { context_switch(cur_tid, next_tid); }
        } else {
            // No runnable at all (shouldn’t happen; idle always exists).
            NEED_RESCHED.store(true, Ordering::Relaxed);
        }
    } else {
        // Not expired; nothing to do.
        NEED_RESCHED.store(false, Ordering::Relaxed);
    }
}

/// Manual reschedule point (for cooperative yield or after wakeups).
/// Can be called with IRQs disabled from safe contexts.
pub fn schedule_now() {
    let now = now_ns();
    let cur_tid = rq::current_tid();

    // Account
    let started = CUR_START_NS.swap(now, Ordering::Relaxed);
    let ran = now.saturating_sub(started);
    if cur_tid.0 != 0 {
        task::on_run_end(cur_tid, ran, /*involuntary=*/false);
    }

    // Enqueue current at tail (unless idle)
    if cur_tid != idle_tid() {
        let prio = task_prio(cur_tid);
        rq::rotate_after_run(cur_tid, prio);
    }

    // Pick next
    let next = rq::pick_next()
        .map(|(tid, prio)| (tid, prio))
        .unwrap_or((idle_tid(), Priority::Idle));

    let tnow = now_ns();
    CUR_START_NS.store(tnow, Ordering::Relaxed);
    let slice_ms = rq::timeslice_ms_for(next.1);
    let end = if slice_ms == 0 { tnow } else { tnow + (slice_ms as u64) * 1_000_000 };
    CUR_SLICE_END_NS.store(end, Ordering::Relaxed);

    unsafe { context_switch(cur_tid, next.0); }
}

/// Used by task_exit(): switch away from a dying task into idle.
/// Never returns on the dying task’s stack.
pub fn switch_to_idle() -> ! {
    let cur = rq::current_tid();
    let idle = idle_tid();
    unsafe { context_switch(cur, idle); }
}

// —————————————————— internal glue ——————————————————

fn idle_tid() -> TaskId { IDLE_TID.lock().expect("idle").clone() }

fn task_prio(tid: TaskId) -> Priority {
    // Read from task table without cloning; we only need the field.
    if let Some(t) = task::get(tid) { t.prio } else { Priority::Normal }
}

#[inline(always)]
unsafe fn context_switch(cur_tid: TaskId, next_tid: TaskId) -> ! {
    if cur_tid.0 == next_tid.0 {
        // Nothing to do.
        return;
    }

    // Obtain &mut Context for both tasks
    let (from_ctx_ptr, to_ctx_ptr) = {
        use core::ptr::NonNull;
        // Access task table to get &mut Context; safe under IRQ-off switching
        let mut from_ctx: *mut Context = core::ptr::null_mut();
        let mut to_ctx:   *mut Context = core::ptr::null_mut();

        // Helper to get raw mutable pointer (task::with_task gives &mut Task).
        let _ = task::get(cur_tid).expect("cur exists"); // debug aid
        let _ = task::get(next_tid).expect("next exists");
        // SAFETY: we’re in IRQ-off window; scheduler is the only mutator.
        from_ctx = &mut (*((rq::current_tid(),))).1 as *mut _; // placeholder to force compile error if misused
        // The above is intentionally invalid to catch misuse at compile time if someone copies blindly.
        // Proper access below using with_task:

        let from_ctx_ptr = task_with_ctx_ptr(cur_tid);
        let to_ctx_ptr   = task_with_ctx_ptr(next_tid);

        (from_ctx_ptr, to_ctx_ptr)
    };

    // Update tracking
    rq::set_current(Some(next_tid));
    CUR_TID.store(next_tid.0, Ordering::Relaxed);
    task::on_run_start(next_tid, now_ns());

    // Jump: when this task is later scheduled again, we’ll return here.
    ctx::switch(from_ctx_ptr, to_ctx_ptr);
    // ! never returns here; execution resumes when we are switched back in.
    core::hint::unreachable_unchecked()
}

/// Borrow &mut Task.ctx without exposing the whole Task.
fn task_with_ctx_ptr(tid: TaskId) -> *mut Context {
    // Use the internal with_task helper to get a temporary &mut Task, then take &mut ctx.
    let mut out: *mut Context = core::ptr::null_mut();
    let _ = task_with(tid, |t| { out = &mut t.ctx as *mut Context; });
    out
}

/// Local helper: scoped access to a mutable task (thin wrapper).
fn task_with<R>(tid: TaskId, f: impl FnOnce(&mut crate::sched::task::Task) -> R) -> R {
    // Reuse the internal helper defined in task.rs via a private export if you added one.
    // If not present, duplicate the minimal table access here.
    use core::ptr::NonNull;
    use heapless::FnvIndexMap;
    extern "Rust" {
        // Expose a tiny accessor from task.rs for this module:
        // pub(crate) fn __nonos_task_table_get_mut(tid: TaskId) -> Option<NonNull<Task>>;
        fn __nonos_task_table_get_mut(tid: TaskId) -> Option<NonNull<crate::sched::task::Task>>;
    }
    let p = unsafe { __nonos_task_table_get_mut(tid) }.expect("task exists");
    let t = unsafe { &mut *p.as_ptr() };
    f(t)
}

// —————————————————— idle ——————————————————

extern "C" fn idle_entry(_arg: usize) -> ! {
    loop {
        // If someone queued work, yield cooperatively.
        if NEED_RESCHED.load(Ordering::Relaxed) {
            schedule_now();
        }
        unsafe { core::arch::asm!("hlt", options(nomem, nostack, preserves_flags)); }
    }
}

// —————————————————— helpers ——————————————————

fn idle_yield_once() {
    // Cooperative nudge to idle to let interrupts fire and schedule
    unsafe { core::arch::asm!("hlt", options(nomem, nostack, preserves_flags)); }
}
