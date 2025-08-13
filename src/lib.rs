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

// --- Enhanced Graphics System ---
const FB_ADDR: *mut u8 = 0xA0000 as *mut u8;
const FB_WIDTH: usize = 320;
const FB_HEIGHT: usize = 200;
const FB_SIZE: usize = FB_WIDTH * FB_HEIGHT;

// Double buffering - back buffer in memory
static mut BACK_BUFFER: [u8; FB_SIZE] = [0; FB_SIZE];
static mut DOUBLE_BUFFER_ENABLED: bool = false;

// Video mode information
#[derive(Copy, Clone)]
struct VideoMode {
    width: usize,
    height: usize,
    bpp: u8,  // bits per pixel
    mode_id: u8,
}

const VIDEO_MODES: [VideoMode; 3] = [
    VideoMode { width: 320, height: 200, bpp: 8, mode_id: 0x13 }, // Mode 13h
    VideoMode { width: 640, height: 480, bpp: 1, mode_id: 0x12 }, // Mode 12h (VGA)
    VideoMode { width: 80, height: 25, bpp: 4, mode_id: 0x03 },   // Text mode
];

static mut CURRENT_MODE: VideoMode = VideoMode { width: 320, height: 200, bpp: 8, mode_id: 0x13 };

// Sprite structure for better sprite handling
#[derive(Copy, Clone)]
struct Sprite {
    width: usize,
    height: usize,
    transparent_color: u8, // Color index to treat as transparent
}

// Animation frame structure
#[derive(Clone)]
struct AnimationFrame<'a> {
    sprite_data: &'a [&'a str],
    duration_ms: u32,
}

// Simple timer for animations (frame counter)
static mut FRAME_COUNTER: u32 = 0;

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

