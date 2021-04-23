//! This file contains basic UEFI FFI structures. These are not complete
//! and are intended to only be filled in with the information that we use
//! in our kernel.
//!
//! If a structure variable name is prefixed with an underscore that means
//! the variable is filled in with an equivalent-size representation, but
//! not the actual type. We'll use this to lazily set a pointer to a
//! complex type as `usize`, if we don't actually have a use for this
//! structure.

use core::sync::atomic::{AtomicPtr, Ordering};
use crate::mm::rangeset::{self, Range, RangeSet};

/// A `Result` type which wraps an EFI error
type Result<T> = core::result::Result<T, Error>;

/// Errors from EFI calls
#[derive(Debug)]
pub enum Error {
    /// The EFI system table has not been registered
    NotRegistered,

    /// We failed to get the memory map from EFI
    MemoryMap(EfiStatus),

    /// We failed to exit EFI boot services
    ExitBootServices(EfiStatus),

    /// An integer overflow occurred when processing EFI memory map data
    MemoryMapIntegerOverflow,

    /// An error occured when trying to construct the memory map `RangeSet`
    MemoryRangeSet(rangeset::Error),
}

/// A strongly typed EFI system table pointer which will disallow the copying
/// of the raw pointer
#[repr(transparent)]
pub struct EfiSystemTablePtr(*mut EfiSystemTable);

impl EfiSystemTablePtr {
    /// Register this system table into a global so it can be used for prints
    /// which do not take a `self` or a pointer as an argument and thus this
    /// must be able to be found on a pointer
    pub unsafe fn register(self) {
        EFI_SYSTEM_TABLE.compare_and_swap(
            core::ptr::null_mut(),
            self.0,
            Ordering::SeqCst);
    }
}


/// A pointer to the EFI system table which is saved upon the entry of the
/// kernel.
///
/// We'll need access to this table to do input and output to the console
///
static EFI_SYSTEM_TABLE: AtomicPtr<EfiSystemTable> =
    AtomicPtr::new(core::ptr::null_mut());

/// Write a `string` to the UEFI console output
pub fn output_string(string: &str) {
    // Get the system_table
    let st = EFI_SYSTEM_TABLE.load(Ordering::SeqCst);

    // We can't do anything if it's null
    if st.is_null() { return; }

    // Get the console out pointer
    let out = unsafe { (*st).console_out };

    // Create a temporary buffer capable of holding 31 characters a a time
    // plus a null terminator
    //
    // We are using UCS-2 and not UTF-16, as that's what UEFI uses. Thus
    // we don't have to worry about 32-bit code points.
    let mut tmp = [0u16; 32];
    let mut in_use = 0;

    // Go through all the characters
    for chr in string.encode_utf16() {
        // Inject carriage return if needed. We always make sure there's
        // room for one based on the way we check the buffer length (-2
        // instead of -1)
        if chr == b'\n' as u16 {
            tmp[in_use] = b'\r' as u16;
            in_use += 1;
        }

        // Write a character into the buffer
        tmp[in_use] = chr;
        in_use += 1;

        // If the temporary buffer could potentially be full on the next
        // iteration, we flush it. We do -2 here because we need room for the
        // worst case which is a carriage return, newline, and null terminator
        // in the next iteration. We also need to do >= because we can
        // potentially skip from 29 in use to 31 in use if the 30th character
        // is a newline.
        if in_use >= (tmp.len() - 2) {
            // Null terminate the buffer
            tmp[in_use] = 0;

            // Write out the buffer
            unsafe { ((*out).output_string)(out, tmp.as_ptr()); }

            // Clear the buffer
            in_use = 0;
        }
    }

    // Write out any remaining characters
    if in_use > 0 {
        // Null terminate the buffer
        tmp[in_use] = 0;

        unsafe { ((*out).output_string)(out, tmp.as_ptr()); }
    }
}

