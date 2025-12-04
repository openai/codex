fn main() {
    println!(
        "cargo:rustc-env=BUILD_TARGET={}",
        std::env::var("TARGET").unwrap()
    );
    println!(
        "cargo:rustc-env=BUILD_HOST={}",
        std::env::var("HOST").unwrap()
    );

    let emscripten_version = std::fs::read_to_string("emscripten-version").unwrap();
    println!("cargo:rustc-env=EMSCRIPTEN_VERSION={emscripten_version}");
}
