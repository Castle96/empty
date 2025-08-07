#![no_std]
#![no_main]
#![allow(dead_code)]
#![allow(static_mut_refs)]

use core::arch::asm;
use core::panic::PanicInfo;

// --- VGA text mode constants and statics ---
const BUFFER_WIDTH: usize = 80;
const BUFFER_HEIGHT: usize = 25;
static mut VGA_BUFFER: *mut u8 = 0xb8000 as *mut u8;
static mut CURSOR_ROW: usize = 0;
static mut CURSOR_COL: usize = 0;

// --- VGA text mode functions ---
fn vga_clear() {
    unsafe {
        for row in 0..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                let offset = row * BUFFER_WIDTH * 2 + col * 2;
                *VGA_BUFFER.add(offset) = b' ';
                *VGA_BUFFER.add(offset + 1) = 0x2f;
            }
        }
        CURSOR_ROW = 0;
        CURSOR_COL = 0;
    }
}

fn vga_print_at(s: &str, mut row: usize, mut col: usize, color: u8) {
    unsafe {
        for byte in s.bytes() {
            match byte {
                b'\n' => {
                    row += 1;
                    col = 0;
                }
                b'\r' => col = 0,
                b => {
                    if row >= BUFFER_HEIGHT {
                        vga_scroll();
                        row = BUFFER_HEIGHT - 1;
                    }
                    let offset = row * BUFFER_WIDTH * 2 + col * 2;
                    *VGA_BUFFER.add(offset) = b;
                    *VGA_BUFFER.add(offset + 1) = color;
                    col += 1;
                    if col >= BUFFER_WIDTH {
                        row += 1;
                        col = 0;
                    }
                }
            }
        }
        CURSOR_ROW = row;
        CURSOR_COL = col;
    }
}

fn vga_scroll() {
    unsafe {
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                let from = row * BUFFER_WIDTH * 2 + col * 2;
                let to = (row - 1) * BUFFER_WIDTH * 2 + col * 2;
                *VGA_BUFFER.add(to) = *VGA_BUFFER.add(from);
                *VGA_BUFFER.add(to + 1) = *VGA_BUFFER.add(from + 1);
            }
        }
        for col in 0..BUFFER_WIDTH {
            let offset = (BUFFER_HEIGHT - 1) * BUFFER_WIDTH * 2 + col * 2;
            *VGA_BUFFER.add(offset) = b' ';
            *VGA_BUFFER.add(offset + 1) = 0x2f;
        }
    }
}

fn vga_print(s: &str, color: u8) {
    unsafe {
        vga_print_at(s, CURSOR_ROW, CURSOR_COL, color);
    }
}

fn vga_print_hex(num: u32, color: u8) {
    const HEX_DIGITS: &[u8; 16] = b"0123456789ABCDEF";
    let mut buf = [b'0'; 10];
    buf[0] = b'0';
    buf[1] = b'x';
    for i in 0..8 {
        buf[2 + i] = HEX_DIGITS[((num >> (28 - i * 4)) & 0xF) as usize];
    }
    let s = core::str::from_utf8(&buf).unwrap_or("");
    vga_print(s, color);
}

// --- Simple RAM-based file system ---
const MAX_FILES: usize = 4;
const MAX_FILE_SIZE: usize = 256;

struct RamFile {
    name: [u8; 16],
    data: [u8; MAX_FILE_SIZE],
    size: usize,
    used: bool,
}

static mut FILES: [RamFile; MAX_FILES] = [
    RamFile { name: [0; 16], data: [0; MAX_FILE_SIZE], size: 0, used: false },
    RamFile { name: [0; 16], data: [0; MAX_FILE_SIZE], size: 0, used: false },
    RamFile { name: [0; 16], data: [0; MAX_FILE_SIZE], size: 0, used: false },
    RamFile { name: [0; 16], data: [0; MAX_FILE_SIZE], size: 0, used: false },
];

fn file_create(name: &str) -> Option<usize> {
    unsafe {
        for (i, file) in FILES.iter_mut().enumerate() {
            if !file.used {
                let name_bytes = name.as_bytes();
                for j in 0..16.min(name_bytes.len()) {
                    file.name[j] = name_bytes[j];
                }
                file.size = 0;
                file.used = true;
                return Some(i);
            }
        }
    }
    None
}

fn file_write(idx: usize, data: &[u8]) -> bool {
    unsafe {
        if idx >= MAX_FILES || !FILES[idx].used { return false; }
        let len = data.len().min(MAX_FILE_SIZE);
        FILES[idx].data[..len].copy_from_slice(&data[..len]);
        FILES[idx].size = len;
        true
    }
}

