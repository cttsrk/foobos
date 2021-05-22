//! A basic serial driver

#![no_std]

use generic_access_structure::{Gas, AccessSize};

/// A `Result` type which wraps a serial error
pub type Result<T> = core::result::Result<T, Error>;

/// Serial device errors
#[derive(Debug)]
pub enum Error {
    /// The interface is not a supported serial device for this driver
    UnsupportedDevice(Interface),

    /// Accessing the device via the [`Gas`] returned an error
    GasError(generic_access_structure::Error),
}

impl From<generic_access_structure::Error> for Error {
    fn from(val: generic_access_structure::Error) -> Self {
        Self::GasError(val)
    }
}

/// Global serial device implementation
static mut SERIAL_DEVICE: Option<Serial> = None;

/// Get a reference to the serial device
pub fn serial_device() -> Option<&'static Serial> {
    unsafe { SERIAL_DEVICE.as_ref() }
}

/// Different baud rates for the serial device
#[derive(Debug, Clone, Copy)]
pub enum BaudRate {
    /// Do not change the baud rate
    AsIs,

    /// 9600
    Baud9600,

    /// 19200
    Baud19200,

    /// 57600
    Baud57600,

    /// 115200
    Baud115200,
}

/// Different types of serial devices
#[derive(Debug, Clone, Copy)]
pub enum Interface {
    /// Full 16550 interface
    Serial16550,

    /// Full 16450 interface (must also accept writing to the 16550 FCR
    /// register)
    Serial16450,

    /// MAX311xxE SPI UART
    Max311,

    /// ARM PL011 UART
    ArmPL011,

    /// MSM8x60 (e.g. 8960)
    Msm8x60,

    /// Nvidia 16550
    Nvidia16550,

    /// TI OMAP
    TiOmap,

    /// APM88xxxx
    Apm88xxxx,

    /// MSM8974
    Msm8974,

    /// SAM5250
    Sam5250,

    /// Intel USIF
    IntelUsif,

    /// i.MX 6
    IMX6,

    /// (deprecated) ARM SBSA (2.x only) Generic UART supporting only 32-bit
    /// accesses
    ArmSbsa32,

    /// ARM SBSA Generic UART
    ArmSbsa,

    /// ARM DCC
    ArmDcc,

    /// BCM2835
    Bcm2835,

    /// SDM845 with a clock rate of 1.8432 MHz
    Sdm845_18432,
    
    /// 16550-compatible with parameters defined in Generic Address Structure
    Serial16550Gas,

    /// SDM845 with a clock rate of 7.362 MHz
    Sdm845_7362,

    /// Intel LPSS
    IntelLpss,

    /// Unknown serial interface
    Unknown(u8),
}

impl From<u8> for Interface {
    fn from(val: u8) -> Self {
        match val {
             0 => Self::Serial16550,
             1 => Self::Serial16450,
             2 => Self::Max311,
             3 => Self::ArmPL011,
             4 => Self::Msm8x60,
             5 => Self::Nvidia16550,
             6 => Self::TiOmap,
             8 => Self::Apm88xxxx,
             9 => Self::Msm8974,
            10 => Self::Sam5250,
            11 => Self::IntelUsif,
            12 => Self::IMX6,
            13 => Self::ArmSbsa32,
            14 => Self::ArmSbsa,
            15 => Self::ArmDcc,
            16 => Self::Bcm2835,
            17 => Self::Sdm845_18432,
            18 => Self::Serial16550Gas,
            19 => Self::Sdm845_7362,
            20 => Self::IntelLpss,
             _ => Self::Unknown(val),
        }
    }
}

/// A serial port driver
pub struct Serial {
    /// Generic Address Structure parsed out of the ACPI tables
    device: Gas,
}

