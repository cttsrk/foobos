//! A library which implements support for the ACPI generic access structure.
//! We implement this in its own library so information parsed out from ACPI
//! can easily be passed to ACPI-unaware code.

#![feature(asm)]
#![no_std]

use core::convert::TryInto;

/// A `Result` type which wraps a GAS error
pub type Result<T> = core::result::Result<T, Error>;

/// A generic access structure error
#[derive(Debug)]
pub enum Error {
    /// A register bit width specified by a Generic Address Structure was zero
    WidthZero,

    /// A register bit width specified by a Generic Address Structure was not
    /// divisible by 8
    WidthNotMod8,

    /// A register bit offset was non-zero, this is allowed by the spec but is
    /// not supposed to happen on architectures supported by this OS
    OffsetNonZero,

    /// An integer overflow occurred when computing a Generic Address Structure
    /// offset
    AddressOverflow,

    /// A Generic Address Structure with an unimplemented type, not supported
    TypeUnimplemented,

    /// An access size was not specified by a Generic Address Structure and we
    /// were unable to satisfy the operation
    InvalidAccessSize,

    /// An I/O port address was specified in a Generic Address Structure and
    /// I/O ports are not supported on this architecture
    #[allow(dead_code)]
    IoPortNotAvailable,

}

/// An acess size for an ACPI Generaic Access Structure
#[derive(Debug, Clone, Copy)]
pub enum AccessSize {
    /// Undefined (legacy reasons)
    Undefined,

    /// Byte access
    Byte,

    /// Word access
    Word,

    /// Dword access
    Dword,

    /// Qword access
    Qword,

    /// Not defined by the spec
    Unspecified,
}

impl From<u8> for AccessSize {
    fn from(val: u8) -> Self {
        match val {
            0 => Self::Undefined,
            1 => Self::Byte,
            2 => Self::Word,
            3 => Self::Dword,
            4 => Self::Qword,
            _ => Self::Unspecified,
        }
    }
}

/// An I/O port address
#[derive(Clone, Copy, Debug)]
pub struct IoAddr(pub u64);

impl IoAddr {
    /// Read a value from an I/O port
    ///
    /// # Returns
    ///
    /// The value read from the I/O port, or en error [`Error`]
    ///
    unsafe fn read_u8(&self) -> Result<u8> {
        #[cfg(target_arch = "x86_64")]
        {
            let val: u8;
            asm!("in al, dx", out("al") val, in("dx") self.0,
                options(nomem, nostack, preserves_flags));
            Ok(val)
        }

        #[cfg(not(target_arch = "x86_64"))]
        Err(Error::IoPortNotAvailable)
    }

    /// Read a value from an I/O port
    ///
    /// # Returns
    ///
    /// The value read from the I/O port, or en error [`Error`]
    ///
    unsafe fn read_u16(&self) -> Result<u16> {
        #[cfg(target_arch = "x86_64")]
        {
            let val: u16;
            asm!("in ax, dx", out("ax") val, in("dx") self.0,
                options(nomem, nostack, preserves_flags));
            Ok(val)
        }

        #[cfg(not(target_arch = "x86_64"))]
        Err(Error::IoPortNotAvailable)
    }

    /// Read a value from an I/O port
    ///
    /// # Returns
    ///
    /// The value read from the I/O port, or en error [`Error`]
    ///
    unsafe fn read_u32(&self) -> Result<u32> {
        #[cfg(target_arch = "x86_64")]
        {
            let val: u32;
            asm!("in eax, dx", out("eax") val, in("dx") self.0,
                options(nomem, nostack, preserves_flags));
            Ok(val)
        }

        #[cfg(not(target_arch = "x86_64"))]
        Err(Error::IoPortNotAvailable)
    }

    /// Write a `_val` to the I/O port
    ///
    /// # Parameters
    ///
    /// * `_val` - The value to write to the I/O port
    ///
    /// # Returns
    ///
    /// `()`, on error [`Error`]
    ///
    unsafe fn write_u8(&self, _val: u8) -> Result<()> {
        #[cfg(target_arch = "x86_64")]
        {
            asm!("out dx, al", in("dx") self.0, in("al") _val,
                options(nomem, nostack, preserves_flags));
            Ok(())
        }

        #[cfg(not(target_arch = "x86_64"))]
        Err(Error::IoPortNotAvailable)
    }

