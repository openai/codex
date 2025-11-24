//! The `config` module provides a generic mechanism for loading and managing
//! request-scoped configuration.
//!
//! # Design Overview
//!
//! This module is centered around two abstractions:
//!
//! - The [`RequestConfigValue`] trait, used to associate a config key type with its value type.
//! - The [`RequestConfig`] struct, which wraps an optional value of the type linked via [`RequestConfigValue`].
//!
//! Under the hood, the [`RequestConfig`] struct holds a single value for the associated config type.
//! This value can be conveniently accessed, inserted, or mutated using [`http::Extensions`],
//! enabling type-safe configuration storage and retrieval on a per-request basis.
//!
//! # Motivation
//!
//! The key design benefit is the ability to store multiple config types—potentially even with the same
//! value type (e.g., [`Duration`])—without code duplication or ambiguity. By leveraging trait association,
//! each config key is distinct at the type level, while code for storage and access remains totally generic.
//!
//! # Usage
//!
//! Implement [`RequestConfigValue`] for any marker type you wish to use as a config key,
//! specifying the associated value type. Then use [`RequestConfig<T>`] in [`Extensions`]
//! to set or retrieve config values for each key type in a uniform way.

use std::any::type_name;
use std::fmt::Debug;
use std::time::Duration;

use http::Extensions;

/// This trait is empty and is only used to associate a configuration key type with its
/// corresponding value type.
pub(crate) trait RequestConfigValue: Copy + Clone + 'static {
    type Value: Clone + Debug + Send + Sync + 'static;
}

/// RequestConfig carries a request-scoped configuration value.
#[derive(Clone, Copy)]
pub(crate) struct RequestConfig<T: RequestConfigValue>(Option<T::Value>);

impl<T: RequestConfigValue> Default for RequestConfig<T> {
    fn default() -> Self {
        RequestConfig(None)
    }
}

impl<T> RequestConfig<T>
where
    T: RequestConfigValue,
{
    pub(crate) fn new(v: Option<T::Value>) -> Self {
        RequestConfig(v)
    }

    /// format request config value as struct field.
    ///
    /// We provide this API directly to avoid leak internal value to callers.
    pub(crate) fn fmt_as_field(&self, f: &mut std::fmt::DebugStruct<'_, '_>) {
        if let Some(v) = &self.0 {
            f.field(type_name::<T>(), v);
        }
    }

    /// Retrieve the value from the request-scoped configuration.
    ///
    /// If the request specifies a value, use that value; otherwise, attempt to retrieve it from the current instance (typically a client instance).
    pub(crate) fn fetch<'client, 'request>(
        &'client self,
        ext: &'request Extensions,
    ) -> Option<&'request T::Value>
    where
        'client: 'request,
    {
        ext.get::<RequestConfig<T>>()
            .and_then(|v| v.0.as_ref())
            .or(self.0.as_ref())
    }

    /// Retrieve the value from the request's Extensions.
    pub(crate) fn get(ext: &Extensions) -> Option<&T::Value> {
        ext.get::<RequestConfig<T>>().and_then(|v| v.0.as_ref())
    }

    /// Retrieve the mutable value from the request's Extensions.
    pub(crate) fn get_mut(ext: &mut Extensions) -> &mut Option<T::Value> {
        let cfg = ext.get_or_insert_default::<RequestConfig<T>>();
        &mut cfg.0
    }
}

// ================================
//
// The following sections are all configuration types
// provided by reqwets.
//
// To add a new config:
//
// 1. create a new struct for the config key like `RequestTimeout`.
// 2. implement `RequestConfigValue` for the struct, the `Value` is the config value's type.
//
// ================================

#[derive(Clone, Copy)]
pub(crate) struct RequestTimeout;

impl RequestConfigValue for RequestTimeout {
    type Value = Duration;
}
