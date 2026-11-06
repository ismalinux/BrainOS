BUILD_DIR := build
ISO_ROOT := $(BUILD_DIR)/iso
LD = ld
NASM = nasm
CARGO = cargo
GRUB_MKRESCUE = grub-mkrescue
XORRISO = xorriso
QEMU = qemu-system-x86_64
QEMU_MEMORY ?= 2G
DISK_IMAGE ?= build/brainos-data.img
RUST_TARGET := x86_64-unknown-none
RUST_LIB := target/$(RUST_TARGET)/release/libbrainos_kernel.a

.PHONY: all kernel iso run run-disk clean

all: kernel

kernel: $(BUILD_DIR)/kernel.elf

$(BUILD_DIR):
	mkdir -p $@

$(BUILD_DIR)/boot.o: boot/boot.asm | $(BUILD_DIR)
	$(NASM) -f elf64 $< -o $@

$(RUST_LIB): Cargo.toml src/main.rs src/ata_pio.rs src/cmos.rs src/cosmic_editor.rs \
	src/framebuffer.rs src/input.rs src/memory_map.rs src/trmnl.rs \
	src/cosmic_desktop.rs src/cosmic_files.rs src/cosmic_settings.rs \
	src/cosmic_store.rs src/recovery_ui.rs kernel/runtime.rs
	$(CARGO) build --release --lib --target $(RUST_TARGET)

$(BUILD_DIR)/kernel.elf: $(BUILD_DIR)/boot.o $(RUST_LIB) linker.ld
	$(LD) -m elf_x86_64 -nostdlib -T linker.ld -o $@ $(BUILD_DIR)/boot.o $(RUST_LIB)

iso: $(BUILD_DIR)/kernel.elf
	@command -v $(GRUB_MKRESCUE) >/dev/null || { echo "Missing $(GRUB_MKRESCUE); install GRUB PC BIOS tools."; exit 1; }
	@command -v $(XORRISO) >/dev/null || { echo "Missing $(XORRISO); install xorriso to build the ISO."; exit 1; }
	mkdir -p $(ISO_ROOT)/boot/grub
	cp $(BUILD_DIR)/kernel.elf $(ISO_ROOT)/boot/kernel.elf
	cp grub/grub.cfg $(ISO_ROOT)/boot/grub/grub.cfg
	$(GRUB_MKRESCUE) -o $(BUILD_DIR)/brainos.iso $(ISO_ROOT)

run: iso
	@command -v $(QEMU) >/dev/null || { echo "Missing $(QEMU); install QEMU to run Brain OS."; exit 1; }
	$(QEMU) -m $(QEMU_MEMORY) -cdrom $(BUILD_DIR)/brainos.iso

$(DISK_IMAGE): | $(BUILD_DIR)
	@command -v qemu-img >/dev/null || { echo "Missing qemu-img; install QEMU utilities for a test disk."; exit 1; }
	qemu-img create -f raw $@ 64M

run-disk: iso $(DISK_IMAGE)
	@command -v $(QEMU) >/dev/null || { echo "Missing $(QEMU); install QEMU to run Brain OS."; exit 1; }
	$(QEMU) -m $(QEMU_MEMORY) -cdrom $(BUILD_DIR)/brainos.iso -drive file=$(DISK_IMAGE),format=raw,if=ide,index=0

clean:
	rm -rf $(BUILD_DIR) target
