# Rust OS Kernel with Enhanced Graphics

A 64-bit operating system kernel written in Rust featuring an advanced graphics system with VGA Mode 13h support, double buffering, sprite animation, and comprehensive drawing primitives.

## Features

### Core OS Features
- **64-bit Architecture**: Full x86_64 support with long mode transition
- **Memory Management**: Custom bump allocator for heap memory
- **Interrupt Handling**: Complete IDT setup with exception handling
- **Keyboard Input**: PS/2 keyboard polling system
- **File System**: Simple RAM-based file system for basic storage

### Enhanced Graphics System
- **VGA Mode 13h**: 320x200 resolution with 256 colors (8-bit color depth)
- **Double Buffering**: Smooth, flicker-free animations with back buffer
- **Drawing Primitives**: 
  - Pixels, rectangles, circles, lines, triangles
  - Filled and outlined shapes
  - Gradient rectangles with smooth color transitions
- **Text Rendering**: Bitmap font system supporting full ASCII character set
- **Sprite System**: Advanced sprite rendering with transparency and multi-frame animation
- **GUI Elements**: Windows, buttons, color palettes, and UI components
- **Screen Effects**: Scrolling, blitting, collision detection support

### Advanced Graphics Features
- **Animation Framework**: Frame-based animation system with timing control
- **Color Management**: Comprehensive VGA color palette support
- **Screen Manipulation**: Fast horizontal/vertical line drawing, area copying
- **Memory Optimization**: Efficient framebuffer access and buffer management

## Building and Running

### Prerequisites
- Rust toolchain with nightly compiler
- QEMU for emulation
- `grub-mkrescue` for ISO creation

### Quick Start
```bash
# Build the kernel
make build

# Run in QEMU
make run

# Create ISO image
make iso
```

### Build Commands
```bash
# Clean build artifacts
make clean

# Build kernel binary
cargo build --release

# Generate GRUB rescue ISO
grub-mkrescue -o build/os-x86_64.iso build/isofiles
```

## Project Structure
```
├── src/
│   ├── lib.rs          # Main kernel implementation
│   └── main.rs         # Entry point stub
├── build/
│   ├── kernel-x86_64.bin
│   └── os-x86_64.iso
├── grub/
│   └── grub.cfg        # GRUB configuration
├── Cargo.toml          # Rust project configuration
├── Makefile            # Build system
├── link.ld             # Linker script
└── .cargo/config.toml  # Cargo configuration
```

## Graphics System Architecture

### Core Components
1. **Framebuffer Management**: Direct VGA memory access at 0xA0000
2. **Double Buffering**: Separate back buffer for smooth rendering
3. **Drawing Engine**: Optimized primitive rendering functions
4. **Font System**: 8x8 bitmap font for text rendering
5. **Sprite Engine**: Multi-frame animation with transparency
6. **Color System**: VGA palette management and color utilities

### Key Functions
- `init_graphics_mode()`: Initialize VGA Mode 13h
- `fb_set_pixel()`: Basic pixel manipulation
- `fb_draw_*()`: Various shape drawing functions
- `fb_draw_text()`: Text rendering with bitmap fonts
- `fb_draw_sprite()`: Sprite rendering with transparency
- `fb_swap_buffers()`: Double buffer management

## Demo Features
The kernel includes an interactive graphics demonstration showcasing:
- Real-time bouncing ball physics
- Multi-frame sprite animation (spinning icon)
- Scrolling text with multiple messages
- GUI elements (windows, buttons)
- Color palette visualization
- Geometric shapes and gradients

### Controls
- **SPACE**: Start animated demonstration
- **ESC**: Exit and halt kernel

## Technical Details

### Memory Layout
- Kernel loaded at higher half (0x100000)
- VGA framebuffer at 0xA0000
- Heap allocation from 0x100000-0x200000
- Stack at 0x90000

### Graphics Specifications
- Resolution: 320x200 pixels
- Color depth: 8-bit (256 colors)
- Framebuffer size: 64,000 bytes
- Refresh rate: Hardware VGA timing
- Double buffer: Full 64KB back buffer

### Performance Features
- Optimized memory access patterns
- Fast horizontal/vertical line drawing
- Efficient sprite blitting with transparency
- Frame-rate controlled animation loops

## Development

### Adding New Graphics Features
1. Implement drawing function in the graphics section
2. Add function to enhanced graphics functions
3. Update demo in `_start()` function
4. Test with `make run`

### Debugging
- Use QEMU monitor for debugging
- VGA text mode output for early boot diagnostics
- Exception handling with custom panic handler

## License
This project is open source and available under standard license terms.

## Contributing
Contributions are welcome! Please feel free to submit issues and pull requests.

---

**Note**: This is an educational operating system kernel project demonstrating low-level graphics programming, memory management, and OS development concepts in Rust.
