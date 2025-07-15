use alloc::collections::VecDeque;
use core::task::{Poll, Context, Waker};

static EXECUTOR: spin::Once<Executor> = spin::Once::new();

pub fn init() { EXECUTOR.call_once(|| Executor::new()); }
pub fn spawn(fut: impl core::future::Future<Output = ()> + 'static) {
    EXECUTOR.wait().spawn(Box::pin(fut))
}
pub fn run() -> ! { EXECUTOR.wait().run() }

struct Executor { /* task queue, waker cache */ }