impl Serial {
    /// Initialize the serial port
    ///
    /// # Parameters
    /// 
    /// * `interface` - Type of serial interface to use for this device
    /// * `device`    - Generic Address Structure which was parsed from the
    ///                 SPCR ACPI table
    /// * `baud_rate` - Baud rate to configure the device at
    ///
    /// # Returns
    ///
    /// `()` on success, on error [`Error`]
    ///
    /// # Safety
    ///
    /// This function initalizes a serial device as if it is an `interface`
    /// with accessible register space at `device`. If these are not compatible
    /// or are invalid, undefined behaviour will occur.
    ///
    /// This function must be called in a single threaded environment as it
    /// initializes a mutable static without locks. This is not a huge
    /// restriction as this function should be called very early in boot before
    /// we are multi-core.
    ///
    pub unsafe fn init(interface: Interface,
                       mut device: Gas, baud_rate: BaudRate) -> Result<()> {
                
        // WORKAROUND: Sometimes the I/O port on 16550 serial
        // interfaces is set to `Undefined` in the SPCR. We know that
        // for x86_64 16550's, the access size should always be byte.
        #[cfg(target_arch = "x86_64")]
        if let Interface::Serial16550 = interface {
            if let Gas::Io { access_size, .. } = &mut device {
                if let AccessSize::Undefined = access_size {
                    *access_size = AccessSize::Byte;
                }
            }
        } else {
            // We do not know how to support this serial device (yet)
            return Err(Error::UnsupportedDevice(interface));

        }

        // Disable all interrupts
        device.write(1, 0x00)?;

        // Convert the baud rate into the divisor
        let divisor = match baud_rate {
            BaudRate::AsIs       => None,
            BaudRate::Baud115200 => Some((0, 1)),
            BaudRate::Baud57600  => Some((0, 2)),
            BaudRate::Baud19200  => Some((0, 6)),
            BaudRate::Baud9600   => Some((0, 12))
        };

        // Program the baud rate for the device
        if let Some((high, low)) = divisor {
            // Set the Divisor Latch Access Bit (DLAB). This maps offsets 0 and
            // 1 to the low and high bytes of the `Divisor register` (instead
            // of the default `Data` and `Interrupt Enable` registers)
            device.write(3, 0x80)?;
            device.write(0, low)?;  // Low byte divisor
            device.write(1, high)?; // High byte divisor
        }

        // It's always 8 bits, 1 stop bit, no parity
        device.write(3, 0x03)?;

        // Set RTS and DTR
        device.write(4, 0x03)?;

        // Create the device
        let ret = Self { device };

        // Drain all bytes pending on the serial port
        while ret.read_byte()?.is_some() {}

        // Set up the serial device global
        SERIAL_DEVICE = Some(ret);
        Ok(())
    }

    /// Read a byte from the serial port
    ///
    /// # Returns
    ///
    /// On success, returns the byte which was read from the serial port or
    /// `None` if no byte was available. On error [`Error`]
    ///
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
    ///
    /// # Parameters
    ///
    /// * `bytes` - The slice of bytes to write to the serial device
    /// 
    /// # Returns
    ///
    /// `()` on success, on error [`Error`]
    ///
    fn write_byte(&self, byte: u8) -> Result<()> {
        // Write a CR prior to all LFs
        if byte == b'\n' { self.write_byte(b'\r')?; }

        unsafe { 
            /*
            // Wait for the output buffer to be ready
            if let Interface::Serial16550 = typ {
                while self.device.read(5)? & 0x20 == 0 {}
            }

            // The control bit is set the other way on UART, according to
            // www.activexperts.com/serial-port-component/tutorials/uart/
            // FIXME something is wrong here or above, but it prints to screen
            // somehow
            if let Interface::ArmPL011 = typ {
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
    ///
    /// # Parameters
    ///
    /// * `bytes` - The slice of bytes to write to the serial device
    ///
    /// # Returns
    ///
    /// `()` on success, on error [`Error`]
    ///
    pub fn write (&self, bytes: &[u8]) -> Result<()> {
        // Go through each byte and write it
        for &byte in bytes {
            self.write_byte(byte)?
        }

        Ok(())
    }
}

