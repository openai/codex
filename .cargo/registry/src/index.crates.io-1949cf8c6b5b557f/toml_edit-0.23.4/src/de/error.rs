/// Errors that can occur when deserializing a type.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Error {
    inner: crate::TomlError,
}

impl Error {
    pub(crate) fn custom<T>(msg: T, span: Option<std::ops::Range<usize>>) -> Self
    where
        T: std::fmt::Display,
    {
        Self {
            inner: crate::TomlError::custom(msg.to_string(), span),
        }
    }

    /// Add key while unwinding
    pub fn add_key(&mut self, key: String) {
        self.inner.add_key(key);
    }

    /// What went wrong
    pub fn message(&self) -> &str {
        self.inner.message()
    }

    /// The start/end index into the original document where the error occurred
    pub fn span(&self) -> Option<std::ops::Range<usize>> {
        self.inner.span()
    }

    pub(crate) fn set_span(&mut self, span: Option<std::ops::Range<usize>>) {
        self.inner.set_span(span);
    }

    /// Provide the encoded TOML the error applies to
    pub fn set_input(&mut self, input: Option<&str>) {
        self.inner.set_input(input);
    }
}

impl serde::de::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        Self::custom(msg, None)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl From<crate::TomlError> for Error {
    fn from(e: crate::TomlError) -> Self {
        Self { inner: e }
    }
}

impl From<Error> for crate::TomlError {
    fn from(e: Error) -> Self {
        e.inner
    }
}

impl std::error::Error for Error {}
