//! Multi-level Scheduler Framework
//!
//! This module implements a hierarchical scheduling framework inspired by Linux's
//! scheduling classes. It supports multiple scheduling policies:
//!
//! - Real-time (RT): SCHED_FIFO and SCHED_RR with static priorities 1-99
//! - Normal (CFS): SCHED_NORMAL/SCHED_OTHER with dynamic nice values
//! - Idle: SCHED_IDLE for lowest priority background tasks
//!
//! # Scheduling Hierarchy
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │           Scheduler Framework           │
//! │  ┌───────────────────────────────────┐  │
//! │  │        RT Class (highest)         │  │
//! │  │   Priority 99 (highest) → 1       │  │
//! │  │   FIFO: runs until yield/block    │  │
//! │  │   RR: round-robin with timeslice  │  │
//! │  └───────────────────────────────────┘  │
//! │                    ↓                    │
//! │  ┌───────────────────────────────────┐  │
//! │  │       CFS Class (normal)          │  │
//! │  │   Nice -20 → +19 (vruntime)       │  │
//! │  │   Fair scheduling based on weight │  │
//! │  └───────────────────────────────────┘  │
//! │                    ↓                    │
//! │  ┌───────────────────────────────────┐  │
//! │  │       Idle Class (lowest)         │  │
//! │  │   Only runs when nothing else     │  │
//! │  └───────────────────────────────────┘  │
//! └─────────────────────────────────────────┘
//! ```

use alloc::collections::VecDeque;
use alloc::sync::Arc;

use super::cfs_scheduler::{SchedEntity, SchedPolicy};
use super::TaskControlBlock;

/// Maximum RT priority (1-99, 99 is highest)
pub const MAX_RT_PRIO: u8 = 99;
/// Number of RT priority levels
pub const RT_PRIO_LEVELS: usize = 100;
/// Default RR timeslice in nanoseconds (100ms)
pub const RR_TIMESLICE_NS: u64 = 100_000_000;

// ============================================================================
// Real-Time Run Queue
// ============================================================================

/// Real-time run queue with priority-based scheduling
/// 
/// Uses an array of FIFOs, one per priority level. Higher priority
/// tasks always run before lower priority tasks.
pub struct RtRunQueue {
    /// Priority queues (index 0 = unused, 1-99 = RT priorities)
    queues: [VecDeque<Arc<TaskControlBlock>>; RT_PRIO_LEVELS],
    /// Bitmap of non-empty priority levels for O(1) highest-priority lookup
    bitmap: u128,
    /// Number of runnable RT tasks
    nr_running: usize,
}

impl Default for RtRunQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl RtRunQueue {
    /// Create a new empty RT run queue
    pub const fn new() -> Self {
        const EMPTY_QUEUE: VecDeque<Arc<TaskControlBlock>> = VecDeque::new();
        Self {
            queues: [EMPTY_QUEUE; RT_PRIO_LEVELS],
            bitmap: 0,
            nr_running: 0,
        }
    }
    
