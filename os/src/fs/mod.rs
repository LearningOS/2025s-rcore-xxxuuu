//! File trait & inode(dir, file, pipe, stdin, stdout)

mod inode;
mod stdio;

use crate::mm::UserBuffer;

/// trait File for all file types
pub trait File: Send + Sync {
    /// the file readable?
    fn readable(&self) -> bool;
    /// the file writable?
    fn writable(&self) -> bool;
    /// read from the file to buf, return the number of bytes read
    fn read(&self, buf: UserBuffer) -> usize;
    /// write to the file from buf, return the number of bytes written
    fn write(&self, buf: UserBuffer) -> usize;
    /// get the stat of a file
    fn stat(&self) -> FileStat;
}

/// The stat of a file
pub enum FileStat {
    /// The stat of a console
    Console,
    /// The stat of a easy-fs file
    EasyFsFileStat(Stat),
}

pub use inode::{list_apps, open_file, link_at, unlink, OSInode, OpenFlags};
pub use stdio::{Stdin, Stdout};
pub use easy_fs::{Stat, StatMode};
