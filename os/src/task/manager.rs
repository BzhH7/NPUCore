/*
    此文件用于管理任务的调度
    使用多级调度框架：RT (FIFO/RR) -> CFS -> Idle
*/
use core::cmp::Ordering;

#[cfg(feature = "oom_handler")]
use crate::config::SYSTEM_TASK_LIMIT;
#[cfg(feature = "oom_handler")]
use alloc::vec::Vec;

use crate::timer::TimeSpec;
use crate::config::MAX_CPU_NUM;
use crate::utils::InterruptGuard;

use super::cfs_scheduler::{CfsRunQueue, SchedPolicy};
use super::sched_class::{RtRunQueue, IdleRunQueue, get_sched_class, SchedClass};
use super::{current_task, TaskControlBlock};
use alloc::collections::{BinaryHeap, VecDeque};
use alloc::sync::{Arc, Weak};
use lazy_static::*;
use spin::Mutex;
use crate::task::processor::current_cpu_id;

#[cfg(feature = "oom_handler")]
/// 任务的激活状态跟踪器
pub struct ActiveTracker {
    /// 存储激活状态的位图
    bitmap: Vec<u64>,
}

#[cfg(feature = "oom_handler")]
#[allow(unused)]
impl ActiveTracker {
    /// 默认大小为128
    pub const DEFAULT_SIZE: usize = SYSTEM_TASK_LIMIT;
    /// 构造函数
    pub fn new() -> Self {
        // 计算位图长度，向上取整
        let len = (Self::DEFAULT_SIZE + 63) / 64;
        // 初始化位图
        let mut bitmap = Vec::with_capacity(len);
        // 位图全部置0
        bitmap.resize(len, 0);
        Self { bitmap }
    }
    /// 检查制定pid的任务是否处于激活状态
    pub fn check_active(&self, pid: usize) -> bool {
        (self.bitmap[pid / 64] & (1 << (pid % 64))) != 0
    }
    /// 检查制定pid的任务是否处于非激活状态
    pub fn check_inactive(&self, pid: usize) -> bool {
        (self.bitmap[pid / 64] & (1 << (pid % 64))) == 0
    }
    /// 标记指定pid的任务为激活状态
    pub fn mark_active(&mut self, pid: usize) {
        self.bitmap[pid / 64] |= 1 << (pid % 64)
    }
    /// 标记指定pid的任务为非激活状态
    pub fn mark_inactive(&mut self, pid: usize) {
        self.bitmap[pid / 64] &= !(1 << (pid % 64))
    }
}

#[cfg(feature = "oom_handler")]
/// 任务管理器 (多级调度：RT -> CFS -> Idle)
pub struct TaskManager {
    /// RT运行队列 (FIFO/RR，最高优先级)
    pub rt_rq: RtRunQueue,
    /// CFS运行队列，用于存储就绪态任务
    pub cfs_rq: CfsRunQueue,
    /// Idle运行队列 (最低优先级)
    pub idle_rq: IdleRunQueue,
    /// 一个双端队列，用于存储可中断状态任务
    pub interruptible_queue: VecDeque<Arc<TaskControlBlock>>,
    /// 任务激活状态跟踪器，用于跟踪任务的激活状态，并在OOM时释放内存
    pub active_tracker: ActiveTracker,
}


#[cfg(not(feature = "oom_handler"))]
pub struct TaskManager {
    /// RT运行队列 (FIFO/RR，最高优先级)
    pub rt_rq: RtRunQueue,
    /// CFS运行队列，用于存储就绪态任务
    pub cfs_rq: CfsRunQueue,
    /// Idle运行队列 (最低优先级)
    pub idle_rq: IdleRunQueue,
    pub interruptible_queue: VecDeque<Arc<TaskControlBlock>>,
}

