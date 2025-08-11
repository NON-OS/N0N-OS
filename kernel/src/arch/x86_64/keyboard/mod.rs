// arch/x86_64/keyboard/mod.rs
//
// PS/2 keyboard (i8042) — 
// - IRQ1 handler (IOAPIC routed), lockless ring buffer of KeyEvent
// - Scancode Set 1 decode: make/break, E0/E1 prefixes, extended keys
// - Modifiers: Ctrl/Shift/Alt/Meta; compose ASCII when possible
// - Public APIs:
//     getchar_blocking() -> u8                     // cooked byte (for simple consumers)
//     get_event_blocking() -> KeyEvent             // full event (for TUI line editor)
//     poll_key() -> Option<KeyEvent>               // non-blocking
// - LED control (Num/Caps/Scroll), typematic rate stub
//
// Zero-state. All input public.

#![allow(dead_code)]

use core::sync::atomic::{AtomicUsize, AtomicU32, Ordering};
use spin::Mutex;

use crate::arch::x86_64::interrupt::{apic, ioapic};
use crate::arch::x86_64::interrupt::idt;
use crate::arch::x86_64::port::{inb, outb};

// —————————————————— HW regs ——————————————————

const PS2_DATA:  u16 = 0x60;
const PS2_STAT:  u16 = 0x64;
const PS2_CMD:   u16 = 0x64;

const STAT_OBF: u8 = 1 << 0; // output buffer full (data->CPU)
const STAT_IBF: u8 = 1 << 1; // input buffer full (CPU->ctrl busy)

const CMD_READ_CFG:  u8 = 0x20;
const CMD_WRITE_CFG: u8 = 0x60;

const KBD_SET_LEDS:  u8 = 0xED;
const KBD_SET_RATE:  u8 = 0xF3;
const KBD_ACK:       u8 = 0xFA;

// —————————————————— public event model ——————————————————

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyCode {
    // printable
    Char(u8),
    // control/navigation
    Enter, Backspace, Delete, Tab,
    Left, Right, Up, Down, Home, End, PageUp, PageDown,
    Insert, Escape,
    // word motions (exposed to TUI)
    WordLeft, WordRight,
    // function keys
    F(u8), // 1..=12
    // lock keys
    CapsLock, NumLock, ScrollLock,
    // unknown/unsupported scancode
    Unknown(u8),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Mods { pub ctrl: bool, pub alt: bool, pub shift: bool, pub meta: bool }

#[derive(Clone, Copy, Debug)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub mods: Mods,
    pub pressed: bool, // true = make, false = break
    pub chr: Option<u8>, // ASCII if printable (after modifiers), else None
}

// —————————————————— decode state ——————————————————

#[derive(Default)]
struct Decode {
    e0: bool,
    e1: u8,
    mods: Mods,
    caps: bool,
    num: bool,
    scroll: bool,
}

impl Decode {
    fn feed(&mut self, sc: u8) -> Option<KeyEvent> {
        // handle prefixes
        if sc == 0xE0 { self.e0 = true; return None; }
        if sc == 0xE1 { self.e1 = 2;   return None; }
        if self.e1 > 0 {
            // swallow two bytes after E1 (Pause/Break)
            self.e1 -= 1;
            return None;
        }

        let break_code = (sc & 0x80) != 0;
        let code = sc & 0x7F;

        // map set1 scancode with e0 flag
        let mut kc = map_scancode(code, self.e0);
        self.e0 = false;

        // normalize
        // modifier tracking
        match kc {
            KeyCode::Unknown(_) => {}
            KeyCode::CapsLock if !break_code => { self.caps = !self.caps; self.apply_leds(); }
            KeyCode::NumLock  if !break_code => { self.num  = !self.num;  self.apply_leds(); }
            KeyCode::ScrollLock if !break_code => { self.scroll = !self.scroll; self.apply_leds(); }
            _ => {}
        }
        // left/right shift
        if (code == 0x2A || code == 0x36) && !self.e0 {
            self.mods.shift = !break_code; // both shifts
        }
        // ctrl
        if (code == 0x1D && !self.e0) || (self.e0 && code == 0x1D) {
            self.mods.ctrl = !break_code;
        }
        // alt
        if (code == 0x38 && !self.e0) || (self.e0 && code == 0x38) {
            self.mods.alt = !break_code;
        }
        // meta/super (on some boards sc 0x5B/0x5C with E0)
        if self.e0 && (code == 0x5B || code == 0x5C) {
            self.mods.meta = !break_code;
        }

        // compose ascii where possible
        let chr = compose_ascii(kc, self.mods, self.caps);

        // promote arrows + ctrl to word motions for TUI convenience
        let mut code_out = kc;
        if self.mods.ctrl && !self.mods.alt && !self.mods.shift {
            match kc {
                KeyCode::Left => code_out = KeyCode::WordLeft,
                KeyCode::Right => code_out = KeyCode::WordRight,
                _ => {}
            }
        }

        Some(KeyEvent {
            code: code_out,
            mods: self.mods,
            pressed: !break_code,
            chr,
        })
    }

