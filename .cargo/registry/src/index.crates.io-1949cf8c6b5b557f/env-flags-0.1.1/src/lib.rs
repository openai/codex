/*!
This crate exports the `env_flags` macro, which allows a convenient way to declare static
environment variables with optional default values and custom parsing functions.

Currently, this crate requires a Rust compiler of at least `1.80` as it uses `std::sync::LazyLock` under the hood.

# Examples
```rust
use env_flags::env_flags;

use std::time::Duration;

env_flags! {
    /// Required env var, panics if missing.
    AUTH_TOKEN: &str;
    /// Env var with a default value if not specified.
    pub(crate) PORT: u16 = 8080;
    /// An optional env var.
    pub OVERRIDE_HOSTNAME: Option<&str> = None;

    /// `Duration` by default is parsed as `f64` seconds.
    TIMEOUT: Duration = Duration::from_secs(5);
    /// Custom parsing function, takes a `String` and returns a `Result<Duration>`.
    TIMEOUT_MS: Duration = Duration::from_millis(30), |value| {
        value.parse().map(Duration::from_millis)
    };

    /// `bool` can be true, false, 1, or 0 (case insensitive)
    /// eg. export ENABLE_FEATURE="true"
    pub ENABLE_FEATURE: bool = true;

    /// `Vec<T>` by default is parsed as a comma-seprated string
    /// eg. export VALID_PORTS="80,443,9121"
    pub VALID_PORTS: Vec<u16> = vec![80, 443, 9121];


    // Attributes are also captured, including docs

    #[cfg(target_os = "linux")]
    /// operating system
    pub OS: &str = "linux";
    #[cfg(not(target_os = "linux"))]
    /// operating system
    pub OS: &str = "not linux";
}
```

For custom types, you can either specify a parsing function manually (see above `TIMEOUT_MS` example), or you can implement the `ParseEnv` trait. An implementation for `ParseEnv` is included for most std types.

*/
use std::collections::HashSet;
use std::convert::Infallible;
use std::fmt;
use std::hash::Hash;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::Duration;

/// Define the parsing function for a type from a `String` environment variable.
///
/// Check the source for the builtin type definitions for this trait if you're concerned about the
/// parsing logic.
pub trait ParseEnv: Sized {
    /// The `std::fmt::Display` of this error message will appear in the panic message when parsing
    /// fails.
    type Err: fmt::Display;
    /// Tries to parse the value from `std::env::var`.
    fn parse_env(value: String) -> Result<Self, Self::Err>;
}

/// Intermediate error type used in parsing failures to generate helpful messages.
#[derive(Debug, Clone)]
pub struct ParseError {
    type_name: &'static str,
    msg: String,
}

impl ParseError {
    pub fn from_msg<T, S: ToString>(msg: S) -> Self {
        Self {
            type_name: std::any::type_name::<T>(),
            msg: msg.to_string(),
        }
    }

    fn with_type_name<T>(mut self) -> Self {
        self.type_name = std::any::type_name::<T>();
        self
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "failed to parse as {}: {}", self.type_name, &self.msg)
    }
}

/// Implements `ParseEnv` for common types based on `std::str::FromStr`.
macro_rules! gen_parse_env_using_fromstr {
    ($ty: ty) => {
        impl ParseEnv for $ty {
            type Err = $crate::ParseError;

            fn parse_env(value: String) -> Result<Self, Self::Err> {
                value
                    .parse()
                    .map_err(|e| $crate::ParseError::from_msg::<Self, _>(e))
            }
        }
    };
}

gen_parse_env_using_fromstr!(f32);
gen_parse_env_using_fromstr!(f64);
gen_parse_env_using_fromstr!(i8);
gen_parse_env_using_fromstr!(i16);
gen_parse_env_using_fromstr!(i32);
gen_parse_env_using_fromstr!(i64);
gen_parse_env_using_fromstr!(i128);
gen_parse_env_using_fromstr!(isize);
gen_parse_env_using_fromstr!(u8);
gen_parse_env_using_fromstr!(u16);
gen_parse_env_using_fromstr!(u32);
gen_parse_env_using_fromstr!(u64);
gen_parse_env_using_fromstr!(u128);
gen_parse_env_using_fromstr!(usize);
gen_parse_env_using_fromstr!(IpAddr);
gen_parse_env_using_fromstr!(Ipv4Addr);
gen_parse_env_using_fromstr!(Ipv6Addr);
gen_parse_env_using_fromstr!(PathBuf);
gen_parse_env_using_fromstr!(SocketAddr);

