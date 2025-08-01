//! NØNOS Boot UI Module - Advanced Terminal UX Framework for ZeroState Boot
//!
//! This interface defines early-stage terminal visuals rendered via UEFI.
//! It provides highly structured, capability-aware diagnostics during the
//! ZeroState environment spin-up. All output is routed through UEFI TextOutput.
//!
//! # Key Features
//! - Themed boot splash with edition + runtime metadata
//! - Color-coded log levels (info, warn, fail)
//! - Fault-detection display for secure capsule boot
//! - Future: interactive recovery menu, multi-capsule select, touch-ready
//!
//! Designed for seamless integration with secure loader + audit trail.

use uefi::proto::console::text::{Color, TextOutput};
use uefi::table::SystemTable;
use uefi::ResultExt;

/// Render the top-level ZeroState boot splash
pub fn draw_boot_banner() {
    let st = unsafe { SystemTable::load() };
    let con_out: &mut TextOutput = st.stdout();

    let _ = con_out.set_color(Color::LightCyan, Color::Black);
    let _ = con_out.clear();

    let banner = [
        "\r\n",
        "              ╔═════════════════════════════════════════════════════════════╗\r\n",
        "              ║                 NØNOS OS :: ZERO-STATE LAUNCHPAD            ║\r\n",
        "              ║           --- Privacy-Native / Identity-Free Kernel ---     ║\r\n",
        "              ║            Edition :: Dev Alpha — Built for Capability       ║\r\n",
        "              ║       Runtime :: UEFI Boot + In-Memory Capsule Dispatch      ║\r\n",
        "              ╚═════════════════════════════════════════════════════════════╝\r\n",
        "\r\n",
    ];

    for line in banner.iter() {
        let _ = con_out.output_string(line);
    }

    let _ = con_out.set_color(Color::Gray, Color::Black);
    let _ = con_out.output_string("     [✓] capsule loader staged :: awaiting kernel envelope\r\n");
    let _ = con_out.set_color(Color::White, Color::Black);
}

/// Display a structured red failure block with reason
pub fn display_failure(msg: &str) {
    let st = unsafe { SystemTable::load() };
    let con_out: &mut TextOutput = st.stdout();

    let _ = con_out.set_color(Color::Red, Color::Black);
    let _ = con_out.output_string("\r\n──────────────────── SYSTEM FAULT DETECTED ────────────────────\r\n");
    let _ = con_out.output_string("[!]: ");
    let _ = con_out.output_string(msg);
    let _ = con_out.output_string("\r\n────────────────────────────────────────────────────────────────\r\n");

    let _ = con_out.set_color(Color::White, Color::Black);
}

/// Emit info-level system telemetry in gray
pub fn log_info(line: &str) {
    let st = unsafe { SystemTable::load() };
    let con_out: &mut TextOutput = st.stdout();

    let _ = con_out.set_color(Color::Gray, Color::Black);
    let _ = con_out.output_string("[info] ");
    let _ = con_out.output_string(line);
    let _ = con_out.output_string("\r\n");
    let _ = con_out.set_color(Color::White, Color::Black);
}

/// Emit warning messages in yellow
pub fn log_warn(line: &str) {
    let st = unsafe { SystemTable::load() };
    let con_out: &mut TextOutput = st.stdout();

    let _ = con_out.set_color(Color::Yellow, Color::Black);
    let _ = con_out.output_string("[warn] ");
    let _ = con_out.output_string(line);
    let _ = con_out.output_string("\r\n");
    let _ = con_out.set_color(Color::White, Color::Black);
}

/// Emit system panic message in red (fallback display)
pub fn log_panic(msg: &str) {
    let st = unsafe { SystemTable::load() };
    let con_out: &mut TextOutput = st.stdout();

    let _ = con_out.set_color(Color::Red, Color::Black);
    let _ = con_out.output_string("[PANIC] :: ");
    let _ = con_out.output_string(msg);
    let _ = con_out.output_string("\r\n");
    let _ = con_out.set_color(Color::White, Color::Black);
}

