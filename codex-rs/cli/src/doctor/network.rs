use std::env;
use std::path::PathBuf;
use std::time::Duration;

use codex_core::config::Config;
#[cfg(target_os = "macos")]
use codex_http_client::MacosSystemProxyConfiguration;
use codex_http_client::RouteAwareClientPool;
use codex_http_client::RouteAwareRequestError;
use codex_http_client::RouteFailureClass;
#[cfg(target_os = "macos")]
use codex_http_client::macos_system_proxy_configuration;
use codex_login::default_client::create_client_without_request_logging;
use http::HeaderMap;
use http::Method;

use super::CheckStatus;
use super::DoctorCheck;
use super::push_proxy_env_details;
use super::read_probe_file;

pub(super) fn check(config: Option<&Config>) -> DoctorCheck {
    let mut details = Vec::new();
    push_proxy_env_details(&mut details);
    #[cfg(target_os = "macos")]
    {
        let request_url = config
            .and_then(|config| config.model_provider.base_url.as_deref())
            .or_else(|| config.map(|config| config.chatgpt_base_url.as_str()))
            .unwrap_or("https://chatgpt.com/backend-api/");
        let configuration = match macos_system_proxy_configuration(request_url) {
            MacosSystemProxyConfiguration::Automatic => "automatic (PAC)",
            MacosSystemProxyConfiguration::Manual => "manual",
            MacosSystemProxyConfiguration::Direct => "direct",
            MacosSystemProxyConfiguration::Unavailable => "unavailable",
        };
        details.push(format!("system proxy: {configuration}"));
    }
    if let Some(config) = config {
        let enabled = |value| if value { "enabled" } else { "disabled" };
        details.push(format!(
            "respect system proxy: {}",
            enabled(config.respect_system_proxy)
        ));
        if config.permissions.network.is_some() {
            details.push("managed proxy: configured".to_string());
        } else {
            details.push("managed proxy: not configured".to_string());
        }
    }

    let mut status = CheckStatus::Ok;
    let mut summary = "network-related environment looks readable".to_string();
    for name in ["CODEX_CA_CERTIFICATE", "SSL_CERT_FILE"] {
        if let Some(raw) = env::var_os(name) {
            let path = PathBuf::from(raw);
            match std::fs::metadata(&path) {
                Ok(metadata) if metadata.is_file() => {
                    if let Err(error) = read_probe_file(&path) {
                        status = CheckStatus::Warning;
                        summary = "custom CA env var points at an unreadable file".to_string();
                        details.push(format!("{name}: {} ({error})", path.display()));
                    } else {
                        details.push(format!("{name}: readable file {}", path.display()));
                    }
                }
                Ok(_) => {
                    status = CheckStatus::Warning;
                    summary = "custom CA env var does not point at a file".to_string();
                    details.push(format!("{name}: not a file {}", path.display()));
                }
                Err(error) => {
                    status = CheckStatus::Warning;
                    summary = "custom CA env var points at an unreadable path".to_string();
                    details.push(format!("{name}: {} ({error})", path.display()));
                }
            }
        }
    }

    DoctorCheck::new("network.env", "network", status, summary).details(details)
}

#[cfg(target_os = "macos")]
pub(super) fn with_system_proxy_remediation(
    mut check: DoctorCheck,
    config: &Config,
    request_url: Option<&str>,
) -> DoctorCheck {
    if check.status == CheckStatus::Fail
        && !config.respect_system_proxy
        && config.permissions.network.is_none()
        && config
            .config_layer_stack
            .requirements()
            .feature_requirements
            .as_ref()
            .and_then(|requirements| requirements.value.entries.get("respect_system_proxy"))
            .is_none_or(|enabled| *enabled)
        && !super::PROXY_ENV_VARS
            .iter()
            .any(|name| !name.eq_ignore_ascii_case("NO_PROXY") && super::env_var_present(name))
        && request_url.is_some_and(|url| {
            matches!(
                macos_system_proxy_configuration(url),
                MacosSystemProxyConfiguration::Automatic | MacosSystemProxyConfiguration::Manual
            )
        })
    {
        check.remediation = Some(
            "A macOS system proxy is configured but unused. If your organization requires it, ask your administrator whether to enable the under-development feature with `codex features enable respect_system_proxy`."
                .to_string(),
        );
    }
    check
}

pub(super) async fn probe_status(
    client: &RouteAwareClientPool,
    url: &str,
    method: Method,
    headers: HeaderMap,
) -> Result<u16, String> {
    let response = if env::var("CODEX_SANDBOX").as_deref() == Ok("seatbelt") {
        create_client_without_request_logging()
            .request(method, url)
            .headers(headers)
            .timeout(Duration::from_secs(8))
            .send()
            .await
            .map_err(RouteAwareRequestError::from)
    } else {
        client
            .request(method, url)
            .headers(headers)
            // Allow five seconds for PAC resolution before the three-second HTTP budget.
            .timeout(Duration::from_secs(8))
            .send()
            .await
    }
    .map_err(request_error)?;
    let status = response.status().as_u16();
    if status == 407 {
        return Err("proxy authentication required (HTTP 407)".to_string());
    }
    Ok(status)
}

pub(super) fn request_error(error: RouteAwareRequestError) -> String {
    match error.failure_class() {
        Some(RouteFailureClass::TlsError) => "TLS handshake or certificate validation failed",
        Some(RouteFailureClass::ProxyAuthenticationRequired) => "proxy authentication required",
        Some(RouteFailureClass::InvalidProxyConfig) => "invalid proxy configuration",
        Some(RouteFailureClass::ProxyResolutionUnavailable) => {
            "system proxy configuration unavailable"
        }
        Some(RouteFailureClass::ConnectTimeout) => "request timed out",
        Some(RouteFailureClass::UnsupportedProxyScheme) => "unsupported proxy configuration",
        Some(RouteFailureClass::ResolverError) => "proxy resolution failed",
        None if error.is_timeout() => "request timed out",
        None if error.is_connect() => "connect failed",
        None => "request failed",
    }
    .to_string()
}

#[cfg(test)]
#[path = "network_tests.rs"]
mod tests;
