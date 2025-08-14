//! x86_64 Architecture Support

pub mod boot;
pub mod gdt;
pub mod idt;
pub mod serial;
pub mod vga;

pub mod interrupt {
    pub mod apic;
    pub mod ioapic;
    pub mod pic_legacy;
}

pub mod keyboard {
    pub mod mod;
}

pub mod time {
    pub mod timer;
}

// Port I/O utilities
pub mod port {
    #[inline(always)]
    pub unsafe fn inb(port: u16) -> u8 {
        let value: u8;
        core::arch::asm!("in al, dx", out("al") value, in("dx") port, options(nomem, nostack, preserves_flags));
        value
    }
    
    #[inline(always)]
    pub unsafe fn outb(port: u16, value: u8) {
        core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
    }
}

// Framebuffer support stub
pub mod framebuffer {
    #[derive(Clone, Copy)]
    pub struct FbInfo {
        pub ptr: usize,
        pub width: u32,
        pub height: u32,
        pub stride: u32,
    }
    
    pub fn probe() -> Option<FbInfo> {
        None // Would be populated from bootloader info
    }
}

// Font support for framebuffer console
pub mod font8x16 {
    pub fn glyph(c: u8) -> &'static [u8; 16] {
        // Simplified: return a blank glyph
        &[0; 16]
    }
}
