# Minimal Rust OS Kernel

This project is a minimal operating system kernel written in Rust. It is intended for educational purposes and follows best practices for OS development, including custom entry points, no_std, and integration with bootloader and architecture-specific code.

## Project Structure
- `src/main.rs`: Kernel entry point
- `src/arch/x86_64/`: Architecture-specific code
- `.github/copilot-instructions.md`: Copilot custom instructions

## Building
To build the kernel, run:
```bash
# Build the Rust kernel
cargo build --target x86_64-unknown-none --release

# Create bootable ISO
make
```

## Running
To run the kernel in QEMU:
```bash
make run
```

## Next Steps
- Add bootloader integration
- Add linker scripts and assembly files in `src/arch/x86_64/`
- Automate build and ISO creation with a Makefile

## Running
You will need a bootloader (e.g., GRUB) and QEMU to run the kernel. See documentation for details.