/// Get the base of the ACPI table RSDP. If EFI did not report an ACPI table
/// then we return `None`.
pub fn get_acpi_table() -> Option<usize> {
    /// ACPI 2.0 or newer tables should use EFI_ACPI_TABLE_GUID
    const EFI_ACPI_TABLE_GUID: EfiGuid = EfiGuid(
        0x8868e871, 0xe4f1, 0x11d3,
        [0xbc, 0x22, 0x00, 0x80, 0xc7, 0x3c, 0x88, 0x81]);  

    /// ACPI 1.0 tables use this GUID
    const ACPI_TABLE_GUID: EfiGuid = EfiGuid(
        0xeb9d2d30, 0x2d88, 0x11d3,
        [0x9a, 0x16, 0x00, 0x90, 0x27, 0x3f, 0xc1, 0x4d]);

    // Get the system table
    let st = EFI_SYSTEM_TABLE.load(Ordering::SeqCst);

    // We can't do anything if it's null
    if st.is_null() { return None; }

    // Get a Rust slice to the tables
    let tables = unsafe {
        core::slice::from_raw_parts(
            (*st).tables,
            (*st).number_of_tables)
    };

    // First look for the ACPI 2.0 table, if we can't find it, then look
    // for the ACPI 1.0 table
    tables.iter().find_map(|EfiConfigurationTable { guid, table }| {
        (guid == &EFI_ACPI_TABLE_GUID).then_some(*table)
    }).or_else(|| {
        tables.iter().find_map(|EfiConfigurationTable { guid, table }| {
            (guid == &ACPI_TABLE_GUID).then_some(*table)
        })
    })
}

/// Get the memory map for the system from the UEFI
pub fn get_memory_map(image_handle: EfiHandle) -> Result<RangeSet> {
    // Get the system table
    let st = EFI_SYSTEM_TABLE.load(Ordering::SeqCst);

    // We can't do anything if it's null
    if st.is_null() { return Err(Error::NotRegistered); }

    // Create an empty memory map
    let mut memory_map = [0u8; 8 * 1024];

    // The Rust memory map
    let mut usable_memory = RangeSet::new();

    unsafe {
        // Set up the initial arguments to the `get_memory_map` EFI call
        let mut size = core::mem::size_of_val(&memory_map);
        let mut key = 0;
        let mut mdesc_size = 0;
        let mut mdesc_version = 0;

        // Get the memory map
        let ret: EfiStatus = ((*(*st).boot_services).get_memory_map)(
            &mut size,
            memory_map.as_mut_ptr(),
            &mut key,
            &mut mdesc_size,
            &mut mdesc_version).into();

        // Check that the memory map was obtained
        if ret != EfiStatus::Success {
            return Err(Error::MemoryMap(ret));
        }

        // Go through each memory map entry
        for off in (0..size).step_by(mdesc_size) {
            // Read the memory as a descriptor
            let entry = core::ptr::read_unaligned(
                memory_map[off..].as_ptr() as *const EfiMemoryDescriptor);

            // Convert the type into our Rust enum
            let typ: EfiMemoryType = entry.typ.into();

            // Check if this memory is usable after we exit boot services
            if typ.avail_post_exit_boot_services() {
                if entry.number_of_pages > 0 {
                    // Get the number of bytes for this memory region
                    let bytes = entry.number_of_pages.checked_mul(4096)
                        .ok_or(Error::MemoryMapIntegerOverflow)?;

                    // Compute the end physical address of this region
                    let end = entry.physical_start.checked_add(bytes - 1)
                        .ok_or(Error::MemoryMapIntegerOverflow)?;

                    // Set the usable memory information
                    usable_memory.insert(Range {
                        start: entry.physical_start,
                        end:   end
                    }).map_err(|e| Error::MemoryRangeSet(e))?;
                }
            }
        }
    
        /*
        // Exit boot services
        let ret: EfiStatus = ((*(*st).boot_services).exit_boot_services)(
            image_handle, key).into();
        if ret != EfiStatus::Success {
            return Err(Error::ExitBootServices(ret));
        }
        */
    }
    
    Ok(usable_memory)
}

/// A collection of related interfaces. Type `VOID *`.
#[derive(Debug)]
#[repr(transparent)]
pub struct EfiHandle(usize);

/// Raw EFI status code
#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct EfiStatusCode(usize);

/// EFI status codes
#[derive(Debug, PartialEq, Eq)]
pub enum EfiStatus {
    /// EFI success
    Success,

    /// An EFI warning (top bit clear)
    Warning(EfiWarning),

    /// An EFI error (top bit set)
    Error(EfiError),
}

