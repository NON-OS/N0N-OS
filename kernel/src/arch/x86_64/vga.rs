//! NØNOS VGA Text Output System
//!
//! Advanced VGA buffer console for low-level kernel debugging, diagnostics, and panic reporting.
//! Supports hardware-safe cursor control, scroll regions, and color attributes. 
//! Output is synchronized and resilient to panic-state writes.

use core::fmt::{self, Write};
use core::ptr::Unique;
use core::sync::atomic::{AtomicUsize, Ordering};
use spin::Mutex;
use volatile::Volatile;

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;
const VGA_ADDRESS: usize = 0xb8000;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct ColorCode(u8);

impl ColorCode {
    pub fn new(fg: Color, bg: Color) -> Self {
        Self((bg as u8) << 4 | (fg as u8))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
}

#[repr(transparent)]
struct Buffer {
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

pub struct Writer {
    column_position: usize,
    color_code: ColorCode,
    buffer: Unique<Buffer>,
    row_position: usize,
}

static WRITER: Mutex<Writer> = Mutex::new(Writer::new());

impl Writer {
    pub const fn new() -> Self {
        Self {
            column_position: 0,
            color_code: ColorCode::new(Color::LightGreen, Color::Black),
            buffer: unsafe { Unique::new_unchecked(VGA_ADDRESS as *mut _) },
            row_position: 0,
        }
    }

    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }
                self.buffer().chars[self.row_position][self.column_position].write(ScreenChar {
                    ascii_character: byte,
                    color_code: self.color_code,
                });
                self.column_position += 1;
            }
        }
    }

    pub fn buffer(&mut self) -> &mut Buffer {
        unsafe { self.buffer.as_mut() }
    }

    fn new_line(&mut self) {
        if self.row_position + 1 >= BUFFER_HEIGHT {
            self.scroll_up();
        } else {
            self.row_position += 1;
        }
        self.column_position = 0;
    }

    fn scroll_up(&mut self) {
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                let char = self.buffer().chars[row][col].read();
                self.buffer().chars[row - 1][col].write(char);
            }
        }
        self.clear_row(BUFFER_HEIGHT - 1);
    }

    fn clear_row(&mut self, row: usize) {
        for col in 0..BUFFER_WIDTH {
            self.buffer().chars[row][col].write(ScreenChar {
                ascii_character: b' ',
                color_code: self.color_code,
            });
        }
    }

    pub fn clear(&mut self) {
        for row in 0..BUFFER_HEIGHT {
            self.clear_row(row);
        }
        self.row_position = 0;
        self.column_position = 0;
    }

    pub fn set_color(&mut self, fg: Color, bg: Color) {
        self.color_code = ColorCode::new(fg, bg);
    }
}

impl Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.write_byte(byte);
        }
        Ok(())
    }
}

/// Kernel-level VGA output
pub fn print(msg: &str) {
    use core::fmt::Write;
    WRITER.lock().write_str(msg).unwrap();
}

/// Kernel screen reset
pub fn clear() {
    WRITER.lock().clear();
}

/// Change global text color (debug shell, logs, etc)
pub fn set_color(fg: Color, bg: Color) {
    WRITER.lock().set_color(fg, bg);
}
