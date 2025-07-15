use spin::Mutex;
use x86_64::structures::paging::PhysFrame;

static FRAME_ALLOC: Mutex<Option<FrameAllocator>> = Mutex::new(None);

pub fn init(mem_map: &[uefi::table::boot::MemoryDescriptor]) {
    *FRAME_ALLOC.lock() = Some(FrameAllocator::new(mem_map));
}

pub fn alloc() -> Option<PhysFrame> { FRAME_ALLOC.lock().as_mut()?.alloc() }

/* … implement simple bitmap allocator … */
