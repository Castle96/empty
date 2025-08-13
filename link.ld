/* Minimal linker script for a Rust kernel */
ENTRY(_start)

SECTIONS
{
    . = 1M;
    .text : {
        *(.text*)
    }
    .rodata : {
        *(.rodata*)
    }
    .data : {
        *(.data*)
    }
    .bss : {
        *(.bss*)
        *(COMMON)
    }
    /DISCARD/ : {
        *(.eh_frame*)
    }
}
