//! Low level terminal capability lookups

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

pub mod windows;

/// Check [CLICOLOR] status
///
/// - When `true`, ANSI colors are supported and should be used when the program isn't piped,
///   similar to [`term_supports_color`]
/// - When `false`, don’t output ANSI color escape codes, similar to [`no_color`]
///
/// See also:
/// - [terminfo](https://crates.io/crates/terminfo) or [term](https://crates.io/crates/term) for
///   checking termcaps
/// - [termbg](https://crates.io/crates/termbg) for detecting background color
///
/// [CLICOLOR]: https://bixense.com/clicolors/
#[inline]
pub fn clicolor() -> Option<bool> {
    let value = std::env::var_os("CLICOLOR")?;
    Some(value != "0")
}

/// Check [CLICOLOR_FORCE] status
///
/// ANSI colors should be enabled no matter what.
///
/// [CLICOLOR_FORCE]: https://bixense.com/clicolors/
#[inline]
pub fn clicolor_force() -> bool {
    non_empty(std::env::var_os("CLICOLOR_FORCE").as_deref())
}

/// Check [NO_COLOR] status
///
/// When `true`, should prevent the addition of ANSI color.
///
/// User-level configuration files and per-instance command-line arguments should override
/// [NO_COLOR]. A user should be able to export `$NO_COLOR` in their shell configuration file as a
/// default, but configure a specific program in its configuration file to specifically enable
/// color.
///
/// [NO_COLOR]: https://no-color.org/
#[inline]
pub fn no_color() -> bool {
    non_empty(std::env::var_os("NO_COLOR").as_deref())
}

/// Check `TERM` for color support
#[inline]
pub fn term_supports_color() -> bool {
    #[cfg(not(windows))]
    {
        match std::env::var_os("TERM") {
            // If TERM isn't set, then we are in a weird environment that
            // probably doesn't support colors.
            None => return false,
            Some(k) => {
                if k == "dumb" {
                    return false;
                }
            }
        }
        true
    }
    #[cfg(windows)]
    {
        // On Windows, if TERM isn't set, then we shouldn't automatically
        // assume that colors aren't allowed. This is unlike Unix environments
        // where TERM is more rigorously set.
        if let Some(k) = std::env::var_os("TERM") {
            if k == "dumb" {
                return false;
            }
        }
        true
    }
}

/// Check `TERM` for ANSI color support
///
/// On Windows, you might need to also check [`windows::enable_ansi_colors`] as ANSI color support
/// is opt-in, rather than assumed.
#[inline]
pub fn term_supports_ansi_color() -> bool {
    #[cfg(not(windows))]
    {
        term_supports_color()
    }
    #[cfg(windows)]
    {
        match std::env::var_os("TERM") {
            None => return false,
            Some(k) => {
                // cygwin doesn't seem to support ANSI escape sequences
                // and instead has its own variety. However, the Windows
                // console API may be available.
                if k == "dumb" || k == "cygwin" {
                    return false;
                }
            }
        }
        true
    }
}

/// Check [COLORTERM] for truecolor support
///
/// [COLORTERM]: https://github.com/termstandard/colors
#[inline]
pub fn truecolor() -> bool {
    let value = std::env::var_os("COLORTERM");
    let value = value.as_deref().unwrap_or_default();
    value == "truecolor" || value == "24bit"
}

/// Report whether this is running in CI
///
/// CI is a common environment where, despite being piped, ansi color codes are supported
///
/// This is not as exhaustive as you'd find in a crate like `is_ci` but it should work in enough
/// cases.
#[inline]
pub fn is_ci() -> bool {
    // Assuming its CI based on presence because who would be setting `CI=false`?
    //
    // This makes it easier to all of the potential values when considering our known values:
    // - Gitlab and Github set it to `true`
    // - Woodpecker sets it to `woodpecker`
    std::env::var_os("CI").is_some()
}

fn non_empty(var: Option<&std::ffi::OsStr>) -> bool {
    !var.unwrap_or_default().is_empty()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn non_empty_not_present() {
        assert!(!non_empty(None));
    }

    #[test]
    fn non_empty_empty() {
        assert!(!non_empty(Some(std::ffi::OsStr::new(""))));
    }

    #[test]
    fn non_empty_texty() {
        assert!(non_empty(Some(std::ffi::OsStr::new("hello"))));
    }
}

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
