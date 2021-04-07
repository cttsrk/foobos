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

/// A pointer to the EFI system table which is saved upon the entry of the
/// kernel.
///
/// We'll need access to this table to do input and output to the console
///
static EFI_SYSTEM_TABLE: AtomicPtr<EfiSystemTable> =
    AtomicPtr::new(core::ptr::null_mut());

/// Register a system table pointer. This is obviously unsafe as it
/// requires the caller to provide a valid EFI system table pointer.
///
/// Only the first non-null system table will be stored into the
/// `EFI_SYSTEM_TABLE` global
///
pub unsafe fn register_system_table(system_table: *mut EfiSystemTable) {
    EFI_SYSTEM_TABLE.compare_and_swap(core::ptr::null_mut(), system_table,
        Ordering::SeqCst);
}

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

        if in_use == (tmp.len() - 2) {
            // Null terminate the buffer
            tmp[in_use] = 0;

            // Write out the buffer
            unsafe {
                ((*out).output_string)(out, tmp.as_ptr());
            }

            // Clear the buffer
            in_use = 0;
        }
    }

    // Write out any remaining characters
    if in_use > 0 {
        // Null terminate the buffer
        tmp[in_use] = 0;

        unsafe {
            ((*out).output_string)(out, tmp.as_ptr());
        }
    }
}

/// Get the memory map for the system from the UEFI
pub fn get_memory_map(image_handle: EfiHandle) {
    // Get the system table
    let st = EFI_SYSTEM_TABLE.load(Ordering::SeqCst);

    // We can't do anything if it's null
    if st.is_null() { return; }

    // Create an empty memory map
    let mut memory_map = [0u8; 8 * 1024];

    let mut free_memory = 0u64;
    unsafe {
        let mut size = core::mem::size_of_val(&memory_map);
        let mut key = 0;
        let mut mdesc_size = 0;
        let mut mdesc_version = 0;

        let ret = ((*(*st).boot_services).get_memory_map)(
            &mut size,
            memory_map.as_mut_ptr(),
            &mut key,
            &mut mdesc_size,
            &mut mdesc_version);

        assert!(ret.0 == 0, "Get memory map failed: {:x?}", ret);


        for off in (0..size).step_by(mdesc_size) {
            let entry = core::ptr::read_unaligned(
                memory_map[off..].as_ptr() as *const EfiMemoryDescriptor);
            let typ: EfiMemoryType = entry.typ.into();

            if typ.avail_post_exit_boot_services() {
                free_memory += entry.number_of_pages * 4096;
            }

            /*
            print!("{:016x} {:016x} {:?}\n",
                entry.physical_start,
                entry.number_of_pages * 4096,
                typ);
            */
        }
    
        // Exit boot services
        let ret = ((*(*st).boot_services).exit_boot_services)(
            image_handle, key + 1);
        assert!(ret.0 == 0, "Failed to exit boot services: {:x?}", ret);

        // Now that we're done with boot services, kill the EFI system
        // table
        EFI_SYSTEM_TABLE.store(core::ptr::null_mut(), Ordering::SeqCst);
    }
}

/// A collection of related interfaces. Type VOID *.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct EfiHandle(usize);

/// Status code
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct EfiStatus(pub usize);

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
    /// limited to the range of 00..99.
    ///
    /// When printed or displayed, UEFI spec revision is referred as
    /// (Major revision).(Minor revision upper decimal).(Minor revision
    /// lower decimal), or in case Minor revision lower decimal is set to
    /// 0 as just (Major revision).(Minor revision upper decimal). For
    /// example:
    /// 
    /// A specification with the revision value ((2<<16) | (30)) would be
    /// referred as 2.3;
    ///
    /// A specification with the revision value ((2<<16) | (31)) would be
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
    /// Type of the memory region. Type EFI_MEMORY_TYPE is defined in the
    /// AllocatePages() function description.
    typ: u32,

    /// Physical address of the first byte in the memory region.
    /// PhysicalStart must be aligned on a 4KiB boundary, and must not be
    /// above 0xfffffffffffff000. Type EFI_PHYSICAL_ADDRESS is defined in
    /// the AllocatePages() function description.
    physical_start: u64,

    /// Virtual address of the first bythe in the memory region.
    /// VirtualStart must be aligned on a 4KiB boundary, and must not be
    /// above 0xffffffffffff000. Type EFI_VIRTUAL_ADDRESS is defined in
    /// "Related Definitions".
    virtual_start: u64,

    /// Number of 4KiB pages in the memory region. NumberOfPages must not
    /// be 0, and must not be any value that would represent a memory page
    /// with a start address, either physical or virtual, above
    /// 0xfffffffffffff000.
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
    /// contains the EFI_BOOT_SERVICES_SIGNATURE and
    /// EFI_BOOT_SERVICES_REVISION values along with the size of the
    /// EFI_BOOT_SERVICES structure and a 32-bit CRC to verify that the
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
                              descriptor_version: &mut u32) -> EfiStatus,

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
                                  map_key:      usize) -> EfiStatus,
}