// --- Basic ASCII font data (8x8 bitmap font for printable characters) ---
fn get_font_char(c: u8) -> [u8; 8] {
    match c {
        b' ' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], // space
        b'!' => [0x30, 0x78, 0x78, 0x30, 0x30, 0x00, 0x30, 0x00], // !
        b'"' => [0x6C, 0x6C, 0x6C, 0x00, 0x00, 0x00, 0x00, 0x00], // "
        b'#' => [0x6C, 0x6C, 0xFE, 0x6C, 0xFE, 0x6C, 0x6C, 0x00], // #
        b'$' => [0x30, 0x7C, 0xC0, 0x78, 0x0C, 0xF8, 0x30, 0x00], // $
        b'%' => [0x00, 0xC6, 0xCC, 0x18, 0x30, 0x66, 0xC6, 0x00], // %
        b'&' => [0x38, 0x6C, 0x38, 0x76, 0xDC, 0xCC, 0x76, 0x00], // &
        b'\'' => [0x60, 0x60, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00], // '
        b'(' => [0x18, 0x30, 0x60, 0x60, 0x60, 0x30, 0x18, 0x00], // (
        b')' => [0x60, 0x30, 0x18, 0x18, 0x18, 0x30, 0x60, 0x00], // )
        b'*' => [0x00, 0x66, 0x3C, 0xFF, 0x3C, 0x66, 0x00, 0x00], // *
        b'+' => [0x00, 0x30, 0x30, 0xFC, 0x30, 0x30, 0x00, 0x00], // +
        b',' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x30, 0x30, 0x60], // ,
        b'-' => [0x00, 0x00, 0x00, 0xFC, 0x00, 0x00, 0x00, 0x00], // -
        b'.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x30, 0x30, 0x00], // .
        b'/' => [0x06, 0x0C, 0x18, 0x30, 0x60, 0xC0, 0x80, 0x00], // /
        // Numbers 0-9
        b'0' => [0x7C, 0xC6, 0xCE, 0xDE, 0xF6, 0xE6, 0x7C, 0x00],
        b'1' => [0x30, 0x70, 0x30, 0x30, 0x30, 0x30, 0xFC, 0x00],
        b'2' => [0x78, 0xCC, 0x0C, 0x38, 0x60, 0xCC, 0xFC, 0x00],
        b'3' => [0x78, 0xCC, 0x0C, 0x38, 0x0C, 0xCC, 0x78, 0x00],
        b'4' => [0x1C, 0x3C, 0x6C, 0xCC, 0xFE, 0x0C, 0x1E, 0x00],
        b'5' => [0xFC, 0xC0, 0xF8, 0x0C, 0x0C, 0xCC, 0x78, 0x00],
        b'6' => [0x38, 0x60, 0xC0, 0xF8, 0xCC, 0xCC, 0x78, 0x00],
        b'7' => [0xFC, 0xCC, 0x0C, 0x18, 0x30, 0x30, 0x30, 0x00],
        b'8' => [0x78, 0xCC, 0xCC, 0x78, 0xCC, 0xCC, 0x78, 0x00],
        b'9' => [0x78, 0xCC, 0xCC, 0x7C, 0x0C, 0x18, 0x70, 0x00],
        b':' => [0x00, 0x30, 0x30, 0x00, 0x00, 0x30, 0x30, 0x00],
        b';' => [0x00, 0x30, 0x30, 0x00, 0x00, 0x30, 0x30, 0x60],
        b'<' => [0x18, 0x30, 0x60, 0xC0, 0x60, 0x30, 0x18, 0x00],
        b'=' => [0x00, 0x00, 0xFC, 0x00, 0x00, 0xFC, 0x00, 0x00],
        b'>' => [0x60, 0x30, 0x18, 0x0C, 0x18, 0x30, 0x60, 0x00],
        b'?' => [0x78, 0xCC, 0x0C, 0x18, 0x30, 0x00, 0x30, 0x00],
        b'@' => [0x7C, 0xC6, 0xDE, 0xDE, 0xDE, 0xC0, 0x78, 0x00],
        // Uppercase A-Z
        b'A' => [0x30, 0x78, 0xCC, 0xCC, 0xFC, 0xCC, 0xCC, 0x00],
        b'B' => [0xFC, 0x66, 0x66, 0x7C, 0x66, 0x66, 0xFC, 0x00],
        b'C' => [0x3C, 0x66, 0xC0, 0xC0, 0xC0, 0x66, 0x3C, 0x00],
        b'D' => [0xF8, 0x6C, 0x66, 0x66, 0x66, 0x6C, 0xF8, 0x00],
        b'E' => [0xFE, 0x62, 0x68, 0x78, 0x68, 0x62, 0xFE, 0x00],
        b'F' => [0xFE, 0x62, 0x68, 0x78, 0x68, 0x60, 0xF0, 0x00],
        b'G' => [0x3C, 0x66, 0xC0, 0xC0, 0xCE, 0x66, 0x3E, 0x00],
        b'H' => [0xCC, 0xCC, 0xCC, 0xFC, 0xCC, 0xCC, 0xCC, 0x00],
        b'I' => [0x78, 0x30, 0x30, 0x30, 0x30, 0x30, 0x78, 0x00],
        b'J' => [0x1E, 0x0C, 0x0C, 0x0C, 0xCC, 0xCC, 0x78, 0x00],
        b'K' => [0xE6, 0x66, 0x6C, 0x78, 0x6C, 0x66, 0xE6, 0x00],
        b'L' => [0xF0, 0x60, 0x60, 0x60, 0x62, 0x66, 0xFE, 0x00],
        b'M' => [0xC6, 0xEE, 0xFE, 0xFE, 0xD6, 0xC6, 0xC6, 0x00],
        b'N' => [0xC6, 0xE6, 0xF6, 0xDE, 0xCE, 0xC6, 0xC6, 0x00],
        b'O' => [0x38, 0x6C, 0xC6, 0xC6, 0xC6, 0x6C, 0x38, 0x00],
        b'P' => [0xFC, 0x66, 0x66, 0x7C, 0x60, 0x60, 0xF0, 0x00],
        b'Q' => [0x78, 0xCC, 0xCC, 0xCC, 0xDC, 0x78, 0x1C, 0x00],
        b'R' => [0xFC, 0x66, 0x66, 0x7C, 0x6C, 0x66, 0xE6, 0x00],
        b'S' => [0x78, 0xCC, 0xE0, 0x70, 0x1C, 0xCC, 0x78, 0x00],
        b'T' => [0xFC, 0xB4, 0x30, 0x30, 0x30, 0x30, 0x78, 0x00],
        b'U' => [0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xFC, 0x00],
        b'V' => [0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0x78, 0x30, 0x00],
        b'W' => [0xC6, 0xC6, 0xC6, 0xD6, 0xFE, 0xEE, 0xC6, 0x00],
        b'X' => [0xC6, 0xC6, 0x6C, 0x38, 0x38, 0x6C, 0xC6, 0x00],
        b'Y' => [0xCC, 0xCC, 0xCC, 0x78, 0x30, 0x30, 0x78, 0x00],
        b'Z' => [0xFE, 0xC6, 0x8C, 0x18, 0x32, 0x66, 0xFE, 0x00],
        // Lowercase a-z
        b'a' => [0x00, 0x00, 0x78, 0x0C, 0x7C, 0xCC, 0x76, 0x00],
        b'b' => [0xE0, 0x60, 0x60, 0x7C, 0x66, 0x66, 0xDC, 0x00],
        b'c' => [0x00, 0x00, 0x78, 0xCC, 0xC0, 0xCC, 0x78, 0x00],
        b'd' => [0x1C, 0x0C, 0x0C, 0x7C, 0xCC, 0xCC, 0x76, 0x00],
        b'e' => [0x00, 0x00, 0x78, 0xCC, 0xFC, 0xC0, 0x78, 0x00],
        b'f' => [0x38, 0x6C, 0x60, 0xF0, 0x60, 0x60, 0xF0, 0x00],
        b'g' => [0x00, 0x00, 0x76, 0xCC, 0xCC, 0x7C, 0x0C, 0xF8],
        b'h' => [0xE0, 0x60, 0x6C, 0x76, 0x66, 0x66, 0xE6, 0x00],
        b'i' => [0x30, 0x00, 0x70, 0x30, 0x30, 0x30, 0x78, 0x00],
        b'j' => [0x0C, 0x00, 0x0C, 0x0C, 0x0C, 0xCC, 0xCC, 0x78],
        b'k' => [0xE0, 0x60, 0x66, 0x6C, 0x78, 0x6C, 0xE6, 0x00],
        b'l' => [0x70, 0x30, 0x30, 0x30, 0x30, 0x30, 0x78, 0x00],
        b'm' => [0x00, 0x00, 0xCC, 0xFE, 0xFE, 0xD6, 0xC6, 0x00],
        b'n' => [0x00, 0x00, 0xF8, 0xCC, 0xCC, 0xCC, 0xCC, 0x00],
        b'o' => [0x00, 0x00, 0x78, 0xCC, 0xCC, 0xCC, 0x78, 0x00],
        b'p' => [0x00, 0x00, 0xDC, 0x66, 0x66, 0x7C, 0x60, 0xF0],
        b'q' => [0x00, 0x00, 0x76, 0xCC, 0xCC, 0x7C, 0x0C, 0x1E],
        b'r' => [0x00, 0x00, 0xDC, 0x76, 0x66, 0x60, 0xF0, 0x00],
        b's' => [0x00, 0x00, 0x7C, 0xC0, 0x78, 0x0C, 0xF8, 0x00],
        b't' => [0x10, 0x30, 0x7C, 0x30, 0x30, 0x34, 0x18, 0x00],
        b'u' => [0x00, 0x00, 0xCC, 0xCC, 0xCC, 0xCC, 0x76, 0x00],
        b'v' => [0x00, 0x00, 0xCC, 0xCC, 0xCC, 0x78, 0x30, 0x00],
        b'w' => [0x00, 0x00, 0xC6, 0xD6, 0xFE, 0xFE, 0x6C, 0x00],
        b'x' => [0x00, 0x00, 0xC6, 0x6C, 0x38, 0x6C, 0xC6, 0x00],
        b'y' => [0x00, 0x00, 0xCC, 0xCC, 0xCC, 0x7C, 0x0C, 0xF8],
        b'z' => [0x00, 0x00, 0xFC, 0x98, 0x30, 0x64, 0xFC, 0x00],
        _ => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], // default for unsupported chars
    }
}

