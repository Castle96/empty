fn main() {
    println!("cargo:rustc-link-arg=-Tsrc/arch/x86_64/linker.ld");
}
