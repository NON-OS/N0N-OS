#!/usr/bin/env bash
set -euo pipefail

# ─────────────────────────────────────────────────────────────
# CONFIG
# ─────────────────────────────────────────────────────────────
PROJECT_ROOT="$HOME/non-OS"          # adjust if needed
BOOT_DIR="$PROJECT_ROOT/boot"
KERNEL_DIR="$PROJECT_ROOT/kernel"
SCRIPTS_DIR="$PROJECT_ROOT/scripts"

echo "🚧  Generating ultra-advanced NON-OS boot + kernel stack…"

# ─────────────────────────────────────────────────────────────
# 1. BOOTLOADER (UEFI, measured)
# ─────────────────────────────────────────────────────────────
mkdir -p "$BOOT_DIR/src" "$BOOT_DIR/.cargo"

cat >"$BOOT_DIR/Cargo.toml" <<'EOF'
[package]
name = "nonos_boot"
version = "0.1.0"
edition = "2021"

[dependencies]
uefi          = "0.27.0"
uefi-services = "0.24.0"
log           = "0.4"
sha2          = "0.10"
EOF

cat >"$BOOT_DIR/.cargo/config.toml" <<'EOF'
[build]
target = "x86_64-unknown-uefi"
EOF

cat >"$BOOT_DIR/src/main.rs" <<'EOF'
#![no_std]
#![no_main]

use core::fmt::Write;
use sha2::{Digest, Sha256};
use uefi::prelude::*;
use uefi::proto::media::file::*;
use uefi::table::boot::{AllocateType, MemoryType};

#[repr(C)]
pub struct BootInfo<'a> {
    pub mem_map:   &'a [uefi::table::boot::MemoryDescriptor],
    pub kernel_sz: usize,
    pub kernel_sha: [u8; 32],
}

#[entry]
fn efi_main(handle: Handle, mut st: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut st).unwrap();
    let stdout = st.stdout();
    writeln!(stdout, "🪐  NON-OS secure bootloader").ok();

    // ── read kernel.bin ──
    let bs = st.boot_services();
    let fs = bs.get_image_file_system(handle).unwrap().interface.get();
    let mut root = unsafe { &mut *fs }.open_volume().unwrap();
    let file_h = root.open("kernel.bin", FileMode::Read, FileAttribute::empty()).unwrap();
    let mut kfile = match file_h.into_type().unwrap() {
        FileType::Regular(f) => f,
        _ => return Status::LOAD_ERROR,
    };
    let info = kfile.get_info::<FileInfo>().unwrap();
    let pages = ((info.file_size() + 0xFFF) / 0x1000) as usize;
    let phys = bs.allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, pages).unwrap();
    let slice = unsafe { core::slice::from_raw_parts_mut(phys as *mut u8, info.file_size() as usize) };
    kfile.read(slice).unwrap();

    // ── measurement ──
    let mut h = Sha256::new(); h.update(slice); let digest = h.finalize();
    writeln!(stdout, "✅ kernel SHA-256 {:02x}", digest).ok();

    // ── build BootInfo on stack ──
    let (_k, map_buf) = bs.memory_map_size();
    let buf = bs.allocate_pool(MemoryType::LOADER_DATA, map_buf.buffer_size()).unwrap();
    let (_, mem_map) = bs.memory_map(buf).unwrap();
    let info_struct = BootInfo {
        mem_map: unsafe { core::slice::from_raw_parts(mem_map.0, mem_map.1) },
        kernel_sz: info.file_size() as usize,
        kernel_sha: digest.into(),
    };
    let info_ptr = &info_struct as *const _ as u64;

    writeln!(stdout, "🚀 jumping to kernel").ok();
    let entry: extern "C" fn(u64) -> ! = unsafe { core::mem::transmute(phys) };
    entry(info_ptr)
}
EOF

# ─────────────────────────────────────────────────────────────
# 2. KERNEL (micro-hyperkernel skeleton)
# ─────────────────────────────────────────────────────────────
mkdir -p "$KERNEL_DIR/src/arch/x86_64" "$KERNEL_DIR/.cargo" "$KERNEL_DIR/src/mem" "$KERNEL_DIR/src/sched" "$KERNEL_DIR/src/syscall" "$KERNEL_DIR/src/crypto"

cat >"$KERNEL_DIR/Cargo.toml" <<'EOF'
[package]
name = "nonos_kernel"
version = "0.1.0"
edition = "2021"

[dependencies]
x86_64     = "0.15"
spin       = "0.9"
lazy_static = "1.4"
uart_16550 = "0.2"
hashbrown  = "0.14"

[build-dependencies]
bootloader = { version = "0.10", default-features = false }

[profile.release]
panic = "abort"
lto   = true
strip = "symbols"
codegen-units = 1
EOF

cat >"$KERNEL_DIR/.cargo/config.toml" <<'EOF'
[build]
target = "x86_64-nonos.json"
EOF