static FONT_OK: [[u8; 8]; 2] = [
    [0x3C, 0x42, 0x81, 0x81, 0x81, 0x81, 0x42, 0x3C],
    [0x81, 0x82, 0x84, 0x88, 0xF0, 0x88, 0x84, 0x82],
];

// --- Enhanced Graphics Functions ---

// Enable or disable double buffering
fn fb_enable_double_buffer(enable: bool) {
    unsafe {
        DOUBLE_BUFFER_ENABLED = enable;
        if enable {
            // Clear back buffer
            for i in 0..FB_SIZE {
                BACK_BUFFER[i] = 0;
            }
        }
    }
}

// Get the current drawing target (back buffer if double buffering, screen otherwise)
fn get_draw_buffer() -> *mut u8 {
    unsafe {
        if DOUBLE_BUFFER_ENABLED {
            BACK_BUFFER.as_mut_ptr()
        } else {
            FB_ADDR
        }
    }
}

// Swap buffers (copy back buffer to screen)
fn fb_swap_buffers() {
    unsafe {
        if DOUBLE_BUFFER_ENABLED {
            // Fast memory copy from back buffer to screen
            for i in 0..FB_SIZE {
                *FB_ADDR.add(i) = BACK_BUFFER[i];
            }
        }
    }
}

