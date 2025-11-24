//! # TextMapPropagator
//!
//! [`TextMapPropagator`] performs the injection and extraction of a cross-cutting concern value as
//! string key/values pairs into carriers that travel in-band across process boundaries.
//!
//! The carrier of propagated data on both the client (injector) and server (extractor) side is
//! usually an HTTP request.
//!
//! In order to increase compatibility, the key/value pairs MUST only consist of US-ASCII characters
//! that make up valid HTTP header fields as per RFC 7230.
use crate::{
    propagation::{Extractor, Injector},
    Context,
};
use std::fmt::Debug;
use std::slice;

/// Methods to inject and extract a value as text into injectors and extractors that travel
/// in-band across process boundaries.
pub trait TextMapPropagator: Debug {
    /// Properly encodes the values of the current [`Context`] and injects them into
    /// the [`Injector`].
    ///
    /// [`Context`]: crate::Context
    /// [`Injector`]: crate::propagation::Injector
    fn inject(&self, injector: &mut dyn Injector) {
        Context::map_current(|cx| self.inject_context(cx, injector))
    }

    /// Properly encodes the values of the [`Context`] and injects them into the
    /// [`Injector`].
    ///
    /// [`Context`]: crate::Context
    /// [`Injector`]: crate::propagation::Injector
    fn inject_context(&self, cx: &Context, injector: &mut dyn Injector);

    /// Retrieves encoded data using the provided [`Extractor`]. If no data for this
    /// format was retrieved OR if the retrieved data is invalid, then the current
    /// [`Context`] is returned.
    ///
    /// [`Context`]: crate::Context
    /// [`Injector`]: crate::propagation::Extractor
    fn extract(&self, extractor: &dyn Extractor) -> Context {
        Context::map_current(|cx| self.extract_with_context(cx, extractor))
    }

    /// Retrieves encoded data using the provided [`Extractor`]. If no data for this
    /// format was retrieved OR if the retrieved data is invalid, then the given
    /// [`Context`] is returned.
    ///
    /// [`Context`]: crate::Context
    /// [`Injector`]: crate::propagation::Extractor
    fn extract_with_context(&self, cx: &Context, extractor: &dyn Extractor) -> Context;

    /// Returns iter of fields used by [`TextMapPropagator`]
    ///
    fn fields(&self) -> FieldIter<'_>;
}

/// An iterator over fields of a [`TextMapPropagator`]
///
#[derive(Debug)]
pub struct FieldIter<'a>(slice::Iter<'a, String>);

impl<'a> FieldIter<'a> {
    /// Create a new `FieldIter` from a slice of propagator fields
    pub fn new(fields: &'a [String]) -> Self {
        FieldIter(fields.iter())
    }
}

impl<'a> Iterator for FieldIter<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|field| field.as_str())
    }
}