    fn apply_leds(&self) {
        // best-effort; ignore ack errors
        unsafe {
            wait_ibf_clear();
            outb(PS2_DATA, KBD_SET_LEDS);
            wait_ibf_clear();
            let mut b = 0u8;
            if self.scroll { b |= 1<<0; }
            if self.num    { b |= 1<<1; }
            if self.caps   { b |= 1<<2; }
            outb(PS2_DATA, b);
        }
    }
}

// —————————————————— ring buffer ——————————————————

const QCAP: usize = 1024;
struct Ring {
    buf: [KeyEvent; QCAP],
    head: AtomicUsize,
    tail: AtomicUsize,
}
impl Ring {
    const fn new() -> Self {
        const NIL: KeyEvent = KeyEvent { code: KeyCode::Unknown(0), mods: Mods{ctrl:false,alt:false,shift:false,meta:false}, pressed:false, chr:None };
        Self { buf: [NIL; QCAP], head: AtomicUsize::new(0), tail: AtomicUsize::new(0) }
    }
    #[inline] fn push_isr(&self, e: KeyEvent) {
        let t = self.tail.load(Ordering::Relaxed);
        let h = self.head.load(Ordering::Acquire);
        if t.wrapping_sub(h) >= QCAP { return; } // drop
        self.buf[t % QCAP] = e;
        self.tail.store(t.wrapping_add(1), Ordering::Release);
    }
    #[inline] fn pop(&self) -> Option<KeyEvent> {
        let h = self.head.load(Ordering::Relaxed);
        let t = self.tail.load(Ordering::Acquire);
        if h == t { return None; }
        let e = self.buf[h % QCAP];
        self.head.store(h.wrapping_add(1), Ordering::Release);
        Some(e)
    }
}

static RING: Ring = Ring::new();
static DEC: Mutex<Decode> = Mutex::new(Decode::default());

// cooked byte buffer for simple getchar()
const CBUF: usize = 512;
static CIRC: Mutex<[u8; CBUF]> = Mutex::new([0; CBUF]);
static CHEAD: AtomicU32 = AtomicU32::new(0);
static CTAIL: AtomicU32 = AtomicU32::new(0);

// —————————————————— init / IRQ routing ——————————————————

pub unsafe fn init() {
    // unmask IRQ1 via IOAPIC and route to our vector
    let vec = idt::VEC_KBD;
    let dest = apic::id();
    ioapic::route_gsi(1, vec, dest, ioapic::RouteFlags::EDGE).ok();

    // enable scanning in controller config (make sure port 1 enabled)
    wait_ibf_clear();
    outb(PS2_CMD, CMD_READ_CFG);
    let mut cfg = wait_read_data();
    // enable IRQ1 bit + translate disabled
    cfg |= 1 << 0; // port1 interrupt
    cfg &= !(1 << 6); // scancode translation off (we want set1)
    wait_ibf_clear();
    outb(PS2_CMD, CMD_WRITE_CFG);
    wait_ibf_clear();
    outb(PS2_DATA, cfg);

    // LEDs off deterministic
    let mut d = DEC.lock();
    d.caps = false; d.num = false; d.scroll = false;
    d.apply_leds();
}