/// 多级调度器
impl TaskManager {
    #[cfg(feature = "oom_handler")]
    /// 构造函数
    pub fn new() -> Self {
        Self {
            rt_rq: RtRunQueue::new(),
            cfs_rq: CfsRunQueue::new(),
            idle_rq: IdleRunQueue::new(),
            interruptible_queue: VecDeque::new(),
            active_tracker: ActiveTracker::new(),
        }
    }
    #[cfg(not(feature = "oom_handler"))]
    pub fn new() -> Self {
        Self {
            rt_rq: RtRunQueue::new(),
            cfs_rq: CfsRunQueue::new(),
            idle_rq: IdleRunQueue::new(),
            interruptible_queue: VecDeque::new(),
        }
    }
    /// 添加一个任务到对应的就绪队列（根据调度策略）
    pub fn add(&mut self, task: Arc<TaskControlBlock>) {
        let mut inner = task.acquire_inner_lock();
        let sched_class = get_sched_class(&inner.sched_entity);
        
        match sched_class {
            SchedClass::Rt => {
                self.rt_rq.enqueue(task.clone(), &inner.sched_entity);
            }
            SchedClass::Cfs => {
                let is_new = inner.sched_entity.sum_exec_runtime == 0;
                self.cfs_rq.enqueue(task.clone(), &mut inner.sched_entity, is_new);
            }
            SchedClass::Idle => {
                self.idle_rq.enqueue(task.clone());
            }
        }
        drop(inner);
    }
    /// 从就绪队列中取出下一个任务（按优先级：RT -> CFS -> Idle）
    #[cfg(feature = "oom_handler")]
    pub fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
        // 1. 先检查RT队列
        if let Some(task) = self.rt_rq.pick_next() {
            self.active_tracker.mark_active(task.pid.0);
            return Some(task);
        }
        // 2. 再检查CFS队列
        if let Some(task) = self.cfs_rq.pick_next() {
            self.active_tracker.mark_active(task.pid.0);
            return Some(task);
        }
        // 3. 最后检查Idle队列
        if let Some(task) = self.idle_rq.pick_next() {
            self.active_tracker.mark_active(task.pid.0);
            return Some(task);
        }
        None
    }
    #[cfg(not(feature = "oom_handler"))]
    pub fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
        // 1. 先检查RT队列
        if let Some(task) = self.rt_rq.pick_next() {
            return Some(task);
        }
        // 2. 再检查CFS队列
        if let Some(task) = self.cfs_rq.pick_next() {
            return Some(task);
        }
        // 3. 最后检查Idle队列
        self.idle_rq.pick_next()
    }
    
    /// 尝试从CFS队列偷取一个任务（用于Work Stealing）
    /// 返回vruntime最大的任务（即最不紧急的任务）
    pub fn steal_from_cfs(&mut self) -> Option<Arc<TaskControlBlock>> {
        // 偷取CFS队列中vruntime最大的任务
        // 这需要CfsRunQueue提供pop_last方法
        // 暂时使用pick_next，后续可以优化
        if self.cfs_rq.len() >= 2 {
            self.cfs_rq.pick_next()
        } else {
            None
        }
    }
    
    /// 获取总任务数
    pub fn total_count(&self) -> usize {
        self.rt_rq.len() + self.cfs_rq.len() + self.idle_rq.len()
    }
    /// 添加一个任务到可中断队列
    pub fn add_interruptible(&mut self, task: Arc<TaskControlBlock>) {
        self.interruptible_queue.push_back(task);
    }
    /// 从可中断队列中删除一个任务
    pub fn drop_interruptible(&mut self, task: &Arc<TaskControlBlock>) {
        self.interruptible_queue
            // 使用retain过滤掉与指定任务相同的任务
            .retain(|task_in_queue| Arc::as_ptr(task_in_queue) != Arc::as_ptr(task));
    }
    /// 根据pid查找任务（搜索所有队列）
    pub fn find_by_pid(&self, pid: usize) -> Option<Arc<TaskControlBlock>> {
        // 先在RT队列中查找
        if let Some(task) = self.rt_rq.find_by_pid(pid) {
            return Some(task);
        }
        // 再在CFS队列中查找
        if let Some(task) = self.cfs_rq.find_by_pid(pid) {
            return Some(task);
        }
        // 再在Idle队列中查找
        if let Some(task) = self.idle_rq.find_by_pid(pid) {
            return Some(task);
        }
        // 最后在可中断队列中查找
        self.interruptible_queue
            .iter()
            .find(|task| task.pid.0 == pid)
            .cloned()
    }
    /// 根据tgid(线程组id)查找任务（搜索所有队列）
    pub fn find_by_tgid(&self, tgid: usize) -> Option<Arc<TaskControlBlock>> {
        // 先在RT队列中查找
        if let Some(task) = self.rt_rq.find_by_tgid(tgid) {
            return Some(task);
        }
        // 再在CFS队列中查找
        if let Some(task) = self.cfs_rq.find_by_tgid(tgid) {
            return Some(task);
        }
        // 再在Idle队列中查找
        if let Some(task) = self.idle_rq.find_by_tgid(tgid) {
            return Some(task);
        }
        // 最后在可中断队列中查找
        self.interruptible_queue
            .iter()
            .find(|task| task.tgid == tgid)
            .cloned()
    }
    /// 就绪队列中任务数量（所有调度类）
    pub fn ready_count(&self) -> u16 {
        (self.rt_rq.len() + self.cfs_rq.len() + self.idle_rq.len()) as u16
    }
    /// 可中断队列中任务数量
    pub fn interruptible_count(&self) -> u16 {
        self.interruptible_queue.len() as u16
    }
    /// 这个函数会将`task`从`interruptible_queue`中删除，并加入`ready_queue`。
    /// 如果一切正常的话，这个`task`将会被加入`ready_queue`。如果`task`已经被唤醒，那么什么也不会发生。
    /// # 注意
    /// 这个函数不会改变`task_status`，你应该手动改变它以保持一致性。
    pub fn wake_interruptible(&mut self, task: Arc<TaskControlBlock>) {
        match self.try_wake_interruptible(task) {
            Ok(_) => {}
            Err(_) => {
                log::trace!("[wake_interruptible] already waken");
            }
        }
    }
    /// 这个函数会将`task`从`interruptible_queue`中删除，并加入CFS就绪队列。
    /// 如果一切正常的话，这个`task`将会被加入CFS就绪队列。如果`task`已经被唤醒，那么返回`Err()`。
    /// # 注意
    /// 这个函数不会改变`task_status`，你应该手动改变它以保持一致性。
    pub fn try_wake_interruptible(
        &mut self,
        task: Arc<TaskControlBlock>,
    ) -> Result<(), WaitQueueError> {
        // 从可中断队列中删除指定任务
        self.drop_interruptible(&task);
        // 如果任务不在就绪队列中，将其加入CFS就绪队列
        if self.find_by_pid(task.pid.0).is_none() {
            self.add(task);
            Ok(())
        } else {
            Err(WaitQueueError::AlreadyWaken)
        }
    }
    #[allow(unused)]
    /// 调试方法
    /// 打印CFS就绪队列中的任务ID
    pub fn show_ready(&self) {
        self.cfs_rq.iter().for_each(|task| {
            log::error!("[show_ready] pid: {}", task.pid.0);
        })
    }
    #[allow(unused)]
    /// 调试方法
    /// 打印可中断队列中的任务ID
    pub fn show_interruptible(&self) {
        self.interruptible_queue.iter().for_each(|task| {
            log::error!("[show_interruptible] pid: {}", task.pid.0);
        })
    }

    #[cfg(feature = "oom_handler")]
    /// 尝试从当前管理器的队列中释放内存
    /// 返回释放的字节数 (或页数，取决于你的实现单位)
    pub fn do_oom_local(&mut self, req: usize) -> usize {
        let mut cleaned = Vec::with_capacity(16);
        let mut local_released = 0;

        // 1. 遍历可中断队列 (优先牺牲睡眠中的任务)
        // 注意：这里使用了 retain 的变体逻辑，手动迭代以避免借用检查问题，
        // 或者直接遍历引用。原代码是 iter()，这里保持一致。
        for task in self.interruptible_queue
            .iter()
            .filter(|task| self.active_tracker.check_active(task.pid.0)) 
        {
            let released = task.vm.lock().do_deep_clean();
            if released > 0 {
                log::warn!("deep clean on task: {}, released: {}", task.tgid, released);
                cleaned.push(task.pid.0);
                local_released += released;
            }
            // 如果已经满足了总需求（注意：这里的 req 是外部传进来的剩余需求）
            if local_released >= req {
                break;
            }
        }

        // 如果在 interruptible 队列中释放够了，处理 active 标记并返回
        if local_released >= req {
            while let Some(pid) = cleaned.pop() {
                self.active_tracker.mark_inactive(pid)
            }
            return local_released;
        }

        // 2. 遍历CFS就绪队列 (遍历所有任务，按vruntime顺序)
        for task in self.cfs_rq
            .iter()
            .filter(|task| self.active_tracker.check_active(task.pid.0))
        {
            let released = task.vm.lock().do_shallow_clean();
            if released > 0 {
                log::warn!("shallow clean on task: {}, released: {}", task.tgid, released);
                cleaned.push(task.pid.0);
                local_released += released;
            }
            if local_released >= req {
                break;
            }
        }

        // 清理 active 标记
        while let Some(pid) = cleaned.pop() {
            self.active_tracker.mark_inactive(pid)
        }

        local_released
    }
}

