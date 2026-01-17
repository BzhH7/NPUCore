//! Completely Fair Scheduler (CFS) Implementation
//!
//! This module implements a CFS-like scheduler inspired by Linux's scheduling algorithm.
//! Unlike the simple FIFO scheduler, CFS aims to fairly distribute CPU time among all
//! runnable tasks based on their virtual runtime.
//!
//! # Algorithm Overview
//!
//! CFS tracks each task's "virtual runtime" (vruntime), which represents how much CPU
//! time the task has consumed, weighted by its priority. Tasks with lower vruntime
//! are scheduled first, ensuring fair distribution.
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                     CFS Run Queue (Red-Black Tree)              │
//! │                                                                 │
//! │                           ┌───────┐                             │
//! │                           │ vrt=50│ (root)                      │
//! │                          /         \                            │
//! │                    ┌───────┐     ┌───────┐                      │
//! │                    │vrt=30 │     │vrt=80 │                      │
//! │                   /    \            \                           │
//! │             ┌───────┐ ┌───────┐  ┌───────┐                      │
//! │             │vrt=20 │ │vrt=40 │  │vrt=100│                      │
//! │             └───────┘ └───────┘  └───────┘                      │
//! │               ↑                                                 │
//! │          leftmost (next to run)                                 │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Key Concepts
//!
//! - **Virtual Runtime (vruntime)**: Weighted measure of CPU consumption
//! - **Nice Value**: Task priority (-20 to 19, lower = higher priority)
//! - **Weight**: Priority converted to scheduling weight
//! - **Time Slice**: Maximum time before preemption
//!
//! # Configuration
//!
//! The scheduler behavior can be tuned via constants:
//! - `SCHED_LATENCY_NS`: Target latency for all tasks to run once
//! - `MIN_GRANULARITY_NS`: Minimum time slice to avoid excessive context switches
//! - `NICE_0_WEIGHT`: Base weight for nice value 0

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::sync::atomic::Ordering as AtomicOrdering;

use crate::task::TaskControlBlock;
use crate::task::task::TASK_NOT_RUNNING;

// ============================================================================
// CFS Configuration Constants
// ============================================================================

/// Target latency: how long until all tasks have run at least once (nanoseconds)
/// This is the "period" over which CFS tries to be fair
pub const SCHED_LATENCY_NS: u64 = 6_000_000; // 6ms

/// Minimum time slice per task (nanoseconds)
/// Prevents excessive context switching with many tasks
pub const MIN_GRANULARITY_NS: u64 = 750_000; // 0.75ms

/// Weight of a task with nice value 0
/// Other weights are derived from this using the weight table
pub const NICE_0_WEIGHT: u32 = 1024;

/// Preemption granularity - minimum vruntime difference to preempt (nanoseconds)
pub const WAKEUP_GRANULARITY_NS: u64 = 1_000_000; // 1ms

// ============================================================================
// Nice Value to Weight Mapping
// ============================================================================

/// Weight lookup table indexed by nice value + 20
/// Weights decrease by ~1.25x for each nice level increase
/// This table is based on the formula: weight = 1024 / 1.25^nice
const NICE_TO_WEIGHT: [u32; 40] = [
    88761, 71755, 56483, 46273, 36291,  // nice -20 to -16
    29154, 23254, 18705, 14949, 11916,  // nice -15 to -11
    9548,  7620,  6100,  4904,  3906,   // nice -10 to -6
    3121,  2501,  1991,  1586,  1277,   // nice -5 to -1
    1024,  820,   655,   526,   423,    // nice 0 to 4
    335,   272,   215,   172,   137,    // nice 5 to 9
    110,   87,    70,    56,    45,     // nice 10 to 14
    36,    29,    23,    18,    15,     // nice 15 to 19
];

/// Convert nice value (-20 to 19) to weight
#[inline]
pub fn nice_to_weight(nice: i8) -> u32 {
    let clamped = nice.clamp(-20, 19);
    NICE_TO_WEIGHT[(clamped + 20) as usize]
}