/// IDT handler (vector idt::VEC_KBD)
#[no_mangle]
pub extern "x86-interrupt" fn kbd_irq(_st: x86_64::structures::idt::InterruptStackFrame) {
    unsafe {
        // read all pending scancodes
        while (inb(PS2_STAT) & STAT_OBF) != 0 {
            let sc = inb(PS2_DATA);
            if let Some(ev) = DEC.lock().feed(sc) {
                // push event
                RING.push_isr(ev);
                // if printable and pressed, copy to cooked circ buf
                if ev.pressed {
                    if let Some(b) = ev.chr {
                        let head = CHEAD.load(Ordering::Relaxed);
                        let tail = CTAIL.load(Ordering::Acquire);
                        if head.wrapping_sub(tail) < CBUF as u32 {
                            CIRC.lock()[(head as usize) % CBUF] = b;
                            CHEAD.store(head.wrapping_add(1), Ordering::Release);
                        }
                    }
                }
            }
        }
        apic::eoi();
    }
}

// —————————————————— public API ——————————————————

pub fn poll_key() -> Option<KeyEvent> { RING.pop() }

pub fn get_event_blocking() -> KeyEvent {
    loop {
        if let Some(e) = RING.pop() { return e; }
        // light pause; interrupts will wake us
        unsafe { core::arch::asm!("hlt", options(nomem, nostack, preserves_flags)); }
    }
}

/// Cooked single byte (printables + CR/LF/TAB/BS)
pub fn getchar_blocking() -> u8 {
    loop {
        // fast-path cooked circ
        {
            let tail = CTAIL.load(Ordering::Relaxed);
            let head = CHEAD.load(Ordering::Acquire);
            if tail != head {
                let b = CIRC.lock()[(tail as usize) % CBUF];
                CTAIL.store(tail.wrapping_add(1), Ordering::Release);
                return b;
            }
        }
        // otherwise block on event and try composing
        let e = get_event_blocking();
        if let Some(b) = e.chr { return b; }
        match e.code {
            KeyCode::Enter => return b'\n',
            KeyCode::Backspace => return 0x08,
            KeyCode::Tab => return b'\t',
            _ => {}
        }
    }
}

// —————————————————— helpers ——————————————————

unsafe fn wait_ibf_clear() {
    while (inb(PS2_STAT) & STAT_IBF) != 0 {
        core::hint::spin_loop();
    }
}
unsafe fn wait_read_data() -> u8 {
    while (inb(PS2_STAT) & STAT_OBF) == 0 {
        core::hint::spin_loop();
    }
    inb(PS2_DATA)
}

