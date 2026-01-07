.section .text.entry
    .globl _start
_start:
    # 1. 恢复 CPU ID 到 tp
    mv tp, a0

    # 2. 计算当前核的栈指针
    # 假设每个核分配 64KB (16 * 4096)
    # sp = boot_stack_top - (hart_id * 65536)
    
    la sp, boot_stack_top   # 加载栈顶基地址
    
    # 【修复】使用位移指令 slli 代替 mul
    # 65536 = 2^16，所以左移 16 位等价于乘以 65536
    # 这样既避免了 Zmmul 扩展依赖导致的编译错误，又更高效
    slli t0, a0, 16         # t0 = hart_id << 16 (即 hart_id * 65536)
    
    # 调整 sp: sp = sp - offset
    sub sp, sp, t0

    # 3. 跳转到 rust_main
    call rust_main

    .section .bss.stack
    .globl boot_stack
boot_stack:
    # 空间大小：4096 * 16 * 4 (支持4个核，每核16页)
    .space 4096 * 16 * 4
    .globl boot_stack_top
boot_stack_top: