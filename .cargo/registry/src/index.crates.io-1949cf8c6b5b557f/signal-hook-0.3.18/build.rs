#[cfg(feature = "extended-siginfo-raw")]
fn main() {
    cc::Build::new()
        .file("src/low_level/extract.c")
        .compile("extract");
}

#[cfg(not(feature = "extended-siginfo-raw"))]
fn main() {}
