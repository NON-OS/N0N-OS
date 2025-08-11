// ui/tui.rs
//
// Text UI 
// - Output: VGA/FB writer with optional ANSI subset (CR, LF, BS, CLS, color attr map)
// - Input: blocking line editor with history + tab-complete hook
// - Zero heap on hot path; fixed buffers; IRQ-safe write path
//
// Integrates with arch keyboard driver exposing getchar_blocking() and poll_key().

#![allow(dead_code)]

use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;

static INITED: AtomicBool = AtomicBool::new(false);

pub fn init_if_framebuffer() {
    INITED.store(true, Ordering::Relaxed);
}

pub fn write(s: &str) {
    // Minimal ANSI: \r, \n, \x08(backspace), \x0C(form feed=clear)
    for b in s.as_bytes() {
        match *b {
            b'\r' => {},
            b'\n' => crate::arch::x86_64::vga::print("\n"),
            0x08 => crate::arch::x86_64::vga::print("\x08 \x08"),
            0x0C => crate::arch::x86_64::vga::clear(),
            _ => {
                let mut ch = [0u8; 1]; ch[0] = *b;
                crate::arch::x86_64::vga::print(core::str::from_utf8(&ch).unwrap_or("?"));
            }
        }
    }
}

pub fn clear() {
    crate::arch::x86_64::vga::clear();
}

// ——— line editor ———

const HIST: usize = 64;
const MAX: usize = 256;

static HISTORY: Mutex<[heapless::String<MAX>; HIST]> = Mutex::new(array_init::array_init(|_| heapless::String::new()));
static HHEAD: Mutex<usize> = Mutex::new(0);

pub fn read_line(buf: &mut [u8]) -> usize {
    let mut n = 0usize;
    let mut hist_idx: isize = -1;

    loop {
        let c = crate::arch::x86_64::keyboard::getchar_blocking();

        match c {
            b'\r' | b'\n' => {
                write("\n");
                remember(&buf[..n]);
                return n;
            }
            0x08 | 0x7F => {
                if n > 0 {
                    n -= 1;
                    write("\x08 \x08");
                }
            }
            // rudimentary history on ^P (up) and ^N (down) via ASCII control keys as placeholder
            0x10 /*^P*/ => {
                if let Some(s) = hist_nav(-1, &mut hist_idx) {
                    redraw_line(buf, &mut n, s.as_bytes());
                }
            }
            0x0e /*^N*/ => {
                if let Some(s) = hist_nav(+1, &mut hist_idx) {
                    redraw_line(buf, &mut n, s.as_bytes());
                } else {
                    // clear to empty
                    redraw_line(buf, &mut n, &[]);
                }
            }
            0x09 /*TAB*/ => {
                if let Some(suggest) = super::cli_suggest_for_tab(core::str::from_utf8(&buf[..n]).unwrap_or("")) {
                    redraw_line(buf, &mut n, suggest.as_bytes());
                }
            }
            b => {
                if n < buf.len() {
                    buf[n] = b; n += 1;
                    let mut ch = [0u8;1]; ch[0] = b;
                    write(core::str::from_utf8(&ch).unwrap_or("?"));
                }
            }
        }
    }
}

fn remember(line: &[u8]) {
    if line.is_empty() { return; }
    let s = core::str::from_utf8(line).unwrap_or("");
    let mut hist = HISTORY.lock();
    let mut head = HHEAD.lock();
    hist[*head].clear();
    let _ = hist[*head].push_str(s);
    *head = (*head + 1) % HIST;
}

fn hist_nav(dir: isize, idx: &mut isize) -> Option<heapless::String<MAX>> {
    let hist = HISTORY.lock();
    let head = *HHEAD.lock() as isize;
    if hist.iter().all(|s| s.is_empty()) { return None; }

    if *idx == -1 { *idx = head - 1; } else { *idx += dir; }
    let len = HIST as isize;
    if *idx < head - len { *idx = head - len; }
    if *idx >= head { return None; }
    let pos = ((*idx % len) + len) % len;
    let s = &hist[pos as usize];
    if s.is_empty() { None } else { Some(s.clone()) }
}

fn redraw_line(buf: &mut [u8], n: &mut usize, new: &[u8]) {
    // naive: backspace current, print new
    while *n > 0 {
        write("\x08 \x08"); *n -= 1;
    }
    let k = core::cmp::min(new.len(), buf.len());
    buf[..k].copy_from_slice(&new[..k]);
    *n = k;
    if let Ok(s) = core::str::from_utf8(&buf[..*n]) { write(s); }
}

// Hook for TAB completion from CLI registry
#[allow(unused)]
pub fn set_tab_hook(_: fn(&str)->Option<heapless::String<MAX>>) {}
