//! This file handles the [`print!`] macro which allows displaying
//! information to the UEFI standard out console via the UEFI API, or to the
//! serial port specified by the ACPI SPCR table.

use core::fmt::{Result, Write, Error};
use serial::serial_device;

/// A dummy screen writing structure we can implement [`Write`] on
pub struct ScreenWriter;

impl Write for ScreenWriter {
    fn write_str(&mut self, string: &str) -> Result {
        if let Some(serial) = serial_device() { 
            serial.write(string.as_bytes()).map_err(|_| Error)
        } else {
            crate::efi::output_string(string).map_err(|_| Error)
        }
    }
}

/// The standard Rust [`print!`] macro!
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        let _ = <$crate::print::ScreenWriter as core::fmt::Write>::write_fmt(
            &mut $crate::print::ScreenWriter,
            format_args!($($arg)*));
    }
}
