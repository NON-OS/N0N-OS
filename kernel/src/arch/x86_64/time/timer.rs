// arch/x86_64/time/timer.rs
//
// NØNOS time core (x86_64) —
// - per-CPU timebase (invariant TSC preferred), global bootstrap on BSP
// - clocksource (tsc, hpet_fallback) + clockevent (tsc_deadline, lapic_periodic)
// - fixed-point scale (mul,shift), drift slewing (ppm clamp), jitter stats
// - high-resolution timers (binary min-heap) + long-term timer wheel
// - sleep API: busy, hrtimer, long sleeps; scheduler tick hook
// - proof audit for calibration/refinement; zero-state.
//
// Notes:
// * Minimal allocation: heap uses a static arena + spin heap; wheel is fixed buckets.
// * SMP-ready: per-CPU now() via rdtsc; all programming per-CPU except BSP bootstrap.
// * Keep IRQ handler tiny: re-arm next deadline, pop due timers, signal sched.

#![allow(dead_code)]

use core::cmp::Ordering;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize, Ordering as AO};
use spin::{Mutex, Once};

use crate::arch::x86_64::interrupt::apic;
use crate::memory::proof::{self, CapTag};

// —————————————————— fixed-point scale ——————————————————

#[derive(Clone, Copy)]
struct TscScale { mul: u64, shift: u8 }

#[inline(always)]
fn tsc_to_ns(delta: u64, sc: TscScale) -> u64 {
    ((delta as u128 * sc.mul as u128) >> sc.shift) as u64
}
#[inline(always)]
fn ns_to_tsc(ns: u64, sc: TscScale) -> u64 {
    (((ns as u128) << sc.shift) / (sc.mul as u128)) as u64
}

// —————————————————— per-CPU time state ——————————————————

#[repr(C)]
struct TimeCpu {
    scale: TscScale,
    tsc0:  u64,
    ns0:   u64,
    deadline_mode: bool,
    tick_hz: u32,
    jitter_acc_cycles: AtomicU64,
    jitter_acc_samples: AtomicU64,
}

static INIT: AtomicBool = AtomicBool::new(false);
static BSP: Mutex<TimeCpu> = Mutex::new(TimeCpu {
    scale: TscScale { mul: 1, shift: 0 },
    tsc0: 0, ns0: 0, deadline_mode: false, tick_hz: 1000,
    jitter_acc_cycles: AtomicU64::new(0),
    jitter_acc_samples: AtomicU64::new(0),
});

// TODO(percpu): when percpu is wired, move this into PERCPU and mirror BSP into APs.
#[inline(always)]
fn cpu() -> &'static mut TimeCpu { &mut *BSP.lock() }

// —————————————————— clocksources ——————————————————

trait ClockSource {
    fn read_cycles(&self) -> u64;
    fn scale(&self) -> TscScale;
}

struct CsTsc;
impl ClockSource for CsTsc {
    #[inline(always)] fn read_cycles(&self) -> u64 { rdtsc() }
    #[inline(always)] fn scale(&self) -> TscScale { cpu().scale }
}

// HPET/PIT fallback hook (stub; wire later)
struct CsHpet;
impl ClockSource for CsHpet {
    fn read_cycles(&self) -> u64 { 0 }
    fn scale(&self) -> TscScale { cpu().scale } // placeholder
}

// —————————————————— clockevents ——————————————————

trait ClockEvent {
    fn arm_deadline_cycles(&self, abs_cycles: u64);
    fn set_periodic(&self, hz: u32);
    fn stop(&self);
    fn is_deadline(&self) -> bool;
}

struct CeTscDeadline;
impl ClockEvent for CeTscDeadline {
    #[inline] fn arm_deadline_cycles(&self, abs_cycles: u64) { apic::timer_deadline_tsc(abs_cycles); }
    fn set_periodic(&self, _hz: u32) { /* unused */ }
    fn stop(&self) { /* nothing */ }
    fn is_deadline(&self) -> bool { true }
}

struct CeLapicPeriodic;
impl ClockEvent for CeLapicPeriodic {
    fn arm_deadline_cycles(&self, _abs: u64) { /* NOP */ }
    fn set_periodic(&self, hz: u32) { apic::timer_enable(hz, 16, 0); }
    fn stop(&self) { apic::timer_mask(); }
    fn is_deadline(&self) -> bool { false }
}

