#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BOOT="$ROOT/boot"
KERNEL="$ROOT/kernel"
ESP="$ROOT/target/esp"
IMG="$ROOT/target/nonos_esp.img"

( cd "$KERNEL" && cargo build --release )
( cd "$BOOT"   && cargo build --release )

rm -rf "$ESP"; mkdir -p "$ESP/EFI/BOOT"
cp "$BOOT/target/release/nonos_boot" "$ESP/EFI/BOOT/BOOTX64.EFI"
cp "$KERNEL/target/x86_64-unknown-none/release/nonos_kernel" "$ESP/kernel.bin"

mkdir -p "$ROOT/target"
truncate -s 64M "$IMG"
mkfs.vfat "$IMG"
mmd   -i "$IMG" ::/EFI ::/EFI/BOOT
mcopy -i "$IMG" "$ESP/EFI/BOOT/BOOTX64.EFI" ::/EFI/BOOT/
mcopy -i "$IMG" "$ESP/kernel.bin" ::/

qemu-system-x86_64 -machine q35 -m 512M \
  -drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE.fd \
  -drive if=pflash,format=raw,file=/usr/share/OVMF/OVMF_VARS.fd \
  -drive format=raw,file="$IMG"