impl ParseEnv for String {
    type Err = Infallible;

    fn parse_env(value: String) -> Result<Self, Self::Err> {
        Ok(value)
    }
}

impl ParseEnv for &'static str {
    type Err = Infallible;

    fn parse_env(value: String) -> Result<Self, Self::Err> {
        Ok(Box::leak(Box::new(value)))
    }
}

/// `Duration` by default is parsed as `f64` seconds;
impl ParseEnv for Duration {
    type Err = ParseError;

    fn parse_env(value: String) -> Result<Self, Self::Err> {
        match ParseEnv::parse_env(value) {
            Ok(secs) => Ok(Duration::from_secs_f64(secs)),
            Err(e) => Err(e.with_type_name::<Self>()),
        }
    }
}

/// `Vec<T>` is by default parsed as comma-separated values.
impl<T> ParseEnv for Vec<T>
where
    T: ParseEnv,
{
    type Err = <T as ParseEnv>::Err;

    fn parse_env(value: String) -> Result<Self, Self::Err> {
        value
            .split(',')
            .map(|v| ParseEnv::parse_env(v.to_owned()))
            .collect()
    }
}

/// `HashSet<T>` is by default parsed as comma-separated values.
impl<T> ParseEnv for HashSet<T>
where
    T: ParseEnv + Eq + Hash,
{
    type Err = <T as ParseEnv>::Err;

    fn parse_env(value: String) -> Result<Self, Self::Err> {
        value
            .split(',')
            .map(|v| ParseEnv::parse_env(v.to_owned()))
            .collect()
    }
}

impl<T> ParseEnv for Option<T>
where
    T: ParseEnv,
{
    type Err = <T as ParseEnv>::Err;

    fn parse_env(value: String) -> Result<Self, Self::Err> {
        Ok(Some(ParseEnv::parse_env(value)?))
    }
}

/// `bool` allows two common conventions:
/// - String either "true" or "false" (case insensitive)
/// - Integer either 0 or 1
///
/// Anything else will result in a `ParseError`.
impl ParseEnv for bool {
    type Err = ParseError;

    fn parse_env(mut value: String) -> Result<Self, Self::Err> {
        value.make_ascii_lowercase();
        match value.as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            _ => Err(ParseError::from_msg::<Self, _>(
                "expected either true or false",
            )),
        }
    }
}

/// Static lazily evaluated environment variable.
pub struct LazyEnv<T> {
    inner: LazyLock<T>,
}

impl<T> LazyEnv<T> {
    #[inline]
    #[doc(hidden)]
    pub const fn new(init_fn: fn() -> T) -> Self {
        Self {
            inner: LazyLock::new(init_fn),
        }
    }
}

impl<T> Deref for LazyEnv<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> fmt::Debug for LazyEnv<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let inner = &*self.inner;
        inner.fmt(f)
    }
}

impl<T> fmt::Display for LazyEnv<T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let inner = &*self.inner;
        inner.fmt(f)
    }
}