impl From<EfiStatusCode> for EfiStatus {
    fn from(val: EfiStatusCode) -> Self {
        // Sign extend the error code to make this code bitness agnostic
        let val = val.0 as i32 as i64 as u64;
        match val {
            0 => {
                Self::Success
            }
            0x0000000000000001..=0x7fffffffffffffff => {
                EfiStatus::Warning(match val & !(1 << 63) {
                    1 => EfiWarning::UnknownGlyph,
                    2 => EfiWarning::DeleteFailure,
                    3 => EfiWarning::WriteFailure,
                    4 => EfiWarning::BufferTooSmall,
                    5 => EfiWarning::StaleData,
                    6 => EfiWarning::FileSystem,
                    7 => EfiWarning::ResetRequired,
                    _ => EfiWarning::Unknown(val),
                })
            }
            0x8000000000000000..=0xcfffffffffffffff => {
                EfiStatus::Error(match val & !(1 << 63) {
                     1 => EfiError::LoadError,
                     2 => EfiError::InvalidParameter,
                     3 => EfiError::Unsupported,
                     4 => EfiError::BadBufferSize,
                     5 => EfiError::BufferTooSmall,
                     6 => EfiError::NotReady,
                     7 => EfiError::DeviceError,
                     8 => EfiError::WriteProtected,
                     9 => EfiError::OutOfResources,
                    10 => EfiError::VolumeCorrupted,
                    11 => EfiError::VolumeFull,
                    12 => EfiError::NoMedia,
                    13 => EfiError::MediaChanged,
                    14 => EfiError::NotFound,
                    15 => EfiError::AccessDenied,
                    16 => EfiError::NoResponse,
                    17 => EfiError::NoMapping,
                    18 => EfiError::Timeout,
                    19 => EfiError::NotStarted,
                    20 => EfiError::AlreadyStarted,
                    21 => EfiError::Aborted,
                    22 => EfiError::IcmpError,
                    23 => EfiError::TftpError,
                    24 => EfiError::ProtocolError,
                    25 => EfiError::IncompatibleVersion,
                    26 => EfiError::SecurityViolation,
                    27 => EfiError::CrcError,
                    28 => EfiError::EndOfMedia,
                    31 => EfiError::EndOfFile,
                    32 => EfiError::InvalidLanguage,
                    33 => EfiError::CompromisedData,
                    34 => EfiError::IpAddressConflict,
                    35 => EfiError::HttpError,
                    _  => EfiError::Unknown(val),
                })
            }
            _ => unreachable!(),
        }
    }
}

/// EFI warning codes
#[derive(Debug, PartialEq, Eq)]
pub enum EfiWarning {
    /// The string contained one or more characters that the device could not
    /// render and were skipped
    UnknownGlyph,

    /// The handle was closed, but the file was not deleted
    DeleteFailure,

    /// The handle was closed, but the data to the file was not flushed
    /// properly
    WriteFailure,

    /// The resulting buffer was too small, and the data was truncated to the
    /// buffer size
    BufferTooSmall,

    /// The data has not been updated within the timeframe set by local policy
    /// for this type of data
    StaleData,

    /// The resulting buffer contains UEFI-compliant file system
    FileSystem,

    /// The operation will proceed across a system reset
    ResetRequired,

    /// An unknown warning
    Unknown(u64),
}

/// EFI error codes
#[derive(Debug, PartialEq, Eq)]
pub enum EfiError {
    /// The image faled to load
    LoadError,

    /// A parameter was incorrect
    InvalidParameter,

    /// The operation is not supported
    Unsupported,

    /// The buffer was not the proper size for the request
    BadBufferSize,

    /// The buffer is not large enough to hold the requested data. The
    /// required buffer size is returned in the appropriate parameter when
    /// this error occurs.
    BufferTooSmall,

    /// There is no data pending upon return
    NotReady,

    /// The physical device reported an error while attempting the operation
    DeviceError,

    /// The device cannot be written to
    WriteProtected,

    /// A resource has run out
    OutOfResources,

    /// An inconstancy was detected on the file system, causing the operation
    /// to fail
    VolumeCorrupted,

    /// There is no more space on the file system
    VolumeFull,

    /// The device does not contain any medium to perform the operation
    NoMedia,

    /// The medium in the device has changed since the last access
    MediaChanged,

    /// The item was not found
    NotFound,

    /// Access was denied
    AccessDenied,

    /// The server was not found or did not respond to the request
    NoResponse,

    /// A mapping to a device does not exist
    NoMapping,

    /// The timeout time expired
    Timeout,

    /// The protocol has not been started
    NotStarted,

    /// The protocol has already started
    AlreadyStarted,

    /// The operation was aborted
    Aborted,

    /// An ICMP error occurred during the network operation
    IcmpError,

    /// A TFTP error occurred during the network operation
    TftpError,

    /// A protocol error occurred during the network operation
    ProtocolError,

    /// The function encountered an internal version that was incompatible
    /// with a version requested by the caller
    IncompatibleVersion,

