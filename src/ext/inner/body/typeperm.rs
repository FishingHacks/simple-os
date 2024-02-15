//! This file describe Type and Permissions first inode field and methods

use crate::ext::{AccessFlags, Errno, FilePerms, FileType};


#[derive(Default, Debug, PartialEq, Copy, Clone, Eq)]
pub struct TypePerm(pub u16);

lazy_static::lazy_static! {
    pub static ref SPECIAL_BITS: u16 = (FilePerms::SetUID as u16) | (FilePerms::SetGID as u16) | (FilePerms::Sticky as u16);
    pub static ref PERMISSIONS_MASK: u16 = FilePerms::AllAllowed as u16;
}
// bitflags! {
//     #[derive(Default, Debug, PartialEq, Copy, Clone, Eq)]
//     pub struct TypePerm: u16 {
//         const S_IFMT = S_IFMT as u16;
//         const UNIX_SOCKET = S_IFSOCK as u16;
//         const SYMBOLIC_LINK = S_IFLNK as u16;
//         const REGULAR_FILE = S_IFREG as u16;
//         const BLOCK_DEVICE = S_IFBLK as u16;
//         const DIRECTORY = S_IFDIR as u16;
//         const CHARACTER_DEVICE = S_IFCHR as u16;
//         const FIFO = S_IFIFO as u16;

//         const SET_USER_ID = S_ISUID as u16;
//         const SET_GROUP_ID = S_ISGID as u16;
//         const STICKY_BIT = S_ISVTX as u16;
//         const SPECIAL_BITS = Self::SET_USER_ID.bits() |
//                             Self::SET_GROUP_ID.bits() |
//                             Self::STICKY_BIT.bits();

//         const S_IRWXU = Mode::S_IRWXU as u16;
//         const USER_READ_PERMISSION = S_IRUSR as u16;
//         const USER_WRITE_PERMISSION = S_IWUSR as u16;
//         const USER_EXECUTE_PERMISSION = S_IXUSR as u16;

//         const S_IRWXG = S_IRWXG as u16;
//         const GROUP_READ_PERMISSION = S_IRGRP as u16;
//         const GROUP_WRITE_PERMISSION = S_IWGRP as u16;
//         const GROUP_EXECUTE_PERMISSION = S_IXGRP as u16;

//         const S_IRWXO = S_IRWXO as u16;
//         const OTHER_READ_PERMISSION = S_IROTH as u16;
//         const OTHER_WRITE_PERMISSION = S_IWOTH as u16;
//         const OTHER_EXECUTE_PERMISSION = S_IXOTH as u16;
//         const PERMISSIONS_MASK = S_IRWXU as u16 |
//                                  S_IRWXG as u16 |
//                                  S_IRWXO as u16;
//     }
// }

#[allow(unused)]
impl TypePerm {
    pub fn remove_mode(&mut self, mode: u16) {
        let new_mode = self.0 & !mode;
        self.0 = new_mode;
    }

    pub fn insert_mode(&mut self, mode: u16) {
        let new_mode = self.0 | mode;
        self.0 = new_mode;
    }

    pub fn extract_type(self) -> FileType {
        let mask = FileType::Unknown as u16;
        FileType::from(self.0 & mask)
    }

    pub fn is_typed(&self) -> bool {
        !self.extract_type().is_empty()
    }

    pub fn is_character_device(&self) -> bool {
        self.extract_type() == FileType::CharacterDevice
    }

    pub fn is_fifo(&self) -> bool {
        self.extract_type() == FileType::FiFo
    }

    pub fn is_regular(&self) -> bool {
        self.extract_type() == FileType::RegularFile
    }

    pub fn is_directory(&self) -> bool {
        self.extract_type() == FileType::Directory
    }

    pub fn is_symlink(&self) -> bool {
        self.extract_type() == FileType::Symlink
    }

    pub fn is_socket(&self) -> bool {
        self.extract_type() == FileType::Socket
    }

    pub fn is_block_device(&self) -> bool {
        self.extract_type() == FileType::BlockDevice
    }

    /// returns the owner rights on the file, in a bitflags Amode
    pub fn owner_access(&self) -> AccessFlags {
        let mask = FilePerms::UserRWX;
        AccessFlags::from(mask as u8 >> 6)
    }

    /// returns the group rights on the file, in a bitflags Amode
    pub fn group_access(&self) -> AccessFlags {
        let mask = FilePerms::GroupRWX;
        AccessFlags::from(mask as u8 >> 3)
    }

    /// returns the other rights on the file, in a bitflags Amode
    pub fn other_access(&self) -> AccessFlags {
        let mask = FilePerms::OtherRWX;
        AccessFlags::from(mask as u8)
    }

    pub fn class_access(&self, class: PermissionClass) -> AccessFlags {
        match class {
            PermissionClass::Owner => self.owner_access(),
            PermissionClass::Group => self.group_access(),
            PermissionClass::Other => self.other_access(),
        }
    }

    /// Returns whether self is solely composed of special bits and/or file permissions bits
    pub fn is_pure_mode(&self) -> bool {
        let mask = *SPECIAL_BITS | *PERMISSIONS_MASK;
        let excess_bits = self.0 & !mask;
        excess_bits == 0
    }
    pub fn extract_pure_mode(self) -> Self {
        let mask = *SPECIAL_BITS | *PERMISSIONS_MASK;
        Self(self.0 & mask)
    }
}

impl TryFrom<(FilePerms, FileType)> for TypePerm {
    type Error = Errno;
    fn try_from(values: (FilePerms, FileType)) -> Result<Self, Self::Error> {
        Ok(Self(((values.1 as u16) << 12) | (values.0 as u16)))
    }
}

/// Also known as File Classes in POSIX-2018.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[allow(unused)]
pub enum PermissionClass {
    Owner,
    Group,
    Other,
}
