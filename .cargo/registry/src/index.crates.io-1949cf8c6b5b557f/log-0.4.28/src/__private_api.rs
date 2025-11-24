//! WARNING: this is not part of the crate's public API and is subject to change at any time

use self::sealed::KVs;
use crate::{logger, Level, Log, Metadata, Record};
use std::fmt::Arguments;
use std::panic::Location;
pub use std::{format_args, module_path, stringify};

#[cfg(not(feature = "kv"))]
pub type Value<'a> = &'a str;

mod sealed {
    /// Types for the `kv` argument.
    pub trait KVs<'a> {
        fn into_kvs(self) -> Option<&'a [(&'a str, super::Value<'a>)]>;
    }
}

// Types for the `kv` argument.

impl<'a> KVs<'a> for &'a [(&'a str, Value<'a>)] {
    #[inline]
    fn into_kvs(self) -> Option<&'a [(&'a str, Value<'a>)]> {
        Some(self)
    }
}

impl<'a> KVs<'a> for () {
    #[inline]
    fn into_kvs(self) -> Option<&'a [(&'a str, Value<'a>)]> {
        None
    }
}

// Log implementation.

/// The global logger proxy.
#[derive(Debug)]
pub struct GlobalLogger;

impl Log for GlobalLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        logger().enabled(metadata)
    }

    fn log(&self, record: &Record) {
        logger().log(record)
    }

    fn flush(&self) {
        logger().flush()
    }
}

// Split from `log` to reduce generics and code size
fn log_impl<L: Log>(
    logger: L,
    args: Arguments,
    level: Level,
    &(target, module_path, loc): &(&str, &'static str, &'static Location),
    kvs: Option<&[(&str, Value)]>,
) {
    #[cfg(not(feature = "kv"))]
    if kvs.is_some() {
        panic!("key-value support is experimental and must be enabled using the `kv` feature")
    }

    let mut builder = Record::builder();

    builder
        .args(args)
        .level(level)
        .target(target)
        .module_path_static(Some(module_path))
        .file_static(Some(loc.file()))
        .line(Some(loc.line()));

    #[cfg(feature = "kv")]
    builder.key_values(&kvs);

    logger.log(&builder.build());
}

pub fn log<'a, K, L>(
    logger: L,
    args: Arguments,
    level: Level,
    target_module_path_and_loc: &(&str, &'static str, &'static Location),
    kvs: K,
) where
    K: KVs<'a>,
    L: Log,
{
    log_impl(
        logger,
        args,
        level,
        target_module_path_and_loc,
        kvs.into_kvs(),
    )
}

pub fn enabled<L: Log>(logger: L, level: Level, target: &str) -> bool {
    logger.enabled(&Metadata::builder().level(level).target(target).build())
}

#[track_caller]
pub fn loc() -> &'static Location<'static> {
    Location::caller()
}

#[cfg(feature = "kv")]
mod kv_support {
    use crate::kv;

    pub type Value<'a> = kv::Value<'a>;

    // NOTE: Many functions here accept a double reference &&V
    // This is so V itself can be ?Sized, while still letting us
    // erase it to some dyn Trait (because &T is sized)

    pub fn capture_to_value<'a, V: kv::ToValue + ?Sized>(v: &'a &'a V) -> Value<'a> {
        v.to_value()
    }

    pub fn capture_debug<'a, V: core::fmt::Debug + ?Sized>(v: &'a &'a V) -> Value<'a> {
        Value::from_debug(v)
    }

    pub fn capture_display<'a, V: core::fmt::Display + ?Sized>(v: &'a &'a V) -> Value<'a> {
        Value::from_display(v)
    }

    #[cfg(feature = "kv_std")]
    pub fn capture_error<'a>(v: &'a (dyn std::error::Error + 'static)) -> Value<'a> {
        Value::from_dyn_error(v)
    }

    #[cfg(feature = "kv_sval")]
    pub fn capture_sval<'a, V: sval::Value + ?Sized>(v: &'a &'a V) -> Value<'a> {
        Value::from_sval(v)
    }

    #[cfg(feature = "kv_serde")]
    pub fn capture_serde<'a, V: serde::Serialize + ?Sized>(v: &'a &'a V) -> Value<'a> {
        Value::from_serde(v)
    }
}

#[cfg(feature = "kv")]
pub use self::kv_support::*;
