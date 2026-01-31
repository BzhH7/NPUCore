# Master Makefile for all applications
# 统一编译所有演示程序

SUBDIRS := kilo tetris snake 2048 utils bench demo

# 默认架构
ARCH ?= riscv64

.PHONY: all clean install $(SUBDIRS)

all: $(SUBDIRS)

$(SUBDIRS):
	@echo "Building $@..."
	$(MAKE) -C $@

clean:
	@for dir in $(SUBDIRS); do \
		echo "Cleaning $$dir..."; \
		$(MAKE) -C $$dir clean; \
	done

# 安装到文件系统镜像目录
install: all
	@echo "Installing binaries to ../fs-img-dir/fs/..."
	@mkdir -p ../fs-img-dir/fs
ifeq ($(ARCH),riscv64)
	@sudo cp -v kilo/build/kilo-riscv64 ../fs-img-dir/fs/kilo 2>/dev/null || true
	@sudo cp -v tetris/build/tetris-riscv64 ../fs-img-dir/fs/tetris 2>/dev/null || true
	@sudo cp -v snake/build/snake-riscv64 ../fs-img-dir/fs/snake 2>/dev/null || true
	@sudo cp -v 2048/build/2048-riscv64 ../fs-img-dir/fs/2048 2>/dev/null || true
	@sudo cp -v bench/build/bench-riscv64 ../fs-img-dir/fs/bench 2>/dev/null || true
	@sudo cp -v demo/build/demo-riscv64 ../fs-img-dir/fs/demo 2>/dev/null || true
	@sudo cp -v utils/build/cat-riscv64 ../fs-img-dir/fs/cat 2>/dev/null || true
	@sudo cp -v utils/build/echo-riscv64 ../fs-img-dir/fs/echo 2>/dev/null || true
	@sudo cp -v utils/build/wc-riscv64 ../fs-img-dir/fs/wc 2>/dev/null || true
	@sudo cp -v utils/build/tree-riscv64 ../fs-img-dir/fs/tree 2>/dev/null || true
	@sudo cp -v utils/build/cal-riscv64 ../fs-img-dir/fs/cal 2>/dev/null || true
	@sudo cp -v utils/build/hexdump-riscv64 ../fs-img-dir/fs/hexdump 2>/dev/null || true
	@sudo cp -v utils/build/uptime-riscv64 ../fs-img-dir/fs/uptime 2>/dev/null || true
	@sudo cp -v utils/build/ls-riscv64 ../fs-img-dir/fs/ls 2>/dev/null || true
	@sudo cp -v utils/build/pwd-riscv64 ../fs-img-dir/fs/pwd 2>/dev/null || true
	@sudo cp -v utils/build/mkdir-riscv64 ../fs-img-dir/fs/mkdir 2>/dev/null || true
	@sudo cp -v utils/build/rm-riscv64 ../fs-img-dir/fs/rm 2>/dev/null || true
	@sudo cp -v utils/build/cp-riscv64 ../fs-img-dir/fs/cp 2>/dev/null || true
	@sudo cp -v utils/build/mv-riscv64 ../fs-img-dir/fs/mv 2>/dev/null || true
	@sudo cp -v utils/build/touch-riscv64 ../fs-img-dir/fs/touch 2>/dev/null || true
	@sudo cp -v utils/build/top-riscv64 ../fs-img-dir/fs/top 2>/dev/null || true
else
	@sudo cp -v kilo/build/kilo-la64 ../fs-img-dir/fs/kilo 2>/dev/null || true
	@sudo cp -v tetris/build/tetris-la64 ../fs-img-dir/fs/tetris 2>/dev/null || true
	@sudo cp -v snake/build/snake-la64 ../fs-img-dir/fs/snake 2>/dev/null || true
	@sudo cp -v 2048/build/2048-la64 ../fs-img-dir/fs/2048 2>/dev/null || true
	@sudo cp -v bench/build/bench-la64 ../fs-img-dir/fs/bench 2>/dev/null || true
	@sudo cp -v demo/build/demo-la64 ../fs-img-dir/fs/demo 2>/dev/null || true
	@sudo cp -v utils/build/cat-la64 ../fs-img-dir/fs/cat 2>/dev/null || true
	@sudo cp -v utils/build/echo-la64 ../fs-img-dir/fs/echo 2>/dev/null || true
	@sudo cp -v utils/build/wc-la64 ../fs-img-dir/fs/wc 2>/dev/null || true
	@sudo cp -v utils/build/tree-la64 ../fs-img-dir/fs/tree 2>/dev/null || true
	@sudo cp -v utils/build/cal-la64 ../fs-img-dir/fs/cal 2>/dev/null || true
	@sudo cp -v utils/build/hexdump-la64 ../fs-img-dir/fs/hexdump 2>/dev/null || true
	@sudo cp -v utils/build/uptime-la64 ../fs-img-dir/fs/uptime 2>/dev/null || true
	@sudo cp -v utils/build/ls-la64 ../fs-img-dir/fs/ls 2>/dev/null || true
	@sudo cp -v utils/build/pwd-la64 ../fs-img-dir/fs/pwd 2>/dev/null || true
	@sudo cp -v utils/build/mkdir-la64 ../fs-img-dir/fs/mkdir 2>/dev/null || true
	@sudo cp -v utils/build/rm-la64 ../fs-img-dir/fs/rm 2>/dev/null || true
	@sudo cp -v utils/build/cp-la64 ../fs-img-dir/fs/cp 2>/dev/null || true
	@sudo cp -v utils/build/mv-la64 ../fs-img-dir/fs/mv 2>/dev/null || true
	@sudo cp -v utils/build/touch-la64 ../fs-img-dir/fs/touch 2>/dev/null || true
	@sudo cp -v utils/build/top-la64 ../fs-img-dir/fs/top 2>/dev/null || true
endif
	@echo "Installation complete!"

# 快捷命令
riscv: ARCH=riscv64
riscv: all install

la64: ARCH=la64
la64: all install

help:
	@echo "OS Kernel Demo Applications Build System"
	@echo ""
	@echo "Usage:"
	@echo "  make              - Build all applications"
	@echo "  make clean        - Clean all build artifacts"
	@echo "  make install      - Install to fs-img-dir/fs/ (default: riscv64)"
	@echo "  make install ARCH=la64    - Install LoongArch64 binaries"
	@echo "  make install ARCH=riscv64 - Install RISC-V64 binaries"
	@echo "  make riscv        - Build and install for RISC-V64"
	@echo "  make la64         - Build and install for LoongArch64"
	@echo ""
	@echo "Applications:"
	@echo "  Games:       tetris, snake, 2048"
	@echo "  Editor:      kilo"
	@echo "  Utilities:   cat, echo, wc, tree, cal, hexdump, uptime"
	@echo "               ls, pwd, mkdir, rm, cp, mv, touch, top"
	@echo "  Benchmark:   bench"
	@echo "  Launcher:    demo"