/// This protocol is used to obtain input from the ConsoleIn device. The
/// EFI specification requires that the EFI_SIMPLE_TEXT_INPUT_PROTOCOL
/// supports the same languages as the corresponding
/// EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL.
#[repr(C)]
struct EfiSimpleTextInputProtocol {
    /// Resets the input device hardware
    reset: unsafe fn(this: *const EfiSimpleTextInputProtocol,
                     extended_verification: bool) -> EfiStatus,
    
    /// Reads the next keystroke from the input device
    read_keystroke: unsafe fn(this: *const EfiSimpleTextInputProtocol,
                              key:  *mut   EfiInputKey) -> EfiStatus,

    /// Event to use with EFI_BOOT_SERVICES.WaitForEvent() to wait for a
    /// key to be available.
    /// We don't use the event API, don't expose this function pointer
    _wait_for_key: usize,
}

/// This protocol is used to control text-based output devices.
#[repr(C)]
struct EfiSimpleTextOutputProtocol {
    /// Resets the text output device hardware
    reset: unsafe fn(this: *const EfiSimpleTextOutputProtocol,
                     extended_verification: bool) -> EfiStatus,

    /// Writes a string to the output device.
    output_string: unsafe fn(this:   *const EfiSimpleTextOutputProtocol,
                             string: *const u16) -> EfiStatus,

    /// Verifies that all characters in a string can be output to the
    /// target device
    test_string: unsafe fn(this:   *const EfiSimpleTextOutputProtocol,
                           string: *const u16) -> EfiStatus,

    /// Returns information for an available text mode that the output
    /// device(s) support
    _query_mode: usize,

    /// Sets the output device(s) to a specified mode
    _set_modfe: usize,

    /// Sets the background and foreground colors for the OutputString()
    /// and ClearScreen() functions
    _set_attribute: usize,

    /// Clears the output device(s) display to the currently selected 
    /// background color
    _clear_screen: usize,

    /// Sets the current coordinates of the cursor position
    _set_cursor_position: usize,

    /// Show or hide the cursor
    _enable_cursor: usize,

    /// Pointer to SIMPLE_TEXT_OUTPUT_MODE data
    _mode: usize,
}

/// Contains pointers to the runtime and boot services tables
#[repr(C)]
pub struct EfiSystemTable {
    /// The table header for an EFI System Table. This header contains the
    /// EFI_SYSTEM_TABLE_SIGNATURE and EFI_SYSTEM_TABLE_REVISION values
    /// along with the size of the EFI_SYSTEM_TABLE structure and a 32-bit
    /// CRC to verify that the contents of the EFI System Table are valid
    header: EfiTableHeader,

    /// A pointer to a null terminated string that identifies the vendor
    /// that produces the system firmware for the platform
    firmware_vendor: *const u16,

    /// A firmware vendor specific value that identifies the revision of
    /// the system firmware for the platform
    firmware_revision: u32,

    /// The handle for the active console input device. This handle must
    /// support EFI_SIMPLE_TEXT_INPUT_PROTOCOL and
    /// EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL.
    console_in_handle: EfiHandle,

    /// A pointer to the EFI_SIMPLE_TEXT_INPUT_PROTOCOL interface that is
    /// associated with ConsoleInHandle
    console_in: *const EfiSimpleTextInputProtocol,

    /// The handle for the active console output device. This handle must
    /// support the EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL.
    console_out_handle: EfiHandle,

    /// A pointer to the EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL interface that is
    /// associated with ConsoleOutHandle
    console_out: *const EfiSimpleTextOutputProtocol,

    /// The handle for the active standard error console device. This
    /// handle must support the EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL
    console_err_handle: EfiHandle,

    /// A pointer to the EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL interface that is
    /// associated with StandardErrorHandle
    console_err: *const EfiSimpleTextOutputProtocol,

    /// A pointer to the EFI Runtime Services Table
    _runtime_services: usize,

    /// A pointer to the EFI Boot Services Table
    boot_services: *const EfiBootServices,
}

