
ENTRY(multiboot_entry)

PHDRS
{
    boot PT_LOAD FLAGS(0x4) ; /* R */
    text PT_LOAD FLAGS(0x5) ; /* R X */
    data PT_LOAD FLAGS(0x6) ; /* R W */
}

SECTIONS
{
    . = 1M;

    /* Multiboot header must be in the first 8 KiB */
    .boot ALIGN(4) : {
        KEEP(*(.multiboot_header))
    } :boot

    .text ALIGN(4) : {
        *(.text*)
    } :text

    .rodata ALIGN(4) : {
        *(.rodata*)
    } :text

    .data ALIGN(4) : {
        *(.data*)
    } :data

    .bss ALIGN(4) (NOLOAD) : {
        *(.bss*)
        *(COMMON)
    } :data
}
