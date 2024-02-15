use alloc::string::String;

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u16)]
pub enum FileType {
    FiFo = 0x1000,
    CharacterDevice = 0x2000,
    Directory = 0x4000,
    BlockDevice = 0x6000,
    RegularFile = 0x8000,
    Symlink = 0xa000,
    Socket = 0xc000,
    Unknown = 0xf000,
}

impl FileType {
    pub fn is_empty(&self) -> bool {
        match *self as u16 {
            0x1000 | 0x2000 | 0x4000 | 0x6000 | 0x8000 | 0xa000 | 0xc000 => true,
            _ => false,
        }
    }

    pub fn from(v: u16) -> Self {
        match v {
            0x1000 => Self::FiFo,
            0x2000 => Self::CharacterDevice,
            0x4000 => Self::Directory,
            0x6000 => Self::BlockDevice,
            0x8000 => Self::RegularFile,
            0xa000 => Self::Symlink,
            0xc000 => Self::Socket,
            _ => Self::Unknown,
        }
    }
}

pub struct DirEntry {
    inode: u32,
    offset: u64,
    file_type: FileType,
    file_name: [i8; 256],
}

impl DirEntry {
    pub fn new(inode: u32, offset: u64, file_type: FileType, file_name: [i8; 256]) -> Self {
        Self {
            file_name,
            file_type,
            inode,
            offset,
        }
    }
}

#[repr(u16)]
pub enum FilePerms {
    UserExec = 0o100,
    UserWrite = 0o200,
    UserRead = 0o400,

    UserRWX = 0o700,
    UserRW = 0o600,
    UserRX = 0o500,
    UserWX = 0o300,

    GroupExec = 0o10,
    GroupWrite = 0o20,
    GroupRead = 0o40,

    GroupRWX = 0o70,
    GroupRW = 0o60,
    GroupRX = 0o50,
    GroupWX = 0o30,

    OtherExec = 0o1,
    OtherWrite = 0o2,
    OtherRead = 0o4,

    OtherRWX = 0o7,
    OtherRW = 0o6,
    OtherRX = 0o5,
    OtherWX = 0o3,

    Sticky = 0o1000,
    SetUID = 0o2000,
    SetGID = 0o4000,

    AllExec = 0o111,
    AllWrite = 0o222,
    AllRead = 0o444,

    AllAllowed = 0o777,
}

pub struct AccessFlags(u8);

impl AccessFlags {
    pub fn from(flags: u8) -> Self {
        Self(flags & 0b111)
    }

    pub fn execute_ok(&self) -> bool {
        (self.0 & 1) > 0
    }

    pub fn write_ok(&self) -> bool {
        (self.0 & 2) > 0
    }

    pub fn read_ok(&self) -> bool {
        (self.0 & 4) > 0
    }
}

pub struct Stat {
    pub device_id: u64,
    pub inode_id: u32,
    pub number_hard_links: u64,
    pub type_and_perms: u32,
    pub user_id: u16,
    pub group_id: u16,
    __padding: i32,
    pub special_device_id: u64,
    pub size: u64,
    pub block_size: u32,
    pub number_blocks: u32,
    pub last_access: u32,
    pub last_access_ns: u32,
    pub last_modification: u32,
    pub last_modification_ns: u32,
    pub creation_time: u32,
    pub creation_time_ns: u32,
    __unused: [i64; 3],
}

pub struct UtimeBuffer {
    pub access_time: u32,
    pub modification_time: u32,
}

#[derive(Debug, PartialEq)]
pub struct Path(String);

impl Path {
    pub fn new<S: Into<String>>(s: S) -> Self {
        Self(s.into())
    }

    pub fn is_absolute(&self) -> bool {
        self.0.starts_with('/')
    }

    pub fn has_relative(&self) -> bool {
        self.0.contains("..")
    }

    pub fn components(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }

    pub fn parent(&self) -> Option<Self> {
        let mut str: String = String::with_capacity(100);

        let mut iter = self.components();
        let mut last = iter.next();
        if last.is_none() { return None; }

        while let Some(v) = iter.next() {
            str.push('/');
            str.push_str(last.unwrap_or_default());
            last = Some(v);
        }

        Some(Self(str))
    }

    pub fn file_name(&self) -> &String {
        &self.0
    }
}

pub unsafe fn compare(a: &[i8], b: &[i8], len: usize) -> bool {
    if a.len() < len || b.len() < len {
        return false;
    }

    for i in 0..len {
        if a[i] != b[i] {
            return false;
        }
    }
    true
}