    /// The operation was not performed due to a security violation
    SecurityViolation,

    /// A CRC error occurred
    CrcError,

    /// Beginning or end of media was reached
    EndOfMedia,

    /// The end of the file was reached
    EndOfFile,

    /// The language specified was invalid
    InvalidLanguage,

    /// The security status of the data is unknown or compromised and the data
    /// must be updated or replaced to restore a valid security statut
    CompromisedData,

    /// There is an address conflict address allocation
    IpAddressConflict,

    /// An HTTP error occurred during the network operation
    HttpError,

    /// An unknown error
    Unknown(u64),
}

/// A scan code and unicode value for an input keypress
#[repr(C)]
struct EfiInputKey {
    /// The scan code for the key pres
    scan_code: u16,

    /// The unicode representation of that key
    unicode_char: u16,
}

/// EFI memory types
#[derive(Clone, Copy, Debug)]
#[repr(C)]
enum EfiMemoryType {
    ReservedMemoryType,
    LoaderCode,
    LoaderData,
    BootServicesCode,
    BootServicesData,
    RuntimeServicesCode,
    RuntimeServicesData,
    ConventionalMemory,
    UnusableMemory,
    ACPIReclaimMemory,
    ACPIMemoryNVS,
    MemoryMappedIO,
    MemoryMappedIOPortSpace,
    PalCode,
    PersistentMemory,
    Invalid,
}

impl EfiMemoryType {
    /// Returns whether or not this memory type is available for general
    /// purpose use after boot services have been exited
    fn avail_post_exit_boot_services(&self) -> bool {
        match self {
            EfiMemoryType::BootServicesCode    |
            EfiMemoryType::BootServicesData    |
            EfiMemoryType::ConventionalMemory  |
            EfiMemoryType::PersistentMemory    => true,
            _ => false
        }
    }
}

impl From<u32> for EfiMemoryType {
    fn from(val: u32) -> Self {
        match val {
             0 => EfiMemoryType::ReservedMemoryType,
             1 => EfiMemoryType::LoaderCode,
             2 => EfiMemoryType::LoaderData,
             3 => EfiMemoryType::BootServicesCode,
             4 => EfiMemoryType::BootServicesData,
             5 => EfiMemoryType::RuntimeServicesCode,
             6 => EfiMemoryType::RuntimeServicesData,
             7 => EfiMemoryType::ConventionalMemory,
             8 => EfiMemoryType::UnusableMemory,
             9 => EfiMemoryType::ACPIReclaimMemory,
            10 => EfiMemoryType::ACPIMemoryNVS,
            11 => EfiMemoryType::MemoryMappedIO,
            12 => EfiMemoryType::MemoryMappedIOPortSpace,
            13 => EfiMemoryType::PalCode,
            14 => EfiMemoryType::PersistentMemory,
             _ => EfiMemoryType::Invalid,
        }
    }
}

/// Data structure that precedes all of the standard EFI table types
#[repr(C)]
struct EfiTableHeader {
    /// A 64-bit signature that identifies the type of table that follows.
    /// Unique signatures have been generated for the EFI System Table,
    /// the EFI Boot Services Table, and the EFI Runtime Services Table.
    signature: u64,

    /// The revision of the EFI specification to which this table
    /// conforms. The upper 16 bits of this field contain the major
    /// revision value, and the lower 16 bits contain the minor revision
    /// value. The minor revision values are binary coded decimals and are
    /// limited to the range of `00..99`.
    ///
    /// When printed or displayed, UEFI spec revision is referred as
    /// `(Major revision).(Minor revision upper decimal).(Minor revision
    /// lower decimal)`, or in case Minor revision lower decimal is set to
    /// 0 as just `(Major revision).(Minor revision upper decimal)`. For
    /// example:
    /// 
    /// A specification with the revision value `((2<<16) | (30))` would be
    /// referred as 2.3;
    ///
    /// A specification with the revision value `((2<<16) | (31))` would be
    /// referred as 2.3.1
    revision: u32,

    /// The size, in bytes, of the entire table including the
    /// `EfiTableHeader`
    header_size: u32,

    /// The 32-bit CRC for the entire table. This value is computed by
    /// setting this field to 0, and computing the 32-bit CRC for
    /// `header_size` bytes.
    crc32: u32,

    /// Reserved field that must be set to 0.
    reserved: u32,
}

