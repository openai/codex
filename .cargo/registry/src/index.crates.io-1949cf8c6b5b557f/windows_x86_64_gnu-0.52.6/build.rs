fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();

    println!(
        "cargo:rustc-link-search=native={}",
        std::path::Path::new(&dir).join("lib").display()
    );
}
