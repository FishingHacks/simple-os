//! this module contains methods of the Ext2 which constitute the posix interface
use crate::ext::inner::body::*;
use crate::ext::inner::DirectoryEntryType;
use crate::ext::inner::Ext2Filesystem;
use crate::ext::inner::{DirectoryEntry, Inode};
use crate::ext::Errno;
use crate::ext::FilePerms;
use crate::ext::FileType;
use crate::ext::IoResult;
use crate::ext::UtimeBuffer;

use core::cmp::min;
use super::RWS;

impl<T: RWS> Ext2Filesystem<T> {
    /// return all the (directory, inode) conainted in inode_nbr
    pub fn lookup_directory<'a>(
        &'a self,
        inode_nbr: u32,
    ) -> IoResult<impl Iterator<Item = Entry> + 'a> {
        Ok(self
            .iter_entries(inode_nbr)?
            .into_iter()
            .filter_map(|(dir, _)| match self.get_inode(dir.get_inode()) {
                Ok((inode, _)) => Some(Entry {
                    directory: dir,
                    inode,
                }),
                Err(_e) => None,
            }))
    }

    pub fn read_inode(&self, inode_number: u32) -> IoResult<Inode> {
        Ok(self.get_inode(inode_number)?.0)
    }

    /// return the block size of ext2
    pub fn get_block_size(&self) -> u32 {
        self.block_size
    }

    /// Superblock ascessor
    pub fn get_superblock(&self) -> super::SuperBlock {
        self.superblock
    }
    /// The utime() function shall set the access and modification
    /// times  of the file named by the path argument.
    ///
    /// If times is a null pointer, the access and modification times
    /// of the file shall be set to the current time.
    pub fn utime(
        &mut self,
        inode_number: u32,
        times: Option<&UtimeBuffer>,
        current_time: u32,
    ) -> IoResult<()> {
        let (mut inode, inode_addr) = self.get_inode(inode_number)?;

        if let Some(times) = times {
            inode.last_access_time = times.access_time;
            inode.last_modification_time = times.modification_time;
        } else {
            inode.last_access_time = current_time;
            inode.last_modification_time = current_time;
        }

        self.disk.borrow_mut().write_struct(inode_addr, &inode)?;
        Ok(())
    }

    /// The chown() function shall change the user and group ownership
    /// of a file.
    pub fn chown(&mut self, inode_nbr: u32, owner: u16, group: u16) -> IoResult<()> {
        let (mut inode, inode_addr) = self.get_inode(inode_nbr)?;

        if owner != u16::max_value() {
            inode.user_id = owner;
        }

        if group != u16::max_value() {
            inode.group_id = group;
        }

        self.disk.borrow_mut().write_struct(inode_addr, &inode)?;
        Ok(())
    }

    /// The lchown() function shall be equivalent to chown(), except
    /// in the case where the named file is a symbolic link. In this
    /// case, lchown() shall change the ownership of the symbolic link
    // pub fn lchown(&mut self, inode_nbr: u32, owner: uid_t, group: gid_t) -> IoResult<()> {
    //     unimplemented!();
    // }

    /// The chmod() function shall change S_ISUID, S_ISGID, [XSI]
    /// [Option Start] S_ISVTX, [Option End] and the file permission
    /// bits of the file
    pub fn chmod(&mut self, inode_nbr: u32, mode: u16) -> IoResult<()> {
        // Ensure that only the file permission bits and special bits are modified.
        let mut mode = mode as u16;
        let mask = *SPECIAL_BITS | *PERMISSIONS_MASK;
        mode &= mask;

        let (mut inode, inode_addr) = self.get_inode(inode_nbr)?;
        inode.type_and_perm.remove_mode(mask);
        inode.type_and_perm.insert_mode(mode);

        self.disk.borrow_mut().write_struct(inode_addr, &inode)?;
        Ok(())
    }

    /// The Truncate() Function Shall cause the regular file named by
    /// path to have a size which shall be equal to length bytes.
    pub fn truncate(&mut self, inode_nbr: u32, new_size: u64) -> IoResult<()> {
        let (mut inode, inode_addr) = self.get_inode(inode_nbr)?;
        if !inode.is_a_regular_file() {
            return Err(Errno::IsDirectory);
        }
        self.truncate_inode((&mut inode, inode_addr), new_size)
    }

    pub fn create(
        &mut self,
        filename: &str,
        parent_inode_nbr: u32,
        timestamp: u32,
        type_perm: TypePerm,
        (owner, group): (u16, u16),
    ) -> IoResult<Entry> {
        let direntry_type = DirectoryEntryType::try_from(type_perm)?;
        let inode_nbr = self.alloc_inode().ok_or(Errno::OutOfSpace)?;
        let (_, inode_addr) = self.get_inode(inode_nbr)?;
        let mut inode = Inode::new(type_perm);

        inode.set_owner(owner);
        inode.set_group(group);
        inode.last_access_time = timestamp;
        inode.creation_time = timestamp;
        inode.last_modification_time = timestamp;

        self.disk.borrow_mut().write_struct(inode_addr, &inode)?;

        let mut new_entry = DirectoryEntry::new(filename, direntry_type, inode_nbr)?;
        self.push_entry(parent_inode_nbr, &mut new_entry)?;
        Ok(Entry {
            directory: new_entry,
            inode,
        })
    }

    /// The unlink() function shall remove a link to a file.
    pub fn unlink(
        &mut self,
        parent_inode_nbr: u32,
        filename: &str,
        free_inode_data: bool,
    ) -> IoResult<()> {
        let entry = self.find_entry_in_inode(parent_inode_nbr, filename)?;
        self.unlink_inode(entry.0.get_inode(), free_inode_data)?;
        self.delete_entry(parent_inode_nbr, entry.1).expect("WTF");
        Ok(())
    }

    // /// create a directory entry and an inode on the Directory inode:
    // /// `parent_inode_nbr`, return the new inode nbr
    pub fn create_dir(
        &mut self,
        parent_inode_nbr: u32,
        filename: &str,
        timestamp: u32,
        mode: u16,
        (owner, group): (u16, u16),
    ) -> IoResult<Entry> {
        let inode_nbr = self.alloc_inode().ok_or(Errno::OutOfSpace)?;
        let (_, inode_addr) = self.get_inode(inode_nbr)?;
        let mut inode = Inode::new(TypePerm(mode | FileType::Directory as u16));
        inode.nbr_hard_links = 2;
        inode.set_owner(owner);
        inode.set_group(group);
        inode.last_access_time = timestamp;
        inode.creation_time = timestamp;
        inode.last_modification_time = timestamp;
        inode.low_size = 1024 << self.superblock.get_log2_block_size();

        self.disk.borrow_mut().write_struct(inode_addr, &inode)?;
        let mut new_entry =
            DirectoryEntry::new(filename, DirectoryEntryType::Directory, inode_nbr)?;
        self.push_entry(parent_inode_nbr, &mut new_entry)?;

        let mut point = DirectoryEntry::new(".", DirectoryEntryType::Directory, inode_nbr)?;
        let mut point_point =
            DirectoryEntry::new("..", DirectoryEntryType::Directory, parent_inode_nbr)?;
        self.push_entry(inode_nbr, &mut point)?;
        self.push_entry(inode_nbr, &mut point_point)?;
        Ok(Entry {
            directory: new_entry,
            inode,
        })
    }

    /// The rmdir() function shall remove the directory pointed by
    /// filename in the parent directory corresponding to
    /// parent_inode_nbr
    pub fn rmdir(&mut self, parent_inode_nbr: u32, filename: &str) -> IoResult<()> {
        let entry = self.find_entry_in_inode(parent_inode_nbr, filename)?;
        let inode_nbr = entry.0.get_inode();
        let (mut inode, inode_addr) = self.get_inode(inode_nbr)?;

        if !inode.is_a_directory() {
            return Err(Errno::NotDirectory);
        }
        self.free_inode((&mut inode, inode_addr), inode_nbr)?;
        self.delete_entry(parent_inode_nbr, entry.1)?;
        Ok(())
    }

    /// for write syscall
    pub fn write(
        &mut self,
        inode_nbr: u32,
        file_offset: &mut u64,
        buf: &[u8],
    ) -> IoResult<(u64, Inode)> {
        let (mut inode, inode_addr) = self.get_inode(inode_nbr)?;
        let file_curr_offset_start = *file_offset;
        if *file_offset > inode.get_size() {
            // panic!("file_offset > inode.get_size()");
            return Ok((0, inode));
        }
        if buf.len() == 0 {
            return Ok((0, inode));
        }
        let data_address = self.inode_data_alloc((&mut inode, inode_addr), *file_offset)?;
        let offset = min(
            self.block_size as u64 - *file_offset % self.block_size as u64,
            buf.len() as u64,
        );
        let data_write = self
            .disk
            .borrow_mut()
            .write_buffer(data_address, &buf[0..offset as usize])?;
        *file_offset += data_write as u64;
        if inode.get_size() < *file_offset {
            inode.update_size(*file_offset, self.block_size);
            self.disk.borrow_mut().write_struct(inode_addr, &inode)?;
        }
        if data_write < offset {
            return Ok((*file_offset - file_curr_offset_start, inode));
        }

        for chunk in buf[offset as usize..].chunks(self.block_size as usize) {
            let data_address = self.inode_data_alloc((&mut inode, inode_addr), *file_offset)?;
            let data_write = self.disk.borrow_mut().write_buffer(data_address, &chunk)?;
            *file_offset += data_write as u64;
            if inode.get_size() < *file_offset {
                inode.update_size(*file_offset, self.block_size);
                self.disk.borrow_mut().write_struct(inode_addr, &inode)?;
            }
            if data_write < chunk.len() as u64 {
                return Ok((*file_offset - file_curr_offset_start, inode));
            }
        }
        Ok((*file_offset - file_curr_offset_start, inode))
    }

    /// for read syscall
    pub fn read(
        &mut self,
        inode_nbr: u32,
        file_offset: &mut u64,
        mut buf: &mut [u8],
    ) -> IoResult<u64> {
        let (mut inode, _inode_addr) = self.get_inode(inode_nbr)?;

        // EOF
        if *file_offset >= inode.get_size() {
            return Ok(0);
        }

        // Resize buf if read overflow
        if *file_offset + buf.len() as u64 > inode.get_size() {
            buf = &mut buf[..(inode.get_size() - *file_offset) as usize];
        }

        // Invalidate the cache used after
        self.cache.invalidate();

        let file_curr_offset_start = *file_offset;
        let block_mask = (self.block_size - 1) as u64;

        while buf.len() != 0 {
            let mut bytes_to_read = 0;

            let mut start_data_address = None;
            let mut last_data_address: Option<u64> = None;
            loop {
                let data_address = self.inode_data(&mut inode, *file_offset).expect("WTF");
                if let Some(last_address) = last_data_address {
                    if data_address != last_address + self.block_size as u64 {
                        break;
                    }
                } else {
                    start_data_address = Some(data_address);
                }
                let mut bytes = min(
                    self.block_size as u64 - (*file_offset & block_mask),
                    inode.get_size() - *file_offset,
                );
                bytes = min(bytes, buf.len() as u64 - bytes_to_read);

                *file_offset += bytes;
                bytes_to_read += bytes;
                if bytes_to_read == buf.len() as u64 {
                    break;
                }
                last_data_address = Some(data_address);
            }
            let data_read = self.disk.borrow_mut().read_buffer(
                start_data_address.expect("WOOT"),
                &mut buf[0..bytes_to_read as usize],
            )?;
            assert!(data_read == bytes_to_read);
            buf = &mut buf[bytes_to_read as usize..];
        }
        Ok(*file_offset - file_curr_offset_start)
    }

    pub fn symlink(
        &mut self,
        parent_inode_nbr: u32,
        target: &str,
        filename: &str,
        timestamp: u32,
    ) -> IoResult<Entry> {
        let direntry_type = DirectoryEntryType::SymbolicLink;
        let inode_nbr = self.alloc_inode().ok_or(Errno::OutOfSpace)?;
        let (_, inode_addr) = self.get_inode(inode_nbr)?;
        // user: rwx
        // group: rwx
        // other: rwx
        let mode = FilePerms::AllAllowed as u16;
        let access_mode = TypePerm(mode | FileType::Symlink as u16);
        let mut inode = Inode::new(access_mode);
        if target.len() <= Inode::FAST_SYMLINK_SIZE_MAX {
            // If target is a fast symlink write the target directly
            // on inode
            inode.write_symlink(target);
        }

        inode.last_access_time = timestamp;
        inode.creation_time = timestamp;
        inode.last_modification_time = timestamp;

        self.disk.borrow_mut().write_struct(inode_addr, &inode)?;
        if target.len() > Inode::FAST_SYMLINK_SIZE_MAX {
            // Else write on the inode data after writing the empty
            // inode on the disk
            let mut offset = 0;
            self.write(inode_nbr, &mut offset, target.as_bytes())?;
            // fetch the inode
            let (inode_updated, _) = self.get_inode(inode_nbr)?;
            inode = inode_updated;
        }

        let mut new_entry = DirectoryEntry::new(filename, direntry_type, inode_nbr)?;
        self.push_entry(parent_inode_nbr, &mut new_entry)?;
        Ok(Entry {
            directory: new_entry,
            inode,
        })
    }

    pub fn link(
        &mut self,
        parent_inode_nbr: u32, // parent directory of new hard link
        target_inode_nbr: u32, // link target
        filename: &str,        // hard link filename
    ) -> IoResult<Entry> {
        let (mut inode, inode_addr) = self.get_inode(target_inode_nbr)?;
        if !inode.is_a_regular_file() {
            return Err(Errno::AccessError);
        }
        let mut new_entry =
            DirectoryEntry::new(filename, DirectoryEntryType::RegularFile, target_inode_nbr)?;
        self.push_entry(parent_inode_nbr, &mut new_entry)?;

        inode.nbr_hard_links += 1;
        self.disk.borrow_mut().write_struct(inode_addr, &inode)?;
        Ok(Entry {
            directory: new_entry,
            inode,
        })
    }

    pub fn rename(
        &mut self,
        parent_inode_nbr: u32,
        filename: &str,
        new_parent_inode_nbr: u32,
        new_filename: &str,
    ) -> IoResult<()> {
        let (mut entry, entry_offset) = self.find_entry_in_inode(parent_inode_nbr, filename)?;
        self.delete_entry(parent_inode_nbr, entry_offset)?;
        entry.set_filename(new_filename)?;

        self.push_entry(new_parent_inode_nbr, &mut entry)?;
        Ok(())
    }

    // /// return the root inode of the ext2
    // pub fn root_inode(&self) -> IoResult<Inode> {
    //     Ok(self.get_inode(2).expect("no inode 2, wtf").0)
    // }

    // pub fn remove_inode(&mut self, inode_nbr: u32) -> IoResult<()> {
    //     let (mut inode, inode_addr) = self.get_inode(inode_nbr)?;
    //     assert!(inode.nbr_hard_links == 0);
    //     self.free_inode((&mut inode, inode_addr), inode_nbr)
    // }
}