fn file_read(idx: usize) -> Option<&'static [u8]> {
    unsafe {
        if idx >= MAX_FILES || !FILES[idx].used { return None; }
        Some(&FILES[idx].data[..FILES[idx].size])
    }
}

fn file_find(name: &str) -> Option<usize> {
    unsafe {
        for (i, file) in FILES.iter().enumerate() {
            if file.used {
                let file_name = core::str::from_utf8(&file.name).unwrap_or("").trim_end_matches('\0');
                if file_name == name {
                    return Some(i);
                }
            }
        }
    }
    None
}

// --- Graphics mode 13h (320x200x256) ---
const FB_ADDR: *mut u8 = 0xA0000 as *mut u8;
const FB_WIDTH: usize = 320;
const FB_HEIGHT: usize = 200;

const VGA_MISC_WRITE: u16 = 0x3C2;
const VGA_CRTC_INDEX: u16 = 0x3D4;
const VGA_CRTC_DATA: u16 = 0x3D5;
const VGA_SEQ_INDEX: u16 = 0x3C4;
const VGA_SEQ_DATA: u16 = 0x3C5;
const VGA_GC_INDEX: u16 = 0x3CE;
const VGA_GC_DATA: u16 = 0x3CF;

fn init_graphics_mode() {
    unsafe {
        asm!("cli");
        outb(VGA_MISC_WRITE, 0x63);
        outb(VGA_SEQ_INDEX, 0x00); outb(VGA_SEQ_DATA, 0x03);
        outb(VGA_SEQ_INDEX, 0x01); outb(VGA_SEQ_DATA, 0x01);
        outb(VGA_SEQ_INDEX, 0x02); outb(VGA_SEQ_DATA, 0x0F);
        outb(VGA_SEQ_INDEX, 0x03); outb(VGA_SEQ_DATA, 0x00);
        outb(VGA_SEQ_INDEX, 0x04); outb(VGA_SEQ_DATA, 0x0E);
        outb(VGA_CRTC_INDEX, 0x11); outb(VGA_CRTC_DATA, 0x0E);
        outb(VGA_CRTC_INDEX, 0x00); outb(VGA_CRTC_DATA, 0x5F);
        outb(VGA_CRTC_INDEX, 0x01); outb(VGA_CRTC_DATA, 0x4F);
        outb(VGA_CRTC_INDEX, 0x02); outb(VGA_CRTC_DATA, 0x50);
        outb(VGA_CRTC_INDEX, 0x03); outb(VGA_CRTC_DATA, 0x82);
        outb(VGA_CRTC_INDEX, 0x04); outb(VGA_CRTC_DATA, 0x54);
        outb(VGA_CRTC_INDEX, 0x05); outb(VGA_CRTC_DATA, 0x80);
        outb(VGA_CRTC_INDEX, 0x06); outb(VGA_CRTC_DATA, 0xBF);
        outb(VGA_CRTC_INDEX, 0x07); outb(VGA_CRTC_DATA, 0x1F);
        outb(VGA_CRTC_INDEX, 0x08); outb(VGA_CRTC_DATA, 0x00);
        outb(VGA_CRTC_INDEX, 0x09); outb(VGA_CRTC_DATA, 0x41);
        outb(VGA_CRTC_INDEX, 0x10); outb(VGA_CRTC_DATA, 0x9C);
        outb(VGA_CRTC_INDEX, 0x11); outb(VGA_CRTC_DATA, 0x8E);
        outb(VGA_CRTC_INDEX, 0x12); outb(VGA_CRTC_DATA, 0x8F);
        outb(VGA_CRTC_INDEX, 0x13); outb(VGA_CRTC_DATA, 0x28);
        outb(VGA_CRTC_INDEX, 0x14); outb(VGA_CRTC_DATA, 0x40);
        outb(VGA_CRTC_INDEX, 0x15); outb(VGA_CRTC_DATA, 0x96);
        outb(VGA_CRTC_INDEX, 0x16); outb(VGA_CRTC_DATA, 0xB9);
        outb(VGA_CRTC_INDEX, 0x17); outb(VGA_CRTC_DATA, 0xA3);
        outb(VGA_GC_INDEX, 0x00); outb(VGA_GC_DATA, 0x00);
        outb(VGA_GC_INDEX, 0x01); outb(VGA_GC_DATA, 0x00);
        outb(VGA_GC_INDEX, 0x02); outb(VGA_GC_DATA, 0x00);
        outb(VGA_GC_INDEX, 0x03); outb(VGA_GC_DATA, 0x00);
        outb(VGA_GC_INDEX, 0x04); outb(VGA_GC_DATA, 0x00);
        outb(VGA_GC_INDEX, 0x05); outb(VGA_GC_DATA, 0x40);
        outb(VGA_GC_INDEX, 0x06); outb(VGA_GC_DATA, 0x05);
        outb(VGA_GC_INDEX, 0x07); outb(VGA_GC_DATA, 0x0F);
        outb(VGA_GC_INDEX, 0x08); outb(VGA_GC_DATA, 0xFF);
        asm!("sti");
    }
}

