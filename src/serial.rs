//! A basic 16550 serial driver

use crate::acpi::{Result, Gas};

/// Global serial device implementation
pub static mut SERIAL_DEVICE: Option<Serial> = None;

/// A generic serial port driver
pub struct Serial {
    /// Generic Address Structure parsed out of the ACPI tables
    device: Gas,
}

impl Serial {
    /// Initialize the serial port to 115200n1.
    pub unsafe fn init(device: Gas) -> Result<()> {

        // Initialize the serial port to a known state:
        // Disable all interrupts
        device.write(1, 0x00)?;

        // Set the Divisor Latch Access Bit (DLAB). This maps offsets 0 and
        // 1 to the low and high bytes of the `Divisor register` (instead
        // of the default `Data` and `Interrupt Enable` registers)
        // device.write(3, 0x80)?;

        // Low byte divisor (1 for 115200 baud)
        // device.write(0, 0x01)?;

        // High byte divisor (0 for 115200 baud)
        // device.write(1, 0x00)?;

        // 8 bits, 1 stop bit, no parity
        // device.write(3, 0x03)?;

        // Set RTS and DTR
        // device.write(4, 0x03)?;

        // Create the device
        let mut ret = Self { device };

        // Drain all bytes pending on the serial port
        while let Some(_) = ret.read_byte()? {}

        // Set up the serial device global
        SERIAL_DEVICE = Some(ret);
        Ok(())
    }

    /// Read a byte from the serial port
    pub fn read_byte(&self) -> Result<Option<u8>> {
        unsafe {
            // Check if there is a byte available
            if (self.device.read(5)? & 1) == 0 {
                // No byte available
                Ok(None)
            } else {
                // Read the byte that was present on this port
                Ok(Some(self.device.read(0)? as u8))
            }
        }
    }

    /// Write a byte to the serial device
    fn write_byte(&self, byte: u8) -> Result<()> {
        // Write a CR prior to all LFs
        if byte == b'\n' { self.write_byte(b'\r')?; }

        unsafe { 
            /*
            // Wait for the output buffer to be ready
            if let SerialInterface::Serial16550 = typ {
                while self.device.read(5)? & 0x20 == 0 {}
            }

            // The control bit is set the other way on UART, according to
            // www.activexperts.com/serial-port-component/tutorials/uart/
            // FIXME something is wrong here or above, but it prints to screen
            // somehow
            if let SerialInterface::ArmPL011 = typ {
            */
                while self.device.read(5)? & 0x20 != 0 {}
            /*
            }
            // Wait for the output buffer to be ready
            while (self.device.read(5)? & 0x20) == 0 {}
            */

            // Write the byte
            self.device.write(0, byte as u64)?;
        }

        Ok(())
    }

    /// Write a slice of bytes to the serial device
    pub fn write (&self, bytes: &[u8]) -> Result<()> {
        // Go through each byte and write it
        for &byte in bytes {
            self.write_byte(byte)?
        }

        Ok(())
    }
}

