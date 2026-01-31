# Benchmark Suite Makefile

SRC := bench.c

CFLAGS := -Wall -W -std=c99 -static -fno-stack-protector -O2

LA64_CC  := loongarch64-linux-gnu-gcc
RISCV_CC := riscv64-linux-gnu-gcc

OUTDIR := build
LA64_OUT  := $(OUTDIR)/bench-la64
RISCV_OUT := $(OUTDIR)/bench-riscv64

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
