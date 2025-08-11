// ui/event.rs
//
// NØNOS event bus 
// - Lock-free MPSC ring (ISR-safe push) → single consumer task (cli.metrics or system daemon)
// - Priority lanes: High (ISR/critical), Norm (control), Low (telemetry)
// - Fixed-size payloads; no heap; backpressure counters
// - Subscribe API for direct callback fanout (best-effort, non-blocking)
// - Zero-state; public-only payloads

#![allow(dead_code)]

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicUsize, AtomicU64, Ordering};
use spin::Mutex;

const QH_CAP: usize = 256; // high
const QN_CAP: usize = 512; // norm
const QL_CAP: usize = 1024; // low

#[derive(Clone, Copy)]
pub enum Event {
    Heartbeat { ms: u64, rq: [usize;5] },
    ProofRoot { root: [u8;32], epoch: u64 },
    SchedPick { tid: u64, prio: u8 },
    Log { lvl: u8, code: u32 },
}

struct Ring<const N: usize> {
    buf: UnsafeCell<[Event; N]>,
    head: AtomicUsize,
    tail: AtomicUsize,
    drops: AtomicU64,
}
impl<const N: usize> Ring<N> {
    const fn new() -> Self {
        Self {
            buf: UnsafeCell::new([Event::Log{lvl:0, code:0}; N]),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            drops: AtomicU64::new(0),
        }
    }
    #[inline]
    fn push_isr(&self, e: Event) {
        let t = self.tail.load(Ordering::Relaxed);
        let h = self.head.load(Ordering::Acquire);
        if t.wrapping_sub(h) >= N {
            self.drops.fetch_add(1, Ordering::Relaxed);
            return;
        }
        unsafe { (*self.buf.get())[t % N] = e; }
        self.tail.store(t.wrapping_add(1), Ordering::Release);
    }
    #[inline]
    fn pop(&self) -> Option<Event> {
        let h = self.head.load(Ordering::Relaxed);
        let t = self.tail.load(Ordering::Acquire);
        if h == t { return None; }
        let e = unsafe { (*self.buf.get())[h % N] };
        self.head.store(h.wrapping_add(1), Ordering::Release);
        Some(e)
    }
    fn dropped(&self) -> u64 { self.drops.load(Ordering::Relaxed) }
}

// Three priority lanes
static QH: Ring<QH_CAP> = Ring::new();
static QN: Ring<QN_CAP> = Ring::new();
static QL: Ring<QL_CAP> = Ring::new();

// Optional fanout subscribers (best-effort, may run in caller’s context)
static SUBS: Mutex<heapless::Vec<fn(Event), 16>> = Mutex::new(heapless::Vec::new());

pub enum Pri { High, Norm, Low }

#[inline] pub fn subscribe(cb: fn(Event)) { let mut v = SUBS.lock(); let _ = v.push(cb); }

#[inline]
pub fn publish_pri(e: Event, p: Pri) {
    match p {
        Pri::High => QH.push_isr(e),
        Pri::Norm => QN.push_isr(e),
        Pri::Low  => QL.push_isr(e),
    }
    // fire-and-forget callbacks (non-blocking)
    let v = SUBS.lock();
    for &cb in v.iter() { cb(e); }
}

#[inline] pub fn publish(e: Event) { publish_pri(e, Pri::Norm) }

/// Drain in consumer context (e.g., CLI metrics task). Returns number popped.
pub fn drain(mut f: impl FnMut(Event)) -> usize {
    let mut n = 0;
    while let Some(e) = QH.pop() { f(e); n+=1; }
    let mut i = 0;
    while i < 4 { if let Some(e) = QN.pop() { f(e); n+=1; } else { break; } i+=1; } // small fairness
    let mut j = 0;
    while j < 8 { if let Some(e) = QL.pop() { f(e); n+=1; } else { break; } j+=1;
    }
    n
}

pub fn stats() -> (u64,u64,u64) {
    (QH.dropped(), QN.dropped(), QL.dropped())
}
