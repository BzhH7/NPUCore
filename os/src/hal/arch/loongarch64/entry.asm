    .section .text.entry
    .globl _start
_start:
# 把默认0x8…和9的窗给关了，全开成0的窗，这样就相当于0的这个部分是地址恒等映射，直接继承原来的代码
    pcaddi      $t0,    0x0
    srli.d      $t0,    $t0,    0x30
    slli.d      $t0,    $t0,    0x30
    addi.d      $t0,    $t0,    0x11
    csrwr       $t0,    0x181   # Make sure the window remains the same after the switch.
    # 前5行是把当前PC所在段给保留下来,存到DMW1
    # 然后改DMW0
    # 使用sub生成0,因为有些版本的虚拟机上面zero会被赋值,避免使用zero
    sub.d       $t0,    $t0,    $t0  # 使$t0为0
    addi.d      $t0,    $t0,    0x11
    csrwr       $t0,    0x180        # 将DMW0设置为0
    pcaddi      $t0,    0x0          # 获取当前PC
    slli.d      $t0,    $t0,    0x10 # 左移16位
    srli.d      $t0,    $t0,    0x10 # 右移16位
    # 上面两条指令的作用为将当前PC的高16位清零
    jirl        $t0,    $t0,    0x10 # 跳0段的下一条指令
    # The barrier
    sub.d       $t0,    $t0,    $t0
    csrwr       $t0,    0x181

    # 设置DMW2用于段8的非缓存IO访问 (UART等外设)
    # DMW2 = 0x8000_0000_0000_0011
    # bit 0: PLV0 = 1 (allow PLV0 access)
    # bit 4: MAT = 1 (Strongly Ordered Uncached)
    # bit 60-63: VSEG = 8
    lu12i.w     $t0,    0x0
    ori         $t0,    $t0,    0x11
    lu32i.d     $t0,    0x0
    lu52i.d     $t0,    $t0,    -2048    # 0x800 -> segment 8
    csrwr       $t0,    0x182            # DMW2

    sub.d       $t0,    $t0,    $t0
    la.global   $sp, boot_stack_top

    # 读取CPU ID作为rust_main的第一个参数 (hart_id)
    csrrd       $a0,    0x20             # 读取CPUID到$a0

    bl          rust_main

    .section .bss.stack
    .globl boot_stack
boot_stack:
    .space 4096 * 16
    .globl boot_stack_top
boot_stack_top:
