//! Removes credential JSON data errors while preserving native storage diagnostics and typed causes.

#[derive(thiserror::Error)]
struct CredentialStorageError {
    context: &'static str,
    #[source]
    source: anyhow::Error,
}

impl std::fmt::Display for CredentialStorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {:#}", self.context, self.source)
    }
}

impl std::fmt::Debug for CredentialStorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

pub(super) fn with_context(
    context: &'static str,
    source: impl Into<anyhow::Error>,
) -> std::io::Error {
    let mut source = source.into();
    let mut cause: Option<&(dyn std::error::Error + 'static)> = Some(source.as_ref());
    while let Some(error) = cause {
        if let Some(error) = error.downcast_ref::<serde_json::Error>()
            && error.is_data()
        {
            // Discard the entire chain: serde errors and their context may contain credentials.
            source = anyhow::anyhow!(
                "invalid credential JSON ({:?} at line {} column {})",
                error.classify(),
                error.line(),
                error.column(),
            );
            break;
        }
        cause = if let Some(error) = error.downcast_ref::<std::io::Error>() {
            // io::Error::source() skips its contained error.
            error.get_ref().map(|error| error as &dyn std::error::Error)
        } else {
            error.source()
        };
    }
    std::io::Error::other(CredentialStorageError { context, source })
}
