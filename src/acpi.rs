//! A very lightweight ACPI implementation for extracting basic information
//! about CPU topography and NUMA memory regions

use core::mem::size_of;

use crate::mm::{self, PhysAddr};
use crate::efi;

/// A `result` type which wraps an ACPI error
type Result<T> = core::result::Result<T, Error>;

/// Different types of ACPI tables, used mainly for error information
#[derive(Debug)]
enum Table {
    /// The root system description pointer
    Rsdp,

    /// The extended ACPI 2.0+ root system description pointer
    RsdpExtended,
}

/// Errors from ACPI table parsing
#[derive(Debug)]
pub enum Error {
    /// The ACPI table address was not reported by UEFI and thus we could
    /// not find the RSDP
    RsdpNotFound,

    /// An RSDP table was processed, which had an invalid checksum
    ChecksumMismatch(Table),

    /// An ACPI table did not match the correct signature
    SignatureMismatch(Table),

    /// An ACPI table did not match the expected length
    LengthMismatch(Table),

    /// An attempt was made to access the extended RSDP but the ACPI
    /// revision of this system is too old and does not support it. ACPI
    /// revision 2.0 is required for extended RSDP.
    RevisionTooOld,
}

/// Root System Descriptor Pointer (RSDP) structure for ACPI 1.0
#[repr(C, packed)]
struct Rsdp {
    /// "RSD PTR "
    signature: [u8; 8],

    /// This is the checksum of the fields defined in the ACPI 1.0
    /// specification. This includes only the first 20 bytes of this
    /// table, bytes 0 to 19, including the checksum field. These bytes
    /// must sum to zero.
    checksum: u8,

    /// OEM-supplied string that identifies the OEM.
    oem_id: [u8; 6],

    /// The revision of this structure. Larger revision numbers are
    /// backward compatible to lower revision numbers. The ACPI version 1.0
    /// revision number of this table is zero. The ACPI version 1.0 RDSP
    /// structure only includes the first 20 bytes of this table. It does
    /// not include the Length field and beyond. The current value for this
    /// field is 2.
    revision: u8,

    /// 32-bit physical address of the RSDT
    rsdt_addr: u32,
}

impl Rsdp {
    /// Load an RSDP structure from `addr`
    unsafe fn from_addr(addr: PhysAddr) -> Result<Self> {
        // Read the base RSDP structure from physical memory
        let rsdp = mm::read_phys::<Rsdp>(addr);
        let bytes = core::slice::from_raw_parts(
            &rsdp as *const Rsdp as *const u8, size_of::<Rsdp>());

        // Compute and check the checksum
        let chk = bytes.iter().fold(0u8, |acc, &x| acc.wrapping_add(x));
        if chk != 0 {
            return Err(Error::ChecksumMismatch(Table::Rsdp));
        }

        // Check the signature
        if &rsdp.signature != b"RSD PTR " {
            return Err(Error::SignatureMismatch(Table::Rsdp));
        }
        
        // Everything looks good, return the RSDP
        Ok(rsdp)
    }
}

/// In-memory representation of an Extended RSDP ACPI structure
#[repr(C, packed)]
struct RsdpExtended {
    /// Base level RSDP table for ACPI 1.0
    base: Rsdp,

    /// The length of the table, in bytes, including the header, starting
    /// from offset 0. This field is used to record the size of the entire
    /// table. This field is not available in the ACIP version 1.0 RSDP
    /// Structure
    length: u32,

    /// 64-bit physical address of the XSDT
    xsdt_addr:         u64,

    /// This is a checksum of the entire table, including both checksums
    extended_checksum: u8,

    /// Reserved field
    reserved:          [u8; 3],
}

impl RsdpExtended {
    /// Load an extended RSDP structure from `addr`
    unsafe fn from_addr(addr: PhysAddr) -> Result<Self> {
        // First read the RSDP. This is the ACPI 1.0 structure and thus is
        // a subset and backwards compatible with all future revisions.
        let rsdp = Rsdp::from_addr(addr)?;

        // The extended RSDP requires ACPI 2.0
        if rsdp.revision < 2 {
            return Err(Error::RevisionTooOld);
        }

        // Now read the extended RSDP
        let rsdp = mm::read_phys::<RsdpExtended>(addr);
        let bytes = core::slice::from_raw_parts(
            &rsdp as *const _ as *const u8, size_of::<RsdpExtended>());
        
        // Check the size
        if rsdp.length as usize != size_of::<RsdpExtended>() {
            return Err(Error::LengthMismatch(Table::RsdpExtended));
        }

        // Compute and check the checksum
        let chk = bytes.iter().fold(0u8, |acc, &x| acc.wrapping_add(x));
        if chk != 0 {
            return Err(Error::ChecksumMismatch(Table::RsdpExtended));
        }

        // Rsdp seem all good
        Ok(rsdp)

    }
}

/*
/// In-memory representation of an ACPI table header
#[derive(Clone, Copy)]
#[repr(C, packed)]
struct Header {
    signature:        [u8; 4],
    length:           u32,
    revision:         u8,
    checksum:         u8,
    oemid:            [u8; 6],
    oem_table_id:     u64,
    oem_revision:     u32,
    creator_id:       u32,
    creator_revision: u32,
}
*/

/// Initialize the ACPI subsystem. Mainly looking for APICs and memory maps.
/// Brings up all cores on the system
pub unsafe fn init() -> Result<()> {
    // Get the ACPI table base from the EFI
    let rsdp_addr = efi::get_acpi_table().ok_or(Error::RsdpNotFound)?;
    
    // Validate and get the RSDP
    let rsdp = RsdpExtended::from_addr(PhysAddr(rsdp_addr as u64))?;
    
    Ok(())
}
