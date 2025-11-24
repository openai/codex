use std::borrow::Cow;

use once_cell::sync::Lazy;
use regex::{Captures, Regex};

static HASH_FUNC_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?x)
        ^(.*)::h[a-f0-9]{16}$
    "#,
    )
    .unwrap()
});

static CRATE_HASH_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?x)
        \b(\[[a-f0-9]{16}\])
    ",
    )
    .unwrap()
});

static CRATE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?x)
        ^
        (?:_?<)?           # trait impl syntax
        (?:\w+\ as \ )?    # anonymous implementor
        ([a-zA-Z0-9_]+?)   # crate name
        (?:\.\.|::|\[)     # crate delimiter (.. or :: or [)
    ",
    )
    .unwrap()
});

static COMMON_RUST_SYMBOL_ESCAPES_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?x)
        \$
            (SP|BP|RF|LT|GT|LP|RP|C|
                u7e|u20|u27|u5b|u5d|u7b|u7d|u3b|u2b|u22)
        \$
    ",
    )
    .unwrap()
});

/// Tries to parse the rust crate from a function name.
pub fn parse_crate_name(func_name: &str) -> Option<String> {
    CRATE_RE
        .captures(func_name)
        .and_then(|caps| caps.get(1))
        .map(|cr| cr.as_str().into())
}

pub fn filename(s: &str) -> &str {
    s.rsplit(&['/', '\\'][..]).next().unwrap()
}

pub fn strip_symbol(s: &str) -> Cow<str> {
    let stripped_trailing_hash = HASH_FUNC_RE
        .captures(s)
        .map(|c| c.get(1).unwrap().as_str())
        .unwrap_or(s);

    CRATE_HASH_RE.replace_all(stripped_trailing_hash, "")
}

pub fn demangle_symbol(s: &str) -> String {
    COMMON_RUST_SYMBOL_ESCAPES_RE
        .replace_all(s, |caps: &Captures<'_>| match &caps[1] {
            "SP" => "@",
            "BP" => "*",
            "RF" => "&",
            "LT" => "<",
            "GT" => ">",
            "LP" => "(",
            "RP" => ")",
            "C" => ",",
            "u7e" => "~",
            "u20" => " ",
            "u27" => "'",
            "u5b" => "[",
            "u5d" => "]",
            "u7b" => "{",
            "u7d" => "}",
            "u3b" => ";",
            "u2b" => "+",
            "u22" => "\"",
            _ => unreachable!(),
        })
        .to_string()
}

/// Checks whether the function name starts with the given pattern.
///
/// In trait implementations, the original type name is wrapped in "_< ... >" and colons are
/// replaced with dots. This function accounts for differences while checking.
pub fn function_starts_with(mut func_name: &str, mut pattern: &str) -> bool {
    if pattern.starts_with('<') {
        while pattern.starts_with('<') {
            pattern = &pattern[1..];

            if func_name.starts_with('<') {
                func_name = &func_name[1..];
            } else if func_name.starts_with("_<") {
                func_name = &func_name[2..];
            } else {
                return false;
            }
        }
    } else {
        func_name = func_name.trim_start_matches('<').trim_start_matches("_<");
    }

    if !func_name.is_char_boundary(pattern.len()) {
        return false;
    }

    func_name
        .chars()
        .zip(pattern.chars())
        .all(|(f, p)| f == p || f == '.' && p == ':')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_starts_with() {
        assert!(function_starts_with(
            "futures::task_impl::std::set",
            "futures::"
        ));

        assert!(!function_starts_with(
            "futures::task_impl::std::set",
            "tokio::"
        ));
    }

    #[test]
    fn test_function_starts_with_impl() {
        assert!(function_starts_with(
            "_<futures..task_impl..Spawn<T>>::enter::_{{closure}}",
            "futures::"
        ));

        assert!(!function_starts_with(
            "_<futures..task_impl..Spawn<T>>::enter::_{{closure}}",
            "tokio::"
        ));
    }

    #[test]
    fn test_function_starts_with_newimpl() {
        assert!(function_starts_with(
            "<futures::task_impl::Spawn<T>>::enter::{{closure}}",
            "futures::"
        ));

        assert!(!function_starts_with(
            "<futures::task_impl::Spawn<T>>::enter::{{closure}}",
            "tokio::"
        ));
    }

    #[test]
    fn test_function_starts_with_impl_pattern() {
        assert!(function_starts_with(
            "_<futures..task_impl..Spawn<T>>::enter::_{{closure}}",
            "<futures::"
        ));

        assert!(function_starts_with(
            "<futures::task_impl::Spawn<T>>::enter::{{closure}}",
            "<futures::"
        ));

        assert!(!function_starts_with(
            "futures::task_impl::std::set",
            "<futures::"
        ));
    }

    #[test]
    fn test_parse_crate_name() {
        assert_eq!(
            parse_crate_name("futures::task_impl::std::set"),
            Some("futures".into())
        );
    }

    #[test]
    fn test_parse_crate_name_impl() {
        assert_eq!(
            parse_crate_name("_<futures..task_impl..Spawn<T>>::enter::_{{closure}}"),
            Some("futures".into())
        );
    }

    #[test]
    fn test_parse_crate_name_anonymous_impl() {
        assert_eq!(
            parse_crate_name("_<F as alloc..boxed..FnBox<A>>::call_box"),
            Some("alloc".into())
        );
    }

    #[test]
    fn strip_crate_hash() {
        assert_eq!(
            &strip_symbol("std::panic::catch_unwind::hd044952603e5f56c"),
            "std::panic::catch_unwind"
        );
        assert_eq!(
            &strip_symbol("std[550525b9dd91a68e]::rt::lang_start::<()>"),
            "std::rt::lang_start::<()>"
        );
        assert_eq!(
            &strip_symbol("<fn() as core[bb3d6b31f0e973c8]::ops::function::FnOnce<()>>::call_once"),
            "<fn() as core::ops::function::FnOnce<()>>::call_once"
        );
        assert_eq!(&strip_symbol("<std[550525b9dd91a68e]::thread::local::LocalKey<(arc_swap[1d34a79be67db79e]::ArcSwapAny<alloc[bc7f897b574022f6]::sync::Arc<sentry_core[1d5336878cce1456]::hub::Hub>>, core[bb3d6b31f0e973c8]::cell::Cell<bool>)>>::with::<<sentry_core[1d5336878cce1456]::hub::Hub>::with<<sentry_core[1d5336878cce1456]::hub::Hub>::with_active<sentry_core[1d5336878cce1456]::api::with_integration<sentry_panic[c87c9124ff32f50e]::PanicIntegration, sentry_panic[c87c9124ff32f50e]::panic_handler::{closure#0}, ()>::{closure#0}, ()>::{closure#0}, ()>::{closure#0}, ()>"), "<std::thread::local::LocalKey<(arc_swap::ArcSwapAny<alloc::sync::Arc<sentry_core::hub::Hub>>, core::cell::Cell<bool>)>>::with::<<sentry_core::hub::Hub>::with<<sentry_core::hub::Hub>::with_active<sentry_core::api::with_integration<sentry_panic::PanicIntegration, sentry_panic::panic_handler::{closure#0}, ()>::{closure#0}, ()>::{closure#0}, ()>::{closure#0}, ()>");
    }

    #[test]
    fn test_parse_crate_name_none() {
        assert_eq!(parse_crate_name("main"), None);
    }

    #[test]
    fn test_parse_crate_name_newstyle() {
        assert_eq!(
            parse_crate_name("<failure::error::Error as core::convert::From<F>>::from"),
            Some("failure".into())
        );
    }

    #[test]
    fn test_parse_crate_name_hash() {
        assert_eq!(
            parse_crate_name("backtrace[856cf81bbf211f65]::backtrace::libunwind::trace"),
            Some("backtrace".into())
        );
        assert_eq!(
            parse_crate_name("<backtrace[856cf81bbf211f65]::capture::Backtrace>::new"),
            Some("backtrace".into())
        );
    }
}