/// Helper function for better compilation errors
#[doc(hidden)]
#[inline]
pub fn __apply_parse_fn<F, T, E>(func: F, key: &'static str, val: String) -> T
where
    F: Fn(String) -> Result<T, E>,
    E: fmt::Display,
{
    match func(val) {
        Ok(val) => val,
        Err(e) => __invalid_env_var(key, e),
    }
}

#[doc(hidden)]
pub fn __invalid_env_var(key: &'static str, err: impl fmt::Display) -> ! {
    panic!("Invalid environment variable {}, {}", key, err)
}

#[doc(hidden)]
pub fn __missing_env_var(key: &'static str) -> ! {
    panic!("Missing required environment variable {}", key)
}

/// private macro for recursively expanding `env_flag`
#[doc(hidden)]
#[macro_export(local_inner_macros)]
macro_rules! __env_flag_inner {
    ($(#[$attr:meta])* ($($vis:tt)*) $key:ident : $ty:ty = $default:expr, $parse_fn:expr; $($rem:tt)*) => {
        $(#[$attr])*
        $($vis)* static $key: $crate::LazyEnv<$ty> = $crate::LazyEnv::new(|| {
            match ::std::env::var(::std::stringify!($key)) {
                Ok(value) => $crate::__apply_parse_fn($parse_fn, ::std::stringify!($key), value),
                Err(_) => $default(),
            }
        });

        env_flags!($($rem)*);
    };
}

/// Declare environment variables with optional defaults and parsing functions.
///
/// Values are static and lazily evaluated once the first time they are dereferenced.
///
/// See the module-level documents for examples.
#[macro_export(local_inner_macros)]
macro_rules! env_flags {
    // key: type;
    ($(#[$attr:meta])* $key:ident : $ty:ty; $($rem:tt)*) => {
        __env_flag_inner!($(#[$attr])* () $key : $ty = || $crate::__missing_env_var(::std::stringify!($key)), $crate::ParseEnv::parse_env; $($rem)*);
    };
    ($(#[$attr:meta])* pub $key:ident : $ty:ty; $($rem:tt)*) => {
        __env_flag_inner!($(#[$attr])* (pub) $key : $ty = || $crate::__missing_env_var(::std::stringify!($key)), $crate::ParseEnv::parse_env; $($rem)*);
    };
    ($(#[$attr:meta])* pub ($($vis:tt)+) $key:ident : $ty:ty; $($rem:tt)*) => {
        __env_flag_inner!($(#[$attr])* (pub ($($vis)+)) $key : $ty = || $crate::__missing_env_var(::std::stringify!($key)), $crate::ParseEnv::parse_env; $($rem)*);
    };

    // key: type = default;
    ($(#[$attr:meta])* $key:ident : $ty:ty = $default:expr; $($rem:tt)*) => {
        __env_flag_inner!($(#[$attr])* () $key : $ty = || $default, $crate::ParseEnv::parse_env; $($rem)*);
    };
    ($(#[$attr:meta])* pub $key:ident : $ty:ty = $default:expr; $($rem:tt)*) => {
        __env_flag_inner!($(#[$attr])* (pub) $key : $ty = || $default, $crate::ParseEnv::parse_env; $($rem)*);
    };
    ($(#[$attr:meta])* pub ($($vis:tt)+) $key:ident : $ty:ty = $default:expr; $($rem:tt)*) => {
        __env_flag_inner!($(#[$attr])* (pub ($($vis)+)) $key : $ty = || $default, $crate::ParseEnv::parse_env; $($rem)*);
    };

    // key: type, parse_fn;
    ($(#[$attr:meta])* $key:ident : $ty:ty, $parse_fn:expr; $($rem:tt)*) => {
        __env_flag_inner!($(#[$attr])* () $key : $ty = || $crate::__missing_env_var(::std::stringify!($key)), $parse_fn; $($rem)*);
    };
    ($(#[$attr:meta])* pub $key:ident : $ty:ty, $parse_fn:expr; $($rem:tt)*) => {
        __env_flag_inner!($(#[$attr])* (pub) $key : $ty = || $crate::__missing_env_var(::std::stringify!($key)), $parse_fn; $($rem)*);
    };
    ($(#[$attr:meta])* pub ($($vis:tt)+) $key:ident : $ty:ty, $parse_fn:expr; $($rem:tt)*) => {
        __env_flag_inner!($(#[$attr])* (pub ($($vis)+)) $key : $ty = || $crate::__missing_env_var(::std::stringify!($key)), $parse_fn; $($rem)*);
    };

    // key: type = default, parse_fn;
    ($(#[$attr:meta])* $key:ident : $ty:ty = $default:expr, $parse_fn:expr; $($rem:tt)*) => {
        __env_flag_inner!($(#[$attr])* () $key : $ty = || $default, $parse_fn; $($rem)*);
    };
    ($(#[$attr:meta])* pub $key:ident : $ty:ty = $default:expr, $parse_fn:expr; $($rem:tt)*) => {
        __env_flag_inner!($(#[$attr])* (pub) $key : $ty = || $default, $parse_fn; $($rem)*);
    };
    ($(#[$attr:meta])* pub ($($vis:tt)+) $key:ident : $ty:ty = $default:expr, $parse_fn:expr; $($rem:tt)*) => {
        __env_flag_inner!($(#[$attr])* (pub ($($vis)+)) $key : $ty = || $default, $parse_fn; $($rem)*);
    };

    () => {};
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_key_type_set() {
        std::env::set_var("ENV_FLAGS_TEST_KEY_TYPE_SET_PRIV", "80");
        std::env::set_var("ENV_FLAGS_TEST_KEY_TYPE_SET_CRATE", "81");
        std::env::set_var("ENV_FLAGS_TEST_KEY_TYPE_SET_PUB", "82");
        env_flags! {
            ENV_FLAGS_TEST_KEY_TYPE_SET_PRIV: u16;
            pub(crate) ENV_FLAGS_TEST_KEY_TYPE_SET_CRATE: u16;
            pub ENV_FLAGS_TEST_KEY_TYPE_SET_PUB: u16;
        };
        assert_eq!(*ENV_FLAGS_TEST_KEY_TYPE_SET_PRIV, 80u16);
        assert_eq!(*ENV_FLAGS_TEST_KEY_TYPE_SET_CRATE, 81u16);
        assert_eq!(*ENV_FLAGS_TEST_KEY_TYPE_SET_PUB, 82u16);
    }

    #[test]
    #[should_panic]
    fn test_key_type_unset() {
        env_flags! {
            pub ENV_FLAGS_TEST_KEY_TYPE_UNSET: u16;
        };
        let _ = *ENV_FLAGS_TEST_KEY_TYPE_UNSET;
    }

    #[test]
    fn test_default() {
        std::env::set_var("ENV_FLAGS_TEST_DEFAULT_SET_PRIV", "goodbye");
        std::env::set_var("ENV_FLAGS_TEST_DEFAULT_SET_CRATE", "goodbye");
        std::env::set_var("ENV_FLAGS_TEST_DEFAULT_SET_PUB", "goodbye");
        env_flags! {
            ENV_FLAGS_TEST_DEFAULT_SET_PRIV: &str = "hello";
            pub(crate) ENV_FLAGS_TEST_DEFAULT_SET_CRATE: &str = "hello";
            pub ENV_FLAGS_TEST_DEFAULT_SET_PUB: &str = "hello";

            pub ENV_FLAGS_TEST_DEFAULT_UNSET: &str = "world";
        };
        assert_eq!(*ENV_FLAGS_TEST_DEFAULT_SET_PRIV, "goodbye");
        assert_eq!(*ENV_FLAGS_TEST_DEFAULT_SET_CRATE, "goodbye");
        assert_eq!(*ENV_FLAGS_TEST_DEFAULT_SET_PUB, "goodbye");

        assert_eq!(*ENV_FLAGS_TEST_DEFAULT_UNSET, "world");
    }

    #[test]
    fn test_parse_fn() {
        std::env::set_var("ENV_FLAGS_TEST_PARSE_FN_PRIV", "250");
        std::env::set_var("ENV_FLAGS_TEST_PARSE_FN_CRATE", "5");
        std::env::set_var("ENV_FLAGS_TEST_PARSE_FN_PUB", "120");
        env_flags! {
            ENV_FLAGS_TEST_PARSE_FN_PRIV: Duration, |val| val.parse().map(Duration::from_millis);
            pub(crate) ENV_FLAGS_TEST_PARSE_FN_CRATE: Duration, |val| val.parse().map(Duration::from_secs);
            pub ENV_FLAGS_TEST_PARSE_FN_PUB: Duration, |val| val.parse().map(Duration::from_nanos);
        };
        assert_eq!(*ENV_FLAGS_TEST_PARSE_FN_PRIV, Duration::from_millis(250));
        assert_eq!(*ENV_FLAGS_TEST_PARSE_FN_CRATE, Duration::from_secs(5));
        assert_eq!(*ENV_FLAGS_TEST_PARSE_FN_PUB, Duration::from_nanos(120));
    }

    #[test]
    #[should_panic]
    fn test_parse_fn_unset() {
        env_flags! {
            pub(crate) ENV_FLAGS_TEST_PARSE_FN_UNSET: Duration, |val| val.parse().map(Duration::from_millis);
        };
        let _ = *ENV_FLAGS_TEST_PARSE_FN_UNSET;
    }

    #[test]
    fn test_parse_fn_default() {
        std::env::set_var("ENV_FLAGS_TEST_PARSE_FN_DEFAULT_PRIV", "10");
        std::env::set_var("ENV_FLAGS_TEST_PARSE_FN_DEFAULT_CRATE", "11");
        std::env::set_var("ENV_FLAGS_TEST_PARSE_FN_DEFAULT_PUB", "12");
        env_flags! {
            ENV_FLAGS_TEST_PARSE_FN_DEFAULT_PRIV: Duration = Duration::from_millis(5), |val| val.parse().map(Duration::from_millis);
            pub(crate) ENV_FLAGS_TEST_PARSE_FN_DEFAULT_CRATE: Duration = Duration::from_millis(5), |val| val.parse().map(Duration::from_millis);
            pub ENV_FLAGS_TEST_PARSE_FN_DEFAULT_PUB: Duration = Duration::from_millis(5), |val| val.parse().map(Duration::from_millis);

            ENV_FLAGS_TEST_PARSE_FN_DEFAULT_UNSET: Duration = Duration::from_millis(5), |val| val.parse().map(Duration::from_millis);
        };
        assert_eq!(
            *ENV_FLAGS_TEST_PARSE_FN_DEFAULT_PRIV,
            Duration::from_millis(10)
        );
        assert_eq!(
            *ENV_FLAGS_TEST_PARSE_FN_DEFAULT_CRATE,
            Duration::from_millis(11)
        );
        assert_eq!(
            *ENV_FLAGS_TEST_PARSE_FN_DEFAULT_PUB,
            Duration::from_millis(12)
        );
        assert_eq!(
            *ENV_FLAGS_TEST_PARSE_FN_DEFAULT_UNSET,
            Duration::from_millis(5)
        );
    }

    #[test]
    fn test_types_f32() {
        std::env::set_var("ENV_FLAGS_TEST_TYPES_F32_POS", "1.2");
        std::env::set_var("ENV_FLAGS_TEST_TYPES_F32_NEG", "-3.2");
        std::env::set_var("ENV_FLAGS_TEST_TYPES_F32_NAN", "nan");
        std::env::set_var("ENV_FLAGS_TEST_TYPES_F32_INF", "inf");
        env_flags! {
            ENV_FLAGS_TEST_TYPES_F32_POS: f32;
            ENV_FLAGS_TEST_TYPES_F32_NEG: f32;
            ENV_FLAGS_TEST_TYPES_F32_NAN: f32;
            ENV_FLAGS_TEST_TYPES_F32_INF: f32;
        };
        assert_eq!(*ENV_FLAGS_TEST_TYPES_F32_POS, 1.2);
        assert_eq!(*ENV_FLAGS_TEST_TYPES_F32_NEG, -3.2);
        assert!(ENV_FLAGS_TEST_TYPES_F32_NAN.is_nan());
        assert!(ENV_FLAGS_TEST_TYPES_F32_INF.is_infinite());
    }

    #[test]
    #[should_panic]
    fn test_invalid_f32() {
        std::env::set_var("ENV_FLAGS_TEST_INVALID_F32", "cat");
        env_flags! {
            ENV_FLAGS_TEST_INVALID_F32: f32 = 0.0;
        };
        let _ = *ENV_FLAGS_TEST_INVALID_F32;
    }

    #[test]
    fn test_types_f64() {
        std::env::set_var("ENV_FLAGS_TEST_TYPES_F64_POS", "41.1");
        std::env::set_var("ENV_FLAGS_TEST_TYPES_F64_NEG", "-0.4");
        std::env::set_var("ENV_FLAGS_TEST_TYPES_F64_NAN", "nan");
        std::env::set_var("ENV_FLAGS_TEST_TYPES_F64_INF", "inf");
        env_flags! {
            ENV_FLAGS_TEST_TYPES_F64_POS: f64;
            ENV_FLAGS_TEST_TYPES_F64_NEG: f64;
            ENV_FLAGS_TEST_TYPES_F64_NAN: f64;
            ENV_FLAGS_TEST_TYPES_F64_INF: f64;
        };
        assert_eq!(*ENV_FLAGS_TEST_TYPES_F64_POS, 41.1);
        assert_eq!(*ENV_FLAGS_TEST_TYPES_F64_NEG, -0.4);
        assert!(ENV_FLAGS_TEST_TYPES_F64_NAN.is_nan());
        assert!(ENV_FLAGS_TEST_TYPES_F64_INF.is_infinite());
    }

    #[test]
    #[should_panic]
    fn test_invalid_f64() {
        std::env::set_var("ENV_FLAGS_TEST_INVALID_F64", "cat");
        env_flags! {
            ENV_FLAGS_TEST_INVALID_F64: f64 = 0.0;
        };
        let _ = *ENV_FLAGS_TEST_INVALID_F64;
    }

    #[test]
    fn test_types_i8() {
        std::env::set_var("ENV_FLAGS_TEST_TYPES_I8_POS", "4");
        std::env::set_var("ENV_FLAGS_TEST_TYPES_I8_NEG", "-4");
        env_flags! {
            ENV_FLAGS_TEST_TYPES_I8_POS: i8;
            ENV_FLAGS_TEST_TYPES_I8_NEG: i8;
        };
        assert_eq!(*ENV_FLAGS_TEST_TYPES_I8_POS, 4);
        assert_eq!(*ENV_FLAGS_TEST_TYPES_I8_NEG, -4);
    }

    #[test]
    #[should_panic]
    fn test_invalid_i8() {
        std::env::set_var("ENV_FLAGS_TEST_INVALID_I8", "128");
        env_flags! {
            ENV_FLAGS_TEST_INVALID_I8: i8;
        };
        let _ = *ENV_FLAGS_TEST_INVALID_I8;
    }

    #[test]
    fn test_types_i16() {
        std::env::set_var("ENV_FLAGS_TEST_TYPES_I16_POS", "2559");
        std::env::set_var("ENV_FLAGS_TEST_TYPES_I16_NEG", "-2559");
        env_flags! {
            ENV_FLAGS_TEST_TYPES_I16_POS: i16;
            ENV_FLAGS_TEST_TYPES_I16_NEG: i16;
        };
        assert_eq!(*ENV_FLAGS_TEST_TYPES_I16_POS, 2559);
        assert_eq!(*ENV_FLAGS_TEST_TYPES_I16_NEG, -2559);
    }

    #[test]
    #[should_panic]
    fn test_invalid_i16() {
        std::env::set_var("ENV_FLAGS_TEST_INVALID_I16", "32768");
        env_flags! {
            ENV_FLAGS_TEST_INVALID_I16: i16 = 0;
        };
        let _ = *ENV_FLAGS_TEST_INVALID_I16;
    }

    #[test]
    fn test_types_i32() {
        std::env::set_var("ENV_FLAGS_TEST_TYPES_I32_POS", "124");
        std::env::set_var("ENV_FLAGS_TEST_TYPES_I32_NEG", "-124");
        env_flags! {
            ENV_FLAGS_TEST_TYPES_I32_POS: i32;
            ENV_FLAGS_TEST_TYPES_I32_NEG: i32;
        };
        assert_eq!(*ENV_FLAGS_TEST_TYPES_I32_POS, 124);
        assert_eq!(*ENV_FLAGS_TEST_TYPES_I32_NEG, -124);
    }

    #[test]
    #[should_panic]
    fn test_invalid_i32() {
        std::env::set_var("ENV_FLAGS_TEST_INVALID_I32", "2147483648");
        env_flags! {
            ENV_FLAGS_TEST_INVALID_I32: i32 = 0;
        };
        let _ = *ENV_FLAGS_TEST_INVALID_I32;
    }

    #[test]
    fn test_types_i64() {
        std::env::set_var("ENV_FLAGS_TEST_TYPES_I64_POS", "13966932211");
        std::env::set_var("ENV_FLAGS_TEST_TYPES_I64_NEG", "-13966932211");
        env_flags! {
            ENV_FLAGS_TEST_TYPES_I64_POS: i64;
            ENV_FLAGS_TEST_TYPES_I64_NEG: i64;
        }
        assert_eq!(*ENV_FLAGS_TEST_TYPES_I64_POS, 13966932211);
        assert_eq!(*ENV_FLAGS_TEST_TYPES_I64_NEG, -13966932211);
    }

    #[test]
    fn test_types_i128() {
        std::env::set_var("ENV_FLAGS_TEST_TYPES_I128_POS", "1020304995959399");
        std::env::set_var("ENV_FLAGS_TEST_TYPES_I128_NEG", "-1020304995959399");
        env_flags! {
            ENV_FLAGS_TEST_TYPES_I128_POS: i128;
            ENV_FLAGS_TEST_TYPES_I128_NEG: i128;
        };
        assert_eq!(*ENV_FLAGS_TEST_TYPES_I128_POS, 1020304995959399);
        assert_eq!(*ENV_FLAGS_TEST_TYPES_I128_NEG, -1020304995959399);
    }

    #[test]
    fn test_types_isize() {
        std::env::set_var("ENV_FLAGS_TEST_TYPES_ISIZE_POS", "29294");
        std::env::set_var("ENV_FLAGS_TEST_TYPES_ISIZE_NEG", "-29294");
        env_flags! {
            ENV_FLAGS_TEST_TYPES_ISIZE_POS: isize;
            ENV_FLAGS_TEST_TYPES_ISIZE_NEG: isize;
        };
        assert_eq!(*ENV_FLAGS_TEST_TYPES_ISIZE_POS, 29294);
        assert_eq!(*ENV_FLAGS_TEST_TYPES_ISIZE_NEG, -29294);
    }

    #[test]
    fn test_types_u8() {
        std::env::set_var("ENV_FLAGS_TEST_TYPES_U8", "10");
        env_flags! {
            ENV_FLAGS_TEST_TYPES_U8: u8;
        };
        assert_eq!(*ENV_FLAGS_TEST_TYPES_U8, 10);
    }

    #[test]
    fn test_types_u16() {
        std::env::set_var("ENV_FLAGS_TEST_TYPES_U16", "7432");
        env_flags! {
            ENV_FLAGS_TEST_TYPES_U16: u16;
        };
        assert_eq!(*ENV_FLAGS_TEST_TYPES_U16, 7432);
    }

    #[test]
    fn test_types_u32() {
        std::env::set_var("ENV_FLAGS_TEST_TYPES_U32", "305528");
        env_flags! {
            ENV_FLAGS_TEST_TYPES_U32: u32;
        };
        assert_eq!(*ENV_FLAGS_TEST_TYPES_U32, 305528);
    }

    #[test]
    fn test_types_u64() {
        std::env::set_var("ENV_FLAGS_TEST_TYPES_U64", "123456789");
        env_flags! {
            ENV_FLAGS_TEST_TYPES_U64: u64;
        };
        assert_eq!(*ENV_FLAGS_TEST_TYPES_U64, 123456789);
    }

    #[test]
    fn test_types_u128() {
        std::env::set_var("ENV_FLAGS_TEST_TYPES_U128", "2919239");
        env_flags! {
            ENV_FLAGS_TEST_TYPES_U128: u128;
        };
        assert_eq!(*ENV_FLAGS_TEST_TYPES_U128, 2919239);
    }

    #[test]
    fn test_types_usize() {
        std::env::set_var("ENV_FLAGS_TEST_TYPES_USIZE", "2939");
        env_flags! {
            ENV_FLAGS_TEST_TYPES_USIZE: usize;
        };
        assert_eq!(*ENV_FLAGS_TEST_TYPES_USIZE, 2939);
    }

    #[test]
    fn test_types_ipaddr() {
        std::env::set_var("ENV_FLAGS_TEST_TYPES_IPADDR", "0.0.0.0");
        env_flags! {
            ENV_FLAGS_TEST_TYPES_IPADDR: IpAddr;
        };
        assert_eq!(*ENV_FLAGS_TEST_TYPES_IPADDR, {
            let ip: Ipv4Addr = "0.0.0.0".parse().unwrap();
            ip
        });
    }

    #[test]
    fn test_types_ipv4addr() {
        std::env::set_var("ENV_FLAGS_TEST_TYPES_IPV4ADDR", "127.0.0.1");
        env_flags! {
            ENV_FLAGS_TEST_TYPES_IPV4ADDR: Ipv4Addr;
        };
        assert_eq!(*ENV_FLAGS_TEST_TYPES_IPV4ADDR, {
            let ip: Ipv4Addr = "127.0.0.1".parse().unwrap();
            ip
        });
    }

    #[test]
    fn test_types_ipv6addr() {
        std::env::set_var(
            "ENV_FLAGS_TEST_TYPES_IPV6ADDR",
            "2001:0000:130F:0000:0000:09C0:876A:130B",
        );
        env_flags! {
            ENV_FLAGS_TEST_TYPES_IPV6ADDR: Ipv6Addr;
        };
        assert_eq!(*ENV_FLAGS_TEST_TYPES_IPV6ADDR, {
            let ip: Ipv6Addr = "2001:0000:130F:0000:0000:09C0:876A:130B".parse().unwrap();
            ip
        });
    }

    #[test]
    fn test_types_socketaddr() {
        std::env::set_var("ENV_FLAGS_TEST_TYPES_SOCKETADDR", "192.168.0.1:8080");
        env_flags! {
            ENV_FLAGS_TEST_TYPES_SOCKETADDR: SocketAddr;
        };
        assert_eq!(
            *ENV_FLAGS_TEST_TYPES_SOCKETADDR,
            "192.168.0.1:8080".parse().unwrap()
        );
    }

    #[test]
    fn test_types_pathbuf() {
        std::env::set_var("ENV_FLAGS_TEST_TYPES_PATHBUF", "/var/lib/file.txt");
        env_flags! {
            ENV_FLAGS_TEST_TYPES_PATHBUF: PathBuf;
        }
        assert_eq!(
            *ENV_FLAGS_TEST_TYPES_PATHBUF,
            PathBuf::from("/var/lib/file.txt")
        );
    }

    #[test]
    fn test_types_option() {
        std::env::set_var("ENV_FLAGS_TEST_OPTION_SET", "cat");
        env_flags! {
            ENV_FLAGS_TEST_OPTION_UNSET: Option<&str> = None;
            ENV_FLAGS_TEST_OPTION_SET: Option<&str> = None;
        };
        assert!(ENV_FLAGS_TEST_OPTION_UNSET.is_none());
        assert_eq!(*ENV_FLAGS_TEST_OPTION_SET, Some("cat"));
    }

    #[test]
    fn test_types_vec() {
        std::env::set_var("ENV_FLAGS_TEST_VEC", "1,2,3,4");
        env_flags! {
            ENV_FLAGS_TEST_VEC: Vec<u32>;
        };
        assert_eq!(*ENV_FLAGS_TEST_VEC, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_types_hash_set() {
        std::env::set_var("ENV_FLAGS_TEST_HASH_SET", "1,2,3,4,1,3");
        env_flags! {
            ENV_FLAGS_TEST_HASH_SET: HashSet<u32>;
        };
        assert_eq!(
            *ENV_FLAGS_TEST_HASH_SET,
            [1, 2, 3, 4].into_iter().collect::<HashSet<u32>>()
        );
    }

    #[test]
    fn test_types_bool() {
        std::env::set_var("ENV_FLAGS_TEST_BOOL_TRUE", "true");
        std::env::set_var("ENV_FLAGS_TEST_BOOL_FALSE", "false");
        std::env::set_var("ENV_FLAGS_TEST_BOOL_TRUE_UPPER", "TRUE");
        std::env::set_var("ENV_FLAGS_TEST_BOOL_FALSE_UPPER", "FALSE");
        std::env::set_var("ENV_FLAGS_TEST_BOOL_0", "0");
        std::env::set_var("ENV_FLAGS_TEST_BOOL_1", "1");
        env_flags! {
            ENV_FLAGS_TEST_BOOL_TRUE: bool;
            ENV_FLAGS_TEST_BOOL_FALSE: bool;
            ENV_FLAGS_TEST_BOOL_TRUE_UPPER: bool;
            ENV_FLAGS_TEST_BOOL_FALSE_UPPER: bool;
            ENV_FLAGS_TEST_BOOL_0: bool;
            ENV_FLAGS_TEST_BOOL_1: bool;
        };
        assert_eq!(*ENV_FLAGS_TEST_BOOL_TRUE, true);
        assert_eq!(*ENV_FLAGS_TEST_BOOL_FALSE, false);
        assert_eq!(*ENV_FLAGS_TEST_BOOL_TRUE_UPPER, true);
        assert_eq!(*ENV_FLAGS_TEST_BOOL_FALSE_UPPER, false);
        assert_eq!(*ENV_FLAGS_TEST_BOOL_0, false);
        assert_eq!(*ENV_FLAGS_TEST_BOOL_1, true);
    }

    #[test]
    fn test_deref() {
        env_flags! {
            ENV_FLAGS_TEST_DEREF: &str = "hello";
        };

        fn print_str(s: &str) {
            println!("{}", s);
        }

        print_str(&ENV_FLAGS_TEST_DEREF);
        print_str(*ENV_FLAGS_TEST_DEREF);
    }

    #[test]
    fn test_debug() {
        env_flags! {
            ENV_FLAGS_TEST_DEBUG: &str = "cat";
        };

        let str = format!("{:?}", ENV_FLAGS_TEST_DEBUG);
        assert_eq!(str, "\"cat\"");
    }

    #[test]
    fn test_display() {
        env_flags! {
            ENV_FLAGS_TEST_DEBUG: &str = "cat";
        };

        let str = format!("{}", ENV_FLAGS_TEST_DEBUG);
        assert_eq!(str, "cat");
    }
}