#[inline]
unsafe fn outb(port: u16, val: u8) {
    asm!("out dx, al", in("dx") port, in("al") val);
}

fn fb_clear(color: u8) {
    unsafe {
        for i in 0..(FB_WIDTH * FB_HEIGHT) {
            *FB_ADDR.add(i) = color;
        }
    }
}

fn fb_set_pixel(x: usize, y: usize, color: u8) {
    if x < FB_WIDTH && y < FB_HEIGHT {
        unsafe {
            *FB_ADDR.add(y * FB_WIDTH + x) = color;
        }
    }
}

fn fb_draw_rect(x: usize, y: usize, w: usize, h: usize, color: u8) {
    for dy in 0..h {
        for dx in 0..w {
            fb_set_pixel(x + dx, y + dy, color);
        }
    }
}

fn fb_draw_line(mut x0: isize, mut y0: isize, x1: isize, y1: isize, color: u8) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let (w, h) = (FB_WIDTH as isize, FB_HEIGHT as isize);
    loop {
        if x0 >= 0 && x0 < w && y0 >= 0 && y0 < h {
            fb_set_pixel(x0 as usize, y0 as usize, color);
        }
        if x0 == x1 && y0 == y1 { break; }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn fb_draw_circle(cx: usize, cy: usize, radius: usize, color: u8) {
    let (w, h) = (FB_WIDTH as isize, FB_HEIGHT as isize);
    let (mut x, mut y) = (radius as isize, 0isize);
    let mut err = 0isize;
    let cx = cx as isize;
    let cy = cy as isize;
    while x >= y {
        let points = [
            (cx + x, cy + y), (cx + y, cy + x), (cx - y, cy + x), (cx - x, cy + y),
            (cx - x, cy - y), (cx - y, cy - x), (cx + y, cy - x), (cx + x, cy - y),
        ];
        for &(px, py) in &points {
            if px >= 0 && px < w && py >= 0 && py < h {
                fb_set_pixel(px as usize, py as usize, color);
            }
        }
        y += 1;
        if err <= 0 {
            err += 2 * y + 1;
        } else {
            x -= 1;
            err -= 2 * x + 1;
        }
    }
}

fn fb_blit_bitmap(x: usize, y: usize, w: usize, h: usize, bitmap: &[u8], color: u8) {
    for row in 0..h {
        for col in 0..w {
            let byte_idx = (row * ((w + 7) / 8)) + (col / 8);
            let bit = 7 - (col % 8);
            if byte_idx < bitmap.len() && (bitmap[byte_idx] & (1 << bit)) != 0 {
                fb_set_pixel(x + col, y + row, color);
            }
        }
    }
}

static FONT_OK: [[u8; 8]; 2] = [
    [0x3C, 0x42, 0x81, 0x81, 0x81, 0x81, 0x42, 0x3C],
    [0x81, 0x82, 0x84, 0x88, 0xF0, 0x88, 0x84, 0x82],
];

// --- Minimal PS/2 keyboard input ---
fn keyboard_poll() -> Option<u8> {
    let mut scancode = None;
    unsafe {
        let mut status: u8;
        asm!("in al, dx", in("dx") 0x64u16, out("al") status);
        if status & 1 != 0 {
            let mut code: u8;
            asm!("in al, dx", in("dx") 0x60u16, out("al") code);
            scancode = Some(code);
        }
    }
    scancode
}

// --- Simple bump allocator for heap memory ---
static mut BUMP_PTR: usize = 0;
static mut BUMP_END: usize = 0;

pub unsafe fn bump_init(start: usize, end: usize) {
    BUMP_PTR = start;
    BUMP_END = end;
}

pub unsafe fn bump_alloc(size: usize) -> *mut u8 {
    let align = 8;
    let size = (size + align - 1) & !(align - 1);
    if BUMP_PTR + size > BUMP_END {
        core::ptr::null_mut()
    } else {
        let ptr = BUMP_PTR as *mut u8;
        BUMP_PTR += size;
        ptr
    }
}

// --- Halt the CPU ---
fn halt() -> ! {
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}

// --- 64-bit entry point called from boot.asm after long mode switch ---
#[no_mangle]
#[unsafe(naked)]
pub unsafe extern "C" fn long_mode_start() -> ! {
    core::arch::naked_asm!(
        "mov rsp, 0x90000",
        "call _start",
        "hlt"
    );
}

// --- Kernel main entry point ---
#[no_mangle]
pub extern "C" fn _start() -> ! {
    init_idt();
    vga_clear();
    vga_print("Welcome to your Rust OS kernel!\n", 0x2f);
    vga_print("Text mode is working.\n", 0x2f);
    vga_print("Testing heap allocation...\n", 0x2f);
    unsafe {
        bump_init(0x100000, 0x200000);
        let ptr1 = bump_alloc(64);
        let ptr2 = bump_alloc(128);
        if !ptr1.is_null() && !ptr2.is_null() {
            vga_print("Heap allocation OK\n", 0x2f);
        } else {
            vga_print("Heap allocation FAILED\n", 0x4f);
        }
    }
    vga_print("Switching to graphics mode...\n", 0x2f);
    init_graphics_mode();
    fb_clear(0x11);
    // TEST: Draw a test pixel and color bars to verify framebuffer output
    fb_set_pixel(10, 10, 0x3F); // bright gray pixel
    for y in 0..FB_HEIGHT {
        for x in 0..FB_WIDTH {
            if x < 32 {
                fb_set_pixel(x, y, (x % 16) as u8);
            }
        }
    }
    // --- GUI demo ---
    let win_x = 60;
    let win_y = 40;
    let win_w = 200;
    let win_h = 120;
    fb_draw_rect(win_x, win_y, win_w, win_h, 0x3F);
    fb_draw_rect(win_x, win_y, win_w, 2, 0x00);
    fb_draw_rect(win_x, win_y + win_h - 2, win_w, 2, 0x00);
    fb_draw_rect(win_x, win_y, 2, win_h, 0x00);
    fb_draw_rect(win_x + win_w - 2, win_y, 2, win_h, 0x00);
    fb_draw_rect(win_x + 2, win_y + 2, win_w - 4, 16, 0x19);
    let btn_x = win_x + 30;
    let btn_y = win_y + win_h - 40;
    let btn_w = 60;
    let btn_h = 24;
    fb_draw_rect(btn_x, btn_y, btn_w, btn_h, 0x2A);
    fb_draw_rect(btn_x, btn_y, btn_w, 2, 0x00);
    fb_draw_rect(btn_x, btn_y + btn_h - 2, btn_w, 2, 0x00);
    fb_draw_rect(btn_x, btn_y, 2, btn_h, 0x00);
    fb_draw_rect(btn_x + btn_w - 2, btn_y, 2, btn_h, 0x00);
    fb_blit_bitmap(btn_x + 18, btn_y + 8, 8, 8, &FONT_OK[0], 0x00);
    fb_blit_bitmap(btn_x + 30, btn_y + 8, 8, 8, &FONT_OK[1], 0x00);
    loop {
        if let Some(sc) = keyboard_poll() {
            if sc == 0x01 {
                break;
            }
        }
    }
    halt();
}

// --- Miniqemu-system-x86_64 -cdrom build/os-x86_64.iso -vga stdmal 64-bit IDT entry (interrupt gate, present, DPL=0) ---
#[repr(C, packed)]
#[derive(Copy, Clone)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    type_attr: u8,
    offset_mid: u16,
    offset_high: u32,
    zero: u32,
}

