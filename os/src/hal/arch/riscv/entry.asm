    .section .text.entry
    .globl _start
_start:
# 1. 恢复 CPU ID 到 tp (你之前改的，保持住)
    mv tp, a0

    # 2. 计算当前核的栈指针
    # 假设每个核分配 64KB (16 * 4096)
    # sp = boot_stack_top - (hart_id * 65536)
    
    la sp, boot_stack_top   # 加载栈顶基地址
    li t0, 65536            # t0 = 栈大小 (16 * 4096 = 0x10000)
    
    # 计算偏移量: offset = hart_id * stack_size
    mul t0, a0, t0          
    
    # 调整 sp: sp = sp - offset
    sub sp, sp, t0

    # 3. 跳转到 rust_main
    call rust_main

    .section .bss.stack
    .globl boot_stack
boot_stack:
    .space 4096 * 16
    .globl boot_stack_top
boot_stack_top:
