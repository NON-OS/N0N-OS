//! loader.rs — NØNOS Capsule Loader: Secure Capsule Discovery + Memory Relocation
//!
//! This UEFI-based capsule loader performs:
//! - Secure UEFI FS read of `nonos_kernel.efi`
//! - Capsule header validation and commitment hash
//! - Memory-safe relocation into LOADER_DATA pages
//! - Capsule signature or zk-SNARK verification
//! - Capsule execution (returning verified entrypoint for jump)
//! - Embeds and prepares ZeroStateBootInfo

use uefi::prelude::*;
use uefi::proto::media::file::{File, FileMode, FileAttribute, FileType};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::CStr16;
use uefi::table::boot::{AllocateType, MemoryType};

use crate::capsule::Capsule;
use crate::handoff::ZeroStateBootInfo;
use crate::log::logger::{log_info, log_warn};

/// Result of secure capsule relocation and execution prep.
pub struct KernelCapsule {
    pub entry_point: usize,
    pub base: *mut u8,
    pub size: usize,
    pub handoff: ZeroStateBootInfo,
}

/// Discover and prepare the primary NØNOS kernel capsule via UEFI.
pub fn load_kernel_capsule(st: &SystemTable<Boot>) -> Result<KernelCapsule, &'static str> {
    let bt = st.boot_services();

    let sfsp = bt
        .locate_protocol::<SimpleFileSystem>()
        .map_err(|_| "[x] Missing SimpleFileSystem")?
        .get();

    let fs = unsafe { &mut *sfsp };
    let mut root = fs.open_volume().map_err(|_| "[x] Cannot open FS volume")?;

    let name = CStr16::from_str_with_buf("nonos_kernel.efi", &mut [0u16; 24])
        .map_err(|_| "[x] Invalid capsule filename")?;

    let file_handle = root.open(name, FileMode::Read, FileAttribute::empty())
        .map_err(|_| "[x] Capsule file not found")?;

    let mut file = match file_handle.into_type().map_err(|_| "[x] Capsule cast failed")? {
        FileType::Regular(f) => f,
        _ => return Err("[x] Capsule is not a regular file"),
    };

    let capsule_size = 32 * 1024 * 1024; // 32MB max
    let buffer = bt
        .allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, capsule_size / 4096)
        .map_err(|_| "[x] Failed to allocate capsule memory")?;

    let capsule_slice =
        unsafe { core::slice::from_raw_parts_mut(buffer as *mut u8, capsule_size) };

    let bytes_read = file
        .read(capsule_slice)
        .map_err(|_| "[x] Failed to read capsule")?;

    if bytes_read == 0 {
        return Err("[x] Capsule file is empty");
    }

    let actual_blob = &capsule_slice[..bytes_read];
    let capsule = Capsule::from_blob(actual_blob)?;

    let result = capsule.verify();
    match result {
        crate::verify::CapsuleVerification::StaticVerified => {
            log_info("loader", "[✓] Capsule statically verified");
        }
        crate::verify::CapsuleVerification::ZkVerified => {
            log_info("loader", "[✓] Capsule verified with zk-SNARK");
        }
        crate::verify::CapsuleVerification::Failed(e) => {
            log_warn("loader", e);
            return Err("[x] Capsule verification failed");
        }
    }

    let handoff = ZeroStateBootInfo::from_capsule(&capsule, st)?;

    log_info("loader", "[✓] Capsule verification complete. Launch ready.");

    Ok(KernelCapsule {
        entry_point: capsule.entry_address(),
        base: buffer as *mut u8,
        size: bytes_read,
        handoff,
    })
}