/// The memory descriptor for a record returned from `GetMemoryMap()`
#[derive(Clone, Copy, Default, Debug)]
#[repr(C)]
struct EfiMemoryDescriptor {
    /// Type of the memory region. Type `EFI_MEMORY_TYPE` is defined in the
    /// `AllocatePages()` function description.
    typ: u32,

    /// Physical address of the first byte in the memory region.
    /// `PhysicalStart` must be aligned on a 4KiB boundary, and must not be
    /// above `0xfffffffffffff000`. Type `EFI_PHYSICAL_ADDRESS` is defined in
    /// the `AllocatePages()` function description.
    physical_start: u64,

    /// Virtual address of the first bythe in the memory region.
    /// `VirtualStart` must be aligned on a 4KiB boundary, and must not be
    /// above `0xffffffffffff000`. Type `EFI_VIRTUAL_ADDRESS` is defined in
    /// "Related Definitions".
    virtual_start: u64,

    /// Number of 4KiB pages in the memory region. `NumberOfPages` must not
    /// be 0, and must not be any value that would represent a memory page
    /// with a start address, either physical or virtual, above
    /// `0xfffffffffffff000`.
    number_of_pages: u64,

    /// Attributes of the memory region that describe the bit mask of
    /// capabilities for that memory region, and not necessarily the
    /// current setting for that memory region. See the following "Memory
    /// Attribute Definitions".
    attribute: u64,
}

/// Contains table header and pointers to all of the boot services.
#[repr(C)]
struct EfiBootServices {
    /// The table header for the EFI Boot Services Table. This header
    /// contains the `EFI_BOOT_SERVICES_SIGNATURE` and
    /// `EFI_BOOT_SERVICES_REVISION` values along with the size of the
    /// `EFI_BOOT_SERVICES` structure and a 32-bit CRC to verify that the
    /// contents of the EFI Boot Services Table are valid.
    header: EfiTableHeader,

    /// Raises the task priority level
    _raise_tpl: usize,

    /// Restores/lowers the task priority level
    _restore_tpl: usize,

    /// Allocates pages of a particular type
    _allocate_pages: usize,

    /// Frees allocated pages
    _free_pages: usize,

    /// Returns the current boot service memory map and memory map key
    get_memory_map: unsafe fn(memory_map_size:    &mut usize,
                              memory_map:         *mut u8,
                              map_key:            &mut usize,
                              descriptor_size:    &mut usize,
                              descriptor_version: &mut u32) -> EfiStatusCode,

    /// Allocates a pool of a particular type
    _allocate_pool: usize,

    /// Frees allocated pool
    _free_pool: usize,

    /// Creates a general-purpose event structure
    _create_event: usize,

    /// Sets an event to be signaled at a particular time
    _set_timer: usize,

    /// Stops execution until an event is signaled
    _wait_for_event: usize,

    /// Signals an event
    _signal_event: usize,

    /// Closes and freezes an event structure
    _close_event: usize,

    /// Checks whether an event is in the signaled state
    _check_event: usize,

    /// Installs a protocol interface on a device handle
    _install_protocol_interface: usize,

    /// Reinstalls a protocol interface on a device handle
    _reinstall_protocol_interface: usize,

    /// Removes a protocol interface from a device handle
    _uninstall_protocol_interface: usize,

    /// Queries a handle to determine if it supports a specified protocol
    _handle_protocol: usize,

    /// Reserved
    _reserved: usize,

    /// Registers an event that is to be signaled whenever an interface is
    /// installed for a specific protocol
    _register_protocol_notify: usize,

    /// Returns an array of handles that support a specified protocol
    _locate_handle: usize,

    /// Locates all devices on a device path that support a specified
    /// protocol and returns the handle to the device that is closest to
    /// the path.
    _locate_device_path: usize,

    /// Adds, updates or removes a configuration table from the EFI System
    /// Table
    _install_configuration_table: usize,

    /// Loads an EFI image into memory
    _load_image: usize,

    /// Transfers control to loaded image's entry point
    _start_image: usize,

    /// Exits the image's entry point
    _exit: usize,

    /// Unloads an image
    _unload_image: usize,

    /// Terminates boot services
    exit_boot_services: unsafe fn(image_handle: EfiHandle,
                                  map_key:      usize) -> EfiStatusCode,
}

/// This protocol is used to obtain input from the ConsoleIn device. The
/// EFI specification requires that the `EFI_SIMPLE_TEXT_INPUT_PROTOCOL`
/// supports the same languages as the corresponding
/// `EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL`.
#[repr(C)]
struct EfiSimpleTextInputProtocol {
    /// Resets the input device hardware
    reset: unsafe fn(this: *const EfiSimpleTextInputProtocol,
                     extended_verification: bool) -> EfiStatusCode,
    
