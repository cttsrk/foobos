//! Physical memory management for the OS

use core::mem::size_of;

/// A strongly typed physical address
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysAddr(pub u64);

impl PhysAddr {
    /*
    /// Read a `T` from the aligned physical address at `self`
    ///
    /// # Returns
    ///
    /// The value in physical memory
    ///
    #[inline]
    pub unsafe fn read<T>(&self) -> T {
        core::ptr::read(self.0 as *const T)
    }

    /// Write a `val` to the aligned physical address at `self`
    ///
    /// # Parameters
    ///
    /// * `val` - The value to write into physical memory
    ///
    #[inline]
    pub unsafe fn write<T>(&self, val: T) {
        core::ptr::write(self.0 as *mut T, val);
    }
    */

    /// Read an unaligned `T` from physical memory address `self`
    ///
    /// # Returns
    ///
    /// The value in physical memory
    ///
    #[inline]
    pub unsafe fn read_unaligned<T>(&self) -> T {
        core::ptr::read_unaligned(self.0 as *const T)
    }
}

/// A consume-able slice of physical memory
pub struct PhysSlice {
    /// The physical address this slice starts at
    addr: PhysAddr,

    /// The length of this physical slice in bytes
    len: usize,
}

impl PhysSlice {
    /// Create a new slice to physical memory at `addr` for `len` bytes
    ///
    /// # Parameters
    ///
    /// * `addr` - The physical address to slice
    /// * `len`  - The size (in bytes) to slice
    /// 
    /// # Returns
    ///
    /// A [`PhysSlice`] representing the memory specified
    ///
    pub unsafe fn new(addr: PhysAddr, len: usize) -> Self {
        PhysSlice { addr, len }
    }

    /// Get the length of the slice
    ///
    /// # Returns
    ///
    /// The length of the slice
    pub fn len(&self) -> usize {
        self.len
    }

    /// Discard `bytes` from the slice by just updating the pointer and length
    ///
    /// # Parameters
    ///
    /// * `bytes` - The number of bytes to discard from the slice, consuming
    ///             from the start of the slice
    ///
    /// # Returns
    ///
    /// `Ok(())` if the bytes were successfully discarded, `Err(())` if the
    /// slice was too small to discard the bytes
    ///
    pub fn discard(&mut self, bytes: usize) -> Result<(), ()> {
        if self.len() >= bytes {
            // Update the pointer and length
            self.addr.0 += bytes as u64;
            self.len    -= bytes;
            Ok(())
        } else {
            Err(())
        }
    }

    /// Read a potentially unaligned `T` from the slice, updating the pointer
    ///
    /// # Returns
    ///
    /// `T` read from the start of the slice, `Err(())` if the slice was too
    /// small to hold a `T`
    ///
    pub unsafe fn consume<T>(&mut self) -> Result<T, ()> {
        // Make sure we have enough data to consume
        if self.len() < size_of::<T>() {
            return Err(());
        }

        // Read the actual data
        let data = self.addr.read_unaligned::<T>();
        
        // Update the pointer and length
        self.addr.0 += size_of::<T>() as u64;
        self.len    -= size_of::<T>();
        Ok(data)
        
    }
}
