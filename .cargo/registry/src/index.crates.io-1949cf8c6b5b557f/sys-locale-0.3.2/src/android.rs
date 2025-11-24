use alloc::{string::String, vec};
use core::convert::TryFrom;

fn get_property(name: &'static [u8]) -> Option<String> {
    let mut value = vec![0u8; libc::PROP_VALUE_MAX as usize];
    // SAFETY: `name` is valid to read from and `value` is valid to write to.
    let len =
        unsafe { libc::__system_property_get(name.as_ptr().cast(), value.as_mut_ptr().cast()) };

    usize::try_from(len)
        .ok()
        .filter(|n| *n != 0)
        .and_then(move |n| {
            // Remove excess bytes and the NUL terminator
            value.resize(n, 0);
            String::from_utf8(value).ok()
        })
}

const LOCALE_KEY: &[u8] = b"persist.sys.locale\0";
const PRODUCT_LOCALE_KEY: &[u8] = b"ro.product.locale\0";

const PRODUCT_LANGUAGE_KEY: &[u8] = b"ro.product.locale.language\0";
const PRODUCT_REGION_KEY: &[u8] = b"ro.product.locale.region\0";

// Android 4.0 and below
const LANG_KEY: &[u8] = b"persist.sys.language\0";
const COUNTRY_KEY: &[u8] = b"persist.sys.country\0";
const LOCALEVAR_KEY: &[u8] = b"persist.sys.localevar\0";

// Ported from https://android.googlesource.com/platform/frameworks/base/+/refs/heads/master/core/jni/AndroidRuntime.cpp#431
fn read_locale() -> Option<String> {
    if let Some(locale) = get_property(LOCALE_KEY) {
        return Some(locale);
    }

    // Android 4.0 and below
    if let Some(mut language) = get_property(LANG_KEY) {
        // The details of this functionality are not publically available, so this is just
        // adapted "best effort" from the original code.
        match get_property(COUNTRY_KEY) {
            Some(country) => {
                language.push('-');
                language.push_str(&country);
            }
            None => {
                if let Some(variant) = get_property(LOCALEVAR_KEY) {
                    language.push('-');
                    language.push_str(&variant);
                }
            }
        };

        return Some(language);
    }

    if let Some(locale) = get_property(PRODUCT_LOCALE_KEY) {
        return Some(locale);
    }

    let product_language = get_property(PRODUCT_LANGUAGE_KEY);
    let product_region = get_property(PRODUCT_REGION_KEY);
    match (product_language, product_region) {
        (Some(mut lang), Some(region)) => {
            lang.push('-');
            lang.push_str(&region);
            Some(lang)
        }
        _ => None,
    }
}

pub(crate) fn get() -> impl Iterator<Item = String> {
    read_locale().into_iter()
}