/// Inverse weight for vruntime calculation (scaled by 2^32 for precision)
const NICE_TO_INV_WEIGHT: [u64; 40] = [
    48388,  59856,  76040,  92818,  118348,  // nice -20 to -16
    147320, 184698, 229616, 287308, 360437,  // nice -15 to -11
    449829, 563644, 704093, 875809, 1099582, // nice -10 to -6
    1376151,1717300,2157191,2708050,3363326, // nice -5 to -1
    4194304,5237765,6557202,8165337,10153587,// nice 0 to 4
    12820798,15790321,19976592,24970740,31350126,// nice 5 to 9
    39045157,49367440,61356676,76695844,95443717,// nice 10 to 14
    119304647,148102320,186737708,238609294,286331153,// nice 15 to 19
];

// ============================================================================
// Scheduler Entity
// ============================================================================

/// Scheduling policy for multi-level scheduling framework
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SchedPolicy {
    /// Normal CFS scheduling (SCHED_OTHER/SCHED_NORMAL)
    Normal = 0,
    /// FIFO real-time scheduling (SCHED_FIFO)
    Fifo = 1,
    /// Round-robin real-time scheduling (SCHED_RR)
    RoundRobin = 2,
    /// Batch scheduling (SCHED_BATCH) - treated as CFS
    Batch = 3,
    /// Idle scheduling (SCHED_IDLE) - lowest priority
    Idle = 5,
}

impl Default for SchedPolicy {
    fn default() -> Self {
        Self::Normal
    }
}

impl SchedPolicy {
    /// Check if this is a real-time policy
    #[inline]
    pub fn is_realtime(&self) -> bool {
        matches!(self, Self::Fifo | Self::RoundRobin)
    }
    
    /// Convert from raw policy number (Linux compatible)
    pub fn from_raw(policy: u32) -> Option<Self> {
        match policy {
            0 => Some(Self::Normal),
            1 => Some(Self::Fifo),
            2 => Some(Self::RoundRobin),
            3 => Some(Self::Batch),
            5 => Some(Self::Idle),
            _ => None,
        }
    }
}

/// Scheduling statistics and state for a task
#[derive(Debug, Clone, Copy)]
pub struct SchedEntity {
    /// Virtual runtime - the key metric for CFS ordering
    pub vruntime: u64,
    /// Nice value (-20 to 19)
    pub nice: i8,
    /// Cached weight from nice value
    pub weight: u32,
    /// Total time this task has run (nanoseconds)
    pub sum_exec_runtime: u64,
    /// Time when task was last scheduled in
    pub exec_start: u64,
    /// Previous total runtime (for delta calculation)
    pub prev_sum_exec_runtime: u64,
    /// Last CPU this task ran on (for wake-up affinity)
    pub last_cpu: usize,
    /// Scheduling policy (Normal, Fifo, RR, etc.)
    pub policy: SchedPolicy,
    /// Real-time priority (1-99, higher is more important)
    pub rt_priority: u8,
    /// CPU affinity mask (bitmask of allowed CPUs)
    pub cpu_affinity: usize,
}

impl Default for SchedEntity {
    fn default() -> Self {
        Self {
            vruntime: 0,
            nice: 0,
            weight: NICE_0_WEIGHT,
            sum_exec_runtime: 0,
            exec_start: 0,
            prev_sum_exec_runtime: 0,
            last_cpu: 0,
            policy: SchedPolicy::default(),
            rt_priority: 0,
            cpu_affinity: usize::MAX, // All CPUs allowed by default
        }
    }
}

impl SchedEntity {
    /// Create new scheduling entity with given nice value
    pub fn new(nice: i8) -> Self {
        Self {
            nice,
            weight: nice_to_weight(nice),
            ..Default::default()
        }
    }
    
    /// Create new scheduling entity with RT policy and priority
    pub fn new_rt(policy: SchedPolicy, priority: u8) -> Self {
        Self {
            policy,
            rt_priority: priority.min(99).max(1),
            ..Default::default()
        }
    }
    
    /// Set the scheduling policy
    pub fn set_policy(&mut self, policy: SchedPolicy, priority: u8) {
        self.policy = policy;
        if policy.is_realtime() {
            self.rt_priority = priority.min(99).max(1);
        } else {
            self.rt_priority = 0;
        }
    }
    
    /// Update last CPU this task ran on
    #[inline]
    pub fn set_last_cpu(&mut self, cpu: usize) {
        self.last_cpu = cpu;
    }
    
