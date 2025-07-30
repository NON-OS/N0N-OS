//! NØNOS Capability-Aware Kernel Scheduler
//!
//! This scheduler provides a secure cooperative multitasking environment
//! for async-capable kernel tasks. It supports:
//! - Capability-tagged task registration (planned)
//! - Priority boot queues and core-task separation (in roadmap)
//! - Preemption placeholder via tick scheduling (planned)
//! - Secure `.mod` future-scoped sandbox execution

use alloc::collections::VecDeque;
use core::task::{Context, Poll, Waker, RawWaker, RawWakerVTable};
use core::future::Future;
use core::pin::Pin;
use core::ptr::null;
use spin::Mutex;

/// Represents a single schedulable kernel task
pub struct Task {
    pub name: &'static str,
    pub future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
    pub waker: Option<Waker>,
    pub priority: u8,
    pub ticks: u64,
}

impl Task {
    pub fn poll(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        self.future.as_mut().poll(cx)
    }
}

/// Global scheduler queue (FIFO, upgrade to priority queue later)
static SCHED_QUEUE: Mutex<VecDeque<Task>> = Mutex::new(VecDeque::new());

/// Spawns a new async kernel task into the global queue
pub fn spawn_task(name: &'static str, fut: impl Future<Output = ()> + Send + 'static, priority: u8) {
    let task = Task {
        name,
        future: Box::pin(fut),
        waker: None,
        priority,
        ticks: 0,
    };
    SCHED_QUEUE.lock().push_back(task);
}

/// Polls the entire scheduler queue cooperatively
pub fn run_scheduler() {
    let waker = unsafe { Waker::from_raw(dummy_raw_waker()) };
    let mut cx = Context::from_waker(&waker);

    loop {
        let mut queue = SCHED_QUEUE.lock();
        if queue.is_empty() {
            break;
        }

        let mut new_queue = VecDeque::new();

        while let Some(mut task) = queue.pop_front() {
            match task.poll(&mut cx) {
                Poll::Ready(()) => log_task_exit(task.name),
                Poll::Pending => {
                    task.ticks += 1;
                    new_queue.push_back(task);
                },
            }
        }

        *queue = new_queue;
    }
}

/// Initializes the kernel scheduler
pub fn init_scheduler() {
    log_init("[SCHED] Kernel scheduler online.");
    // Placeholder for future APIC tick config or multi-core queues
}

/// RawWaker for pre-init environments
fn dummy_raw_waker() -> RawWaker {
    fn no_op(_: *const ()) {}
    fn clone(_: *const ()) -> RawWaker { dummy_raw_waker() }

    let vtable = &RawWakerVTable::new(clone, no_op, no_op, no_op);
    RawWaker::new(null(), vtable)
}

/// Simple scheduler-level logging
fn log_task_exit(task: &str) {
    if let Some(logger) = crate::log::logger::try_get_logger() {
        logger.log(&format!("[SCHED] Task '{}' completed.", task));
    }
}

fn log_init(msg: &str) {
    if let Some(logger) = crate::log::logger::try_get_logger() {
        logger.log(msg);
    }
}