// Enhanced clear function that works with double buffering
fn fb_clear_enhanced(color: u8) {
    unsafe {
        let buffer = get_draw_buffer();
        for i in 0..FB_SIZE {
            *buffer.add(i) = color;
        }
    }
}

// Enhanced pixel setting that works with double buffering
fn fb_set_pixel_enhanced(x: usize, y: usize, color: u8) {
    if x < FB_WIDTH && y < FB_HEIGHT {
        unsafe {
            let buffer = get_draw_buffer();
            *buffer.add(y * FB_WIDTH + x) = color;
        }
    }
}

// Enhanced rectangle drawing
fn fb_draw_rect_enhanced(x: usize, y: usize, w: usize, h: usize, color: u8) {
    for dy in 0..h {
        for dx in 0..w {
            fb_set_pixel_enhanced(x + dx, y + dy, color);
        }
    }
}

// Fast horizontal line for better performance
fn fb_draw_hline(x: usize, y: usize, width: usize, color: u8) {
    if y < FB_HEIGHT {
        unsafe {
            let buffer = get_draw_buffer();
            let start = y * FB_WIDTH + x;
            let end = start + width.min(FB_WIDTH - x);
            for i in start..end {
                *buffer.add(i) = color;
            }
        }
    }
}

// Fast vertical line for better performance
fn fb_draw_vline(x: usize, y: usize, height: usize, color: u8) {
    if x < FB_WIDTH {
        unsafe {
            let buffer = get_draw_buffer();
            for dy in 0..height.min(FB_HEIGHT - y) {
                *buffer.add((y + dy) * FB_WIDTH + x) = color;
            }
        }
    }
}

// Enhanced sprite drawing with transparency support
fn fb_draw_sprite_enhanced(x: usize, y: usize, sprite: &Sprite, sprite_data: &[&str], colors: &[u8]) {
    for (row, line) in sprite_data.iter().enumerate().take(sprite.height) {
        if y + row >= FB_HEIGHT { break; }
        for (col, ch) in line.chars().enumerate().take(sprite.width) {
            if x + col >= FB_WIDTH { break; }
            if let Some(color_index) = ch.to_digit(10) {
                let color_idx = color_index as u8;
                // Check for transparency
                if color_idx != sprite.transparent_color && (color_idx as usize) < colors.len() {
                    fb_set_pixel_enhanced(x + col, y + row, colors[color_idx as usize]);
                }
            }
        }
    }
}

// Animation support - draw animated sprite
fn fb_draw_animation(x: usize, y: usize, frames: &[AnimationFrame], sprite: &Sprite, colors: &[u8]) {
    unsafe {
        // Simple frame selection based on frame counter
        let frame_idx = (FRAME_COUNTER / 10) % frames.len() as u32; // Change frame every 10 ticks
        if let Some(frame) = frames.get(frame_idx as usize) {
            fb_draw_sprite_enhanced(x, y, sprite, frame.sprite_data, colors);
        }
    }
}