    /// Check if task is allowed to run on given CPU
    #[inline]
    pub fn can_run_on(&self, cpu: usize) -> bool {
        (self.cpu_affinity & (1 << cpu)) != 0
    }
    
    /// Set CPU affinity mask
    pub fn set_affinity(&mut self, mask: usize) {
        self.cpu_affinity = mask;
    }

    /// Update nice value and recalculate weight
    pub fn set_nice(&mut self, nice: i8) {
        self.nice = nice.clamp(-20, 19);
        self.weight = nice_to_weight(self.nice);
    }

    /// Calculate vruntime delta for given actual runtime
    /// vruntime_delta = runtime * (NICE_0_WEIGHT / weight)
    #[inline]
    pub fn calc_delta_vruntime(&self, delta_exec: u64) -> u64 {
        if self.weight == NICE_0_WEIGHT {
            delta_exec
        } else {
            // Use inverse weight for precision
            let inv_weight = NICE_TO_INV_WEIGHT[(self.nice + 20) as usize];
            (delta_exec * inv_weight) >> 22 // 2^22 scaling factor
        }
    }

    /// Update runtime statistics after execution
    pub fn update_runtime(&mut self, now: u64) {
        if self.exec_start == 0 {
            return;
        }
        
        let delta_exec = now.saturating_sub(self.exec_start);
        self.exec_start = now;
        
        self.sum_exec_runtime += delta_exec;
        self.vruntime += self.calc_delta_vruntime(delta_exec);
    }
}

// ============================================================================
// CFS Run Queue
// ============================================================================

/// Key for ordering tasks in the run queue
/// Combines vruntime with task ID for uniqueness
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct RunQueueKey {
    vruntime: u64,
    tid: usize,
}

impl Ord for RunQueueKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.vruntime
            .cmp(&other.vruntime)
            .then_with(|| self.tid.cmp(&other.tid))
    }
}

impl PartialOrd for RunQueueKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// CFS Run Queue using a BTreeMap for O(log n) operations
/// 
/// In Linux, this would be a red-black tree, but Rust's BTreeMap
/// provides similar O(log n) guarantees with better cache locality.
pub struct CfsRunQueue {
    /// Tasks ordered by vruntime
    tasks: BTreeMap<RunQueueKey, Arc<TaskControlBlock>>,
    /// Minimum vruntime in the queue (for new task placement)
    min_vruntime: u64,
    /// Total weight of all runnable tasks
    total_weight: u64,
    /// Number of runnable tasks
    nr_running: usize,
}

impl Default for CfsRunQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl CfsRunQueue {
    /// Create a new empty CFS run queue
    pub fn new() -> Self {
        Self {
            tasks: BTreeMap::new(),
            min_vruntime: 0,
            total_weight: 0,
            nr_running: 0,
        }
    }

