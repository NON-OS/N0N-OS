// kernel/src/arch/x86_64/serial.rs
//! Serial port driver for early debugging

use core::fmt;
use spin::Mutex;
use x86_64::instructions::port::Port;

pub struct SerialPort {
    data: Port<u8>,
    int_enable: Port<u8>,
    fifo_ctrl: Port<u8>,
    line_ctrl: Port<u8>,
    modem_ctrl: Port<u8>,
    line_status: Port<u8>,
}

impl SerialPort {
    /// Creates a new serial port interface for the given base port.
    pub const unsafe fn new(base: u16) -> Self {
        SerialPort {
            data: Port::new(base),
            int_enable: Port::new(base + 1),
            fifo_ctrl: Port::new(base + 2),
            line_ctrl: Port::new(base + 3),
            modem_ctrl: Port::new(base + 4),
            line_status: Port::new(base + 5),
        }
    }

    /// Initializes the serial port.
    pub fn init(&mut self) {
        unsafe {
            // Disable interrupts
            self.int_enable.write(0x00);

            // Enable DLAB (set baud rate divisor)
            self.line_ctrl.write(0x80);

            // Set divisor to 3 (lo byte) 38400 baud
            self.data.write(0x03);
            self.int_enable.write(0x00); // (hi byte)

            // 8 bits, no parity, one stop bit
            self.line_ctrl.write(0x03);

            // Enable FIFO, clear them, with 14-byte threshold
            self.fifo_ctrl.write(0xC7);

            // RTS/DSR set
            self.modem_ctrl.write(0x0B);

            // Enable interrupts
            self.int_enable.write(0x01);
        }
    }

    fn is_transmit_empty(&mut self) -> bool {
        unsafe { self.line_status.read() & 0x20 != 0 }
    }

    pub fn send(&mut self, data: u8) {
        while !self.is_transmit_empty() {
            core::hint::spin_loop();
        }
        unsafe {
            self.data.write(data);
        }
    }

    pub fn send_string(&mut self, s: &str) {
        for byte in s.bytes() {
            self.send(byte);
        }
    }
}

impl fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.send_string(s);
        Ok(())
    }
}

// Global serial port for COM1
static SERIAL1: Mutex<Option<SerialPort>> = Mutex::new(None);

pub fn init() {
    let mut serial = unsafe { SerialPort::new(0x3F8) }; // COM1
    serial.init();
    *SERIAL1.lock() = Some(serial);
}

pub unsafe fn get_serial() -> Option<&'static mut SerialPort> {
    SERIAL1.lock().as_mut().map(|s| {
        &mut *(s as *mut SerialPort)
    })
}

// Convenience macros
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::arch::x86_64::serial::_print(format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($($arg:tt)*) => {
        $crate::serial_print!("{}\n", format_args!($($arg)*));
    };
}

pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    if let Some(mut serial) = SERIAL1.lock().as_mut() {
        let _ = serial.write_fmt(args);
    }
}