lazy_static! {
    // /// 全局任务管理器（带互斥锁）
    // pub static ref TASK_MANAGER: Mutex<TaskManager> = Mutex::new(TaskManager::new());
    /// Per-CPU 任务管理器列表
    /// 每个元素对应一个 CPU 核的 TaskManager
    pub static ref TASK_MANAGERS: Vec<Mutex<TaskManager>> = {
        let mut v = Vec::new();
        for _ in 0..MAX_CPU_NUM {
            v.push(Mutex::new(TaskManager::new()));
        }
        v
    };
}

/// 添加一个任务到任务管理器（支持Wake-up Affinity）
pub fn add_task(task: Arc<TaskControlBlock>) {
    let _guard = InterruptGuard::new();
    
    // Wake-up Affinity: 优先使用任务上次运行的CPU
    let last_cpu = {
        let inner = task.acquire_inner_lock();
        inner.sched_entity.last_cpu
    };
    
    let current_cpu = current_cpu_id();
    
    // 如果last_cpu有效且可用，尝试将任务添加到last_cpu
    if last_cpu < MAX_CPU_NUM && last_cpu != current_cpu {
        // 使用try_lock避免死锁
        if let Some(mut manager) = TASK_MANAGERS[last_cpu].try_lock() {
            manager.add(task);
            return;
        }
    }
    
    // Fallback: 添加到当前CPU
    TASK_MANAGERS[current_cpu].lock().add(task);
}