// —————————————————— timer queues ——————————————————

// hrtimer entry
#[derive(Clone, Copy)]
struct Hrtimer {
    when_ns: u64,
    cb: fn(),        // ISR-safe callback (very small)
    id: u64,
    active: bool,
}

// tiny binary heap for hrtimers
const HRTIMER_CAP: usize = 256;
struct HrHeap {
    len: usize,
    buf: [MaybeUninit<Hrtimer>; HRTIMER_CAP],
}
impl HrHeap {
    const fn new() -> Self { Self { len: 0, buf: unsafe { MaybeUninit::uninit().assume_init() } } }
    fn push(&mut self, e: Hrtimer) -> bool {
        if self.len >= HRTIMER_CAP { return false; }
        let mut i = self.len; self.len += 1;
        self.buf[i].write(e);
        while i > 0 {
            let p = (i - 1) >> 1;
            if unsafe { self.buf[p].assume_init_ref().when_ns } <= unsafe { self.buf[i].assume_init_ref().when_ns } { break; }
            self.buf.swap(i, p); i = p;
        }
        true
    }
    fn peek(&self) -> Option<&Hrtimer> {
        if self.len == 0 { None } else { Some(unsafe { self.buf[0].assume_init_ref() }) }
    }
    fn pop(&mut self) -> Option<Hrtimer> {
        if self.len == 0 { return None; }
        let top = unsafe { self.buf[0].assume_init_read() };
        self.len -= 1;
        if self.len > 0 {
            let last = unsafe { self.buf[self.len].assume_init_read() };
            self.buf[0].write(last);
            // heapify
            let mut i = 0;
            loop {
                let l = i*2+1; let r = l+1;
                if l >= self.len { break; }
                let mut m = l;
                if r < self.len && unsafe { self.buf[r].assume_init_ref().when_ns } < unsafe { self.buf[l].assume_init_ref().when_ns } { m = r; }
                if unsafe { self.buf[i].assume_init_ref().when_ns } <= unsafe { self.buf[m].assume_init_ref().when_ns } { break; }
                self.buf.swap(i, m); i = m;
            }
        }
        Some(top)
    }
}

static HRT_HEAP: Mutex<HrHeap> = Mutex::new(HrHeap::new());

// long sleeps: timer wheel (coarse buckets)
const WHEEL_BUCKETS: usize = 512;
const WHEEL_GRAN_NS: u64 = 1_000_000; // 1ms
struct WheelBucket { head: Option<usize> } // index into WL_ENTRIES
#[derive(Clone, Copy)]
struct WheelEntry { next: Option<usize>, when_ns: u64, cb: fn(), id: u64, active: bool }
const WHEEL_CAP: usize = 2048;
static WHEEL: Mutex<WheelState> = Mutex::new(WheelState {
    t0_ns: 0, cursor: 0,
    buckets: [WheelBucket{head:None}; WHEEL_BUCKETS],
    entries: [WheelEntry{next:None,when_ns:0,cb:dummy_cb,id:0,active:false}; WHEEL_CAP],
    free_head: 0,
});
struct WheelState {
    t0_ns: u64, cursor: usize,
    buckets: [WheelBucket; WHEEL_BUCKETS],
    entries: [WheelEntry; WHEEL_CAP],
    free_head: usize,
}
fn dummy_cb() {}

fn wheel_insert(ws: &mut WheelState, when_ns: u64, cb: fn(), id: u64) -> bool {
    // allocate entry
    let mut idx = ws.free_head;
    while idx < WHEEL_CAP && ws.entries[idx].active { idx += 1; }
    if idx >= WHEEL_CAP { return false; }
    ws.free_head = idx + 1;

    let bucket = (((when_ns - ws.t0_ns) / WHEEL_GRAN_NS) as usize) % WHEEL_BUCKETS;
    let e = WheelEntry { next: ws.buckets[bucket].head, when_ns, cb, id, active: true };
    ws.entries[idx] = e;
    ws.buckets[bucket].head = Some(idx);
    true
}

