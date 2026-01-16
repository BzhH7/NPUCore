//! Process State Machine
//!
//! This module provides a state machine abstraction for process lifecycle management.
//! Instead of using raw status checks scattered throughout the code, we use a type-safe
//! state machine that enforces valid state transitions at compile time where possible.
//!
//! # State Diagram
//!
//! ```text
//!              ┌──────────┐
//!              │  Ready   │◄──────────────────┐
//!              └────┬─────┘                   │
//!                   │ schedule                │
//!                   ▼                         │ yield/preempt
//!              ┌──────────┐                   │
//!     ┌───────►│ Running  │───────────────────┘
//!     │        └────┬─────┘
//!     │             │
//!     │             ├─── exit ───►┌──────────┐
//!     │             │             │  Zombie  │
//!     │             │             └────┬─────┘
//!     │             │                  │ reap
//!     │             │                  ▼
//!     │             │             ┌──────────┐
//!     │             │             │  Exited  │
//!     │             │             └──────────┘
//!     │             │
//!     │             └─── block ──►┌──────────┐
//!     │                           │ Blocked  │
//!     │                           └────┬─────┘
//!     │                                │ wake
//!     └────────────────────────────────┘
//! ```
//!
//! # Design Goals
//!
//! 1. **Type Safety**: Invalid transitions are prevented at compile time
//! 2. **Explicit Transitions**: All state changes go through defined methods
//! 3. **Observability**: State history can be tracked for debugging
//! 4. **Consistency**: Single source of truth for state semantics

use core::sync::atomic::{AtomicU8, Ordering};

/// Process states with explicit discriminants for ABI stability
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProcessState {
    /// Process is created but not yet ready to run
    Created = 0,
    /// Process is ready to be scheduled
    Ready = 1,
    /// Process is currently executing
    Running = 2,
    /// Process is waiting for an event
    Blocked = 3,
    /// Process has exited but not yet reaped
    Zombie = 4,
    /// Process has been reaped and can be cleaned up
    Exited = 5,
    /// Process is being stopped (e.g., for debugging)
    Stopped = 6,
}

impl ProcessState {
    /// Convert from raw u8 value
    #[inline]
    pub const fn from_raw(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Created),
            1 => Some(Self::Ready),
            2 => Some(Self::Running),
            3 => Some(Self::Blocked),
            4 => Some(Self::Zombie),
            5 => Some(Self::Exited),
            6 => Some(Self::Stopped),
            _ => None,
        }
    }

    /// Check if process is in a runnable state
    #[inline]
    pub const fn is_runnable(&self) -> bool {
        matches!(self, Self::Ready | Self::Running)
    }

    /// Check if process has terminated
    #[inline]
    pub const fn is_terminated(&self) -> bool {
        matches!(self, Self::Zombie | Self::Exited)
    }

    /// Check if process can be scheduled
    #[inline]
    pub const fn can_schedule(&self) -> bool {
        matches!(self, Self::Ready)
    }

    /// Human-readable state name
    #[inline]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Ready => "ready",
            Self::Running => "running",
            Self::Blocked => "blocked",
            Self::Zombie => "zombie",
            Self::Exited => "exited",
            Self::Stopped => "stopped",
        }
    }
}

impl core::fmt::Display for ProcessState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Reason for blocking a process
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockReason {
    /// Waiting for I/O completion
    Io,
    /// Waiting for a futex
    Futex,
    /// Waiting for a child process
    WaitChild,
    /// Waiting for a signal
    Signal,
    /// Sleeping (nanosleep, clock_nanosleep)
    Sleep,
    /// Waiting for a mutex/semaphore
    Synchronization,
    /// Waiting for network data
    Network,
    /// Waiting for pipe data
    Pipe,
    /// Other/unspecified reason
    Other,
}

impl BlockReason {
    /// Human-readable description
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Io => "I/O",
            Self::Futex => "futex",
            Self::WaitChild => "wait_child",
            Self::Signal => "signal",
            Self::Sleep => "sleep",
            Self::Synchronization => "sync",
            Self::Network => "network",
            Self::Pipe => "pipe",
            Self::Other => "other",
        }
    }
}

/// Exit reason for terminated processes
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitReason {
    /// Normal exit with code
    Normal(i32),
    /// Killed by signal
    Signal(i32),
    /// Abnormal termination (kernel error)
    Abnormal,
}

