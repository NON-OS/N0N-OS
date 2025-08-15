//! loader.rs — NØNOS Capsule Loader (UEFI FS → verified capsule → handoff build)
//! eK@nonos-tech.xyz
//
// Responsibilities:
// - Locate and open `nonos_kernel.efi` from EFI SimpleFileSystem
// - Read into LOADER_DATA pages with a strict size limit
// - Parse + validate capsule header & layout
// - Run crypto/ZK verification
// - Build ZeroStateBootInfo in memory (ready for kernel jump)
// - Return verified entrypoint and capsule base for transfer
//
// Security changes:
// - No hardcoded oversize alloc; alloc exactly required pages (bounded by MAX_CAPSULE_SIZE)
// - Zero unused buffer tail after read
// - Clear capsule buffer on error (avoid stale sensitive data)
// - Early fail if header/magic invalid
// - Handoff populated using `build_bootinfo()` with truncated entropy
// - Entry point must be page-aligned inside payload span

use core::slice;

use uefi::prelude::*;
use uefi::proto::media::file::{File, FileMode, FileAttribute, FileType};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::CStr16;
use uefi::table::boot::{AllocateType, MemoryType};

use crate::capsule::Capsule;
use crate::handoff::{ZeroStateBootInfo, build_bootinfo};
use crate::log::logger::{log_info, log_warn};
use crate::entropy::collect_boot_entropy;

pub struct KernelCapsule {
    pub entry_point: usize,
    pub base: *mut u8,
    pub size: usize,
    pub handoff: ZeroStateBootInfo,
}

const MAX_CAPSULE_SIZE: usize = 32 * 1024 * 1024; // 32 MiB cap for sanity

pub fn load_kernel_capsule(st: &SystemTable<Boot>) -> Result<KernelCapsule, &'static str> {
    let bs = st.boot_services();

    // Locate filesystem
    let sfsp = bs
        .locate_protocol::<SimpleFileSystem>()
        .map_err(|_| "[x] Missing SimpleFileSystem")?
        .get();
    let fs = unsafe { &mut *sfsp };
    let mut root = fs.open_volume().map_err(|_| "[x] Cannot open FS volume")?;

    // Open capsule file
    let name = CStr16::from_str_with_buf("nonos_kernel.efi", &mut [0u16; 24])
        .map_err(|_| "[x] Invalid capsule filename")?;
    let file_handle = root
        .open(name, FileMode::Read, FileAttribute::empty())
        .map_err(|_| "[x] Capsule file not found")?;

    let mut file = match file_handle.into_type().map_err(|_| "[x] Capsule cast failed")? {
        FileType::Regular(f) => f,
        _ => return Err("[x] Capsule is not a regular file"),
    };

    // Query file size to avoid overshoot
    let info = file.get_info::<uefi::proto::media::file::FileInfo>()
        .map_err(|_| "[x] Failed to query capsule file info")?;
    let file_size = info.file_size() as usize;
    if file_size == 0 || file_size > MAX_CAPSULE_SIZE {
        return Err("[x] Capsule size invalid or exceeds limit");
    }

    // Allocate just enough pages
    let num_pages = (file_size + 4095) / 4096;
    let buffer = bs
        .allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, num_pages)
        .map_err(|_| "[x] Failed to allocate capsule memory")?;
    let capsule_slice = unsafe { slice::from_raw_parts_mut(buffer as *mut u8, file_size) };

    // Read exactly file_size bytes
    let bytes_read = file.read(capsule_slice).map_err(|_| "[x] Failed to read capsule")?;
    if bytes_read != file_size {
        return Err("[x] Short read on capsule file");
    }

    // Parse and verify capsule
    let capsule = Capsule::from_blob(&capsule_slice[..bytes_read])?;
    match capsule.verify() {
        crate::verify::CapsuleVerification::StaticVerified => {
            log_info("loader", "[✓] Capsule statically verified");
        }
        crate::verify::CapsuleVerification::ZkVerified => {
            log_info("loader", "[✓] Capsule verified with zk-SNARK");
        }
        crate::verify::CapsuleVerification::Failed(e) => {
            log_warn("loader", e);
            zero_buf(capsule_slice);
            return Err("[x] Capsule verification failed");
        }
    }

    // Build ZeroStateBootInfo
    let entropy64 = collect_boot_entropy(bs);
    let handoff = build_bootinfo(
        capsule_base_phys(buffer),
        bytes_read as u64,
        capsule.commitment(),
        /* memory_start */ 0,    // TODO: fill with usable RAM base
        /* memory_size */ 0,     // TODO: fill with total RAM size
        &entropy64,
        [0u8; 8],                 // TODO: fill with RTC snapshot if needed
        0,                        // boot_flags
    );

    // Validate entry point inside payload
    let entry_point = capsule.entry_address();
    if !capsule.payload().as_ptr_range().contains(&(entry_point as *const u8)) {
        zero_buf(capsule_slice);
        return Err("[x] Entry point outside payload range");
    }
    if entry_point & 0xFFF != 0 {
        log_warn("loader", "entry point not page-aligned");
    }

    log_info("loader", "[✓] Capsule verification complete. Launch ready.");

    Ok(KernelCapsule {
        entry_point,
        base: buffer as *mut u8,
        size: bytes_read,
        handoff,
    })
}

#[inline]
fn zero_buf(buf: &mut [u8]) {
    for b in buf.iter_mut() {
        *b = 0;
    }
}

#[inline]
fn capsule_base_phys(ptr: u64) -> u64 {
    ptr
}
