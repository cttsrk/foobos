//! Memory management routines

pub mod rangeset;

use core::mem::size_of;

/// A strongly typed physical address
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysAddr(pub u64);


/// A consume-able slice of physical memory
pub struct PhysSlice(PhysAddr, usize);

impl PhysSlice {
    /// Create a new slice to physical memory
    pub unsafe fn new(addr: PhysAddr, size: usize) -> Self {
        PhysSlice(addr, size)
    }

    /// Get the remaining length of the slice
    pub fn len(&self) -> usize {
        self.1
    }

    /// Discard `bytes` from the slice by just updating the pointer and length
    pub fn discard(&mut self, bytes: usize) -> Result<(), ()> {
        if self.1 >= bytes {
            // Update the pointer and length
            (self.0).0 += bytes as u64;
            self.1     -= bytes;
            Ok(())
        } else {
            Err(())
        }
    }

    /// Read a `T` from the slice, updating the pointer
    pub unsafe fn consume<T>(&mut self) -> Result<T, ()> {
        // Make sure we have enough data to consume
        if self.1 < size_of::<T>() {
            return Err(());
        }

        // Read the actual data
        let data = read_phys_unaligned::<T>(self.0);
        
        // Update the pointer and length
        (self.0).0 += size_of::<T>() as u64;
        self.1     -= size_of::<T>();
        Ok(data)
        
    }
}

/// Read a `T` from physical memory address `paddr`
#[inline]
pub unsafe fn read_phys<T>(paddr: PhysAddr) -> T {
    core::ptr::read(paddr.0 as *const T)
}

/// Read an unaligned `T` from physical memory address `paddr`
#[inline]
pub unsafe fn read_phys_unaligned<T>(paddr: PhysAddr) -> T {
    core::ptr::read_unaligned(paddr.0 as *const T)
}