    /// Write a `_val` to the I/O port
    ///
    /// # Parameters
    ///
    /// * `_val` - The value to write to the I/O port
    ///
    /// # Returns
    ///
    /// `()`, on error [`Error`]
    ///
    unsafe fn write_u16(&self, _val: u16) -> Result<()> {
        #[cfg(target_arch = "x86_64")]
        {
            asm!("out dx, ax", in("dx") self.0, in("ax") _val,
                options(nomem, nostack, preserves_flags));
            Ok(())
        }

        #[cfg(not(target_arch = "x86_64"))]
        Err(Error::IoPortNotAvailable)
    }

    /// Write a `_val` to the I/O port
    ///
    /// # Parameters
    ///
    /// * `_val` - The value to write to the I/O port
    ///
    /// # Returns
    ///
    /// `()`, on error [`Error`]
    ///
    unsafe fn write_u32(&self, _val: u32) -> Result<()> {
        #[cfg(target_arch = "x86_64")]
        {
            asm!("out dx, eax", in("dx") self.0, in("eax") _val,
                options(nomem, nostack, preserves_flags));
            Ok(())
        }

        #[cfg(not(target_arch = "x86_64"))]
        Err(Error::IoPortNotAvailable)
    }
}

/// An ACPI Generic Access Structure
#[derive(Debug)]
pub enum Gas {
    /// An address in system memory space
    Memory {
        /// Base address
        addr: *mut u8,

        /// Width of a register (in bits) (i.e. the stride to access indices)
        register_width: u8,

        /// Register offset (in bits)
        register_offset: u8,

        /// Access size
        access_size: AccessSize,
    },

    /// An address in system I/O space
    Io {
        /// Base I/O address
        addr: IoAddr,

        /// Width of a register (in bits) (i.e. the stride to access indices)
        register_width: u8,

        /// Register offset (in bits)
        register_offset: u8,

        /// Access size
        access_size: AccessSize,
    },

    /// An unimplemented `Gas` type
    Unimplemented,
}

/// An abbreviated format for a `Gas` which has been indexed and checked
pub enum GasType {
    /// An address in system memory space
    Memory {
        /// Address
        addr: *mut u8, 
        /// Access size
        access_size: AccessSize
    },

    /// An address in system I/O space
    Io {
        /// I/O address
        addr: IoAddr,

        /// Access size
        access_size: AccessSize
    },
}

impl GasType {
    /// Read a value from the location specified by `self`
    ///
    /// # Returns
    ///
    /// A zero-extended version of the read value. The original size is
    /// specified by the [`AccessSize`].
    ///
    unsafe fn read(&self) -> Result<u64> {
        Ok(match self {
            Self::Io { addr, access_size } => {
                // Perform the read
                match access_size {
                    AccessSize::Byte  => addr.read_u8()?  as u64,
                    AccessSize::Word  => addr.read_u16()? as u64,
                    AccessSize::Dword => addr.read_u32()? as u64,
                    _ => return Err(Error::InvalidAccessSize),
                }
            },
            Self::Memory { addr, access_size } => {
                // Perform the read
                match access_size {
                    AccessSize::Byte  => *(*addr as *const u8)  as u64,
                    AccessSize::Word  => *(*addr as *const u16) as u64,
                    AccessSize::Dword => *(*addr as *const u32) as u64,
                    AccessSize::Qword => *(*addr as *const u64) as u64,
                    _ => return Err(Error::InvalidAccessSize),
                }
            },
        })
    }

    /// Write a value to the location specified by `self`
    ///
    /// # Parameters
    ///
    /// * `val` - A value to be written to the location. This value is
    ///           truncated to the size specified by the [`AccessSize`] of the
    ///           [`GasType`] .
    ///
    /// # Returns
    ///
    /// `()`, on error [`Error`]
    ///
    unsafe fn write(&self, val: u64) -> Result<()> {
        match self {
            Self::Io { addr, access_size } => {
                // Perform the write
                match access_size {
                    AccessSize::Byte  => addr.write_u8( val as u8)?,
                    AccessSize::Word  => addr.write_u16(val as u16)?,
                    AccessSize::Dword => addr.write_u32(val as u32)?,
                    _ => return Err(Error::InvalidAccessSize),
                }
            },
            Self::Memory { addr, access_size } => {
                // Perform the write
                match access_size {
                    AccessSize::Byte  => *(*addr as *mut u8)  = val as u8,
                    AccessSize::Word  => *(*addr as *mut u16) = val as u16,
                    AccessSize::Dword => *(*addr as *mut u32) = val as u32,
                    AccessSize::Qword => *(*addr as *mut u64) = val as u64,
                    _ => return Err(Error::InvalidAccessSize),
                }
            },
        }

        Ok(())
    }
}

