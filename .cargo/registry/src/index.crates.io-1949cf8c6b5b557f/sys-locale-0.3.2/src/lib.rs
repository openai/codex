//! A library to safely and easily obtain the current locale on the system or for an application.
//!
//! This library currently supports the following platforms:
//! - Android
//! - iOS (and derivatives such as watchOS, tvOS, and visionOS)
//! - macOS
//! - Linux, BSD, and other UNIX variations
//! - WebAssembly on the web (via the `js` feature)
//! - Windows
#![cfg_attr(any(not(unix), target_vendor = "apple", target_os = "android"), no_std)]
extern crate alloc;
use alloc::string::String;

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
use android as provider;

#[cfg(target_vendor = "apple")]
mod apple;
#[cfg(target_vendor = "apple")]
use apple as provider;

#[cfg(all(unix, not(any(target_vendor = "apple", target_os = "android"))))]
mod unix;
#[cfg(all(unix, not(any(target_vendor = "apple", target_os = "android"))))]
use unix as provider;

#[cfg(all(target_family = "wasm", feature = "js", not(unix)))]
mod wasm;
#[cfg(all(target_family = "wasm", feature = "js", not(unix)))]
use wasm as provider;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as provider;

#[cfg(not(any(unix, all(target_family = "wasm", feature = "js", not(unix)), windows)))]
mod provider {
    pub fn get() -> impl Iterator<Item = alloc::string::String> {
        core::iter::empty()
    }
}

/// Returns the most preferred locale for the system or application.
///
/// This is equivalent to `get_locales().next()` (the first entry).
///
/// # Returns
///
/// Returns [`Some(String)`] with a BCP 47 language tag inside.  
/// If the locale couldn't be obtained, [`None`] is returned instead.
///
/// # Example
///
/// ```no_run
/// use sys_locale::get_locale;
///
/// let current_locale = get_locale().unwrap_or_else(|| String::from("en-US"));
///
/// println!("The locale is {}", current_locale);
/// ```
pub fn get_locale() -> Option<String> {
    get_locales().next()
}

/// Returns the preferred locales for the system or application, in descending order of preference.
///
/// # Returns
///
/// Returns an [`Iterator`] with any number of BCP 47 language tags inside.  
/// If no locale preferences could be obtained, the iterator will be empty.
///
/// # Example
///
/// ```no_run
/// use sys_locale::get_locales;
///
/// let mut  locales = get_locales();
///
/// println!("The most preferred locale is {}", locales.next().unwrap_or("en-US".to_string()));
/// println!("The least preferred locale is {}", locales.last().unwrap_or("en-US".to_string()));
/// ```
pub fn get_locales() -> impl Iterator<Item = String> {
    provider::get()
}

#[cfg(test)]
mod tests {
    use super::{get_locale, get_locales};
    extern crate std;

    #[cfg(all(target_family = "wasm", feature = "js", not(unix)))]
    use wasm_bindgen_test::wasm_bindgen_test as test;
    #[cfg(all(target_family = "wasm", feature = "js", not(unix)))]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[test]
    fn can_obtain_locale() {
        assert!(get_locale().is_some(), "no locales were returned");
        let locales = get_locales();
        for (i, locale) in locales.enumerate() {
            assert!(!locale.is_empty(), "locale string {} was empty", i);
            assert!(
                !locale.ends_with('\0'),
                "locale {} contained trailing NUL",
                i
            );
        }
    }
}
