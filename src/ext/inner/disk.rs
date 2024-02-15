use crate::ext::{Errno, IoResult};
use core::mem::{size_of, MaybeUninit};

pub trait RWS {
    fn read(&mut self, buf: &mut [u8])-> IoResult<u64>;
    fn read_at(&mut self, addr: u64, buf: &mut [u8])-> IoResult<u64>;
    fn write(&mut self, buf: &[u8])-> IoResult<u64>;
    fn write_at(&mut self, addr: u64, buf: &[u8])-> IoResult<u64>;
    fn seek(&mut self, offset: u64)-> IoResult<()>;
    fn seek_absolute(&mut self, to: u64)-> IoResult<()>;
    fn rewind(&mut self) -> IoResult<()> {
        self.seek_absolute(0)
    }
}

pub struct Disk<T: RWS>(pub T);

impl<T: RWS> Disk<T> {
    pub fn write_buffer(&mut self, offset: u64, buf: &[u8]) -> IoResult<u64> {
        let _r = self.0.seek_absolute(offset);
        self.0.write(buf)
    }

    pub fn read_buffer(&mut self, offset: u64, buf: &mut [u8]) -> IoResult<u64> {
        let _r = self.0.seek_absolute(offset);
        self.0.read(buf)
    }

    /// Write a particulary struct inside file object
    pub fn write_struct<C: Copy>(&mut self, offset: u64, t: &C) -> IoResult<u64> {
        let s = unsafe { core::slice::from_raw_parts(t as *const _ as *const u8, size_of::<C>()) };
        self.write_buffer(offset, s)
    }

    /// Read a particulary struct in file object
    pub fn read_struct<C: Copy>(&mut self, offset: u64) -> IoResult<C> {
        let t = MaybeUninit::<C>::uninit();
        let count = self.read_buffer(offset, unsafe {
            core::slice::from_raw_parts_mut(t.as_ptr() as *mut u8, size_of::<C>())
        })?;
        let t = unsafe { t.assume_init() };
        if count as usize != size_of::<C>() {
            return Err(Errno::OutOfSpace);
        }
        Ok(t)
    }
}
