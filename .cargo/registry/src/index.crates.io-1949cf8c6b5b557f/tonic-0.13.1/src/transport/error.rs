use std::{error::Error as StdError, fmt};

type Source = Box<dyn StdError + Send + Sync + 'static>;

/// Error's that originate from the client or server;
pub struct Error {
    inner: ErrorImpl,
}

struct ErrorImpl {
    kind: Kind,
    source: Option<Source>,
}

#[derive(Debug)]
pub(crate) enum Kind {
    Transport,
    #[cfg(feature = "channel")]
    InvalidUri,
    #[cfg(feature = "channel")]
    InvalidUserAgent,
    #[cfg(all(feature = "_tls-any", feature = "channel"))]
    InvalidTlsConfigForUds,
}

impl Error {
    pub(crate) fn new(kind: Kind) -> Self {
        Self {
            inner: ErrorImpl { kind, source: None },
        }
    }

    pub(crate) fn with(mut self, source: impl Into<Source>) -> Self {
        self.inner.source = Some(source.into());
        self
    }

    pub(crate) fn from_source(source: impl Into<crate::BoxError>) -> Self {
        Error::new(Kind::Transport).with(source)
    }

    #[cfg(feature = "channel")]
    pub(crate) fn new_invalid_uri() -> Self {
        Error::new(Kind::InvalidUri)
    }

    #[cfg(feature = "channel")]
    pub(crate) fn new_invalid_user_agent() -> Self {
        Error::new(Kind::InvalidUserAgent)
    }

    fn description(&self) -> &str {
        match &self.inner.kind {
            Kind::Transport => "transport error",
            #[cfg(feature = "channel")]
            Kind::InvalidUri => "invalid URI",
            #[cfg(feature = "channel")]
            Kind::InvalidUserAgent => "user agent is not a valid header value",
            #[cfg(all(feature = "_tls-any", feature = "channel"))]
            Kind::InvalidTlsConfigForUds => "cannot apply TLS config for unix domain socket",
        }
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut f = f.debug_tuple("tonic::transport::Error");

        f.field(&self.inner.kind);

        if let Some(source) = &self.inner.source {
            f.field(source);
        }

        f.finish()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.description())
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.inner
            .source
            .as_ref()
            .map(|source| &**source as &(dyn StdError + 'static))
    }
}
