//! Types related to task management

// AI assistance: OpenAI Codex; reference: rCore Tutorial Guide, chapter 3 exercise.
use super::TaskContext;

/// The task control block (TCB) of a task.
#[derive(Copy, Clone)]
pub struct TaskControlBlock {
    /// Per-task counters for the syscall IDs supported by this kernel.
    pub syscall_counts: [usize; 8],
    /// The task status in it's lifecycle
    pub task_status: TaskStatus,
    /// The task context
    pub task_cx: TaskContext,
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