// Scancode set 1 mapping (subset + extended via E0)
fn map_scancode(code: u8, e0: bool) -> KeyCode {
    if e0 {
        return match code {
            0x48 => KeyCode::Up,
            0x50 => KeyCode::Down,
            0x4B => KeyCode::Left,
            0x4D => KeyCode::Right,
            0x47 => KeyCode::Home,
            0x4F => KeyCode::End,
            0x49 => KeyCode::PageUp,
            0x51 => KeyCode::PageDown,
            0x52 => KeyCode::Insert,
            0x53 => KeyCode::Delete,
            0x1C => KeyCode::Enter,
            0x38 => KeyCode::Unknown(0xE0), // AltGr handled via mods
            0x5B => KeyCode::Unknown(0xE0), // Left Meta
            0x5C => KeyCode::Unknown(0xE0), // Right Meta
            _ => KeyCode::Unknown(code),
        };
    }
    match code {
        0x01 => KeyCode::Escape,
        0x0E => KeyCode::Backspace,
        0x0F => KeyCode::Tab,
        0x1C => KeyCode::Enter,
        0x3A => KeyCode::CapsLock,
        0x45 => KeyCode::NumLock,
        0x46 => KeyCode::ScrollLock,

        0x3B..=0x44 => KeyCode::F(code - 0x3A), // F1..F10
        0x57 => KeyCode::F(11),
        0x58 => KeyCode::F(12),

        // main alphanumerics
        0x02 => KeyCode::Char(b'1'), 0x03 => KeyCode::Char(b'2'),
        0x04 => KeyCode::Char(b'3'), 0x05 => KeyCode::Char(b'4'),
        0x06 => KeyCode::Char(b'5'), 0x07 => KeyCode::Char(b'6'),
        0x08 => KeyCode::Char(b'7'), 0x09 => KeyCode::Char(b'8'),
        0x0A => KeyCode::Char(b'9'), 0x0B => KeyCode::Char(b'0'),
        0x0C => KeyCode::Char(b'-'), 0x0D => KeyCode::Char(b'='),

        0x10 => KeyCode::Char(b'q'), 0x11 => KeyCode::Char(b'w'),
        0x12 => KeyCode::Char(b'e'), 0x13 => KeyCode::Char(b'r'),
        0x14 => KeyCode::Char(b't'), 0x15 => KeyCode::Char(b'y'),
        0x16 => KeyCode::Char(b'u'), 0x17 => KeyCode::Char(b'i'),
        0x18 => KeyCode::Char(b'o'), 0x19 => KeyCode::Char(b'p'),
        0x1A => KeyCode::Char(b'['), 0x1B => KeyCode::Char(b']'),

        0x1E => KeyCode::Char(b'a'), 0x1F => KeyCode::Char(b's'),
        0x20 => KeyCode::Char(b'd'), 0x21 => KeyCode::Char(b'f'),
        0x22 => KeyCode::Char(b'g'), 0x23 => KeyCode::Char(b'h'),
        0x24 => KeyCode::Char(b'j'), 0x25 => KeyCode::Char(b'k'),
        0x26 => KeyCode::Char(b'l'), 0x27 => KeyCode::Char(b';'),
        0x28 => KeyCode::Char(b'\''), 0x29 => KeyCode::Char(b'`'),

        0x2C => KeyCode::Char(b'z'), 0x2D => KeyCode::Char(b'x'),
        0x2E => KeyCode::Char(b'c'), 0x2F => KeyCode::Char(b'v'),
        0x30 => KeyCode::Char(b'b'), 0x31 => KeyCode::Char(b'n'),
        0x32 => KeyCode::Char(b'm'), 0x33 => KeyCode::Char(b','),
        0x34 => KeyCode::Char(b'.'), 0x35 => KeyCode::Char(b'/'),
        0x39 => KeyCode::Char(b' '),

        _ => KeyCode::Unknown(code),
    }
}

fn compose_ascii(kc: KeyCode, m: Mods, caps: bool) -> Option<u8> {
    match kc {
        KeyCode::Char(mut b) => {
            // letters
            if (b'a'..=b'z').contains(&b) {
                let upper = (caps ^ m.shift);
                if upper { b = b - b'a' + b'A'; }
                return Some(b);
            }
            // digits and symbols
            let shifted = match b {
                b'1' => b'!', b'2' => b'@', b'3' => b'#', b'4' => b'$', b'5' => b'%',
                b'6' => b'^', b'7' => b'&', b'8' => b'*', b'9' => b'(', b'0' => b')',
                b'-' => b'_', b'=' => b'+',
                b'[' => b'{', b']' => b'}',
                b';' => b':', b'\''=> b'"', b'`' => b'~',
                b',' => b'<', b'.' => b'>', b'/' => b'?', _ => b,
            };
            Some(if m.shift { shifted } else { b })
        }
        _ => None,
    }
}

// —————————————————— IDT hook ——————————————————

#[doc(hidden)]
pub fn install_idt_gate(idt: &mut x86_64::structures::idt::InterruptDescriptorTable) {
    idt[usize::from(idt::VEC_KBD)].set_handler_fn(kbd_irq);
}
