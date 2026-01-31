# System Utilities Makefile

SRCS := cat.c echo.c wc.c tree.c cal.c hexdump.c uptime.c ls.c pwd.c mkdir.c rm.c cp.c mv.c touch.c top.c

CFLAGS := -Wall -W -std=c99 -static -fno-stack-protector -O2

LA64_CC  := loongarch64-linux-gnu-gcc
RISCV_CC := riscv64-linux-gnu-gcc

OUTDIR := build

# 生成目标文件名列表
LA64_OUTS  := $(patsubst %.c,$(OUTDIR)/%-la64,$(SRCS))
RISCV_OUTS := $(patsubst %.c,$(OUTDIR)/%-riscv64,$(SRCS))

all: $(LA64_OUTS) $(RISCV_OUTS)

$(OUTDIR)/%-la64: %.c
	mkdir -p $(OUTDIR)
	$(LA64_CC) $(CFLAGS) -o $@ $<

$(OUTDIR)/%-riscv64: %.c
	mkdir -p $(OUTDIR)
	$(RISCV_CC) $(CFLAGS) -o $@ $<

clean:
	rm -rf $(OUTDIR)

.PHONY: all clean