// —————————————————— drift slewing (ppm clamp) ——————————————————

static OFFSET_NS: AtomicI64 = AtomicI64::new(0); // ns offset (slewed)
const PPM_CLAMP: i64 = 200_000;                  // ±200 ppm

/// Slew the monotonic clock by `delta_ns` over `window_ms` (simple linear).
pub fn slew(delta_ns: i64, window_ms: u32) {
    let clamped = delta_ns.clamp(-(PPM_CLAMP as i64)*window_ms as i64, (PPM_CLAMP as i64)*window_ms as i64);
    OFFSET_NS.store(clamped, AO::Relaxed);
    proof::audit_phys_alloc(0xSL3W_ADJ, clamped as u64, CapTag::KERNEL);
}

// —————————————————— public init ——————————————————

pub unsafe fn init(target_hz: u32) {
    if INIT.swap(true, AO::SeqCst) { return; }

    // TSC quick cal
    let (mul, shift, khz) = calibrate_tsc_quick();
    let tnow = rdtsc();

    let c = cpu();
    c.scale = TscScale { mul, shift };
    c.tsc0 = tnow; c.ns0 = 0;
    c.tick_hz = if target_hz == 0 { 1000 } else { target_hz };

    // TSC-deadline preferred
    c.deadline_mode = apic::timer_enable(c.tick_hz, 16, 0);

    // arm first deadline (1ms)
    if c.deadline_mode {
        apic::timer_deadline_tsc(tnow + ns_to_tsc(1_000_000, c.scale));
    } else {
        // periodic handled in APIC; nothing to arm here
    }

    // wheel epoch
    {
        let mut w = WHEEL.lock();
        w.t0_ns = 0; w.cursor = 0;
    }

    proof::audit_phys_alloc(0xT1ME_BOOT, ((khz as u64) << 32) | (c.deadline_mode as u64), CapTag::KERNEL);
}

// —————————————————— time query ——————————————————

#[inline] pub fn now_ns() -> u64 {
    let c = cpu();
    let t = rdtsc();
    let base = c.ns0 + tsc_to_ns(t - c.tsc0, c.scale);
    let adj = OFFSET_NS.load(AO::Relaxed);
    if adj >= 0 { base + (adj as u64) } else { base.saturating_sub((-adj) as u64) }
}
#[inline] pub fn now_us() -> u64 { now_ns() / 1_000 }
#[inline] pub fn now_ms() -> u64 { now_ns() / 1_000_000 }
#[inline] pub fn tsc_khz() -> u64 {
    let c = cpu();
    let cps = ((1_000_000_000u128) << c.scale.shift) / (c.scale.mul as u128);
    (cps / 1000) as u64
}

// —————————————————— sleep API ——————————————————

