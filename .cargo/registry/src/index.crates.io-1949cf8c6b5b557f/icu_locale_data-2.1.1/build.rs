// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

fn main() {
    if std::env::var("ICU4X_DATA_DIR").is_ok() {
        println!("cargo:rustc-cfg=icu4x_custom_data");
    }
    println!("cargo:rerun-if-env-changed=ICU4X_DATA_DIR");
    println!("cargo:rustc-check-cfg=cfg(icu4c_enable_renaming)");
}