cat >"$KERNEL_DIR/x86_64-nonos.json" <<'EOF'
{
  "llvm-target": "x86_64-unknown-none",
  "data-layout": "e-m:e-i64:64-f80:128-n8:16:32:64-S128",
  "arch": "x86_64",
  "os": "none",
  "executables": true,
"linker-flavor": "ld.lld",
  "panic-strategy": "abort",
  "disable-redzone": true,
  "relocation-model": "static"
}
EOF

cat >"$KERNEL_DIR/src/lib.rs" <<'EOF'
#![no_std]
#![no_main]

mod mem;
mod arch;
mod sched;
mod syscall;
mod crypto;

use core::panic::PanicInfo;
use crypto::measurement::BootInfo;
use arch::x86_64::{gdt, idt, vga};

#[no_mangle]
pub extern "C" fn _start(info: &'static BootInfo) -> ! {
    gdt::init();
    idt::init();
    mem::init(info.mem_map);
    vga::print("🚀 NON-OS kernel online\n");
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    arch::x86_64::vga::print("💥 kernel panic\n");
    arch::x86_64::vga::print(info.payload().downcast_ref::<&str>().unwrap_or(&""));
    loop {}
}
EOF

cat >"$KERNEL_DIR/src/arch/x86_64/gdt.rs" <<'EOF'
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable};
static mut GDT: Option<GlobalDescriptorTable> = None;

pub fn init() {
    unsafe {
        GDT = Some(GlobalDescriptorTable::new());
        GDT.as_mut().unwrap().add_entry(Descriptor::kernel_code_segment());
        GDT.as_ref().unwrap().load();
    }
}
EOF

cat >"$KERNEL_DIR/src/arch/x86_64/idt.rs" <<'EOF'
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

pub fn init() {
    unsafe {
        IDT.breakpoint.set_handler_fn(breakpoint_handler);
        IDT.load();
    }
}
extern "x86-interrupt" fn breakpoint_handler(_stack: InterruptStackFrame) {
    crate::arch::x86_64::vga::print("[BP]");
}
EOF

cat >"$KERNEL_DIR/src/arch/x86_64/vga.rs" <<'EOF'
use core::ptr::write_volatile;
const VGA: *mut u8 = 0xb8000 as *mut u8;

pub fn print(s: &str) {
    for (i, b) in s.bytes().enumerate() {
        unsafe {
            write_volatile(VGA.add(i * 2), b);
            write_volatile(VGA.add(i * 2 + 1), 0x0f);
        }
    }
}
EOF

cat >"$KERNEL_DIR/src/mem/mod.rs" <<'EOF'
pub fn init(_map: &[uefi::table::boot::MemoryDescriptor]) {
    // TODO: frame allocator + heap
}
EOF

# tiny stub modules
echo "pub fn init() {}" >"$KERNEL_DIR/src/sched/mod.rs"
echo "pub fn init() {}" >"$KERNEL_DIR/src/syscall/mod.rs"
mkdir -p "$KERNEL_DIR/src/crypto"; echo "pub mod measurement { #[repr(C)] pub struct BootInfo<'a>{ pub mem_map: &'a [usize], pub kernel_sz: usize, pub kernel_sha: [u8;32] } }" >"$KERNEL_DIR/src/crypto/mod.rs"

# ─────────────────────────────────────────────────────────────
# 3. scripts/ build + qemu wrappers
# ─────────────────────────────────────────────────────────────
mkdir -p "$SCRIPTS_DIR"

cat >"$SCRIPTS_DIR/build.sh" <<'EOF'
#!/usr/bin/env bash
set -e
cargo +stable build --manifest-path boot/Cargo.toml --release
cargo +stable build --manifest-path kernel/Cargo.toml --release
echo "✅ Boot + kernel built."
EOF
chmod +x "$SCRIPTS_DIR/build.sh"

cat >"$SCRIPTS_DIR/qemu.sh" <<'EOF'
#!/usr/bin/env bash
set -e
BOOT_EFI=boot/target/x86_64-unknown-uefi/release/nonos_boot.efi
KERN_BIN=kernel/target/x86_64-unknown-none/release/nonos_kernel
IMG=nonos_fat.img
rm -f "$IMG"; truncate -s 64M "$IMG"; mkfs.vfat -n NONOS "$IMG"
mmd -i "$IMG" ::/EFI ::/EFI/BOOT
mcopy -i "$IMG" "$BOOT_EFI" ::/EFI/BOOT/BOOTX64.EFI
mcopy -i "$IMG" "$KERN_BIN" ::/kernel.bin
qemu-system-x86_64 -m 1024 -bios /usr/share/OVMF/OVMF_CODE.fd \
  -drive format=raw,file="$IMG" -serial stdio
EOF
chmod +x "$SCRIPTS_DIR/qemu.sh"

echo "✅ Non-OS boot + kernel skeleton created."
echo "Run:  $SCRIPTS_DIR/build.sh  &&  $SCRIPTS_DIR/qemu.sh"
