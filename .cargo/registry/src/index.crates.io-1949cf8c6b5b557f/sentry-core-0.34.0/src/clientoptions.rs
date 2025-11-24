use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use crate::constants::USER_AGENT;
use crate::performance::TracesSampler;
use crate::protocol::{Breadcrumb, Event};
use crate::types::Dsn;
use crate::{Integration, IntoDsn, TransportFactory};

/// Type alias for before event/breadcrumb handlers.
pub type BeforeCallback<T> = Arc<dyn Fn(T) -> Option<T> + Send + Sync>;

/// The Session Mode of the SDK.
///
/// Depending on the use-case, the SDK can be set to two different session modes:
///
/// * **Application Mode Sessions**:
///   This mode should be used for user-attended programs, which typically have
///   a single long running session that span the applications' lifetime.
///
/// * **Request Mode Sessions**:
///   This mode is intended for servers that use one session per incoming
///   request, and thus have a lot of very short lived sessions.
///
/// Setting the SDK to *request-mode* sessions means that session durations will
/// not be tracked, and sessions will be pre-aggregated before being sent upstream.
/// This applies both to automatic and manually triggered sessions.
///
/// **NOTE**: Support for *request-mode* sessions was added in Sentry `21.2`.
///
/// See the [Documentation on Session Modes](https://develop.sentry.dev/sdk/sessions/#sdk-considerations)
/// for more information.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SessionMode {
    /// Long running application session.
    Application,
    /// Lots of short per-request sessions.
    Request,
}

/// Configuration settings for the client.
///
/// These options are explained in more detail in the general
/// [sentry documentation](https://docs.sentry.io/error-reporting/configuration/?platform=rust).
///
/// # Examples
///
/// ```
/// let _options = sentry::ClientOptions {
///     debug: true,
///     ..Default::default()
/// };
/// ```
#[derive(Clone)]
pub struct ClientOptions {
    // Common options
    /// The DSN to use.  If not set the client is effectively disabled.
    pub dsn: Option<Dsn>,
    /// Enables debug mode.
    ///
    /// In debug mode debug information is printed to stderr to help you understand what
    /// sentry is doing.  When the `log` feature is enabled, Sentry will instead
    /// log to the `sentry` logger independently of this flag with the `Debug` level.
    pub debug: bool,
    /// The release to be sent with events.
    pub release: Option<Cow<'static, str>>,
    /// The environment to be sent with events.
    ///
    /// Defaults to either `"development"` or `"production"` depending on the
    /// `debug_assertions` cfg-attribute.
    pub environment: Option<Cow<'static, str>>,
    /// The sample rate for event submission. (0.0 - 1.0, defaults to 1.0)
    pub sample_rate: f32,
    /// The sample rate for tracing transactions. (0.0 - 1.0, defaults to 0.0)
    pub traces_sample_rate: f32,
    /// If given, called with a SamplingContext for each transaction to determine the sampling rate.
    ///
    /// Return a sample rate between 0.0 and 1.0 for the transaction in question.
    /// Takes priority over the `sample_rate`.
    pub traces_sampler: Option<Arc<TracesSampler>>,
    /// Maximum number of breadcrumbs. (defaults to 100)
    pub max_breadcrumbs: usize,
    /// Attaches stacktraces to messages.
    pub attach_stacktrace: bool,
    /// If turned on some default PII informat is attached.
    pub send_default_pii: bool,
    /// The server name to be reported.
    pub server_name: Option<Cow<'static, str>>,
    /// Module prefixes that are always considered "in_app".
    pub in_app_include: Vec<&'static str>,
    /// Module prefixes that are never "in_app".
    pub in_app_exclude: Vec<&'static str>,
    // Integration options
    /// A list of integrations to enable.
    ///
    /// See [`sentry::integrations`](integrations/index.html#installing-integrations) for
    /// how to use this to enable extra integrations.
    pub integrations: Vec<Arc<dyn Integration>>,
    /// Whether to add default integrations.
    ///
    /// See [`sentry::integrations`](integrations/index.html#default-integrations) for
    /// details how this works and interacts with manually installed integrations.
    pub default_integrations: bool,
    // Hooks
    /// Callback that is executed before event sending.
    pub before_send: Option<BeforeCallback<Event<'static>>>,
    /// Callback that is executed for each Breadcrumb being added.
    pub before_breadcrumb: Option<BeforeCallback<Breadcrumb>>,
    // Transport options
    /// The transport to use.
    ///
    /// This is typically either a boxed function taking the client options by
    /// reference and returning a `Transport`, a boxed `Arc<Transport>` or
    /// alternatively the `DefaultTransportFactory`.
    pub transport: Option<Arc<dyn TransportFactory>>,
    /// An optional HTTP proxy to use.
    ///
    /// This will default to the `http_proxy` environment variable.
    pub http_proxy: Option<Cow<'static, str>>,
    /// An optional HTTPS proxy to use.
    ///
    /// This will default to the `HTTPS_PROXY` environment variable
    /// or `http_proxy` if that one exists.
    pub https_proxy: Option<Cow<'static, str>>,
    /// The timeout on client drop for draining events on shutdown.
    pub shutdown_timeout: Duration,
    // Other options not documented in Unified API
    /// Disable SSL verification.
    ///
    /// # Warning
    ///
    /// This introduces significant vulnerabilities, and should only be used as a last resort.
    pub accept_invalid_certs: bool,
    /// Enable Release Health Session tracking.
    ///
    /// When automatic session tracking is enabled, a new "user-mode" session
    /// is started at the time of `sentry::init`, and will persist for the
    /// application lifetime.
    pub auto_session_tracking: bool,
    /// Determine how Sessions are being tracked.
    pub session_mode: SessionMode,
    /// Border frames which indicate a border from a backtrace to
    /// useless internals. Some are automatically included.
    pub extra_border_frames: Vec<&'static str>,
    /// Automatically trim backtraces of junk before sending. (defaults to true)
    pub trim_backtraces: bool,
    /// The user agent that should be reported.
    pub user_agent: Cow<'static, str>,
}

