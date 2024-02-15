//! This module provide common structures and methods for EXT2 filesystems
mod directory_entry;
mod inode;
mod typeperm;

use core::{borrow::Borrow, cmp::Ordering};

pub use directory_entry::{DirectoryEntry, DirectoryEntryType};
pub use inode::Inode;
pub use typeperm::{TypePerm, PERMISSIONS_MASK, SPECIAL_BITS};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(align(512))]
pub struct Entry {
    pub directory: DirectoryEntry,
    pub inode: Inode,
}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let s1: &str = self.borrow();
        let s2: &str = other.borrow();
        Some(s1.cmp(s2))
    }
}

impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Borrow<str> for Entry {
    fn borrow(&self) -> &str {
        unsafe { self.directory.get_filename() }
    }
}
