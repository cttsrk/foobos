//! A very lightweight ACPI implementation for extracting basic information
//! about CPU topography and NUMA memory regions

use core::mem::size_of;

use crate::mm::physmem::{PhysAddr, PhysSlice};
use crate::efi;

use serial::{BaudRate, Interface};
use generic_access_structure::Gas;

/// Maximum number of cores on the system
const MAX_CORES: usize = 2;

/// A `Result` type which wraps an ACPI error
pub type Result<T> = core::result::Result<T, Error>;

/// Different types of ACPI tables, used mainly for error information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableType {
    /// The root system description pointer
    Rsdp,

    /// The extended ACPI 2.0+ root system description pointer
    RsdpExtended,

    /// Extended System Description Table
    Xsdt,

    /// Multiple APIC Description Table
    Madt,

    /// System Resource Affinity Table
    Srat,

    /// Serial Port Console Redirection Table
    Spcr,

    /// An unknown table type
    Unknown([u8; 4]),
}

impl From<[u8; 4]> for TableType {
    fn from(val: [u8; 4]) -> Self {
        match &val {
            b"XSDT" => Self::Xsdt,
            b"APIC" => Self::Madt,
            b"SRAT" => Self::Srat,
            b"SPCR" => Self::Spcr,
                  _ => Self::Unknown(val),
        }
    }
}

/// Errors from ACPI table parsing
#[derive(Debug)]
pub enum Error {
    /// An EFI API returned an error
    EfiError(efi::Error),

    /// An ACPI table had an invalid checksum
    ChecksumMismatch(TableType),

    /// An ACPI table did not match the correct signature
    SignatureMismatch(TableType),

    /// An ACPI table did not match the expected length
    LengthMismatch(TableType),

    /// An attempt was made to access the extended RSDP but the ACPI
    /// revision of this system is too old and does not support it. ACPI
    /// revision 2.0 is required for extended RSDP.
    RevisionTooOld,

    /// The XSDT table size was not evenly divisible by the array element size
    XsdtBadEntries,
    
    /// An integer overflow occured
    IntegerOverflow,

    /// More APICs have been detected than we statically allocate room for
    TooManyApics,

    /// More x2APICs have been detected than we statically allocate room for
    TooManyX2Apics,
    
    /// The SPCR did not specify zero parity bits (all other values are reserved)
    InvalidParityBits,

    /// The SPCR did not specify one stop bit (all other values are reserved)
    InvalidStopBits,

    /// The SPCR specified a reserved baud rate
    InvalidBaudRate,
}

