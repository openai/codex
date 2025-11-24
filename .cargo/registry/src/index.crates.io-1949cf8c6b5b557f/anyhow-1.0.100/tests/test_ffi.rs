#![deny(improper_ctypes, improper_ctypes_definitions)]

use anyhow::anyhow;

#[no_mangle]
pub extern "C" fn anyhow1(err: anyhow::Error) {
    println!("{:?}", err);
}

#[no_mangle]
pub extern "C" fn anyhow2(err: &mut Option<anyhow::Error>) {
    *err = Some(anyhow!("ffi error"));
}

#[no_mangle]
pub extern "C" fn anyhow3() -> Option<anyhow::Error> {
    Some(anyhow!("ffi error"))
}