// Blit one area of the screen to another (useful for scrolling)
fn fb_blit(src_x: usize, src_y: usize, dst_x: usize, dst_y: usize, w: usize, h: usize) {
    unsafe {
        let buffer = get_draw_buffer();
        for dy in 0..h {
            if src_y + dy >= FB_HEIGHT || dst_y + dy >= FB_HEIGHT { continue; }
            for dx in 0..w {
                if src_x + dx >= FB_WIDTH || dst_x + dx >= FB_WIDTH { continue; }
                let src_pixel = *buffer.add((src_y + dy) * FB_WIDTH + (src_x + dx));
                *buffer.add((dst_y + dy) * FB_WIDTH + (dst_x + dx)) = src_pixel;
            }
        }
    }
}

// Screen scrolling functions
fn fb_scroll_up(lines: usize, fill_color: u8) {
    if lines >= FB_HEIGHT {
        fb_clear_enhanced(fill_color);
        return;
    }
    
    unsafe {
        let buffer = get_draw_buffer();
        // Move pixels up
        for y in lines..FB_HEIGHT {
            for x in 0..FB_WIDTH {
                let src = y * FB_WIDTH + x;
                let dst = (y - lines) * FB_WIDTH + x;
                *buffer.add(dst) = *buffer.add(src);
            }
        }
        // Fill bottom with fill_color
        for y in (FB_HEIGHT - lines)..FB_HEIGHT {
            for x in 0..FB_WIDTH {
                *buffer.add(y * FB_WIDTH + x) = fill_color;
            }
        }
    }
}

fn fb_scroll_down(lines: usize, fill_color: u8) {
    if lines >= FB_HEIGHT {
        fb_clear_enhanced(fill_color);
        return;
    }
    
    unsafe {
        let buffer = get_draw_buffer();
        // Move pixels down (start from bottom)
        for y in (0..(FB_HEIGHT - lines)).rev() {
            for x in 0..FB_WIDTH {
                let src = y * FB_WIDTH + x;
                let dst = (y + lines) * FB_WIDTH + x;
                *buffer.add(dst) = *buffer.add(src);
            }
        }
        // Fill top with fill_color
        for y in 0..lines {
            for x in 0..FB_WIDTH {
                *buffer.add(y * FB_WIDTH + x) = fill_color;
            }
        }
    }
}

// Get pixel color at position (useful for collision detection)
fn fb_get_pixel(x: usize, y: usize) -> u8 {
    if x < FB_WIDTH && y < FB_HEIGHT {
        unsafe {
            let buffer = get_draw_buffer();
            *buffer.add(y * FB_WIDTH + x)
        }
    } else {
        0
    }
}

// Draw text using the bitmap font (enhanced version)
fn fb_draw_text_enhanced(x: usize, y: usize, text: &str, color: u8) {
    let mut char_x = x;
    let mut char_y = y;
    
    for c in text.bytes() {
        match c {
            b'\n' => {
                char_y += 8; // Move to next line
                char_x = x;  // Reset to start of line
                if char_y + 8 >= FB_HEIGHT { break; }
            }
            b'\r' => char_x = x, // Carriage return
            _ => {
                if char_x + 8 >= FB_WIDTH {
                    // Auto-wrap to next line
                    char_y += 8;
                    char_x = x;
                    if char_y + 8 >= FB_HEIGHT { break; }
                }
                let font_data = get_font_char(c);
                fb_blit_bitmap_enhanced(char_x, char_y, 8, 8, &font_data, color);
                char_x += 8; // Move to next character position
            }
        }
    }
}

// Enhanced bitmap blitting with double buffer support
fn fb_blit_bitmap_enhanced(x: usize, y: usize, w: usize, h: usize, bitmap: &[u8], color: u8) {
    for row in 0..h {
        if y + row >= FB_HEIGHT { break; }
        for col in 0..w {
            if x + col >= FB_WIDTH { break; }
            let byte_idx = (row * ((w + 7) / 8)) + (col / 8);
            let bit = 7 - (col % 8);
            if byte_idx < bitmap.len() && (bitmap[byte_idx] & (1 << bit)) != 0 {
                fb_set_pixel_enhanced(x + col, y + row, color);
            }
        }
    }
}

