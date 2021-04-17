//! Memory management routines

/// A strongly typed physical address
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysAddr(pub u64);

/// Read a `T` from physical memory address `paddr`
#[inline]
pub unsafe fn read_phys<T>(paddr: PhysAddr) -> T {
    core::ptr::read_volatile(paddr.0 as *const T)
}
