#![feature(asm, panic_info_message, bool_to_option)]
#![no_std]
#![no_main]

#[macro_use] mod print;
mod core_requirements;
mod efi;
mod mm;
mod acpi;
mod rangeset;

use core::panic::PanicInfo;

use crate::efi::{EfiHandle, EfiSystemTablePtr, EfiStatusCode};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    print!("!!! PANIC !!!\n");
    if let Some(location) = info.location() {
        print!("{}:{}:{}\n",
            location.file(), location.line(), location.column());
    }
    if let Some(message) = info.message() {
        print!("{}\n", message);
    }

    loop {
        unsafe { asm!("hlt"); }
    }
}

#[no_mangle]
extern fn efi_main(image_handle: EfiHandle,
                   system_table: EfiSystemTablePtr) -> EfiStatusCode {
    unsafe {
        // First, register the system table in a global so we can use it in
        // other places such as a `print!` macro
        system_table.register();

        print!("EFI MAIN {:#x}\n", efi_main as u64);
        print!("\n\n\nFOOBOS INIT\n\n");

        // Initialize ACPI
        acpi::init().expect("Failed to initialize ACPI");

        // Get the memory map and exit boot services
        let mm = efi::get_memory_map(image_handle)
            .expect("Failed to get EFI memory map");

        print!("{:#x?}\n", mm.entries());
        print!("Physical free: {}\n", mm.sum().unwrap());

        print!("EFI MAIN {:#x}\n", efi_main as usize);
    }

    loop {
    }
}
