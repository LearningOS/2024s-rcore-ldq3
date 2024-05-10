//! Types related to task management

use super::TaskContext;
use alloc::collections::BTreeMap;

/// The task control block (TCB) of a task.
#[derive(Clone)]
pub struct TaskControlBlock {
    /// The task status in it's lifecycle
    pub task_status: TaskStatus,
    /// The task context
    pub task_cx: TaskContext,
    /// The times of every syscall called by the task
    pub syscall_times: BTreeMap<usize, u32>,
    /// The start time of the task
    pub start_time: usize
}

/// The status of a task
#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    /// uninitialized
    UnInit,
    /// ready to run
    Ready,
    /// running
    Running,
    /// exited
    Exited,
}
