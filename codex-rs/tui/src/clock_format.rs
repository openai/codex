//! Detects the TUI host's clock preference once. Unknown preferences preserve 12-hour output;
//! detection never changes the process locale or runs while rendering a history cell.

use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClockFormat {
    TwelveHour,
    TwentyFourHour,
}

impl ClockFormat {
    pub(crate) fn system() -> Self {
        static FORMAT: OnceLock<ClockFormat> = OnceLock::new();
        *FORMAT.get_or_init(|| detect().unwrap_or(Self::TwelveHour))
    }

    pub(crate) fn time_format(self) -> &'static str {
        match self {
            Self::TwelveHour => "%-I:%M %p",
            Self::TwentyFourHour => "%H:%M",
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "android", test))]
fn time_locale(
    mut get: impl FnMut(&str) -> Option<std::ffi::OsString>,
) -> Option<std::ffi::OsString> {
    ["LC_ALL", "LC_TIME", "LANG"]
        .into_iter()
        .find_map(|name| get(name).filter(|value| !value.is_empty()))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn detect() -> Option<ClockFormat> {
    let locale = time_locale(|name| std::env::var_os(name))?;
    detect_time_locale(locale.to_str()?)
}

#[cfg(any(target_os = "linux", target_os = "android", test))]
fn detect_time_locale(locale: &str) -> Option<ClockFormat> {
    if matches!(locale, "C" | "POSIX" | "C.UTF-8" | "C.utf8") {
        return Some(ClockFormat::TwentyFourHour);
    }
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        let name = std::ffi::CString::new(locale).ok()?;
        // SAFETY: name is NUL-terminated, the mask is valid, and a null base creates
        // an independent locale. Its borrowed pattern is read before freeing it.
        unsafe {
            let locale = libc::newlocale(libc::LC_TIME_MASK, name.as_ptr(), std::ptr::null_mut());
            if locale.is_null() {
                return None;
            }
            let pattern = libc::nl_langinfo_l(libc::T_FMT, locale);
            let result = if pattern.is_null() {
                None
            } else {
                parse_strftime(std::ffi::CStr::from_ptr(pattern).to_bytes())
            };
            libc::freelocale(locale);
            result
        }
    }
    // musl and bionic can return C patterns for unsupported locales. Do not
    // mistake that fallback for the user's actual clock preference.
    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    None
}

#[cfg(any(all(target_os = "linux", target_env = "gnu"), test))]
fn parse_strftime(pattern: &[u8]) -> Option<ClockFormat> {
    let mut chars = pattern.iter().copied().peekable();
    let mut twelve = false;
    let mut twenty_four = false;
    while let Some(ch) = chars.next() {
        if ch != b'%' {
            continue;
        }
        while chars
            .next_if(|ch| matches!(ch, b'_' | b'-' | b'0' | b'^' | b'#') || ch.is_ascii_digit())
            .is_some()
        {}
        chars.next_if(|ch| matches!(ch, b'E' | b'O'));
        match chars.next()? {
            b'I' | b'l' | b'r' => twelve = true,
            b'H' | b'k' | b'R' | b'T' => twenty_four = true,
            b'%' | b'M' | b'S' | b'p' | b'P' | b'Z' | b'z' | b'n' | b't' => {}
            // In particular, do not recurse into %X or %c.
            _ => return None,
        }
    }
    match (twelve, twenty_four) {
        (true, false) => Some(ClockFormat::TwelveHour),
        (false, true) => Some(ClockFormat::TwentyFourHour),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
fn detect() -> Option<ClockFormat> {
    use objc2_foundation::NSDateFormatter;
    use objc2_foundation::NSLocale;
    use objc2_foundation::NSString;

    let locale = NSLocale::currentLocale();
    let pattern = NSDateFormatter::dateFormatFromTemplate_options_locale(
        &NSString::from_str("j"),
        /*opts*/ 0,
        Some(&locale),
    )?;
    parse_native_pattern(&pattern.to_string(), "hHKk")
}

#[cfg(windows)]
fn detect() -> Option<ClockFormat> {
    use windows_sys::Win32::Globalization::GetLocaleInfoEx;
    use windows_sys::Win32::Globalization::LOCALE_SSHORTTIME;

    // SAFETY: the first call queries the size; the second receives a writable
    // buffer of that size. A null locale name selects the user's default locale,
    // and user overrides remain enabled.
    let pattern = unsafe {
        let length = GetLocaleInfoEx(
            std::ptr::null(),
            LOCALE_SSHORTTIME,
            std::ptr::null_mut(),
            /*cchdata*/ 0,
        );
        if length <= 1 {
            return None;
        }
        let mut buffer = vec![0; length as usize];
        let written = GetLocaleInfoEx(
            std::ptr::null(),
            LOCALE_SSHORTTIME,
            buffer.as_mut_ptr(),
            length,
        );
        if written <= 1 {
            return None;
        }
        String::from_utf16(&buffer[..written as usize - 1]).ok()?
    };
    parse_native_pattern(&pattern, "hH")
}

#[cfg(any(target_os = "macos", windows, test))]
fn parse_native_pattern(pattern: &str, hour_symbols: &str) -> Option<ClockFormat> {
    let mut quoted = false;
    let mut twelve = false;
    let mut twenty_four = false;
    for ch in pattern.chars() {
        if ch == '\'' {
            // Two adjacent apostrophes restore the quote state, representing
            // a literal apostrophe in both native pattern syntaxes.
            quoted = !quoted;
        } else if !quoted {
            if ch == ';' {
                break; // Windows lists the preferred pattern first.
            }
            if hour_symbols.contains(ch) {
                match ch {
                    'h' | 'K' => twelve = true,
                    'H' | 'k' => twenty_four = true,
                    _ => return None,
                }
            }
        }
    }
    match (quoted, twelve, twenty_four) {
        (false, true, false) => Some(ClockFormat::TwelveHour),
        (false, false, true) => Some(ClockFormat::TwentyFourHour),
        _ => None,
    }
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    windows
)))]
fn detect() -> Option<ClockFormat> {
    None
}

#[cfg(test)]
#[path = "clock_format_tests.rs"]
mod tests;