/// 添加任务到指定CPU的队列（用于work stealing后的re-add）
pub fn add_task_to_cpu(task: Arc<TaskControlBlock>, cpu_id: usize) {
    let _guard = InterruptGuard::new();
    if cpu_id < MAX_CPU_NUM {
        TASK_MANAGERS[cpu_id].lock().add(task);
    } else {
        TASK_MANAGERS[current_cpu_id()].lock().add(task);
    }
}

/// 从任务管理器中取出一个任务（支持Try-Lock Work Stealing）
pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    let _guard = InterruptGuard::new();
    let cpu_id = current_cpu_id();
    
    // 1. 尝试从本地获取
    let task = TASK_MANAGERS[cpu_id].lock().fetch();
    if task.is_some() {
        return task;
    }

    // 2. Work Stealing with Try-Lock
    // 使用try_lock避免死锁，如果其他核正在忙就跳过
    for i in 0..MAX_CPU_NUM {
        if i == cpu_id {
            continue;
        }
        // 使用try_lock避免死锁
        if let Some(mut other_manager) = TASK_MANAGERS[i].try_lock() {
            // 只在对方有足够任务时才偷取（避免饥饿）
            if other_manager.total_count() >= 2 {
                // 优先偷取CFS任务（RT任务优先级高，不适合偷取）
                if let Some(task) = other_manager.steal_from_cfs() {
                    #[cfg(feature = "oom_handler")]
                    other_manager.active_tracker.mark_active(task.pid.0);
                    
                    return Some(task);
                }
            }
        }
    }
    
    None
}

