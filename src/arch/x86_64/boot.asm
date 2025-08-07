
; Multiboot2-compliant 32-bit entry for x86_64 kernel
; Switches to long mode and jumps to Rust _start

global multiboot_entry
extern long_mode_start

section .text
bits 32

multiboot_entry:
    ; Set up stack (temporary, 16 KiB at 0x90000)
    mov esp, 0x90000

    ; Set up GDT for long mode
    lgdt [gdt64_ptr]

    ; Set up minimal identity-mapped page tables in low memory
    ; PML4 at 0x8000, PDPT at 0x9000, PD at 0xA000
    xor eax, eax
    mov edi, 0x8000
    mov ecx, 0x1000 * 3 / 4 ; clear 3 pages (PML4, PDPT, PD)
    rep stosd

    ; PML4[0] -> PDPT
    mov dword [0x8000], 0x9003
    ; PDPT[0] -> PD
    mov dword [0x9000], 0xA003
    ; PD[0] -> 1GiB 2MiB pages, present|write|PS
    mov eax, 0x83 ; present|write|PS
    mov ecx, 512
    mov edi, 0xA000
.set_pd:
    mov dword [edi], eax
    add eax, 0x200000 ; next 2MiB
    add edi, 8
    loop .set_pd

    ; Load PML4 into CR3
    mov eax, 0x8000
    mov cr3, eax

    ; Enable PAE
    mov eax, cr4
    or eax, 1 << 5
    mov cr4, eax

    ; Enable long mode (LME) in EFER
    mov ecx, 0xC0000080
    rdmsr
    or eax, 1 << 8
    wrmsr

    ; Enable paging
    mov eax, cr0
    or eax, 0x80000000
    mov cr0, eax

    ; Far jump to 64-bit code segment
    jmp 0x08:long_mode_start

align 16

; 64-bit GDT
section .rodata
align 8
gdt64:
    dq 0x0000000000000000 ; null
    dq 0x0020980000000000 ; code segment
    dq 0x0000920000000000 ; data segment
gdt64_ptr:
    dw gdt64_end - gdt64 - 1
    dq gdt64
gdt64_end:

; Mark stack as non-executable for GNU ld compatibility
section .note.GNU-stack noalloc noexec nowrite progbits
