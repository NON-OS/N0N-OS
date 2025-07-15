//! boot/src/main.rs
#![no_main]
#![no_std]
#![feature(alloc_error_handler)]

extern crate alloc;

use alloc::vec::Vec;
use core::{fmt::Write, ptr::copy_nonoverlapping};

use uefi::{prelude::*, CStr16};
use uefi::proto::media::{fs::SimpleFileSystem, file::*};
use uefi::table::boot::{AllocateType, MemoryType, MemoryDescriptor};
use elf::{ElfBytes, endian::AnyEndian};

use shared::bootinfo::{BootInfo, MemoryRegion, MemoryRegionType};

/// Thin global allocator for boot-phase heap.
#[global_allocator]
static ALLOC: uefi::alloc::UefiAlloc = uefi::alloc::UefiAlloc;

#[entry]
fn efi_main(handle: Handle, mut st: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut st).expect("uefi_services");

    let con_out = st.stdout();
    con_out.reset(false).ok();
    con_out.set_color(uefi::proto::console::text::Color::Cyan,
                      uefi::proto::console::text::Color::Black).ok();
    writeln!(con_out, "\n🦀  NØN-OS  Secure Bootloader").ok();

    // ──- Load and verify the kernel ELF ─────────────────────────────────────────
    let elf_buf = load_kernel(&mut st).expect("kernel load failed");
    verify_kernel_sha256(&elf_buf, &mut st);

    let entry = load_elf_segments(&elf_buf, &mut st)
        .expect("ELF segment load failed");

    writeln!(con_out, "   ↳  kernel entry @ {:#018x}", entry).ok();

    // ──- Grab memory-map & RSDP for the kernel -─────────────────────────────────
    let (efi_mem, key) = gather_memory_map(&mut st);
    let rsdp = st.config_table()
                 .iter()
                 .find(|t| t.guid == uefi::table::cfg::ACPI2_GUID)
                 .map(|t| t.address as u64);

    // Copy memory map into a kernel-owned buffer (identity-mapped later).
    let mut regions = Vec::<MemoryRegion>::with_capacity(efi_mem.len());
    for d in &efi_mem {
        regions.push(MemoryRegion {
            start: d.phys_start,
            len:   d.page_count * 4096,
            ty:    match d.ty {
                MemoryType::CONVENTIONAL => MemoryRegionType::Usable,
                MemoryType::BOOT_SERVICES_CODE |
                MemoryType::BOOT_SERVICES_DATA |
                MemoryType::LOADER_DATA          => MemoryRegionType::Reserved,
                MemoryType::ACPI_RECLAIM         => MemoryRegionType::AcpiReclaimable,
                MemoryType::ACPI_NON_VOLATILE    => MemoryRegionType::AcpiNvs,
                _                                => MemoryRegionType::Reserved,
            },
        });
    }

    // Allocate contiguous pages for BootInfo + copied map.
    let pages = ((core::mem::size_of::<BootInfo>() +
                  regions.len() * core::mem::size_of::<MemoryRegion>() + 0xFFF) / 4096) as usize;
    let bootinfo_ptr = st.boot_services()
        .allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, pages)
        .expect("bootinfo alloc") as *mut BootInfo;

    unsafe {
        // Write BootInfo struct.
        (*bootinfo_ptr) = BootInfo {
            phys_memory_offset: 0, // kernel decides virt-offset later
            memory_map: core::slice::from_raw_parts_mut(
                (bootinfo_ptr as *mut u8)
                    .add(core::mem::size_of::<BootInfo>()) as *mut MemoryRegion,
                regions.len()
            ),
            rsdp_addr: rsdp,
            cmdline: "",
            kernel_load_start: elf_buf.as_ptr() as u64,
            kernel_load_size:  elf_buf.len()  as u64,
        };
        // Copy region slice after the struct.
        (*bootinfo_ptr).memory_map.copy_from_slice(&regions);
    }

    // ──- Exit boot services -────────────────────────────────────────────────────
    unsafe { st.exit_boot_services(handle, key).expect("exit_boot_services"); }

    // ──- Jump! (never returns) -─────────────────────────────────────────────────
    unsafe {
        let kernel_entry: extern "C" fn(&'static BootInfo) -> ! =
            core::mem::transmute(entry);
        kernel_entry(&*bootinfo_ptr);
    }
}