#[cfg(feature = "oom_handler")]
pub fn do_oom(req: usize) -> Result<(), ()> {
    let _guard = InterruptGuard::new();
    let mut total_released = 0;

    // 遍历所有的 CPU 任务管理器
    for manager_lock in TASK_MANAGERS.iter() {
        // 如果已经满足需求，直接返回
        if total_released >= req {
            return Ok(());
        }

        let mut manager = manager_lock.lock();
        let needed = req - total_released;
        total_released += manager.do_oom_local(needed);
    }

    if total_released >= req {
        Ok(())
    } else {
        log::error!("OOM failed: required {}, released {}", req, total_released);
        Err(())
    }
}

#[cfg(not(feature = "oom_handler"))]
#[allow(unused)]
pub fn do_oom(_req: usize) -> Result<(), ()> {
    Err(()) // 或者 panic，取决于设计
}

/// # 警告
/// 这里的`pid`是唯一的，用户会将其视为`tid`
pub fn find_task_by_pid(pid: usize) -> Option<Arc<TaskControlBlock>> {
    let _guard = InterruptGuard::new();

    let current = super::processor::current_task(); 
    if let Some(task) = current {
        if task.pid.0 == pid {
            return Some(task);
        }
    }

    for manager in TASK_MANAGERS.iter() {
        let manager = manager.lock();
        if let Some(task) = manager.find_by_pid(pid) {
            return Some(task);
        }
    }
    None
}

/// 返回线程组ID为`tgid`的任意任务。
pub fn find_task_by_tgid(tgid: usize) -> Option<Arc<TaskControlBlock>> {
    let _guard = InterruptGuard::new();
    let current = super::processor::current_task();
    if let Some(task) = current {
        if task.tgid == tgid {
            return Some(task);
        }
    }

    for manager in TASK_MANAGERS.iter() {
        let manager = manager.lock();
        if let Some(task) = manager.find_by_tgid(tgid) {
            return Some(task);
        }
    }
    None
}

/*todo()
// 在 TCB 中记录 CPU ID（更高效） 在 TaskControlBlock 结构体中增加 pub last_cpu: usize 字段。
在 add_task 或 sleep 时更新 last_cpu。
wake_interruptible 时直接锁 TASK_MANAGERS[task.last_cpu] 进行唤醒。
*/
//简单遍历（推荐初期使用） 唤醒时遍历所有核的管理器，找到并唤醒。
pub fn sleep_interruptible(task: Arc<TaskControlBlock>) {
    let _guard = InterruptGuard::new();
    let cpu_id = current_cpu_id();
    log::info!("[sleep_interruptible] Locking TASK_MANAGERS[{}]...", cpu_id);
    TASK_MANAGERS[cpu_id].lock().add_interruptible(task);
    log::info!("[sleep_interruptible] Task added to queue. Unlocked.");
}

