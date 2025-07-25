//! NØNOS Kernel Heap Initialization
//!
//! This module sets up a virtual heap for dynamic memory allocation in the kernel
//! using `linked_list_allocator`. The heap is mapped during paging init and supports
//! RAM-only operation under the ZeroState runtime. Future extensions may include
//! multiple heap pools, fragmentation diagnostics, and mod-specific allocators.

use core::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;
use core::sync::atomic::{AtomicBool, Ordering};
use linked_list_allocator::LockedHeap;
use spin::Mutex;

/// Static bounds for heap (will later support dynamic regions)
pub const HEAP_START: usize = 0x_4444_0000;
pub const HEAP_SIZE: usize = 1024 * 1024 * 2; // 2 MiB

/// Global kernel heap instance
#[global_allocator]
static KERNEL_HEAP: LockedHeap = LockedHeap::empty();

/// Optional heap enablement tracking
static HEAP_ENABLED: AtomicBool = AtomicBool::new(false);

/// Initializes the global heap for kernel use
pub fn init_kernel_heap() {
    unsafe {
        KERNEL_HEAP.lock().init(HEAP_START as *mut u8, HEAP_SIZE);
        HEAP_ENABLED.store(true, Ordering::SeqCst);
    }
    log_heap_status("[HEAP] Kernel heap initialized");
}

/// Log message to VGA or logging backend
fn log_heap_status(msg: &str) {
    if let Some(logger) = crate::log::logger::try_get_logger() {
        logger.log(msg);
    } else {
        // fallback to VGA for very early init
        let vga = 0xb8000 as *mut u8;
        for (i, byte) in msg.bytes().enumerate().take(80) {
            unsafe {
                *vga.offset(i as isize * 2) = byte;
            }
        }
    }
}

/// Custom allocator fallback used in early boot
pub struct DummyAllocator;

unsafe impl GlobalAlloc for DummyAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if HEAP_ENABLED.load(Ordering::SeqCst) {
            KERNEL_HEAP.alloc(layout)
        } else {
            null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if HEAP_ENABLED.load(Ordering::SeqCst) {
            KERNEL_HEAP.dealloc(ptr, layout)
        }
    }
}

/// Handles out-of-memory conditions
#[alloc_error_handler]
fn alloc_error(layout: Layout) -> ! {
    log_heap_status("[HEAP] ALLOCATION FAILURE");
    panic!("[HEAP] Out of memory: {:?}", layout);
}
