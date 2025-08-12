arch ?= x86_64
kernel := build/kernel-$(arch).bin
iso := build/os-$(arch).iso

linker_script := src/arch/$(arch)/linker.ld
grub_cfg := src/arch/$(arch)/grub.cfg
assembly_source_files := $(wildcard src/arch/$(arch)/*.asm)
assembly_object_files := $(patsubst src/arch/$(arch)/%.asm, \
	build/arch/$(arch)/%.o, $(assembly_source_files))

.PHONY: all clean run iso

all: $(kernel)

clean:
	@rm -rf build

run: $(iso)
	@qemu-system-x86_64 -boot d -cdrom $(iso) -monitor stdio -d int,cpu_reset -no-reboot -no-shutdown
# Check for Multiboot2 header in kernel binary
.PHONY: check-multiboot
check-multiboot:
	@echo "Checking for Multiboot2 header (e85250d6) in first 8 KiB of $(kernel):"
	hexdump -C $(kernel) | head -320 | grep 'e8 52 50 d6' && echo 'Multiboot2 header found.' || echo 'Multiboot2 header NOT found!'

iso: $(iso)

$(iso): $(kernel) $(grub_cfg)
	@mkdir -p build/isofiles/boot/grub
	@cp $(kernel) build/isofiles/boot/kernel.bin
	@cp $(grub_cfg) build/isofiles/boot/grub/grub.cfg
	grub-mkrescue -o $(iso) build/isofiles
	@rm -rf build/isofiles

# List ISO contents for debugging
.PHONY: iso-list
iso-list:
	@isoinfo -i $(iso) -R -f || echo "isoinfo not installed"


$(kernel): cargo $(assembly_object_files) $(linker_script)
	@ld -n -T $(linker_script) -o $(kernel) build/arch/x86_64/multiboot_header.o $(filter-out build/arch/x86_64/multiboot_header.o,$(assembly_object_files)) target/x86_64-unknown-none/release/libempty.rlib

cargo:
	@cargo +nightly build -Z build-std=core,compiler_builtins --target x86_64-unknown-none.json --release

# compile assembly files
build/arch/$(arch)/%.o: src/arch/$(arch)/%.asm
	@mkdir -p $(shell dirname $@)
	@nasm -felf64 $< -o $@