    /// Check if the queue is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.nr_running == 0
    }

    /// Get number of runnable tasks
    #[inline]
    pub fn len(&self) -> usize {
        self.nr_running
    }

    /// Calculate time slice for a task based on its weight and total load
    pub fn calc_time_slice(&self, weight: u32) -> u64 {
        if self.nr_running <= 1 {
            return SCHED_LATENCY_NS;
        }

        // Time slice proportional to weight
        let slice = (SCHED_LATENCY_NS * weight as u64) / self.total_weight.max(1);
        
        // Enforce minimum granularity
        slice.max(MIN_GRANULARITY_NS)
    }

    /// Place a new task's vruntime appropriately
    /// New tasks get the current minimum vruntime to prevent starvation
    fn place_entity(&self, entity: &mut SchedEntity, initial: bool) {
        let mut vruntime = self.min_vruntime;
        
        if initial {
            // New tasks: start slightly behind to prevent immediate preemption
            // of existing tasks, but not so far that they wait forever
            let thresh = SCHED_LATENCY_NS / 2;
            vruntime = vruntime.saturating_add(thresh);
        }
        
        // Don't go backwards
        entity.vruntime = entity.vruntime.max(vruntime);
    }

    /// Add a task to the run queue
    pub fn enqueue(&mut self, task: Arc<TaskControlBlock>, entity: &mut SchedEntity, is_new: bool) {
        self.place_entity(entity, is_new);
        
        let key = RunQueueKey {
            vruntime: entity.vruntime,
            tid: task.pid.0,
        };
        
        self.tasks.insert(key, task);
        self.total_weight += entity.weight as u64;
        self.nr_running += 1;
    }

    /// Remove a task from the run queue
    pub fn dequeue(&mut self, task: &Arc<TaskControlBlock>, entity: &SchedEntity) {
        let key = RunQueueKey {
            vruntime: entity.vruntime,
            tid: task.pid.0,
        };
        
        if self.tasks.remove(&key).is_some() {
            self.total_weight = self.total_weight.saturating_sub(entity.weight as u64);
            self.nr_running = self.nr_running.saturating_sub(1);
        }
    }

    /// Pick the task with the lowest vruntime (leftmost in the tree)
    pub fn pick_next(&mut self) -> Option<Arc<TaskControlBlock>> {
        let (key, task) = self.tasks.pop_first()?;
        
        // Update min_vruntime
        self.min_vruntime = self.min_vruntime.max(key.vruntime);
        self.total_weight = self.total_weight.saturating_sub(NICE_0_WEIGHT as u64); // Approximate
        self.nr_running = self.nr_running.saturating_sub(1);
        
        Some(task)
    }

    /// Steal a task that can run on the target CPU (for work stealing)
    /// Returns a task whose CPU affinity allows running on target_cpu
    /// Prefers tasks with higher vruntime (less urgent) to minimize impact
    /// 
    /// Safety: Only steals tasks with valid context (task_cx.ra != 0)
    /// Safety: Only steals tasks not currently running on any CPU
    pub fn steal_for_cpu(&mut self, target_cpu: usize) -> Option<Arc<TaskControlBlock>> {
        // Find a task that can run on target_cpu
        // We iterate from the back (highest vruntime = least urgent) for fairness
        let key_to_steal = self.tasks
            .iter()
            .rev()  // Start from highest vruntime (least urgent)
            .find_map(|(key, task)| {
                // 【关键安全检查】检查任务是否正在其他 CPU 上运行
                // 这可以捕获潜在的并发错误
                let running_cpu = task.running_on_cpu.load(AtomicOrdering::SeqCst);
                if running_cpu != TASK_NOT_RUNNING {
                    // 任务正在某个 CPU 上运行，不应该在队列中
                    log::warn!("[steal_for_cpu] Task pid={} found in queue but running_on_cpu={}", 
                               task.pid.0, running_cpu);
                    return None;
                }
                
                let inner = task.acquire_inner_lock();
                
                // 【关键安全检查】只偷取上下文有效的任务
                // task_cx.ra == 0 表示任务上下文尚未初始化或已损坏
                let ra = inner.task_cx.ra;
                if ra == 0 || ra < 0x80000000 {
                    // 无效的 ra，跳过这个任务
                    return None;
                }
                
                // 检查 CPU 亲和性
                if inner.sched_entity.can_run_on(target_cpu) {
                    Some(*key)
                } else {
                    None
                }
            });
        
        if let Some(key) = key_to_steal {
            if let Some(task) = self.tasks.remove(&key) {
                // Update accounting
                let weight = task.acquire_inner_lock().sched_entity.weight as u64;
                self.total_weight = self.total_weight.saturating_sub(weight);
                self.nr_running = self.nr_running.saturating_sub(1);
                return Some(task);
            }
        }
        
        None
    }

    /// Peek at the next task without removing it
    pub fn peek_next(&self) -> Option<&Arc<TaskControlBlock>> {
        self.tasks.first_key_value().map(|(_, task)| task)
    }

    /// Check if a waking task should preempt the current task
    pub fn should_preempt(&self, curr_entity: &SchedEntity, wake_entity: &SchedEntity) -> bool {
        // The waking task should preempt if its vruntime is significantly less
        let vdiff = curr_entity.vruntime.saturating_sub(wake_entity.vruntime);
        vdiff > WAKEUP_GRANULARITY_NS
    }

    /// Update min_vruntime from current queue state
    fn update_min_vruntime(&mut self) {
        if let Some((key, _)) = self.tasks.first_key_value() {
            self.min_vruntime = self.min_vruntime.max(key.vruntime);
        }
    }

    /// Find task by PID
    pub fn find_by_pid(&self, pid: usize) -> Option<Arc<TaskControlBlock>> {
        self.tasks
            .values()
            .find(|t| t.pid.0 == pid)
            .cloned()
    }

    /// Find task by TGID
    pub fn find_by_tgid(&self, tgid: usize) -> Option<Arc<TaskControlBlock>> {
        self.tasks
            .values()
            .find(|t| t.tgid == tgid)
            .cloned()
    }

    /// Remove all tasks with a given TGID (for thread group exit)
    /// Returns a Vec of removed tasks
    pub fn remove_by_tgid(&mut self, tgid: usize) -> Vec<Arc<TaskControlBlock>> {
        let mut removed = Vec::new();
        let keys_to_remove: Vec<_> = self.tasks
            .iter()
            .filter(|(_, task)| task.tgid == tgid)
            .map(|(key, _)| *key)
            .collect();
        
        for key in keys_to_remove {
            if let Some(task) = self.tasks.remove(&key) {
                // Update accounting
                let weight = task.acquire_inner_lock().sched_entity.weight as u64;
                self.total_weight = self.total_weight.saturating_sub(weight);
                self.nr_running = self.nr_running.saturating_sub(1);
                removed.push(task);
            }
        }
        removed
    }

    /// Retain only tasks that satisfy the predicate
    pub fn retain<F>(&mut self, mut f: F) 
    where
        F: FnMut(&Arc<TaskControlBlock>) -> bool
    {
        let keys_to_remove: Vec<_> = self.tasks
            .iter()
            .filter(|(_, task)| !f(task))
            .map(|(key, _)| *key)
            .collect();
        
        for key in keys_to_remove {
            if let Some(task) = self.tasks.remove(&key) {
                let weight = task.acquire_inner_lock().sched_entity.weight as u64;
                self.total_weight = self.total_weight.saturating_sub(weight);
                self.nr_running = self.nr_running.saturating_sub(1);
            }
        }
    }

    /// Get all tasks (for debugging)
    pub fn iter(&self) -> impl Iterator<Item = &Arc<TaskControlBlock>> {
        self.tasks.values()
    }
}

