use alloc::{string::String, vec::Vec};

#[path = "./windows_sys.rs"]
mod windows_sys;
use windows_sys::{GetUserPreferredUILanguages, MUI_LANGUAGE_NAME, TRUE};

#[allow(clippy::as_conversions)]
pub(crate) fn get() -> impl Iterator<Item = String> {
    let mut num_languages: u32 = 0;
    let mut buffer_length: u32 = 0;

    // Calling this with null buffer will retrieve the required buffer length
    let success = unsafe {
        GetUserPreferredUILanguages(
            MUI_LANGUAGE_NAME,
            &mut num_languages,
            core::ptr::null_mut(),
            &mut buffer_length,
        )
    } == TRUE;
    if !success {
        return Vec::new().into_iter();
    }

    let mut buffer = Vec::<u16>::with_capacity(buffer_length as usize);

    // Now that we have an appropriate buffer, we can query the names
    let mut result = Vec::with_capacity(num_languages as usize);
    let success = unsafe {
        GetUserPreferredUILanguages(
            MUI_LANGUAGE_NAME,
            &mut num_languages,
            buffer.as_mut_ptr(),
            &mut buffer_length,
        )
    } == TRUE;

    if success {
        // SAFETY: Windows wrote the required length worth of UTF-16 into our buffer, which initialized it.
        unsafe { buffer.set_len(buffer_length as usize) };
        // The buffer contains names split by null char (0), and ends with two null chars (00)
        for part in buffer.split(|i| *i == 0).filter(|p| !p.is_empty()) {
            if let Ok(locale) = String::from_utf16(part) {
                result.push(locale);
            }
        }
    }

    result.into_iter()
}