impl ClientOptions {
    /// Creates new Options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a configured integration to the options.
    ///
    /// # Examples
    ///
    /// ```
    /// struct MyIntegration;
    ///
    /// impl sentry::Integration for MyIntegration {}
    ///
    /// let options = sentry::ClientOptions::new().add_integration(MyIntegration);
    /// assert_eq!(options.integrations.len(), 1);
    /// ```
    #[must_use]
    pub fn add_integration<I: Integration>(mut self, integration: I) -> Self {
        self.integrations.push(Arc::new(integration));
        self
    }
}

impl fmt::Debug for ClientOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[derive(Debug)]
        struct BeforeSend;
        let before_send = self.before_send.as_ref().map(|_| BeforeSend);
        #[derive(Debug)]
        struct BeforeBreadcrumb;
        let before_breadcrumb = self.before_breadcrumb.as_ref().map(|_| BeforeBreadcrumb);
        #[derive(Debug)]
        struct TransportFactory;

        let integrations: Vec<_> = self.integrations.iter().map(|i| i.name()).collect();

        f.debug_struct("ClientOptions")
            .field("dsn", &self.dsn)
            .field("debug", &self.debug)
            .field("release", &self.release)
            .field("environment", &self.environment)
            .field("sample_rate", &self.sample_rate)
            .field("traces_sample_rate", &self.traces_sample_rate)
            .field(
                "traces_sampler",
                &self
                    .traces_sampler
                    .as_ref()
                    .map(|arc| std::ptr::addr_of!(**arc)),
            )
            .field("max_breadcrumbs", &self.max_breadcrumbs)
            .field("attach_stacktrace", &self.attach_stacktrace)
            .field("send_default_pii", &self.send_default_pii)
            .field("server_name", &self.server_name)
            .field("in_app_include", &self.in_app_include)
            .field("in_app_exclude", &self.in_app_exclude)
            .field("integrations", &integrations)
            .field("default_integrations", &self.default_integrations)
            .field("before_send", &before_send)
            .field("before_breadcrumb", &before_breadcrumb)
            .field("transport", &TransportFactory)
            .field("http_proxy", &self.http_proxy)
            .field("https_proxy", &self.https_proxy)
            .field("shutdown_timeout", &self.shutdown_timeout)
            .field("accept_invalid_certs", &self.accept_invalid_certs)
            .field("auto_session_tracking", &self.auto_session_tracking)
            .field("session_mode", &self.session_mode)
            .field("extra_border_frames", &self.extra_border_frames)
            .field("trim_backtraces", &self.trim_backtraces)
            .field("user_agent", &self.user_agent)
            .finish()
    }
}

impl Default for ClientOptions {
    fn default() -> ClientOptions {
        ClientOptions {
            dsn: None,
            debug: false,
            release: None,
            environment: None,
            sample_rate: 1.0,
            traces_sample_rate: 0.0,
            traces_sampler: None,
            max_breadcrumbs: 100,
            attach_stacktrace: false,
            send_default_pii: false,
            server_name: None,
            in_app_include: vec![],
            in_app_exclude: vec![],
            integrations: vec![],
            default_integrations: true,
            before_send: None,
            before_breadcrumb: None,
            transport: None,
            http_proxy: None,
            https_proxy: None,
            shutdown_timeout: Duration::from_secs(2),
            accept_invalid_certs: false,
            auto_session_tracking: false,
            session_mode: SessionMode::Application,
            extra_border_frames: vec![],
            trim_backtraces: true,
            user_agent: Cow::Borrowed(USER_AGENT),
        }
    }
}

impl<T: IntoDsn> From<(T, ClientOptions)> for ClientOptions {
    fn from((into_dsn, mut opts): (T, ClientOptions)) -> ClientOptions {
        opts.dsn = into_dsn.into_dsn().expect("invalid value for DSN");
        opts
    }
}

impl<T: IntoDsn> From<T> for ClientOptions {
    fn from(into_dsn: T) -> ClientOptions {
        ClientOptions {
            dsn: into_dsn.into_dsn().expect("invalid value for DSN"),
            ..ClientOptions::default()
        }
    }
}
