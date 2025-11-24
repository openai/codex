use std::env;
use std::{borrow::Cow, sync::Arc};

use crate::transports::DefaultTransportFactory;
use crate::types::Dsn;
use crate::{ClientOptions, Integration};

/// Apply default client options.
///
/// Extends the given `ClientOptions` with default options such as a default
/// transport, a set of default integrations if not requested otherwise, and
/// also sets the `dsn`, `release`, `environment`, and proxy settings based on
/// environment variables.
///
/// When the [`ClientOptions::default_integrations`] option is set to
/// `true` (the default), the following integrations will be added *before*
/// any manually defined integrations, depending on enabled feature flags:
///
/// 1. [`AttachStacktraceIntegration`] (`feature = "backtrace"`)
/// 2. [`DebugImagesIntegration`] (`feature = "debug-images"`)
/// 3. [`ContextIntegration`] (`feature = "contexts"`)
/// 4. [`PanicIntegration`] (`feature = "panic"`)
/// 5. [`ProcessStacktraceIntegration`] (`feature = "backtrace"`)
///
/// Some integrations can be used multiple times, however, the
/// [`PanicIntegration`] can not, and it will not pick up custom panic
/// extractors when it is defined multiple times.
///
/// # Examples
/// ```
/// std::env::set_var("SENTRY_RELEASE", "release-from-env");
///
/// let options = sentry::ClientOptions::default();
/// assert_eq!(options.release, None);
/// assert!(options.transport.is_none());
///
/// let options = sentry::apply_defaults(options);
/// assert_eq!(options.release, Some("release-from-env".into()));
/// assert!(options.transport.is_some());
/// ```
///
/// [`AttachStacktraceIntegration`]: integrations/backtrace/struct.AttachStacktraceIntegration.html
/// [`DebugImagesIntegration`]: integrations/debug_images/struct.DebugImagesIntegration.html
/// [`ContextIntegration`]: integrations/contexts/struct.ContextIntegration.html
/// [`PanicIntegration`]: integrations/panic/struct.PanicIntegration.html
/// [`ProcessStacktraceIntegration`]: integrations/backtrace/struct.ProcessStacktraceIntegration.html
pub fn apply_defaults(mut opts: ClientOptions) -> ClientOptions {
    if opts.transport.is_none() {
        opts.transport = Some(Arc::new(DefaultTransportFactory));
    }
    if opts.default_integrations {
        // default integrations need to be ordered *before* custom integrations,
        // since they also process events in order
        let mut integrations: Vec<Arc<dyn Integration>> = vec![];
        #[cfg(feature = "backtrace")]
        {
            integrations.push(Arc::new(sentry_backtrace::AttachStacktraceIntegration));
        }
        #[cfg(feature = "debug-images")]
        {
            integrations.push(Arc::new(
                sentry_debug_images::DebugImagesIntegration::default(),
            ))
        }
        #[cfg(feature = "contexts")]
        {
            integrations.push(Arc::new(sentry_contexts::ContextIntegration::default()));
        }
        #[cfg(feature = "panic")]
        {
            integrations.push(Arc::new(sentry_panic::PanicIntegration::default()));
        }
        #[cfg(feature = "backtrace")]
        {
            integrations.push(Arc::new(sentry_backtrace::ProcessStacktraceIntegration));
        }
        integrations.extend(opts.integrations);
        opts.integrations = integrations;
    }
    if opts.dsn.is_none() {
        opts.dsn = env::var("SENTRY_DSN")
            .ok()
            .and_then(|dsn| dsn.parse::<Dsn>().ok());
    }
    if opts.release.is_none() {
        opts.release = env::var("SENTRY_RELEASE").ok().map(Cow::Owned);
    }
    if opts.environment.is_none() {
        opts.environment =
            env::var("SENTRY_ENVIRONMENT")
                .ok()
                .map(Cow::Owned)
                .or(Some(Cow::Borrowed(if cfg!(debug_assertions) {
                    "development"
                } else {
                    "production"
                })));
    }
    if opts.http_proxy.is_none() {
        opts.http_proxy = std::env::var("HTTP_PROXY")
            .ok()
            .map(Cow::Owned)
            .or_else(|| std::env::var("http_proxy").ok().map(Cow::Owned));
    }
    if opts.https_proxy.is_none() {
        opts.https_proxy = std::env::var("HTTPS_PROXY")
            .ok()
            .map(Cow::Owned)
            .or_else(|| std::env::var("https_proxy").ok().map(Cow::Owned))
            .or_else(|| opts.http_proxy.clone());
    }
    if let Ok(accept_invalid_certs) = std::env::var("SSL_VERIFY") {
        opts.accept_invalid_certs = !accept_invalid_certs.parse().unwrap_or(true);
    }
    opts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_environment() {
        let opts = ClientOptions {
            environment: Some("explicit-env".into()),
            ..Default::default()
        };
        let opts = apply_defaults(opts);
        assert_eq!(opts.environment.unwrap(), "explicit-env");

        let opts = apply_defaults(Default::default());
        // I doubt anyone runs test code without debug assertions
        assert_eq!(opts.environment.unwrap(), "development");

        env::set_var("SENTRY_ENVIRONMENT", "env-from-env");
        let opts = apply_defaults(Default::default());
        assert_eq!(opts.environment.unwrap(), "env-from-env");
    }
}
