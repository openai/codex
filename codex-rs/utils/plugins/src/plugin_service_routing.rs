use std::ffi::OsStr;

/// Process signal set by an eligible host to opt plugin-service requests into preview routing.
pub const CODEX_PLUGIN_SERVICE_PREVIEW_ENV_VAR: &str = "CODEX_PLUGIN_SERVICE_PREVIEW";

const PLUGIN_SERVICE_PREVIEW_ENABLED_VALUE: &str = "1";
const PLUGIN_SERVICE_PREVIEW_COOKIE_NAME: &[u8] = b"oai-chat-plugin-service-preview";
/// Routing cookie added to eligible plugin-service requests.
pub const PLUGIN_SERVICE_PREVIEW_COOKIE: &str = "oai-chat-plugin-service-preview=true";

/// Returns whether the host opted this process into plugin-service preview routing.
///
/// The host owns employee eligibility. This signal is defense-in-depth routing, not an
/// authorization boundary; authentication and authorization remain the responsibility of the
/// existing request path and plugin-service.
pub fn plugin_service_preview_enabled() -> bool {
    plugin_service_preview_enabled_from_value(
        std::env::var_os(CODEX_PLUGIN_SERVICE_PREVIEW_ENV_VAR).as_deref(),
    )
}

/// Rewrites plugin-service cookies so callers cannot override the process routing signal.
///
/// Unrelated cookies are preserved. Any caller-provided preview cookie is removed before the
/// canonical routing cookie is added when preview routing is enabled.
pub fn plugin_service_routing_cookie(
    existing_cookie_headers: &[&[u8]],
    preview_enabled: bool,
) -> Option<Vec<u8>> {
    let mut cookies = existing_cookie_headers
        .iter()
        .flat_map(|header| header.split(|byte| *byte == b';'))
        .map(trim_cookie_whitespace)
        .filter(|segment| !segment.is_empty())
        .filter(|segment| {
            let name = segment
                .iter()
                .position(|byte| *byte == b'=')
                .map_or(*segment, |separator| &segment[..separator]);
            trim_cookie_whitespace(name) != PLUGIN_SERVICE_PREVIEW_COOKIE_NAME
        })
        .map(<[u8]>::to_vec)
        .collect::<Vec<_>>();

    if preview_enabled {
        cookies.push(PLUGIN_SERVICE_PREVIEW_COOKIE.as_bytes().to_vec());
    }

    (!cookies.is_empty()).then(|| cookies.join(&b"; "[..]))
}

fn plugin_service_preview_enabled_from_value(value: Option<&OsStr>) -> bool {
    value == Some(OsStr::new(PLUGIN_SERVICE_PREVIEW_ENABLED_VALUE))
}

fn trim_cookie_whitespace(mut value: &[u8]) -> &[u8] {
    while value
        .first()
        .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
    {
        value = &value[1..];
    }
    while value
        .last()
        .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
    {
        value = &value[..value.len() - 1];
    }
    value
}

#[cfg(test)]
#[path = "plugin_service_routing_tests.rs"]
mod tests;
