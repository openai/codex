use std::ffi::OsStr;

use clap::Args;
use clap::builder::TypedValueParser;
use clap::error::ErrorKind;
use url::Url;

/// Selects the code-mode host for a single app-server process.
#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct AppServerCodeModeHostArgs {
    /// Connect to a remote code-mode host instead of starting a local host.
    #[arg(
        long = "code-mode-host",
        value_name = "URL",
        value_parser = RedactedHostUrlParser
    )]
    pub code_mode_host: Option<Url>,
}

/// Process-scoped transport used to reach the code-mode host.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum CodeModeHostTransport {
    /// Start and own the default local code-mode host.
    #[default]
    Local,
    /// Share a connection to the specified remote code-mode host.
    WebSocket(Url),
    /// Share an HTTP/2 gRPC connection to the specified remote code-mode host.
    Grpc(Url),
}

impl From<AppServerCodeModeHostArgs> for CodeModeHostTransport {
    fn from(args: AppServerCodeModeHostArgs) -> Self {
        match args.code_mode_host {
            Some(url) if matches!(url.scheme(), "http" | "https") => Self::Grpc(url),
            Some(url) => Self::WebSocket(url),
            None => Self::Local,
        }
    }
}

#[derive(Clone)]
struct RedactedHostUrlParser;

impl TypedValueParser for RedactedHostUrlParser {
    type Value = Url;

    fn parse_ref(
        &self,
        command: &clap::Command,
        _argument: Option<&clap::Arg>,
        value: &OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let value = value.to_str().ok_or_else(|| {
            clap::Error::raw(
                ErrorKind::InvalidUtf8,
                "code-mode host URL must contain valid UTF-8",
            )
            .with_cmd(command)
        })?;

        parse_host_url(value)
            .map_err(|error| clap::Error::raw(ErrorKind::ValueValidation, error).with_cmd(command))
    }
}

fn parse_host_url(value: &str) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|error| format!("invalid code-mode host URL: {error}"))?;
    if !matches!(url.scheme(), "ws" | "wss" | "http" | "https") || url.host_str().is_none() {
        return Err(
            "code-mode host URL must use ws://, wss://, http://, or https:// with a host"
                .to_string(),
        );
    }
    if url.fragment().is_some() {
        return Err("code-mode host URL must not contain a fragment".to_string());
    }
    if matches!(url.scheme(), "http" | "https") {
        if !url.username().is_empty() || url.password().is_some() {
            return Err("gRPC code-mode host URL must not contain credentials".to_string());
        }
        if url.path() != "/" || url.query().is_some() {
            return Err("gRPC code-mode host URL must not contain a path or query".to_string());
        }
    }
    Ok(url)
}

#[cfg(test)]
#[path = "code_mode_host_tests.rs"]
mod tests;
