# paths
ROOT := $(abspath $(CURDIR)/..)
BOOT := $(ROOT)/boot
KERN := $(ROOT)/kernel
ESP  := $(ROOT)/target/esp
IMG  := $(ROOT)/target/nonos_esp.img
OVMF_CODE ?= /usr/share/OVMF/OVMF_CODE.fd
OVMF_VARS ?= /usr/share/OVMF/OVMF_VARS.fd

QEMU := qemu-system-x86_64
MTOOLS := mmd mcopy mkfs.vfat

.PHONY: all build release run rrun esp img qemu lint fmt clippy check clean ci-smoke

all: build

build:
	cargo build

release:
	cargo build --release

run: esp img qemu

rrun:
	NONOS_RUNNER=1 cargo run --release

esp:
	rm -rf $(ESP); mkdir -p $(ESP)/EFI/BOOT
	$(MAKE) -C $(KERN) release
	$(MAKE) -C $(BOOT) release
	cp $(BOOT)/target/release/nonos_boot $(ESP)/EFI/BOOT/BOOTX64.EFI
	cp $(KERN)/target/x86_64-unknown-none/release/nonos_kernel $(ESP)/kernel.bin

img:
	mkdir -p $(ROOT)/target
	truncate -s 64M $(IMG)
	mkfs.vfat $(IMG)
	mmd -i $(IMG) ::/EFI ::/EFI/BOOT
	mcopy -i $(IMG) $(ESP)/EFI/BOOT/BOOTX64.EFI ::/EFI/BOOT/
	mcopy -i $(IMG) $(ESP)/kernel.bin ::/

qemu:
	$(QEMU) -machine q35 -m 512M \
	  -drive if=pflash,format=raw,readonly=on,file=$(OVMF_CODE) \
	  -drive if=pflash,format=raw,file=$(OVMF_VARS) \
	  -drive format=raw,file=$(IMG) \
	  -serial stdio

fmt:
	cargo fmt --all

clippy:
	cargo clippy --no-deps -- -D warnings

lint: fmt clippy

check:
	cargo check

clean:
	cargo clean
	rm -rf $(ESP) $(IMG)

ci-smoke: esp img
	timeout 10s $(QEMU) -nographic -serial stdio \
	  -machine q35 -m 512M \
	  -drive if=pflash,format=raw,readonly=on,file=$(OVMF_CODE) \
	  -drive if=pflash,format=raw,file=$(OVMF_VARS) \
	  -drive format=raw,file=$(IMG) || true
