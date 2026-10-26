//! File trait & inode(dir, file, pipe, stdin, stdout)

// AI assistance: OpenAI Codex; reference: rCore Tutorial Guide, chapter 6 exercise.
mod inode;
mod stdio;

use crate::mm::UserBuffer;

/// trait File for all file types
pub trait File: Send + Sync {
    /// Return metadata for regular files, or None for unsupported file types.
    fn stat(&self) -> Option<Stat> {
        None
    }
    /// the file readable?
    fn readable(&self) -> bool;
    /// the file writable?
    fn writable(&self) -> bool;
    /// read from the file to buf, return the number of bytes read
    fn read(&self, buf: UserBuffer) -> usize;
    /// write to the file from buf, return the number of bytes written
    fn write(&self, buf: UserBuffer) -> usize;
}

/// The stat of a inode
#[repr(C)]
#[derive(Debug)]
pub struct Stat {
    /// ID of device containing file
    pub dev: u64,
    /// inode number
    pub ino: u64,
    /// file type and mode
    pub mode: StatMode,
    /// number of hard links
    pub nlink: u32,
    /// unused pad
    pad: [u64; 7],
}

bitflags! {
    /// The mode of a inode
    /// whether a directory or a file
    pub struct StatMode: u32 {
        /// null
        const NULL  = 0;
        /// directory
        const DIR   = 0o040000;
        /// ordinary regular file
        const FILE  = 0o100000;
    }
}

pub use inode::{list_apps, open_file, OSInode, OpenFlags, ROOT_INODE};
pub use stdio::{Stdin, Stdout};

impl Stat {
    /// Create a fully initialized ABI-compatible file status.
    pub fn regular(ino: u64, directory: bool, nlink: u32) -> Self {
        Self {
            dev: 0,
            ino,
            mode: if directory {
                StatMode::DIR
            } else {
                StatMode::FILE
            },
            nlink,
            pad: [0; 7],
        }
    }
}