// Update frame counter (call this in your main loop)
fn fb_update_frame_counter() {
    unsafe {
        FRAME_COUNTER = FRAME_COUNTER.wrapping_add(1);
    }
}

// Get current frame counter value
fn fb_get_frame_counter() -> u32 {
    unsafe { FRAME_COUNTER }
}

// Draw text using the bitmap font (backward compatibility)
fn fb_draw_text(x: usize, y: usize, text: &str, color: u8) {
    fb_draw_text_enhanced(x, y, text, color);
}

// Draw a filled circle
fn fb_draw_filled_circle(cx: usize, cy: usize, radius: usize, color: u8) {
    let r_sq = (radius * radius) as isize;
    let cx = cx as isize;
    let cy = cy as isize;
    
    for y in (cy - radius as isize)..(cy + radius as isize + 1) {
        for x in (cx - radius as isize)..(cx + radius as isize + 1) {
            let dx = x - cx;
            let dy = y - cy;
            if dx * dx + dy * dy <= r_sq {
                if x >= 0 && x < FB_WIDTH as isize && y >= 0 && y < FB_HEIGHT as isize {
                    fb_set_pixel(x as usize, y as usize, color);
                }
            }
        }
    }
}

// Draw a rectangle with outline
fn fb_draw_rect_outline(x: usize, y: usize, w: usize, h: usize, color: u8, thickness: usize) {
    // Top and bottom borders
    for t in 0..thickness.min(h) {
        fb_draw_rect(x, y + t, w, 1, color); // Top
        fb_draw_rect(x, y + h - 1 - t, w, 1, color); // Bottom
    }
    // Left and right borders
    for t in 0..thickness.min(w) {
        fb_draw_rect(x + t, y, 1, h, color); // Left
        fb_draw_rect(x + w - 1 - t, y, 1, h, color); // Right
    }
}

// Draw a gradient rectangle (vertical gradient)
fn fb_draw_gradient_rect(x: usize, y: usize, w: usize, h: usize, start_color: u8, end_color: u8) {
    for row in 0..h {
        let ratio = (row * 255) / h.max(1);
        let color = if start_color < end_color {
            let diff = (end_color - start_color) as usize;
            start_color + ((ratio * diff) / 255) as u8
        } else {
            let diff = (start_color - end_color) as usize;
            start_color - ((ratio * diff) / 255) as u8
        };
        fb_draw_rect(x, y + row, w, 1, color);
    }
}

// Draw a triangle using three points
fn fb_draw_triangle(x0: usize, y0: usize, x1: usize, y1: usize, x2: usize, y2: usize, color: u8) {
    fb_draw_line(x0 as isize, y0 as isize, x1 as isize, y1 as isize, color);
    fb_draw_line(x1 as isize, y1 as isize, x2 as isize, y2 as isize, color);
    fb_draw_line(x2 as isize, y2 as isize, x0 as isize, y0 as isize, color);
}

// Draw a simple button with text
fn fb_draw_button(x: usize, y: usize, w: usize, h: usize, text: &str, bg_color: u8, text_color: u8, border_color: u8) {
    // Fill button background
    fb_draw_rect(x, y, w, h, bg_color);
    // Draw border
    fb_draw_rect_outline(x, y, w, h, border_color, 1);
    // Draw text centered
    let text_len = text.len().min(w / 8); // Max chars that fit
    let text_x = x + (w - text_len * 8) / 2;
    let text_y = y + (h - 8) / 2;
    fb_draw_text(text_x, text_y, &text[..text_len], text_color);
}

// Draw a simple window frame
fn fb_draw_window(x: usize, y: usize, w: usize, h: usize, title: &str, bg_color: u8, title_bg: u8, border_color: u8) {
    // Draw main window background
    fb_draw_rect(x, y, w, h, bg_color);
    // Draw title bar
    fb_draw_rect(x, y, w, 16, title_bg);
    // Draw border
    fb_draw_rect_outline(x, y, w, h, border_color, 2);
    // Draw title text
    fb_draw_text(x + 4, y + 4, title, 0x00);
}