/// Wake a task from interruptible state.
/// 
/// This function searches through all CPU's task managers to find and wake the specified task.
/// 
/// # Multi-core Safety
/// Uses try_lock() to avoid deadlocks when other CPUs have locked their managers.
/// If a manager is locked by another CPU, we skip it and retry the entire loop.
/// This is safe because the task can only be in one manager's interruptible queue.
pub fn wake_interruptible(task: Arc<TaskControlBlock>) {
    let _guard = InterruptGuard::new();
    
    // 使用重试循环，避免跨 CPU 死锁
    loop {
        let mut all_checked = true;
        
        for manager in TASK_MANAGERS.iter() {
            // 使用 try_lock 避免阻塞等待其他 CPU 的锁
            if let Some(mut manager) = manager.try_lock() {
                if manager.try_wake_interruptible(Arc::clone(&task)).is_ok() {
                    return; // 成功唤醒
                }
            } else {
                // 有锁竞争，标记需要重试
                all_checked = false;
            }
        }
        
        // 如果检查了所有 manager 都没找到，说明任务已被唤醒或不在队列中
        if all_checked {
            return;
        }
        
        // 短暂让出 CPU，减少锁竞争
        core::hint::spin_loop();
    }
}

/// 返回就绪队列中的任务数量
pub fn procs_count() -> u16 {
    let _guard = InterruptGuard::new();
    let mut total = 0;
    for manager in TASK_MANAGERS.iter() {
        let manager = manager.lock();
        total += manager.ready_count() + manager.interruptible_count();
    }
    total
}

/// 等待队列错误类型
pub enum WaitQueueError {
    /// 已经唤醒
    AlreadyWaken,
}

/// 等待队列
/// 内部是一个存储任务控制块弱引用的双端队列
pub struct WaitQueue {
    inner: VecDeque<Weak<TaskControlBlock>>,
}

#[allow(unused)]
impl WaitQueue {
    /// 构造函数
    pub fn new() -> Self {
        Self {
            inner: VecDeque::new(),
        }
    }
    /// 这个函数将一个`task`添加到 `WaitQueue`但是不会阻塞这个任务
    /// 如果想要阻塞一个`task`，使用`block_current_and_run_next()`
    pub fn add_task(&mut self, task: Weak<TaskControlBlock>) {
        // 将task添加到back端
        self.inner.push_back(task);
    }
    /// 这个函数会尝试从`WaitQueue`中弹出一个`task`，但是不会唤醒它
    pub fn pop_task(&mut self) -> Option<Weak<TaskControlBlock>> {
        // 将front端的任务弹出
        self.inner.pop_front()
    }
    /// 判断等待队列是否包含给定的task
    pub fn contains(&self, task: &Weak<TaskControlBlock>) -> bool {
        self.inner
            .iter()
            .any(|task_in_queue| Weak::as_ptr(task_in_queue) == Weak::as_ptr(task))
    }
    /// 判断等待队列是否为空
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    /// 这个函数将会唤醒等待队列中所有的任务，并将它们的任务状态改变为就绪态，
    /// 如果一切正常，这些任务会在将来被调度。
    /// # 警告
    /// 这个函数会为每个被唤醒的`task`调用`acquire_inner_lock`，请注意**死锁**
    pub fn wake_all(&mut self) -> usize {
        self.wake_at_most(usize::MAX)
    }
    /// 唤醒不超过`limit`个`task`，返回唤醒的`task`数量。
    /// # 警告
    /// 这个函数会为每个被唤醒的`task`调用`acquire_inner_lock`，请注意**死锁**
    pub fn wake_at_most(&mut self, limit: usize) -> usize {
        // 如果limit为0，直接返回0
        if limit == 0 {
            return 0;
        }
        
        // 这里不需要开关中断，因为 WaitQueue 通常由外部锁保护
        // 或者它是在局部使用的。如果它是全局的，调用者应该负责加锁。
        // 但 TASK_MANAGERS 的锁是在内部获取的。
        // 为了安全起见，我们在操作全局 TASK_MANAGERS 时关中断。

        let _guard = InterruptGuard::new();
        let cpu_id = current_cpu_id();

        // 获取全局任务管理器
        // 注意：这里持有 manager 的锁
        let mut manager = TASK_MANAGERS[cpu_id].lock();
        
        // 初始化计数器
        let mut cnt = 0;
        // 遍历内部队列，从self.inner中逐个取出任务处理
        while let Some(task) = self.inner.pop_front() {
            // 检查任务的弱引用是否仍然有效
            // 将弱引用升级为强引用
            match task.upgrade() {
                Some(task) => {
                    // 获取任务的内部锁
                    let mut inner = task.acquire_inner_lock();
                    // 检查任务状态
                    match inner.task_status {
                        // 可中断状态
                        super::TaskStatus::Interruptible => {
                            // 将任务状态改为就绪态
                            inner.task_status = super::task::TaskStatus::Ready
                        }
                        // 对于处于 就绪态或运行态的任务，不需要做唤醒操作
                        // 对于处于僵尸态的任务，做唤醒操作会搞砸进程管理
                        _ => continue,
                    }
                    // 释放内部锁
                    drop(inner);
                    // // 唤醒任务
                    // if manager.try_wake_interruptible(task).is_ok() {
                    //     cnt += 1;
                    // }
                    
                    // 这里直接调用全局的 wake_interruptible 会导致死锁，因为它会尝试获取锁
                    // 但我们已经持有 manager 锁了（假设 task 在当前 manager）
                    // 不过 super::wake_interruptible 会遍历所有核。
                    // 这是一个复杂点。原来的代码是 try_wake_interruptible，只检查当前核。
                    // 建议改回使用当前 manager 的方法，避免死锁。
                    
                     if manager.try_wake_interruptible(task.clone()).is_ok() {
                        cnt += 1;
                     } else {
                        // 如果任务不在当前核，我们需要释放当前锁去唤醒其他核吗？
                        // 或者先收集起来，最后统一唤醒？
                        // 简单起见，这里假设唤醒的任务大多在当前核。
                        // 如果不在，我们可能需要暂时释放锁。
                        drop(manager);
                        // 尝试全局唤醒
                        super::wake_interruptible(task);
                        // 重新获取锁
                        manager = TASK_MANAGERS[cpu_id].lock();
                        cnt += 1;
                     }
                    
                    // cnt += 1;
                    // 到达数量限制，停止遍历
                    if cnt == limit {
                        break;
                    }
                }
                // task is dead, just ignore
                None => continue,
            }
        }
        cnt
    }
}

