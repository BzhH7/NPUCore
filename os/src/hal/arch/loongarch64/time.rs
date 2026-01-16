use core::arch::asm;

use crate::config;

pub const TICKS_PER_SEC: usize = 25;

/// Return current time measured by ticks, which is NOT divided by frequency.
pub fn get_time() -> usize {
    let mut counter: usize;
    unsafe {
        asm!(
        "rdtime.d {},{}",
        out(reg)counter,
        out(reg)_,
        );
    }
    counter
}

#[inline(always)]
pub fn get_clock_freq() -> usize {
    unsafe { super::config::CLOCK_FREQ }
}
pub fn get_timer_freq_first_time() {
    // 获取时钟晶振频率
    // 配置信息字index:4
    let base_freq = config::CPUCfg4::read().get_bits(0, 31);
    // 获取时钟倍频因子
    // 配置信息字index:5 位:0-15
    let cfg5 = config::CPUCfg5::read();
    let mul = cfg5.get_bits(0, 15);
    let div = cfg5.get_bits(16, 31);
    
    // 计算时钟频率 (防止除以0)
    let cc_freq = if div != 0 && mul != 0 {
        base_freq * mul / div
    } else if base_freq != 0 {
        // 如果 mul/div 无效，使用 base_freq 作为时钟频率
        base_freq
    } else {
        // 使用默认的 100MHz 作为后备值
        100_000_000
    };
    println!(
        "[get_timer_freq_first_time] clk freq: {}(from CPUCFG)",
        cc_freq
    );
    unsafe { super::config::CLOCK_FREQ = cc_freq as usize }
}
