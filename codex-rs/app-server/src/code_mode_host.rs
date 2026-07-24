use clap::Args;
use url::Url;

/// Selects the code-mode host for a single app-server process.
#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct AppServerCodeModeHostArgs {
    /// Connect to a remote code-mode host instead of starting a local host.
    #[arg(
        long = "code-mode-host",
        value_name = "WS_URL",
        value_parser = parse_websocket_url
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
}

impl From<AppServerCodeModeHostArgs> for CodeModeHostTransport {
    fn from(args: AppServerCodeModeHostArgs) -> Self {
        match args.code_mode_host {
            Some(url) => Self::WebSocket(url),
            None => Self::Local,
        }
    }
}

fn parse_websocket_url(value: &str) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|error| format!("invalid websocket URL: {error}"))?;
    if !matches!(url.scheme(), "ws" | "wss") || url.host_str().is_none() {
        return Err("code-mode host URL must use ws:// or wss:// with a host".to_string());
    }
    if url.fragment().is_some() {
        return Err("code-mode host URL must not contain a fragment".to_string());
    }
    Ok(url)
}

#[cfg(test)]
#[path = "code_mode_host_tests.rs"]
mod tests;
