#![no_std]
#![no_main]

use core::panic::PanicInfo;
use bootloader::{entry_point, BootInfo};

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    let fb = boot_info.framebuffer.as_ref().expect("No framebuffer");
    let info = fb.info();
    let buffer = fb.buffer_mut();
    let width = info.width;
    let height = info.height;
    let stride = info.stride;
    let bytes_per_pixel = info.bytes_per_pixel;

    // Fill the screen with a blue background
    for y in 0..height {
        for x in 0..width {
            let idx = y * stride + x;
            let pixel = &mut buffer[idx * bytes_per_pixel..][..bytes_per_pixel];
            // ARGB (little endian): 0xFF1122FF = opaque blue
            pixel.copy_from_slice(&[0xFF, 0x22, 0x11, 0xFF][..bytes_per_pixel]);
        }
    }
    // Draw a white rectangle
    for y in 100..200 {
        for x in 100..400 {
            if x < width && y < height {
                let idx = y * stride + x;
                let pixel = &mut buffer[idx * bytes_per_pixel..][..bytes_per_pixel];
                pixel.copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF][..bytes_per_pixel]);
            }
        }
    }
    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