#[repr(C, align(16))]
struct Idt([IdtEntry; 256]);

static mut IDT: Idt = Idt([IdtEntry {
    offset_low: 0,
    selector: 0,
    ist: 0,
    type_attr: 0,
    offset_mid: 0,
    offset_high: 0,
    zero: 0,
}; 256]);

extern "C" fn default_handler() {
    loop {
        unsafe { core::arch::asm!("hlt", options(nomem, nostack, preserves_flags)); }
    }
}

unsafe fn set_idt_entry(idx: usize, handler: extern "C" fn()) {
    let addr = handler as u64;
    IDT.0[idx] = IdtEntry {
        offset_low: addr as u16,
        selector: 0x08,
        ist: 0,
        type_attr: 0x8E,
        offset_mid: (addr >> 16) as u16,
        offset_high: (addr >> 32) as u32,
        zero: 0,
    };
}

#[repr(C, packed)]
struct IdtPtr {
    limit: u16,
    base: u64,
}

#[no_mangle]
pub extern "C" fn init_idt() {
    unsafe {
        for i in 0..256 {
            set_idt_entry(i, default_handler);
        }
        let idt_ptr = IdtPtr {
            limit: core::mem::size_of::<Idt>() as u16 - 1,
            base: &IDT as *const _ as u64,
        };
        core::arch::asm!(
            "lidt [{}]", in(reg) &idt_ptr, options(readonly, nostack, preserves_flags)
        );
    }
}

// --- Custom panic handler (must be last) ---
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    vga_clear();
    unsafe {
        CURSOR_ROW = 0;
        CURSOR_COL = 0;
    }
    vga_print("KERNEL PANIC!\n", 0x4f);
    halt();
}
