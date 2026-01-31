# 2048 Game Makefile

SRC := 2048.c

CFLAGS := -Wall -W -pedantic -std=c99 -static -fno-stack-protector -O2

LA64_CC  := loongarch64-linux-gnu-gcc
RISCV_CC := riscv64-linux-gnu-gcc

OUTDIR := build
LA64_OUT  := $(OUTDIR)/2048-la64
RISCV_OUT := $(OUTDIR)/2048-riscv64

all: $(LA64_OUT) $(RISCV_OUT)

$(LA64_OUT): $(SRC)
	mkdir -p $(OUTDIR)
	$(LA64_CC) $(CFLAGS) -o $@ $<

$(RISCV_OUT): $(SRC)
	mkdir -p $(OUTDIR)
	$(RISCV_CC) $(CFLAGS) -o $@ $<

clean:
	rm -rf $(OUTDIR)

.PHONY: all clean
