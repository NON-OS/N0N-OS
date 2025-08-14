/* kernel/linker.ld */
/* NØNOS Kernel Linker Script for x86_64 */

ENTRY(_start)
OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)

/* Define kernel virtual base address (higher-half) */
KERNEL_BASE = 0xFFFFFFFF80000000;

SECTIONS
{
    /* Start at 2MB to avoid conflicts with bootloader */
    . = KERNEL_BASE + 0x200000;
    
    /* Store kernel start address */
    __kernel_start = .;
    
    /* Code section - Read + Execute */
    .text ALIGN(4K) : AT(ADDR(.text) - KERNEL_BASE)
    {
        __kernel_text_start = .;
        *(.text.boot)           /* Boot code first */
        *(.text .text.*)
        __kernel_text_end = .;
    }
    
    /* Read-only data - Read only */
    .rodata ALIGN(4K) : AT(ADDR(.rodata) - KERNEL_BASE)
    {
        __kernel_rodata_start = .;
        *(.rodata .rodata.*)
        
        /* Ensure string constants are here */
        *(.rodata.str1.1)
        *(.rodata.str1.4)
        *(.rodata.str1.8)
        __kernel_rodata_end = .;
    }
    
    /* Initialized data - Read + Write */
    .data ALIGN(4K) : AT(ADDR(.data) - KERNEL_BASE)
    {
        __kernel_data_start = .;
        *(.data .data.*)
        
        /* Global Offset Table */
        *(.got)
        *(.got.plt)
        __kernel_data_end = .;
    }
    
    /* Per-CPU data section */
    .percpu ALIGN(4K) : AT(ADDR(.percpu) - KERNEL_BASE)
    {
        __percpu_start = .;
        *(.percpu .percpu.*)
        __percpu_end = .;
    }
    
    /* Uninitialized data - Read + Write */
    .bss ALIGN(4K) : AT(ADDR(.bss) - KERNEL_BASE)
    {
        __kernel_bss_start = .;
        *(COMMON)
        *(.bss .bss.*)
        . = ALIGN(8);
        __kernel_bss_end = .;
    }
    
    /* Boot-time stacks (will be reclaimed) */
    .boot_stack ALIGN(4K) : AT(ADDR(.boot_stack) - KERNEL_BASE)
    {
        __boot_stacks_start = .;
        . = . + 0x4000; /* 16KB boot stack */
        __boot_stack_top = .;
        __boot_stacks_end = .;
    }
    
    /* Store kernel end address */
    __kernel_end = .;
    
    /* Discard sections we don't need */
    /DISCARD/ : {
        *(.eh_frame*)
        *(.note*)
        *(.comment*)
    }
}
