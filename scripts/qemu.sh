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