    /// Reads the next keystroke from the input device
    read_keystroke: unsafe fn(this: *const EfiSimpleTextInputProtocol,
                              key:  *mut   EfiInputKey) -> EfiStatusCode,

    /// Event to use with `EFI_BOOT_SERVICES.WaitForEvent()` to wait for a
    /// key to be available.
    /// We don't use the event API, don't expose this function pointer
    _wait_for_key: usize,
}

/// This protocol is used to control text-based output devices.
#[repr(C)]
struct EfiSimpleTextOutputProtocol {
    /// Resets the text output device hardware
    reset: unsafe fn(this: *const EfiSimpleTextOutputProtocol,
                     extended_verification: bool) -> EfiStatusCode,

    /// Writes a string to the output device.
    output_string: unsafe fn(this:   *const EfiSimpleTextOutputProtocol,
                             string: *const u16) -> EfiStatusCode,

    /// Verifies that all characters in a string can be output to the
    /// target device
    test_string: unsafe fn(this:   *const EfiSimpleTextOutputProtocol,
                           string: *const u16) -> EfiStatusCode,

    /// Returns information for an available text mode that the output
    /// device(s) support
    _query_mode: usize,

    /// Sets the output device(s) to a specified mode
    _set_modfe: usize,

    /// Sets the background and foreground colors for the `OutputString()`
    /// and `ClearScreen()` functions
    _set_attribute: usize,

    /// Clears the output device(s) display to the currently selected 
    /// background color
    _clear_screen: usize,

    /// Sets the current coordinates of the cursor position
    _set_cursor_position: usize,

    /// Show or hide the cursor
    _enable_cursor: usize,

    /// Pointer to `SIMPLE_TEXT_OUTPUT_MODE` data
    _mode: usize,
}

/// Contains pointers to the runtime and boot services tables
#[repr(C)]
struct EfiSystemTable {
    /// The table header for an EFI System Table. This header contains the
    /// `EFI_SYSTEM_TABLE_SIGNATURE` and `EFI_SYSTEM_TABLE_REVISION` values
    /// along with the size of the `EFI_SYSTEM_TABLE structure` and a 32-bit
    /// CRC to verify that the contents of the EFI System Table are valid
    header: EfiTableHeader,

    /// A pointer to a null terminated string that identifies the vendor
    /// that produces the system firmware for the platform
    firmware_vendor: *const u16,

    /// A firmware vendor specific value that identifies the revision of
    /// the system firmware for the platform
    firmware_revision: u32,

    /// The handle for the active console input device. This handle must
    /// support `EFI_SIMPLE_TEXT_INPUT_PROTOCOL` and
    /// `EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL`.
    console_in_handle: EfiHandle,

    /// A pointer to the `EFI_SIMPLE_TEXT_INPUT_PROTOCOL` interface that is
    /// associated with `ConsoleInHandle`
    console_in: *const EfiSimpleTextInputProtocol,

    /// The handle for the active console output device. This handle must
    /// support the `EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL`.
    console_out_handle: EfiHandle,

    /// A pointer to the `EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL` interface that is
    /// associated with `ConsoleOutHandle`
    console_out: *const EfiSimpleTextOutputProtocol,

    /// The handle for the active standard error console device. This
    /// handle must support the `EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL`
    console_err_handle: EfiHandle,

    /// A pointer to the `EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL` interface that is
    /// associated with `StandardErrorHandle`
    console_err: *const EfiSimpleTextOutputProtocol,

    /// A pointer to the EFI Runtime Services Table
    _runtime_services: usize,

    /// A pointer to the EFI Boot Services Table
    boot_services: *const EfiBootServices,

    /// Number of EFI tables
    number_of_tables: usize,

    /// Pointer to EFI table array
    tables: *const EfiConfigurationTable,
}

/// The entry for an EFI configuration table
#[derive(Debug)]
#[repr(C)]
struct EfiConfigurationTable {
    /// The 128-bit GUID value that uniqely identifies the system
    /// configuration table
    guid: EfiGuid,

    /// A ppointer to the table associated with `guid` 
    table: usize,
}

/// An EFI `guid` representation
#[derive(Debug, PartialEq, Eq)]
#[repr(C)]
struct EfiGuid(u32, u16, u16, [u8; 8]);
