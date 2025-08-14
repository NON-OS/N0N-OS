#!/bin/bash
# scripts/build-and-run.sh
# Complete build script for NØNOS kernel

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Paths
KERNEL_DIR="$(dirname "$0")/../kernel"
TARGET_DIR="$KERNEL_DIR/target/x86_64-nonos/release"
BUILD_DIR="$KERNEL_DIR/target/build"

echo -e "${GREEN}Building NØNOS Kernel...${NC}"

# Step 1: Build the kernel
cd "$KERNEL_DIR"
cargo build --release --target x86_64-nonos.json

echo -e "${GREEN}Kernel built successfully!${NC}"

# Step 2: Create bootable image using bootimage
if ! command -v bootimage &> /dev/null; then
    echo -e "${YELLOW}Installing bootimage tool...${NC}"
    cargo install bootimage
fi

# Step 3: Build bootable image
echo -e "${GREEN}Creating bootable image...${NC}"
cargo bootimage --target x86_64-nonos.json --release

# Step 4: Create ESP (EFI System Partition) structure
echo -e "${GREEN}Creating ESP structure...${NC}"
mkdir -p "$BUILD_DIR/esp/EFI/BOOT"

# Copy kernel to ESP
cp "$TARGET_DIR/nonos_kernel" "$BUILD_DIR/esp/kernel.bin"

# Step 5: Create disk image
echo -e "${GREEN}Creating disk image...${NC}"
dd if=/dev/zero of="$BUILD_DIR/nonos.img" bs=1M count=64 2>/dev/null
mkfs.vfat "$BUILD_DIR/nonos.img"

# Mount and copy files (requires sudo)
if [ "$EUID" -eq 0 ]; then
    MOUNT_DIR="/tmp/nonos_mount_$$"
    mkdir -p "$MOUNT_DIR"
    mount -o loop "$BUILD_DIR/nonos.img" "$MOUNT_DIR"
    cp -r "$BUILD_DIR/esp/"* "$MOUNT_DIR/"
    umount "$MOUNT_DIR"
    rmdir "$MOUNT_DIR"
else
    echo -e "${YELLOW}Skipping disk image population (requires sudo)${NC}"
fi

# Step 6: Run in QEMU
echo -e "${GREEN}Launching QEMU...${NC}"

# Check if KVM is available
KVM_ARGS=""
if [ -w /dev/kvm ]; then
    KVM_ARGS="-enable-kvm -cpu host"
    echo -e "${GREEN}KVM acceleration enabled${NC}"
else
    echo -e "${YELLOW}KVM not available, using TCG${NC}"
fi

# QEMU command
qemu-system-x86_64 \
    $KVM_ARGS \
    -machine q35 \
    -m 512M \
    -smp 2 \
    -serial stdio \
    -drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE.fd \
    -drive if=pflash,format=raw,file=/tmp/OVMF_VARS_$$.fd \
    -drive format=raw,file="$BUILD_DIR/nonos.img" \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -display gtk \
    -monitor telnet:127.0.0.1:45454,server,nowait \
    -gdb tcp::9000 \
    -d int,cpu_reset \
    -no-reboot \
    -no-shutdown

# Cleanup
rm -f /tmp/OVMF_VARS_$$.fd
echo -e "${GREEN}Done!${NC}"
