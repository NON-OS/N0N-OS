//! ui.rs — NØNOS Boot UI (safe console, sections, progress, spinner)
//! eK@nonos-tech.xyz
//!
//! - No globals: pass &SystemTable<Boot> once and hold a &mut TextOutput
//! - Structured helpers: banner, section, kv, info/warn/ok/fail, panic
//! - Progress bar + spinner (ASCII-safe for UEFI text mode)
//! - Color themes w/ automatic reset; errors propagated (no silent drops)

#![allow(dead_code)]

use uefi::prelude::SystemTable;
use uefi::proto::console::text::{Color, TextOutput};
use uefi::Status;

/// Boot UI with a borrowed console handle (no unsafe global loads).
pub struct Ui<'a> {
    con: &'a mut TextOutput,
    theme: Theme,
}

#[derive(Clone, Copy)]
pub struct Theme {
    pub bg: Color,
    pub info: Color,
    pub ok: Color,
    pub warn: Color,
    pub err: Color,
    pub title: Color,
    pub text: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: Color::Black,
            info: Color::Gray,
            ok: Color::LightGreen,
            warn: Color::Yellow,
            err: Color::LightRed,
            title: Color::LightCyan,
            text: Color::White,
        }
    }
}

impl<'a> Ui<'a> {
    /// Acquire UI from a system table safely.
    pub fn new(st: &'a SystemTable<uefi::table::Boot>) -> Self {
        // SAFETY: UEFI gives us a mutable TextOutput via &mut from SystemTable
        let con = st.stdout();
        Ui { con, theme: Theme::default() }
    }

    /// Replace color theme.
    pub fn set_theme(&mut self, t: Theme) { self.theme = t; }

    /// Clear screen and draw a banner.
    pub fn banner(&mut self) -> Result<(), Status> {
        self.color(self.theme.title, self.theme.bg)?;
        self.con.clear()?;
        self.line("")?;
        self.raw("              ╔═════════════════════════════════════════════════════════════╗")?;
        self.raw("              ║                 NØNOS :: ZERO-STATE LAUNCHPAD              ║")?;
        self.raw("              ║         Privacy-Native / Identity-Free / Capsule-First     ║")?;
        self.raw("              ║        UEFI Boot  →  Verified Capsule  →  Kernel Jump      ║")?;
        self.raw("              ╚═════════════════════════════════════════════════════════════╝")?;
        self.line("")?;
        self.color(self.theme.text, self.theme.bg)
    }

    /// Start a titled section.
    pub fn section(&mut self, title: &str) -> Result<(), Status> {
        self.color(self.theme.title, self.theme.bg)?;
        self.raw("── ")?;
        self.raw(title)?;
        self.raw(" ")?;
        self.rule(60)?;
        self.color(self.theme.text, self.theme.bg)
    }

    /// Key/Value aligned line.
    pub fn kv(&mut self, key: &str, val: &str) -> Result<(), Status> {
        self.color(self.theme.info, self.theme.bg)?;
        self.raw("• ")?;
        self.raw(key)?;
        self.raw(": ")?;
        self.color(self.theme.text, self.theme.bg)?;
        self.line(val)
    }

    /// Info / OK / Warn / Fail log lines.
    pub fn info(&mut self, msg: &str) -> Result<(), Status> {
        self.level(self.theme.info, "[info] ", msg)
    }
    pub fn ok(&mut self, msg: &str) -> Result<(), Status> {
        self.level(self.theme.ok, "[ ok ] ", msg)
    }
    pub fn warn(&mut self, msg: &str) -> Result<(), Status> {
        self.level(self.theme.warn, "[warn] ", msg)
    }
    pub fn fail(&mut self, msg: &str) -> Result<(), Status> {
        self.level(self.theme.err, "[FAIL] ", msg)
    }

    /// Panic/fatal block in red.
    pub fn panic_block(&mut self, msg: &str) -> Result<(), Status> {
        self.color(self.theme.err, self.theme.bg)?;
        self.line("")?;
        self.raw("──────────────────── SYSTEM FAULT DETECTED ────────────────────")?;
        self.line("")?;
        self.raw("[!] ")?; self.line(msg)?;
        self.raw("───────────────────────────────────────────────────────────────")?;
        self.line("")?;
        self.color(self.theme.text, self.theme.bg)
    }

    /// Draw a simple progress bar: current/total (0..total).
    pub fn progress(&mut self, current: usize, total: usize, label: &str) -> Result<(), Status> {
        let total = total.max(1);
        let width = 32usize;
        let filled = ((current.min(total) * width) / total).min(width);
        let mut bar = [b' '; 32];
        for i in 0..filled { bar[i] = b'='; }
        self.color(self.theme.info, self.theme.bg)?;
        self.raw("[")?;
        self.raw(core::str::from_utf8(&bar).unwrap_or("                                "))?;
        self.raw("] ")?;
        self.color(self.theme.text, self.theme.bg)?;
        self.line(label)
    }

    /// A tiny spinner (ASCII) you can tick in loops.
    pub fn spinner(&mut self, i: usize, label: &str) -> Result<(), Status> {
        const FR: &[u8] = b"|/-\\";
        let ch = FR[i % FR.len()];
        self.color(self.theme.info, self.theme.bg)?;
        self.raw("[")?;
        self.raw_char(ch as char)?;
        self.raw("] ")?;
        self.color(self.theme.text, self.theme.bg)?;
        self.line(label)
    }

    /* ------------- low-level helpers (color, write, etc.) ------------- */

    #[inline]
    fn color(&mut self, fg: Color, bg: Color) -> Result<(), Status> {
        self.con.set_color(fg, bg)
    }

    #[inline]
    fn raw(&mut self, s: &str) -> Result<(), Status> {
        // UEFI TextOutput accepts UCS-2. The `uefi` crate maps &str → UCS-2 internally.
        self.con.output_string(s)
    }

    #[inline]
    fn raw_char(&mut self, c: char) -> Result<(), Status> {
        let mut buf = [0u8; 4];
        let s = c.encode_utf8(&mut buf);
        self.con.output_string(s)
    }

    #[inline]
    fn line(&mut self, s: &str) -> Result<(), Status> {
        self.con.output_string(s)?;
        self.con.output_string("\r\n")
    }

    fn level(&mut self, fg: Color, tag: &str, msg: &str) -> Result<(), Status> {
        self.color(fg, self.theme.bg)?;
        self.raw(tag)?;
        self.color(self.theme.text, self.theme.bg)?;
        self.line(msg)
    }

    fn rule(&mut self, n: usize) -> Result<(), Status> {
        const DASH: &str = "─";
        for _ in 0..n { self.raw(DASH)?; }
        self.line("")
    }
}
