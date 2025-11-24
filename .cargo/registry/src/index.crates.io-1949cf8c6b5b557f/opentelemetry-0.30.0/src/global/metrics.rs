use crate::metrics::{self, Meter, MeterProvider};
use crate::{otel_error, otel_info, InstrumentationScope};
use std::sync::{Arc, OnceLock, RwLock};

type GlobalMeterProvider = Arc<dyn MeterProvider + Send + Sync>;

/// The global `MeterProvider` singleton.
static GLOBAL_METER_PROVIDER: OnceLock<RwLock<GlobalMeterProvider>> = OnceLock::new();

#[inline]
fn global_meter_provider() -> &'static RwLock<GlobalMeterProvider> {
    GLOBAL_METER_PROVIDER
        .get_or_init(|| RwLock::new(Arc::new(crate::metrics::noop::NoopMeterProvider::new())))
}

/// Sets the given [`MeterProvider`] instance as the current global meter
/// provider.
/// Libraries should NOT call this function. It is intended for applications/executables.
///
/// **NOTE:** This function should be called before getting [`Meter`] instances via [`meter()`] or [`meter_with_scope()`]. Otherwise, you could get no-op [`Meter`] instances.
pub fn set_meter_provider<P>(new_provider: P)
where
    P: metrics::MeterProvider + Send + Sync + 'static,
{
    // Try to set the global meter provider. If the RwLock is poisoned, we'll log an error.
    let mut global_provider = global_meter_provider().write();
    if let Ok(ref mut provider) = global_provider {
        **provider = Arc::new(new_provider);
        otel_info!(name: "MeterProvider.GlobalSet", message = "Global meter provider is set. Meters can now be created using global::meter() or global::meter_with_scope().");
    } else {
        otel_error!(name: "MeterProvider.GlobalSetFailed", message = "Setting global meter provider failed. Meters created using global::meter() or global::meter_with_scope() will not function. Report this issue in OpenTelemetry repo.");
    }
}

/// Returns an instance of the currently configured global [`MeterProvider`].
pub fn meter_provider() -> GlobalMeterProvider {
    // Try to get the global meter provider. If the RwLock is poisoned, we'll log an error and return a NoopMeterProvider.
    let global_provider = global_meter_provider().read();
    if let Ok(provider) = global_provider {
        provider.clone()
    } else {
        otel_error!(name: "MeterProvider.GlobalGetFailed", message = "Getting global meter provider failed. Meters created using global::meter() or global::meter_with_scope() will not function. Report this issue in OpenTelemetry repo.");
        Arc::new(crate::metrics::noop::NoopMeterProvider::new())
    }
}

/// Creates a named [`Meter`] via the currently configured global [`MeterProvider`].
///
/// This is a more convenient way of expressing `global::meter_provider().meter(name)`.
///
/// **NOTE:** Calls to [`meter()`] return a [`Meter`] backed by the global [`MeterProvider`] configured during the method invocation.
/// If the global [`MeterProvider`] is changed after getting [`Meter`] instances from these calls, the [`Meter`] instances returned will not reflect the change.
pub fn meter(name: &'static str) -> Meter {
    meter_provider().meter(name)
}

/// Creates a [`Meter`] with the given instrumentation scope.
///
/// This is a simpler alternative to `global::meter_provider().meter_with_scope(...)`
///
/// **NOTE:** Calls to [`meter_with_scope()`] return a [`Meter`] backed by the global [`MeterProvider`] configured during the method invocation.
/// If the global [`MeterProvider`] is changed after getting [`Meter`] instances from these calls, the [`Meter`] instances returned will not reflect the change.
///
/// # Example
///
/// ```
/// use std::sync::Arc;
/// use opentelemetry::global::meter_with_scope;
/// use opentelemetry::InstrumentationScope;
/// use opentelemetry::KeyValue;
///
/// let scope = InstrumentationScope::builder("io.opentelemetry")
///     .with_version("0.17")
///     .with_schema_url("https://opentelemetry.io/schema/1.2.0")
///     .with_attributes(vec![(KeyValue::new("key", "value"))])
///     .build();
///
/// let meter = meter_with_scope(scope);
/// ```
pub fn meter_with_scope(scope: InstrumentationScope) -> Meter {
    meter_provider().meter_with_scope(scope)
}