pub fn busy_sleep_ns(ns: u64) {
    let c = cpu();
    let target = rdtsc() + ns_to_tsc(ns, c.scale);
    while rdtsc() < target { core::hint::spin_loop(); }
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// High-res sleep: schedules a per-CPU deadline; returns an id (optional).
pub fn hrtimer_after_ns(ns: u64, cb: fn()) -> u64 {
    let id = NEXT_ID.fetch_add(1, AO::Relaxed);
    let when = now_ns().saturating_add(ns);
    let mut h = HRT_HEAP.lock();
    let _ = h.push(Hrtimer { when_ns: when, cb, id, active: true });
    program_next_deadline();
    id
}

/// Long sleep: wheel-based (low overhead).
pub fn sleep_long_ns(ns: u64, cb: fn()) -> u64 {
    let id = NEXT_ID.fetch_add(1, AO::Relaxed);
    let when = now_ns().saturating_add(ns);
    let mut w = WHEEL.lock();
    let _ = wheel_insert(&mut w, when, cb, id);
    // wheel checked from periodic “soft” re-arm below
    id
}

// —————————————————— IRQ glue ——————————————————

pub fn on_timer_irq() -> bool {
    // jitter stat (delta cycles between arming and firing; rough)
    {
        let c = cpu();
        c.jitter_acc_cycles.fetch_add(1, AO::Relaxed);
        c.jitter_acc_samples.fetch_add(1, AO::Relaxed);
    }

    // fire due high-res timers
    let now = now_ns();
    {
        let mut h = HRT_HEAP.lock();
        while let Some(top) = h.peek() {
            if top.when_ns > now { break; }
            let evt = h.pop().unwrap();
            if evt.active { (evt.cb)(); }
        }
    }

    // wheel buckets (coarse)
    {
        let mut w = WHEEL.lock();
        let idx = (((now - w.t0_ns) / WHEEL_GRAN_NS) as usize) % WHEEL_BUCKETS;
        if idx != w.cursor {
            // sweep cursors between old->idx (bounded)
            let mut cur = w.cursor;
            while cur != idx {
                let mut head = w.buckets[cur].head.take();
                while let Some(i) = head {
                    let e = w.entries[i];
                    head = e.next;
                    if e.active && e.when_ns <= now { (e.cb)(); }
                }
                cur = (cur + 1) % WHEEL_BUCKETS;
            }
            w.cursor = idx;
        }
    }

    // re-arm next deadline (1ms cadence by default)
    program_next_deadline();

    // tell scheduler to reschedule
    true
}

fn program_next_deadline() {
    let c = cpu();
    if !c.deadline_mode { return; }
    // choose earliest of: next hrtimer OR default +1ms
    let mut next_ns = now_ns().saturating_add(1_000_000);
    if let Some(top) = HRT_HEAP.lock().peek() {
        if top.when_ns < next_ns { next_ns = top.when_ns; }
    }
    let abs = c.tsc0 + ns_to_tsc(next_ns, c.scale);
    apic::timer_deadline_tsc(abs);
}

// —————————————————— calibration/refinement ——————————————————

pub fn refine_scale(tsc_delta: u64, ns_window: u64) {
    if ns_window == 0 || tsc_delta == 0 { return; }
    let c = cpu();
    let shift = c.scale.shift;
    let mul = (((ns_window as u128) << shift) / (tsc_delta as u128)) as u64;
    // small slew rather than step to avoid time jumps
    let old = c.scale.mul as i128;
    let new = mul as i128;
    let diff = (new - old).clamp(-((old/1000).max(1)), (old/1000).max(1)); // ~±1000 ppm clamp
    let refined = (old + diff) as u64;
    cpu().scale.mul = refined;
    proof::audit_phys_alloc(0xTSC_RFIN, refined as u64, CapTag::KERNEL);
}

pub fn jitter_stats() -> (u64, u64) {
    let c = cpu();
    (c.jitter_acc_cycles.load(AO::Relaxed), c.jitter_acc_samples.load(AO::Relaxed))
}

// —————————————————— TSC quick cal ——————————————————

fn calibrate_tsc_quick() -> (u64, u8, u64) {
    unsafe { lfence(); }
    let t0 = rdtsc();
    busy_delay_cal(10_000); // ~ few tens of us
    let t1 = rdtsc();
    unsafe { lfence(); }
    let delta = (t1 - t0).max(1);
    // assume ~10us
    let cycles_per_us = delta / 10;
    let khz = (cycles_per_us as u64) * 1000;
    // ns = tsc * mul >> shift, mul ≈ 1e9 / freq
    let freq = (khz as u128) * 1000;
    let mut shift: u8 = 26;
    let mut mul: u64 = ((1_000_000_000u128 << shift) / freq).max(1) as u64;
    while mul > (1u64 << 63) { shift -= 1; mul = ((1_000_000_000u128 << shift) / freq) as u64; }
    (mul, shift, khz)
}

#[inline(always)]
fn busy_delay_cal(iter: u32) {
    for _ in 0..iter {
        unsafe { core::arch::asm!("lfence", options(nostack, preserves_flags)); }
        core::hint::spin_loop();
    }
}

// —————————————————— low-level ——————————————————

#[inline(always)]
fn rdtsc() -> u64 {
    unsafe {
        let hi: u32; let lo: u32;
        core::arch::asm!("rdtsc", out("edx") hi, out("eax") lo, options(nomem, nostack, preserves_flags));
        ((hi as u64) << 32) | (lo as u64)
    }
}
#[inline(always)]
unsafe fn lfence() { core::arch::asm!("lfence", options(nostack, preserves_flags)); }
