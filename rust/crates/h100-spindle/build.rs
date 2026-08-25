fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-link-arg-cdylib=-Wl,--allow-shlib-undefined");
}
