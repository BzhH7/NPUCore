//! Thread synchronization primitives
//!
//! This module implements:
//! - Futex (Fast Userspace Mutex) operations
//! - Wait queue management for blocking threads
//! - Timeout-based waiting

/*
    此文件内容用于
    内容与RISCV版本相同，无需修改
*/
use crate::{syscall::errno::*, task::current_task, timer::TimeSpec};
use alloc::{collections::BTreeMap, sync::Arc};
use log::*;
use num_enum::FromPrimitive;

use super::{
    block_current_and_run_next,
    manager::{wait_with_timeout, WaitQueue},
};

#[allow(unused)]
#[derive(Debug, Eq, PartialEq, FromPrimitive)]
#[repr(u32)]
/// Futex operation commands
///
/// Defines the types of operations supported by the futex system call
pub enum FutexCmd {
    /// Wait on futex if value matches
    ///
    /// Tests that the futex word contains the expected value,
    /// then sleeps waiting for FUTEX_WAKE. The load, comparison,
    /// and sleep are atomic.
    Wait = 0,

    /// Wake waiters on futex
    ///
    /// Wakes at most `val` waiters waiting on this futex.
    /// Common values: 1 (single waiter) or INT_MAX (all waiters)
    Wake = 1,

    /// File descriptor operations (not implemented)
    Fd = 2,
    /// Requeue waiters (not implemented)
    Requeue = 3,
    /// Compare and requeue (not implemented)
    CmpRequeue = 4,
    /// Wake with operation (not implemented)
    WakeOp = 5,
    /// Priority inheritance lock (not implemented)
    LockPi = 6,
    /// Priority inheritance unlock (not implemented)
    UnlockPi = 7,
    /// Try priority inheritance lock (not implemented)
    TrylockPi = 8,
    /// Wait with bitset (not implemented)
    WaitBitset = 9,

    #[num_enum(default)]
    /// Invalid operation
    Invalid,
}

/// Fast Userspace Mutex (Futex)
///
/// Manages wait queues for futex operations. Maps futex addresses
/// to their associated wait queues.
pub struct Futex {
    inner: BTreeMap<usize, WaitQueue>,
}

/// Implement futex wait operation
///
/// Atomically checks if the futex word matches the expected value,
/// and if so, blocks the current task until woken or timeout expires.
///
/// # Arguments
/// * `futex_word` - Pointer to the futex variable
/// * `val` - Expected value; fails if current value doesn't match
/// * `timeout` - Optional timeout duration
///
/// # Returns
/// * `SUCCESS` on successful wake
/// * `EAGAIN` if futex value doesn't match
/// * `EINTR` if interrupted by signal
///
/// # Note
/// Currently ignores the `rt_clk` parameter
pub fn do_futex_wait(futex_word: &mut u32, val: u32, timeout: Option<TimeSpec>) -> isize {
    // Convert relative timeout to absolute time
    let timeout = timeout.map(|t| t + TimeSpec::now());

    // Get futex address as key
    let futex_word_addr = futex_word as *const u32 as usize;

    // Atomically check value and block
    if *futex_word != val {
        trace!(
            "[futex] --wait-- **not match** futex: {:X}, val: {:X}",
            *futex_word,
            val
        );
        return EAGAIN;
    } else {
        let task = current_task().unwrap();

        // 获取 Futex 的锁，以便修改等待队列。
        let mut futex = task.futex.lock();

        // 从 Futex 的等待队列中移除当前地址对应的队列（如果存在），否则创建一个新的等待队列。
        let mut wait_queue = if let Some(wait_queue) = futex.inner.remove(&futex_word_addr) {
            wait_queue
        } else {
            WaitQueue::new()
        };

        // 将当前任务添加到等待队列中
        // 使用 `Arc::downgrade` 将任务的强引用转换为弱引用，避免循环利用
        wait_queue.add_task(Arc::downgrade(&task));

        // 将更新后的等待队列重新插入到 Futex 的等待队列中。
        futex.inner.insert(futex_word_addr, wait_queue);

        // 如果指定了超时时间，将任务添加到超时等待队列中
        if let Some(timeout) = timeout {
            trace!("[do_futex_wait] sleep with timeout: {:?}", timeout);
            wait_with_timeout(Arc::downgrade(&task), timeout);
        }

        // 释放 Futex 锁和任务引用，避免死锁
        drop(futex);
        drop(task);

        // 阻塞当前任务并切换到下一个任务。
        block_current_and_run_next();

        // 当前任务被唤醒后，重新获取当前任务的引用。
        let task = current_task().unwrap();

        // 获取任务内部锁，以便检查信号。
        let inner = task.acquire_inner_lock();
        // 检查是否有未屏蔽的信号挂起
        if !inner.sigpending.difference(inner.sigmask).is_empty() {
            // 有未屏蔽的信号，返回 `EINTR` 错误。
            return EINTR;
        }

        // 如果没有信号中断，返回成功。
        SUCCESS
    }
}

// Futex的方法实现
impl Futex {
    /// 创建一个新的Futex
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    /// 唤醒等待在指定 Futex 地址上的最多 val 个任务
    pub fn wake(&mut self, futex_word_addr: usize, val: u32) -> isize {
        if let Some(mut wait_queue) = self.inner.remove(&futex_word_addr) {
            let ret = wait_queue.wake_at_most(val as usize);
            if !wait_queue.is_empty() {
                self.inner.insert(futex_word_addr, wait_queue);
            }
            ret as isize
        } else {
            0
        }
    }

    /// 重新排列
    pub fn requeue(&mut self, futex_word: &u32, futex_word_2: &u32, val: u32, val2: u32) -> isize {
        let futex_word_addr = futex_word as *const u32 as usize;
        let futex_word_addr_2 = futex_word_2 as *const u32 as usize;
        let wake_cnt = if val != 0 {
            self.wake(futex_word_addr, val)
        } else {
            0
        };
        if let Some(mut wait_queue) = self.inner.remove(&futex_word_addr) {
            let mut wait_queue_2 = if let Some(wait_queue) = self.inner.remove(&futex_word_addr_2) {
                wait_queue
            } else {
                WaitQueue::new()
            };
            let mut requeue_cnt = 0;
            if val2 != 0 {
                while let Some(task) = wait_queue.pop_task() {
                    wait_queue_2.add_task(task);
                    requeue_cnt += 1;
                    if requeue_cnt == val2 as isize {
                        break;
                    }
                }
            }
            if !wait_queue.is_empty() {
                self.inner.insert(futex_word_addr, wait_queue);
            }
            if !wait_queue_2.is_empty() {
                self.inner.insert(futex_word_addr_2, wait_queue_2);
            }
            wake_cnt + requeue_cnt
        } else {
            wake_cnt
        }
    }

    /// 清空队列
    pub fn clear(&mut self) {
        self.inner.clear();
    }
}
