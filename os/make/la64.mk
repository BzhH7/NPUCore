# Target configuration
TARGET := loongarch64-unknown-none
MODE := release
KERNEL_ELF := target/$(TARGET)/$(MODE)/os
KERNEL_BIN := $(KERNEL_ELF).bin
DISASM_TMP := target/$(TARGET)/$(MODE)/asm
BLK_MODE := virt_pci
FS_MODE ?= ext4
ROOTFS_IMG_NAME := rootfs-la.img
ROOTFS_IMG_DIR := ../fs-img-dir
CORE_NUM := 1
LOG := off
KERNEL_LA := ../kernel-la
SDCARD_LA := sdcard.img 

# BOARD
BOARD ?= 2k1000

# Logging config
ifndef LOG
	LOG_OPTION := "log_off"
else
	LOG_OPTION := "log_${LOG}"
endif

KERNEL_ENTRY_PA := 0x9000000008000000
LA_LOAD_ADDR := 0x9000000008000000
LA_ENTRY_POINT := 0x9000000008000000
QEMU_BIOS_DIR := ../util/tmp/qemu/2k1000
QEMU_BIOS_SRC := $(QEMU_BIOS_DIR)/u-boot-with-spl.bin
QEMU_BIOS_RUN := target/u-boot-run.bin


KERNEL_UIMG := target/$(TARGET)/$(MODE)/uImage

TFTP_DIR := ../fs-img-dir
DISK_IMG := $(TFTP_DIR)/rootfs-la.img

# Binutils
OBJDUMP := rust-objdump --arch-name=loongarch64
OBJCOPY := rust-objcopy --binary-architecture=loongarch64

# Applications
APPS := ../user/src/bin/*

# FS image
ROOTFS_IMG := ${ROOTFS_IMG_DIR}/${ROOTFS_IMG_NAME}

# Build rules
all: fs-img build

mv:
	cp -f $(KERNEL_ELF) $(KERNEL_LA)

build: env $(KERNEL_BIN) mv

env:
	(rustup target list | grep "$(TARGET) (installed)") || rustup target add $(TARGET)
	rustup component add rust-src
	rustup component add llvm-tools-preview

# build all user programs
user:
	@cd ../user && make rust-user BOARD=$(BOARD) MODE=$(MODE)

$(KERNEL_BIN): kernel
	@$(OBJCOPY) $(KERNEL_ELF) --strip-all -O binary $@

fs-img: user
	./buildfs.sh "$(ROOTFS_IMG)" "$(BOARD)" $(MODE) $(FS_MODE)

kernel:
	@echo "Restoring .cargo configuration..."
	@if [ -d cargo_config ]; then \
		rm -rf .cargo; \
		mv cargo_config .cargo; \
	fi
	@echo Platform: $(BOARD)
ifeq ($(MODE), debug)
	@LOG=$(LOG) cargo build --features "board_$(BOARD) $(LOG_OPTION) block_$(BLK_MODE) oom_handler" --no-default-features --target loongarch64-unknown-none
else
	@LOG=$(LOG) cargo build --release --features "board_$(BOARD) $(LOG_OPTION) block_$(BLK_MODE) oom_handler" --no-default-features --target loongarch64-unknown-none
endif
	@mv .cargo cargo_config;

clean:
	@cargo clean
	@rm -rf $(KERNEL_LA)
	-@cd ../../user && make clean

uimage: $(KERNEL_BIN)
	../util/mkimage -A loongarch -O linux -T kernel -C none \
		-a $(LA_LOAD_ADDR) -e $(LA_ENTRY_POINT) \
		-n NPUcore+ -d $(KERNEL_BIN) $(KERNEL_UIMG)
	
	@echo "uImage generated at $(KERNEL_UIMG)"

RUN_SCRIPT := ./run_script

prepare-qemu: uimage
	@echo "Preparing QEMU environment..."
	@if [ -f $(QEMU_BIOS_SRC) ]; then \
		cp $(QEMU_BIOS_SRC) $(QEMU_BIOS_RUN); \
		truncate -s 16M $(QEMU_BIOS_RUN); \
	else \
		echo "Error: BIOS file not found at $(QEMU_BIOS_SRC)"; \
		exit 1; \
	fi
	@dd if=/dev/zero bs=1M count=16 2>/dev/null | tr '\000' '\377' > target/nand.dat
	@mkdir -p $(TFTP_DIR)
	@cp $(KERNEL_UIMG) $(TFTP_DIR)/uImage
	@echo "uImage copied to $(TFTP_DIR)/uImage"

run: prepare-qemu
	DEBUG_GMAC_PHYAD=0 $(RUN_SCRIPT) \
	qemu-system-loongarch64 \
	-M ls2k \
	-m 1024 \
	-smp threads=1 \
	-serial stdio \
	-vnc :0 \
	-drive if=pflash,file=$(QEMU_BIOS_RUN),format=raw \
	-drive if=mtd,file=target/nand.dat,format=raw \
	-hda $(DISK_IMG) \
	-net nic -net user,net=192.168.1.2/24,tftp=$(TFTP_DIR) \
	-s

monitor:
	loongarch64-unknown-elf-gdb -ex 'file target/loongarch64-unknown-none/debug/os' -ex 'set arch loongarch64' -ex 'target remote localhost:1234'

.PHONY: all build kernel fs-img user clean run monitor