/// 表示一个等待超时的任务
pub struct TimeoutWaiter {
    /// 任务的弱引用
    task: Weak<TaskControlBlock>,
    /// 任务超时时间
    timeout: TimeSpec,
}

// 二叉堆是最大堆，所以我们需要反转排序
impl Ord for TimeoutWaiter {
    fn cmp(&self, other: &Self) -> Ordering {
        Ordering::reverse(self.timeout.cmp(&other.timeout))
    }
}

impl PartialOrd for TimeoutWaiter {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for TimeoutWaiter {}

impl PartialEq for TimeoutWaiter {
    /// 仅通过比较timeout字段
    fn eq(&self, other: &Self) -> bool {
        self.timeout.eq(&other.timeout)
    }
}

/// 等待超时任务队列
pub struct TimeoutWaitQueue {
    /// 使用二叉堆存储任务（最大堆），按超时时间排序
    inner: BinaryHeap<TimeoutWaiter>,
}

impl TimeoutWaitQueue {
    /// 构造函数
    pub fn new() -> Self {
        Self {
            inner: BinaryHeap::new(),
        }
    }
    /// 这个函数会将一个`task`添加到`WaitQueue`但是**不会**阻塞这个任务，
    /// 如果想要阻塞一个`task`，使用`block_current_and_run_next()`函数
    pub fn add_task(&mut self, task: Weak<TaskControlBlock>, timeout: TimeSpec) {
        self.inner.push(TimeoutWaiter { task, timeout });
    }
    /// 唤醒所有超时的任务
    pub fn wake_expired(&mut self, now: TimeSpec) {
        // 入口日志，确认中断是否触发
        // log::info!("[wake_expired] Enter. Checking expired tasks...");
        // 获取任务管理器
        let cpu_id = current_cpu_id();

        // 调试日志：尝试获取 TASK_MANAGERS 锁
        // log::info!("[wake_expired] Trying to lock TASK_MANAGERS[{}]...", cpu_id);
        
        // 注意：wake_expired 通常在中断上下文中调用（或者已经被 do_wake_expired 保护了）
        // 但这里我们再次获取 manager 锁。
        let mut manager = TASK_MANAGERS[cpu_id].lock();
        // log::info!("[wake_expired] Locked TASK_MANAGERS. Processing...");

        // 循环处理超时任务
        while let Some(waiter) = self.inner.pop() {
            // 堆中剩下的任务还没有超时
            if waiter.timeout > now {
                // 若超时时间大于当前时间，说明后面的任务都没有超时
                self.inner.push(waiter);
                break;
            // 唤醒超时任务
            } else {
                // 将弱引用升级为强引用
                match waiter.task.upgrade() {
                    Some(task) => {
                        // ==== 修改开始 ====
                        let pid = task.pid.0;
                        let mut inner = task.acquire_inner_lock();
                        // let current_status = inner.task_status;

                        // 打印调试日志，查看检查时的状态
                        // log::info!("[Timer] Checking timeout for Task {}. Status: {:?}", pid, current_status);

                        match inner.task_status {
                            super::TaskStatus::Interruptible => {
                                // log::info!("[Timer] Waking up Task {}", pid);
                                inner.task_status = super::task::TaskStatus::Ready
                            }
                            // ⚠️ 关键点：如果这里捕获到了 Running 状态，说明发生了竞态条件
                            _ => {
                                // log::warn!("[Timer] RACE DETECTED! Task {} timeout triggered but status is {:?} (not Interruptible). Task was DROPPED from queue!", pid, current_status);
                                drop(inner);
                                continue; 
                            }
                        }
                        drop(inner);
                        // log::trace!(
                        //     "[wake_expired] pid: {}, timeout: {:?}",
                        //     task.pid.0,
                        //     waiter.timeout
                        // );
                        
                        // 优先尝试在本地唤醒
                        if manager.try_wake_interruptible(task.clone()).is_err() {
                            // 如果不在本地，需要释放锁去调用全局唤醒
                             drop(manager);
                             super::wake_interruptible(task);
                             manager = TASK_MANAGERS[cpu_id].lock();
                        }
                    }
                    // task is dead, just ignore
                    None => {
                        // log::error!("[Wake] Failed to wake task: Task dropped/deallocated!");
                        continue;
                    }
                }
            }
        }
        // log::info!("[wake_expired] Finished. Unlocking TASK_MANAGERS.");
    }
    #[allow(unused)]
    // debug use only
    pub fn show_waiter(&self) {
        for waiter in self.inner.iter() {
            log::error!("[show_waiter] timeout: {:?}", waiter.timeout);
        }
    }
}

lazy_static! {
    /// 全局超时等待队列
    pub static ref TIMEOUT_WAITQUEUE: Mutex<TimeoutWaitQueue> = Mutex::new(TimeoutWaitQueue::new());
}

/// 这个函数会将一个`task`添加到全局超时等待队列中，但是不会阻塞它
/// 如果想要阻塞一个任务，使用`block_current_and_run_next()`函数
pub fn wait_with_timeout(task: Weak<TaskControlBlock>, timeout: TimeSpec) {
    let _guard = InterruptGuard::new();
    let mut queue = TIMEOUT_WAITQUEUE.lock();
    queue.add_task(task, timeout);
}

/// 唤醒全局超时等待队列中所有已超时的任务
pub fn do_wake_expired() {
    let _guard = InterruptGuard::new();
    TIMEOUT_WAITQUEUE
        .lock()
        .wake_expired(crate::timer::TimeSpec::now());
}