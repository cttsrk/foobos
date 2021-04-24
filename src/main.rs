#![feature(asm, panic_info_message, bool_to_option)]
#![no_std]
#![no_main]

#[macro_use] mod print;
mod core_requirements;
mod efi;
mod mm;
mod acpi;

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
    }
}

#[no_mangle]
extern fn efi_main(image_handle: EfiHandle,
                   system_table: EfiSystemTablePtr) -> EfiStatusCode {
    unsafe {
        // First, register the system table in a global so we can use it in
        // other places such as a `print!` macro
        system_table.register();

        #[cfg(target_arch = "aarch64")]
        let arch = "aarch64";

        #[cfg(target_arch = "x86_64")]
        let arch = "x86_64";

        print!("\n\nFOOBOS INIT: booting {}\n\n", arch);

        // Initialize ACPI
        acpi::init().expect("Failed to initialize ACPI");

        // Get the memory map and exit boot services
        let mut mm = efi::get_memory_map(image_handle)
            .expect("Failed to get EFI memory map");

        let addr = mm.allocate(1024 * 1024, 4096).unwrap();
        print!("Allocated {:#x}\n", addr);

        print!("{:#x?}\n", mm.entries());
        print!("Physical free: {}\n", mm.sum().unwrap());

        print!("EFI MAIN {:#x}\n", efi_main as usize);
    }

    loop {}
}

#[no_mangle]
fn __chkstk() {}