    /// Check if the queue is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.nr_running == 0
    }
    
    /// Get number of runnable RT tasks
    #[inline]
    pub fn len(&self) -> usize {
        self.nr_running
    }
    
    /// Add a task to the RT run queue
    pub fn enqueue(&mut self, task: Arc<TaskControlBlock>, entity: &SchedEntity) {
        let prio = entity.rt_priority.min(MAX_RT_PRIO) as usize;
        if prio == 0 {
            return; // Invalid RT priority
        }
        
        // For FIFO, add to back; for RR, also add to back
        self.queues[prio].push_back(task);
        self.bitmap |= 1u128 << prio;
        self.nr_running += 1;
    }
    
    /// Remove a task from the RT run queue
    pub fn dequeue(&mut self, task: &Arc<TaskControlBlock>, entity: &SchedEntity) {
        let prio = entity.rt_priority.min(MAX_RT_PRIO) as usize;
        if prio == 0 {
            return;
        }
        
        let queue = &mut self.queues[prio];
        if let Some(pos) = queue.iter().position(|t| Arc::ptr_eq(t, task)) {
            queue.remove(pos);
            self.nr_running = self.nr_running.saturating_sub(1);
            
            if queue.is_empty() {
                self.bitmap &= !(1u128 << prio);
            }
        }
    }
    
    /// Pick the highest priority RT task
    pub fn pick_next(&mut self) -> Option<Arc<TaskControlBlock>> {
        if self.bitmap == 0 {
            return None;
        }
        
        // Find highest set bit (highest priority)
        let highest_prio = 127 - self.bitmap.leading_zeros() as usize;
        
        let queue = &mut self.queues[highest_prio];
        let task = queue.pop_front()?;
        
        self.nr_running = self.nr_running.saturating_sub(1);
        if queue.is_empty() {
            self.bitmap &= !(1u128 << highest_prio);
        }
        
        Some(task)
    }
    
    /// Peek at the highest priority RT task without removing
    pub fn peek_next(&self) -> Option<&Arc<TaskControlBlock>> {
        if self.bitmap == 0 {
            return None;
        }
        
        let highest_prio = 127 - self.bitmap.leading_zeros() as usize;
        self.queues[highest_prio].front()
    }
    
    /// Requeue a RR task to the back of its priority queue
    /// Used when a RR task's timeslice expires
    pub fn requeue_rr(&mut self, task: Arc<TaskControlBlock>, entity: &SchedEntity) {
        let prio = entity.rt_priority.min(MAX_RT_PRIO) as usize;
        if prio == 0 {
            return;
        }
        
        self.queues[prio].push_back(task);
        self.bitmap |= 1u128 << prio;
    }
    
    /// Check if a waking RT task should preempt the current task
    pub fn should_preempt(&self, curr_entity: &SchedEntity, wake_entity: &SchedEntity) -> bool {
        // RT tasks always preempt non-RT tasks
        if !curr_entity.policy.is_realtime() {
            return true;
        }
        
        // Higher RT priority preempts lower
        wake_entity.rt_priority > curr_entity.rt_priority
    }
    
    /// Find task by PID in RT queue
    pub fn find_by_pid(&self, pid: usize) -> Option<Arc<TaskControlBlock>> {
        for queue in &self.queues {
            if let Some(task) = queue.iter().find(|t| t.pid.0 == pid) {
                return Some(task.clone());
            }
        }
        None
    }
    
    /// Find task by TGID in RT queue
    pub fn find_by_tgid(&self, tgid: usize) -> Option<Arc<TaskControlBlock>> {
        for queue in &self.queues {
            if let Some(task) = queue.iter().find(|t| t.tgid == tgid) {
                return Some(task.clone());
            }
        }
        None
    }
    
    /// Iterate over all RT tasks
    pub fn iter(&self) -> impl Iterator<Item = &Arc<TaskControlBlock>> {
        self.queues.iter().flat_map(|q| q.iter())
    }
}

// ============================================================================
// Idle Run Queue
// ============================================================================

/// Simple FIFO queue for idle-priority tasks
pub struct IdleRunQueue {
    queue: VecDeque<Arc<TaskControlBlock>>,
}

impl Default for IdleRunQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl IdleRunQueue {
    /// Create a new empty idle queue
    pub const fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }
    
    /// Check if empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
    
    /// Get number of idle tasks
    #[inline]
    pub fn len(&self) -> usize {
        self.queue.len()
    }
    
    /// Add task
    pub fn enqueue(&mut self, task: Arc<TaskControlBlock>) {
        self.queue.push_back(task);
    }
    
    /// Remove task
    pub fn dequeue(&mut self, task: &Arc<TaskControlBlock>) {
        if let Some(pos) = self.queue.iter().position(|t| Arc::ptr_eq(t, task)) {
            self.queue.remove(pos);
        }
    }
    
    /// Pick next idle task
    pub fn pick_next(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.queue.pop_front()
    }
    
    /// Find by PID
    pub fn find_by_pid(&self, pid: usize) -> Option<Arc<TaskControlBlock>> {
        self.queue.iter().find(|t| t.pid.0 == pid).cloned()
    }
    
    /// Find by TGID
    pub fn find_by_tgid(&self, tgid: usize) -> Option<Arc<TaskControlBlock>> {
        self.queue.iter().find(|t| t.tgid == tgid).cloned()
    }
    
    /// Iterate
    pub fn iter(&self) -> impl Iterator<Item = &Arc<TaskControlBlock>> {
        self.queue.iter()
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Determine which scheduler class a task belongs to based on its policy
#[inline]
pub fn get_sched_class(entity: &SchedEntity) -> SchedClass {
    match entity.policy {
        SchedPolicy::Fifo | SchedPolicy::RoundRobin => SchedClass::Rt,
        SchedPolicy::Idle => SchedClass::Idle,
        SchedPolicy::Normal | SchedPolicy::Batch => SchedClass::Cfs,
    }
}

/// Scheduler class enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedClass {
    /// Real-time scheduler (highest priority)
    Rt,
    /// Completely Fair Scheduler (normal priority)
    Cfs,
    /// Idle scheduler (lowest priority)
    Idle,
}
