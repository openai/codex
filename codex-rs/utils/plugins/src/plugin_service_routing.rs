const PLUGIN_SERVICE_PREVIEW_COOKIE_NAME: &[u8] = b"oai-chat-plugin-service-preview";
/// Routing cookie added to eligible plugin-service requests.
///
/// This cookie is intentionally public and untrusted. It selects a deployment after normal
/// authentication; the gateway must independently restrict preview routing to internal traffic.
pub const PLUGIN_SERVICE_PREVIEW_COOKIE: &str = "oai-chat-plugin-service-preview=true";

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