// ============================================================================
// CFS Scheduler Statistics
// ============================================================================

/// Statistics for scheduler analysis and tuning
#[derive(Debug, Default, Clone, Copy)]
pub struct CfsStats {
    /// Number of context switches
    pub context_switches: u64,
    /// Number of voluntary preemptions
    pub voluntary_preempt: u64,
    /// Number of involuntary preemptions (time slice expired)
    pub involuntary_preempt: u64,
    /// Total time tasks spent waiting
    pub wait_time: u64,
    /// Total time tasks spent running
    pub run_time: u64,
}

impl CfsStats {
    /// Record a context switch
    pub fn record_switch(&mut self, voluntary: bool) {
        self.context_switches += 1;
        if voluntary {
            self.voluntary_preempt += 1;
        } else {
            self.involuntary_preempt += 1;
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Check if a given nice value is valid
#[inline]
pub const fn is_valid_nice(nice: i8) -> bool {
    nice >= -20 && nice <= 19
}

/// Convert kernel priority to nice value
/// Kernel priority 100-139 maps to nice -20 to 19
#[inline]
pub const fn prio_to_nice(prio: i32) -> i8 {
    (prio - 120) as i8
}

/// Convert nice value to kernel priority
#[inline]
pub const fn nice_to_prio(nice: i8) -> i32 {
    nice as i32 + 120
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nice_to_weight() {
        assert_eq!(nice_to_weight(0), NICE_0_WEIGHT);
        assert!(nice_to_weight(-20) > nice_to_weight(0));
        assert!(nice_to_weight(0) > nice_to_weight(19));
    }

    #[test]
    fn test_vruntime_delta() {
        let entity = SchedEntity::new(0);
        assert_eq!(entity.calc_delta_vruntime(1000), 1000);
        
        let high_prio = SchedEntity::new(-10);
        let low_prio = SchedEntity::new(10);
        
        // Higher priority (lower nice) should accumulate less vruntime
        assert!(high_prio.calc_delta_vruntime(1000) < low_prio.calc_delta_vruntime(1000));
    }
}