// Create a simple color palette for VGA Mode 13h
fn get_palette_color(index: u8) -> u8 {
    match index % 16 {
        0 => 0x00,  // Black
        1 => 0x01,  // Dark Blue
        2 => 0x02,  // Dark Green
        3 => 0x03,  // Dark Cyan
        4 => 0x04,  // Dark Red
        5 => 0x05,  // Dark Magenta
        6 => 0x14,  // Brown
        7 => 0x07,  // Light Gray
        8 => 0x38,  // Dark Gray
        9 => 0x39,  // Light Blue
        10 => 0x3A, // Light Green
        11 => 0x3B, // Light Cyan
        12 => 0x3C, // Light Red
        13 => 0x3D, // Light Magenta
        14 => 0x3E, // Yellow
        15 => 0x3F, // White
        _ => 0x07,  // Default to light gray
    }
}

// Draw a simple sprite/icon
fn fb_draw_sprite(x: usize, y: usize, sprite_data: &[&str], colors: &[u8]) {
    for (row, line) in sprite_data.iter().enumerate() {
        for (col, ch) in line.chars().enumerate() {
            if let Some(color_index) = ch.to_digit(10) {
                if color_index > 0 && (color_index as usize) < colors.len() {
                    fb_set_pixel(x + col, y + row, colors[color_index as usize]);
                }
            }
        }
    }
}

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
    
    // --- Simple Graphics Demo ---
    // Test basic framebuffer access
    unsafe {
        // Fill screen with a simple pattern to test if graphics mode works
        for i in 0..FB_SIZE {
            *FB_ADDR.add(i) = ((i / FB_WIDTH) % 256) as u8;
        }
    }
    
    // Clear screen to blue
    fb_clear(get_palette_color(1));
    
    // Draw gradient background
    fb_draw_gradient_rect(0, 0, FB_WIDTH, 40, get_palette_color(1), get_palette_color(9));
    
    // Draw title text
    fb_draw_text(10, 10, "Rust OS - Graphics Demo", get_palette_color(15));
    fb_draw_text(10, 20, "Basic VGA Mode 13h", get_palette_color(14));
    
    // Draw a main window
    fb_draw_window(50, 60, 220, 100, "Graphics Window", 
                   get_palette_color(7), get_palette_color(3), get_palette_color(0));
    
    // Draw some geometric shapes
    fb_draw_filled_circle(100, 110, 15, get_palette_color(12)); // Red circle
    fb_draw_circle(140, 110, 20, get_palette_color(10)); // Green circle outline
    fb_draw_triangle(170, 95, 190, 125, 150, 125, get_palette_color(14)); // Yellow triangle
    
    // Draw some buttons
    fb_draw_button(80, 130, 60, 20, "OK", get_palette_color(2), get_palette_color(15), get_palette_color(0));
    fb_draw_button(150, 130, 60, 20, "Cancel", get_palette_color(4), get_palette_color(15), get_palette_color(0));
    
    // Draw a sprite/icon example
    let sprite_data = &[
        "0011100",
        "0122210",
        "1222221",
        "1223221",
        "1222221",
        "0122210",
        "0011100",
    ];
    let sprite_colors = &[0x00, get_palette_color(0), get_palette_color(14), get_palette_color(12)];
    fb_draw_sprite(280, 80, sprite_data, sprite_colors);
    
    // Draw some text samples
    fb_draw_text(10, 180, "Text rendering with bitmap font!", get_palette_color(11));
    
    // Draw color palette demonstration
    for i in 0..16 {
        fb_draw_rect(10 + i * 18, 50, 16, 8, get_palette_color(i as u8));
    }
    fb_draw_text(10, 42, "Color Palette:", get_palette_color(15));
    
    // Draw some lines for decoration
    for i in 0..5 {
        fb_draw_line(20, 170 + i * 2, 300, 170 + i * 2, get_palette_color(8 + i as u8));
    }
    
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