impl ExitReason {
    /// Extract exit code for wait() family
    #[inline]
    pub const fn wait_status(&self) -> i32 {
        match self {
            Self::Normal(code) => (*code & 0xff) << 8,
            Self::Signal(sig) => *sig & 0x7f,
            Self::Abnormal => 0x80, // Core dump flag
        }
    }

    /// Check if exited normally
    #[inline]
    pub const fn is_normal(&self) -> bool {
        matches!(self, Self::Normal(_))
    }
}

/// Atomic process state container
/// 
/// Provides lock-free state access with atomic operations.
/// Useful for status checks that don't need full synchronization.
pub struct AtomicProcessState {
    state: AtomicU8,
}

impl AtomicProcessState {
    /// Create new atomic state
    #[inline]
    pub const fn new(initial: ProcessState) -> Self {
        Self {
            state: AtomicU8::new(initial as u8),
        }
    }

    /// Load current state
    #[inline]
    pub fn load(&self) -> ProcessState {
        ProcessState::from_raw(self.state.load(Ordering::Acquire))
            .unwrap_or(ProcessState::Exited)
    }

    /// Store new state
    #[inline]
    pub fn store(&self, new_state: ProcessState) {
        self.state.store(new_state as u8, Ordering::Release);
    }

    /// Compare and swap state
    #[inline]
    pub fn compare_exchange(
        &self,
        expected: ProcessState,
        new: ProcessState,
    ) -> Result<ProcessState, ProcessState> {
        self.state
            .compare_exchange(
                expected as u8,
                new as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .map(|v| ProcessState::from_raw(v).unwrap_or(ProcessState::Exited))
            .map_err(|v| ProcessState::from_raw(v).unwrap_or(ProcessState::Exited))
    }
}

impl Default for AtomicProcessState {
    fn default() -> Self {
        Self::new(ProcessState::Created)
    }
}

/// State transition validator
///
/// Enforces valid state transitions according to the state machine.
pub struct StateTransitionValidator;

impl StateTransitionValidator {
    /// Check if transition from `from` to `to` is valid
    #[inline]
    pub const fn is_valid_transition(from: ProcessState, to: ProcessState) -> bool {
        use ProcessState::*;
        
        match (from, to) {
            // From Created
            (Created, Ready) => true,
            
            // From Ready
            (Ready, Running) => true,
            (Ready, Zombie) => true,  // Exit before ever running
            
            // From Running
            (Running, Ready) => true,     // Preempted
            (Running, Blocked) => true,   // Waiting for resource
            (Running, Zombie) => true,    // exit()
            (Running, Stopped) => true,   // SIGSTOP
            
            // From Blocked
            (Blocked, Ready) => true,     // Resource available
            (Blocked, Zombie) => true,    // Killed while blocked
            
            // From Zombie
            (Zombie, Exited) => true,     // Reaped by parent
            
            // From Stopped
            (Stopped, Ready) => true,     // SIGCONT
            (Stopped, Zombie) => true,    // Killed while stopped
            
            // All other transitions are invalid
            _ => false,
        }
    }

    /// Attempt transition, returning error if invalid
    pub fn try_transition(
        current: &AtomicProcessState,
        new_state: ProcessState,
    ) -> Result<ProcessState, TransitionError> {
        let old = current.load();
        
        if !Self::is_valid_transition(old, new_state) {
            return Err(TransitionError {
                from: old,
                to: new_state,
            });
        }
        
        match current.compare_exchange(old, new_state) {
            Ok(prev) => Ok(prev),
            Err(actual) => Err(TransitionError {
                from: actual,
                to: new_state,
            }),
        }
    }
}

/// Error for invalid state transitions
#[derive(Debug, Clone, Copy)]
pub struct TransitionError {
    /// State we were transitioning from
    pub from: ProcessState,
    /// State we were trying to reach
    pub to: ProcessState,
}

impl core::fmt::Display for TransitionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "invalid process state transition: {} -> {}",
            self.from, self.to
        )
    }
}

// ============================================================================
// Process State Query Helpers
// ============================================================================

/// Check if a task is ready to run
#[inline]
pub fn is_task_ready(state: ProcessState) -> bool {
    state.can_schedule()
}

/// Check if a task should be considered for scheduling
#[inline]
pub fn is_schedulable(state: ProcessState) -> bool {
    state.is_runnable()
}

/// Check if a task has terminated
#[inline]
pub fn is_task_terminated(state: ProcessState) -> bool {
    state.is_terminated()
}

/// Check if a task is waiting for something
#[inline]
pub fn is_task_blocked(state: ProcessState) -> bool {
    matches!(state, ProcessState::Blocked | ProcessState::Stopped)
}
