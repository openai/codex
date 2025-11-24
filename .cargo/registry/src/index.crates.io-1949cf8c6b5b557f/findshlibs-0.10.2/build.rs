extern crate cc;

use std::env;

fn main() {
    match env::var("CARGO_CFG_TARGET_OS").unwrap_or_default().as_str() {
        "android" => build_android(),
        _ => {}
    }
}

fn build_android() {
    let expansion = match cc::Build::new().file("src/android-api.c").try_expand() {
        Ok(result) => result,
        Err(e) => {
            println!("cargo:warning=failed to run C compiler: {}", e);
            return;
        }
    };

    let expansion = match std::str::from_utf8(&expansion) {
        Ok(s) => s,
        Err(_) => return,
    };

    let marker = "APIVERSION";
    let i = expansion.find(marker).unwrap_or_default();

    let version = expansion[i + marker.len() + 1..]
        .split_whitespace()
        .next()
        .unwrap_or("");
    let version = version.parse::<u32>().unwrap_or_else(|_| {
        println!("cargo:warning=failed to get android api version.");
        0
    });

    if version >= 21 {
        println!("cargo:rustc-cfg=feature=\"dl_iterate_phdr\"");
    }
}
