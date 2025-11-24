//! Implementation for ESP-IDF
use crate::Error;
use core::{ffi::c_void, mem::MaybeUninit};

extern "C" {
    fn esp_fill_random(buf: *mut c_void, len: usize) -> u32;
}

pub fn getrandom_inner(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    // Not that NOT enabling WiFi, BT, or the voltage noise entropy source (via `bootloader_random_enable`)
    // will cause ESP-IDF to return pseudo-random numbers based on the voltage noise entropy, after the initial boot process:
    // https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-reference/system/random.html
    //
    // However tracking if some of these entropy sources is enabled is way too difficult to implement here
    unsafe { esp_fill_random(dest.as_mut_ptr().cast(), dest.len()) };

    Ok(())
}
