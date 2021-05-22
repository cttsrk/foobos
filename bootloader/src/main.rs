//! Main bootlader entry for foobOS

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
use serial::Serial;

/// Entry point for panics
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    print!("!!! PANIC !!!\n");

    // Print the location if there is one
    if let Some(location) = info.location() {
        print!("{}:{}:{}\n",
            location.file(), location.line(), location.column());
    }

    // Print the panic message
    if let Some(message) = info.message() {
        print!("{}\n", message);
    }

    // Loop forever
    loop { core::hint::spin_loop(); }
}

/// EFI entry point
#[no_mangle]
extern fn efi_main(image_handle: EfiHandle,
                   system_table: EfiSystemTablePtr) -> EfiStatusCode {
    unsafe {
        // First, register the system table in a global so we can use it in
        // other places such as a `print!` macro
        system_table.register();

        // Seems there's no Rust std for the UEFI target, so can't use e.g.
        // std::env::consts::ARCH
        #[cfg(target_arch = "aarch64")] let arch = "aarch64";
        #[cfg(target_arch = "x86_64")]  let arch = "x86_64";
        #[cfg(target_arch = "riscv64")] let arch = "riscv64";
        print!("\nFoobOS/{} boot\n\n", arch);

        // Initialize ACPI
        let acpi = acpi::init().expect("Failed to initialize ACPI");
        print!("{:#x?}\n", acpi);
        
        // Initialize serial
        let spcr = acpi.spcr.as_ref()
            .expect("ACPI did not report an SPCR, cannot initialize serial");

        // Initialize the serial device
        Serial::init(spcr.interface_type, spcr.address, spcr.baud_rate)
            .expect("Failed to initialize the serial device");
        
        // Get the memory map and exit boot services
        let mm = efi::get_memory_map_and_exit_boot_services(image_handle)
            .expect("Failed to get EFI memory map");
        print!("Exited boot services, bye EFI\n");

        print!("Physical free: {}\n", mm.sum().unwrap());

        print!("EFI MAIN {:#x}\n", efi_main as usize);
    }

    panic!("exiting");
}

/// The stack-probe implementation for Windows targets. This is currently
/// needed for aarch64 because Rust doesn't disable/generate a stub for probes.
#[no_mangle]
fn __chkstk() {}
