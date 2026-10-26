//! Synchronization and interior mutability primitives

// AI assistance: OpenAI Codex; reference: rCore Tutorial Guide, chapter 8 exercise.
mod condvar;
mod mutex;
mod semaphore;
mod up;

pub use condvar::Condvar;
pub use mutex::{Mutex, MutexBlocking, MutexSpin};
pub use semaphore::Semaphore;
pub use up::UPSafeCell;

mod deadlock;
pub use deadlock::DeadlockDetector;