/// Load \EFI\N0N\kernel.elf into a Vec<u8>.
fn load_kernel(st: &mut SystemTable<Boot>) -> Result<Vec<u8>, Status> {
    let bs  = st.
boot_services();
    let sfs = unsafe { &mut *bs.locate_protocol::<SimpleFileSystem>()?.get() };
    let mut root = sfs.open_volume()?;

    let mut name_buf = [0u16; 24];
    let ker = root.open(
        CStr16::from_str_with_buf("\\EFI\\N0N\\kernel.elf", &mut name_buf)?,
        FileMode::Read, FileAttribute::empty())?;

    let mut file = match ker.into_type()? { FileType::Regular(f) => f, _ => return Err(Status::NOT_FOUND) };
    let mut buf  = Vec::new(); buf.resize(file.info::<FileInfo>()?.file_size() as usize, 0);
    file.read(&mut buf)?;
    Ok(buf)
}

/// Constant-time SHA-256 of the kernel, compared with a *fused* public value.
/// Replace with TPM or signature verification in production.
fn verify_kernel_sha256(buf: &[u8], st: &mut SystemTable<Boot>) {
    use sha2::{Sha256, Digest};
    const EXPECTED: [u8; 32] = *include_bytes!("kernel.sha256");
    let mut hasher = Sha256::new(); hasher.update(buf);
    let hash = hasher.finalize();

    if hash.as_slice() != EXPECTED {
        let _ = st.stdio().stdout().write_str("!!! HASH MISMATCH -- ABORTING !!!\n");
        loop {} // hang: secure fail-stop
    }
}

/// Parse ELF headers & load PT_LOAD segments at their physical addresses.
fn load_elf_segments(elf: &[u8], st: &mut SystemTable<Boot>) -> Result<u64, Status> {
    let elf = ElfBytes::<AnyEndian>::minimal_parse(elf).map_err(|_| Status::LOAD_ERROR)?;
    for ph in elf.segments().map_err(|_| Status::LOAD_ERROR)? {
        if ph.p_type != elf::abi::PT_LOAD { continue; }
        let dest = st.boot_services().allocate_pages(
            AllocateType::Address(ph.p_paddr),
            MemoryType::LOADER_CODE,
            ((ph.p_memsz + 0xFFF) / 4096) as usize
        ).map_err(|_| Status::OUT_OF_RESOURCES)? as *mut u8;

        unsafe {
            copy_nonoverlapping(
                &elf.input()[ph.file_range().unwrap()] as *const _ as *const u8,
                dest,
                ph.p_filesz as usize);
            if ph.p_memsz > ph.p_filesz {
                core::ptr::write_bytes(dest.add(ph.p_filesz as usize), 0,
                                       (ph.p_memsz - ph.p_filesz) as usize);
            }
        }
    }
    Ok(elf.ehdr.e_entry)
}

/// Fetch current memory map (with retry loop) and return Vec + key.
fn gather_memory_map(st: &mut SystemTable<Boot>)
    -> (Vec<MemoryDescriptor>, usize)
{
    let bs = st.boot_services();
    loop {
        let map_size = bs.memory_map_size();
        let mut buf  = vec![0u8; map_size.map_size + 8*core::mem::size_of::<MemoryDescriptor>()];
        match bs.memory_map(&mut buf) {
            Ok((key, iter)) => return (iter.copied().collect(), key),
            Err(uefi::Status::BUFFER_TOO_SMALL) => continue, // try again
            Err(e) => panic!("mem map: {:?}", e),
        }
    }
}

/// alloc_error_handler ⇒ immediate halt (no heap recovery in boot phase).
#[alloc_error_handler]
fn oom(_: core::alloc::Layout) -> ! { loop {} }
