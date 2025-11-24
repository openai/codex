use crate::{trace::Tracer, InstrumentationScope};
use std::borrow::Cow;

/// Types that can create instances of [`Tracer`].
///
/// See the [`global`] module for examples of storing and retrieving tracer
/// provider instances.
///
/// [`global`]: crate::global
pub trait TracerProvider {
    /// The [`Tracer`] type that this provider will return.
    type Tracer: Tracer;

    /// Returns a new tracer with the given name.
    ///
    /// The `name` should be the application name or the name of the library
    /// providing instrumentation. If the name is empty, then an
    /// implementation-defined default name may be used instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::{global, trace::TracerProvider};
    /// use opentelemetry::KeyValue;
    ///
    /// let provider = global::tracer_provider();
    ///
    /// // tracer used in applications/binaries
    /// let tracer = provider.tracer("my_app");
    /// ```
    fn tracer(&self, name: impl Into<Cow<'static, str>>) -> Self::Tracer {
        let scope = InstrumentationScope::builder(name).build();
        self.tracer_with_scope(scope)
    }

    /// Returns a new versioned tracer with the given instrumentation scope.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::{global, InstrumentationScope, trace::TracerProvider};
    ///
    /// let provider = global::tracer_provider();
    ///
    /// // tracer used in applications/binaries
    /// let tracer = provider.tracer("my_app");
    ///
    /// // tracer used in libraries/crates that optionally includes version and schema url
    /// let scope =
    ///     InstrumentationScope::builder(env!("CARGO_PKG_NAME"))
    ///         .with_version(env!("CARGO_PKG_VERSION"))
    ///         .with_schema_url("https://opentelemetry.io/schema/1.0.0")
    ///         .build();
    ///
    /// let tracer = provider.tracer_with_scope(scope);
    /// ```
    fn tracer_with_scope(&self, scope: InstrumentationScope) -> Self::Tracer;
}