impl Gas {
    /// Read a value from the location specified by `self` offset by `idx`
    ///
    /// # Parameters
    ///
    /// * `idx` - The zero-indexed offset to be applied to the base of the
    ///           location, specified in number of elements. The width of an
    ///           element is determined by the `register_width`.
    ///
    /// # Returns
    ///
    /// A zero-extended version of the read value. The original size is
    /// specified by the [`AccessSize`].
    ///
    /// # Safety
    ///
    /// This function directly access the memory addressed by the `Gas`. This
    /// likely will result in MMIO accesses and thus needs to be handled with
    /// the correct memory mappings and device models for the target device.
    ///
    pub unsafe fn read(&self, idx: usize) -> Result<u64> {
        self.addr(idx)?.read()
    }

    /// Write a value to the location specified by `self`
    ///
    /// # Parameters
    ///
    /// * `idx` - The zero-indexed offset to be applied to the base of the
    ///           location, specified in number of elements. The width of an
    ///           element is determined by the `register_width`.
    /// * `val` - A value to be written to the location. This value is
    ///           truncated to the size specified by the [`AccessSize`] of the
    ///           [`GasType`] .
    ///
    /// # Returns
    ///
    /// `()`, on error [`Error`]
    ///
    /// # Safety
    ///
    /// This function directly access the memory addressed by the `Gas`. This
    /// likely will result in MMIO accesses and thus needs to be handled with
    /// the correct memory mappings and device models for the target device.
    ///
    pub unsafe fn write(&self, idx: usize, val: u64) -> Result<()> {
        self.addr(idx)?.write(val)
    }

    /// Compute the address to access the register associated with this [`Gas`] 
    /// based on the `idx`
    ///
    /// # Parameters
    ///
    /// * `idx` - The zero-indexed offset to be applied to the base of the
    ///           location, specified in number of elements. The width of an
    ///           element is determined by the `register_width`.
    /// 
    /// # Returns
    /// 
    /// The [`GasType`] which contains a simplified post-indexed representation
    /// of a [`Gas`], on error [`Error`]
    ///
    fn addr(&self, idx: usize) -> Result<GasType> {
        Ok(match self {
            Self::Io { addr, register_width,
                       register_offset, access_size} => {
                // Check the sanity of the register
                if *register_width == 0 {
                    return Err(Error::WidthZero);
                }
                if *register_width % 8 != 0 {
                    return Err(Error::WidthNotMod8);
                }
                if *register_offset != 0 {
                    return Err(Error::OffsetNonZero);
                }

                // Compute the address of the register to access
                let addr = IoAddr(
                    (idx as u64).checked_mul((register_width / 8) as u64)
                    .and_then(|x| x.checked_add(addr.0))
                    .ok_or(Error::AddressOverflow)?);

                GasType::Io { addr, access_size: *access_size }
            }
            Self::Memory { addr, register_width,
                           register_offset, access_size} => {
                // Check the sanity of the register
                if *register_width == 0 {
                    return Err(Error::WidthZero);
                }
                if *register_width % 8 != 0 {
                    return Err(Error::WidthNotMod8);
                }
                if *register_offset != 0 {
                    return Err(Error::OffsetNonZero);
                }

                // Compute the address of the register to access
                let addr = 
                    idx.checked_mul((register_width / 8) as usize)
                    .and_then(|x| x.checked_add(*addr as usize))
                    .ok_or(Error::AddressOverflow)? as *mut u8;

                GasType::Memory { addr, access_size: *access_size }
            }

            Self::Unimplemented => {
                return Err(Error::TypeUnimplemented);
            }
        })
    }
}

impl From<[u8; 12]> for Gas {
    fn from(val: [u8; 12]) -> Self {
        match val[0] {
            0 => Self::Memory {
                addr: u64::from_le_bytes(
                        val[4..12].try_into().unwrap()) as *mut u8,
                register_width:  val[1],
                register_offset: val[2],
                access_size:     val[3].into(),
            },
            1 => Self::Io {
                addr: IoAddr(u64::from_le_bytes(
                    val[4..12].try_into().unwrap())),
                register_width:  val[1],
                register_offset: val[2],
                access_size: val[3].into(),
            },
            _ => Self::Unimplemented,
        }
    }
}