/// Compute an ACPI checksum on physical memory
/// 
/// # Parameters
///
/// * `addr` - The physical address to performa a checksum on
/// * `size` - The length (in bytes) of the memory to checksum
/// * `typ`  - The type of the table which is being checksummed. This is simply
///            used to affect the error value that is returned if the checksum
///            is invalid.
///
/// # Returns
///
/// `() if the checksum is valid, [`Errror`] on errors
///
unsafe fn checksum(addr: PhysAddr, size: usize, typ: TableType) -> Result<()> {

    // Compute checksum
    let chk = (0..size as u64).try_fold(0u8, |acc, offset| {
        Ok(acc.wrapping_add(
            PhysAddr(addr.0.checked_add(offset)
                .ok_or(Error::IntegerOverflow)?).read_unaligned::<u8>()))
    })?;

    // Validate checksum
    if chk == 0 {
        Ok(())
    } else {
        Err(Error::ChecksumMismatch(typ))
    }
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
    /// Load an RSDP structure
    ///
    /// # Parameters
    ///
    /// * `addr` - The physical address of the memory to be interpreted as an
    ///            RSDP table
    ///
    /// # Returns
    ///
    /// A well formed [`Rsdp`] if `addr` references a valid RSDP table.
    /// [`Error`] on errors.
    ///
    unsafe fn from_addr(addr: PhysAddr) -> Result<Self> {
        // Validate the checksum
        checksum(addr, size_of::<Self>(), TableType::Rsdp)?;

        // Get the RSDP table
        let rsdp = addr.read_unaligned::<Self>();

        // Check the signature
        if &rsdp.signature != b"RSD PTR " {
            return Err(Error::SignatureMismatch(TableType::Rsdp));
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
    /// Load an extended RSDP structure
    ///
    /// # Parameters
    ///
    /// * `addr` - The physical address of the memory to be interpreted as an
    ///            extended RSDP table
    ///
    /// # Returns
    ///
    /// A well formed [`RsdpExtended`] if `addr` references a valid extended 
    /// RSDP table. [`Error`] on errors.
    ///
    unsafe fn from_addr(addr: PhysAddr) -> Result<Self> {
        // First read the RSDP. This is the ACPI 1.0 structure and thus is
        // a subset and backwards compatible with all future revisions.
        let rsdp = Rsdp::from_addr(addr)?;

        // The extended RSDP requires ACPI 2.0
        if rsdp.revision < 2 {
            return Err(Error::RevisionTooOld);
        }

        // Validate the checksum
        checksum(addr, size_of::<Self>(), TableType::Rsdp)?;

        // Get the extended RSDP table
        let rsdp = addr.read_unaligned::<Self>();

        // Check the size
        if rsdp.length as usize != size_of::<Self>() {
            return Err(Error::LengthMismatch(TableType::RsdpExtended));
        }

        // Rsdp seems all good!
        Ok(rsdp)
    }
}

/// In-memory representation of an ACPI table header
#[repr(C, packed)]
struct Table {
    /// The ASCII string representation of the table identifier
    signature: [u8; 4],

    /// The length of the table, in bytes, including the header, starting
    /// from offset 0. This field is used to record the size of the entire
    /// table.
    length: u32,

    /// The revision of the structure corresponding to the signature field
    /// for this table. Larger revision numbers are backward compatible to
    /// lower revision numbers with the same signature.
    revision: u8,

    /// The entire table, including the checksum field, must add to zero to
    /// be considered valid
    checksum: u8,

    /// An OEM-supplied string that identifies the OEM
    oemid: [u8; 6],

    /// An OEM-supplied string that the OEM uses to identify the particular
    /// data table. This field is particularly useful when defining a
    /// definition block to distinguish definition block functions. The OEM
    /// assigns each dissimiar table a new OEM Table ID.
    oem_table_id: u64,

    /// An OEM-supplied revision number. Larger numbers are assumed to be
    /// newer revisions.
    oem_revision: u32,

    /// Vendor ID of utility that created the table. For tables containing
    /// Definition blocks, this is the ID of the ASL compiler.
    creator_id: u32,

    /// Revision utility that created the table. For tables containing
    /// Definition blocks, this is the revision of the ASL compiler.
    creator_revision: u32,
}

impl Table {
    /// Load a generic ACPI table with the standard ACPI table header
    ///
    /// # Parameters
    ///
    /// * `addr` - The physical address of the memory to be interpreted as an
    ///            ACPI table
    ///
    /// # Returns
    ///
    /// A tuple containing the following:
    ///
    /// 0. A [`Table`] containing the parsed table header
    /// 1. A [`TableType`] containing the type of ACPI table which was
    ///    identified
    /// 2. The physical address of the opaque payload of the table
    /// 3. The size (in bytes) of the payload
    ///
    /// On error, an [`Error`]
    ///
    unsafe fn from_addr(addr: PhysAddr)
            -> Result<(Self, TableType, PhysAddr, usize)> {
        // Read the table
        let table = addr.read_unaligned::<Self>();

        // Get the type of this table
        let typ = TableType::from(table.signature);

        // Validate the checksum
        checksum(addr, table.length as usize, typ)?;

        // Computer the address of the table's payload and its size in bytes
        let header_size  = size_of::<Self>();
        let payload_size = (table.length as usize).checked_sub(header_size)
            .ok_or(Error::LengthMismatch(typ))?;
        let payload_addr = PhysAddr(addr.0.checked_add(header_size as u64)
            .ok_or(Error::IntegerOverflow)?);

        // Return the parsed information 
        Ok((table, typ, payload_addr, payload_size))
    }
}

/// The Multiple APIC Description Table
#[derive(Debug)]
pub struct Madt {
    /// Local APICs detected from ACPI
    apics: [LocalApic; MAX_CORES],

    /// Number of APCIs which have been initialized in `apics`
    num_apics: usize,

    /// Number of x2APICs detected from ACPI
    x2apics: [LocalX2Apic; MAX_CORES],

    /// Number of X2APCIs which have been initialized in `x2apics`
    num_x2apics: usize,
}

/// Processor Local APIC structure
#[derive(Default, Debug, Clone, Copy)]
#[repr(C, packed)]
struct LocalApic {
    /// The OS associates this local APIC structure with a processor object in
    /// the namespace when the _UID child object of the processor's device
    /// object (or the ProcessorId listed in the Processor declaration
    /// operator) evaluates to a numeric value that matches the numeric value
    /// in this field.
    acpi_processor_uid: u8,

    /// The processor's Local APIC ID
    apic_id: u8,

    /// Local APIC flags
    ///
    /// Bit 0: Enabled (set if ready for use)
    /// Bit 1: Online Capable (RAZ is enabled, indicates
    /// if the APIC can be enabled at runtime)
    flags: u32,
}

/// Processor Local x2APIC Structure
#[derive(Default, Debug, Clone, Copy)]
#[repr(C, packed)]
struct LocalX2Apic {
    /// Reserved - must be zero
    reserved: u16,

    /// The processor's local X2APIC ID
    x2apic_id: u32,

    /// Same as Local APIC flags
    flags: u32,

    /// OSPM associates the X2APIC Structure with a processor object declared
    /// in the namespace using the Device statement, when the _UID child object
    /// of the processor device evaluates to a numeric value, by matching the
    /// numeric value with this field
    acpi_processor_uid: u32,
}

impl Madt {
    /// Parse the payload of an ACPI MADT table
    ///
    /// # Parameters
    ///
    /// * `addr` - The physical address of the start of an MADT payload
    /// * `size` - The size (in bytes) of the MADT payload
    /// 
    /// # Returns
    ///
    /// A parsed representation of the [`Madt`], on error [`Error`]
    /// 
    unsafe fn from_addr(addr: PhysAddr, size: usize) -> Result<Self> {
        /// The error type to throw when the MADT is truncated
        const E: Error = Error::LengthMismatch(TableType::Madt);

        // Create a slice to the physical memory
        let mut slice = PhysSlice::new(addr, size);

        // Read the local APIC physical address
        let _local_apic_addr = slice.consume::<u32>().map_err(|_| E)?;

        // Get the APIC flags
        let _flags = slice.consume::<u32>().map_err(|_| E)?;

        // Create an empty `Madt`
        let mut ret = Self {
            apics:   [Default::default(); MAX_CORES],
            num_apics:   0,
            x2apics: [Default::default(); MAX_CORES],
            num_x2apics: 0,
        };

        // Handle Interrupt Controller Structures
        while slice.len() > 0 {
            // Read the interrupt controller structure headeo:
            let typ = slice.consume::<u8>().map_err(|_| E)?;
            let len = slice.consume::<u8>().map_err(|_| E)?
                .checked_sub(2).ok_or(E)?;
            
            match typ {
                0 => {
                    // Ensure the data is the correct size
                    if len as usize != size_of::<LocalApic>() {
                        return Err(E);
                    }
                    
                    // Get the `LocalApic` information
                    let apic = slice.consume::<LocalApic>().map_err(|_| E)?;

                    // Update APIC information
                    *ret.apics.get_mut(ret.num_apics)
                        .ok_or(Error::TooManyApics)? = apic;
                    ret.num_apics += 1;
                }
                9 => {
                    // Ensure the data is the correct size
                    if len as usize != size_of::<LocalX2Apic>() {
                        return Err(E);
                    }
                    
                    // Get the `LocalX2Apic` information
                    let x2apic =
                        slice.consume::<LocalX2Apic>().map_err(|_| E)?;
                    
                    // Update x2APIC information
                    *ret.x2apics.get_mut(ret.num_x2apics)
                        .ok_or(Error::TooManyX2Apics)? = x2apic;
                    ret.num_x2apics += 1;
                }
                _ => {
                    // Unknown type, just discard the data
                    slice.discard(len as usize).map_err(|_| E)?;
                }
            }
        }
        
        Ok(ret)
    }
}

/// The Serial Port Console Redirection table
#[derive(Debug)]
pub struct Spcr {
    /// Type of the serial port register interface
    pub interface_type: Interface,

    /// Address to access the serial port
    pub address: Gas,

    /// Baud rate to use for the serial port
    pub baud_rate: BaudRate,
}

impl Spcr {
    /// Parse the payload of an ACPI SPCR table
    ///
    /// # Parameters
    ///
    /// * `addr` - The physical address of the start of an SPCR payload
    /// * `size` - The size (in bytes) of the SPCR payload
    /// 
    /// # Returns
    ///
    /// A parsed representation of the [`Spcr`], on error [`Error`]
    /// 
    unsafe fn from_addr(addr: PhysAddr, size: usize) -> Result<Self> {
        /// The error type to throw when the SPCR is truncated
        const E: Error = Error::LengthMismatch(TableType::Spcr);

        // Create a slice to the physical memory
        let mut slice = PhysSlice::new(addr, size);

        // Get the serial interface type
        let typ: Interface =
            slice.consume::<u8>().map_err(|_| E)?.into();

        // Reserved (3 bytes)
        slice.discard(3).map_err(|_| E)?;

        // The generic address structure
        let info: Gas = slice.consume::<[u8; 12]>().map_err(|_| E)?.into();

        // Interrupt types, do not care
        slice.discard(1).map_err(|_| E)?;

        // IRQ, do not care
        slice.discard(1).map_err(|_| E)?;

        // Global system interrupt vector, do not care
        slice.discard(4).map_err(|_| E)?;

        // Get the baud rate
        let baud_rate = match slice.consume::<u8>().map_err(|_| E)? {
            0 => BaudRate::AsIs,
            3 => BaudRate::Baud9600,
            4 => BaudRate::Baud19200,
            6 => BaudRate::Baud57600,
            7 => BaudRate::Baud115200,
            _ => return Err(Error::InvalidBaudRate),
        };

        // Get parity and stop bit information
        let parity_bits = slice.consume::<u8>().map_err(|_| E)?;
        let stop_bits   = slice.consume::<u8>().map_err(|_| E)?;

        // Currently SPCR spec only allows for no parity and one stop bit
        if parity_bits != 0 { return Err(Error::InvalidParityBits) };
        if stop_bits   != 1 { return Err(Error::InvalidStopBits)   };

        // Return out the serial port info
        Ok(Self {
            interface_type: typ,
            address:        info,
            baud_rate:      baud_rate,
        })
    }
}

/// Information parsed out of ACPI
#[derive(Debug)]
pub struct Acpi {
    /// Contains information about the APICs from the MADT
    pub madt: Option<Madt>,

    /// Contains information from ACPI data structures about the serial device
    pub spcr: Option<Spcr>,
}

/// Initialize the ACPI subsystem
///
/// # Returns
///
/// Parsed [`Acpi`] information on success, on error [`Error`]
///
pub unsafe fn init() -> Result<Acpi> {
    // Get the ACPI table base from the EFI
    let rsdp_addr = efi::get_acpi_table().map_err(Error::EfiError)?;
    
    // Validate and get the RSDP
    let rsdp = RsdpExtended::from_addr(PhysAddr(rsdp_addr as u64))?;
    
    // Get the XSDT
    let (_, typ, xsdt, len) = Table::from_addr(PhysAddr(rsdp.xsdt_addr))?;
    if typ != TableType::Xsdt {
        return Err(Error::SignatureMismatch(TableType::Xsdt));
    }

    // Make sure the XSDT size is module a 64-bit address size
    if len % size_of::<u64>() != 0 {
        return Err(Error::XsdtBadEntries);
    }

    // Get the number of entries in the XSDT
    let entries = len / size_of::<u64>();

    // Parsed ACPI information
    let mut ret = Acpi {
        madt: None,
        spcr: None,
    };

    // Go through each table in the XSDT
    for idx in 0..entries {
        // Get the physical address of the XSDT entry
        let entry_addr = idx.checked_mul(size_of::<u64>()).and_then(|x| {
            x.checked_add(xsdt.0 as usize)
        }).ok_or(Error::IntegerOverflow)?;

        // Get the table address by reading the XSDT entry. It has been
        // observed in some versions of OVMF that these addresses can
        // sometimes be unaligned.
        let table_addr = PhysAddr(entry_addr as u64).read_unaligned::<u64>();

        // Parse and validate the table header
        let (_, typ, data, len) = Table::from_addr(PhysAddr(table_addr))?;

        match typ {
            TableType::Madt => {
                ret.madt = Some(Madt::from_addr(data, len)?);
            }

            TableType::Spcr => {
                ret.spcr = Some(Spcr::from_addr(data, len)?);
            }

            // Unknown 
            _ => {}
        }
    }
    
    Ok(ret)
}
