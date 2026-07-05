use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
enum BrowserErrorCode {
    Unavailable,
    UnsupportedVersion,
    BrowserBusy,
    NavigationTimeout,
    StaleHandle,
    Crashed,
    PolicyChanged,
    ApprovalDenied,
    InvalidInput,
    Internal,
}

/// Sanitized error returned to a model-facing terminal-browser tool call.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserToolFailure {
    code: BrowserErrorCode,
    message: &'static str,
    retryable: bool,
}

/// Classifies an internal browser error into a bounded, secret-free tool failure.
pub fn classify_browser_error(error: &anyhow::Error) -> BrowserToolFailure {
    let message = error.to_string().to_ascii_lowercase();
    if message.contains("browser_busy")
        || message.contains("another terminal browser action")
        || message.contains("human_control_active")
    {
        return failure(
            BrowserErrorCode::BrowserBusy,
            "another browser action is still running",
            /*retryable*/ true,
        );
    }
    if message.contains("unsupported carbonyl version") {
        return failure(
            BrowserErrorCode::UnsupportedVersion,
            "the installed Carbonyl version is not supported",
            /*retryable*/ false,
        );
    }
    if message.contains("timed out") || message.contains("navigation_timeout") {
        return failure(
            BrowserErrorCode::NavigationTimeout,
            "the browser did not reach the requested state before the timeout",
            /*retryable*/ true,
        );
    }
    if message.contains("node_not_found")
        || message.contains("invalid nodeid")
        || message.contains("could not find node")
        || message.contains("no node with given id")
    {
        return failure(
            BrowserErrorCode::StaleHandle,
            "the page changed or the node handle is stale; take a new snapshot",
            /*retryable*/ true,
        );
    }
    if message.contains("network policy changed")
        || message.contains("permission profile")
        || message.contains("permission policy")
        || message.contains("sandbox network namespace")
        || message.contains("managed terminal-browser networking is not yet supported")
    {
        return failure(
            BrowserErrorCode::PolicyChanged,
            "the active browser permission policy does not allow this action",
            /*retryable*/ false,
        );
    }
    if message.contains("approval") && message.contains("denied") {
        return failure(
            BrowserErrorCode::ApprovalDenied,
            "the user did not approve this browser action",
            /*retryable*/ false,
        );
    }
    if message.contains("carbonyl exited") || message.contains("devtools connection") {
        return failure(
            BrowserErrorCode::Crashed,
            "the Carbonyl browser process is no longer available",
            /*retryable*/ true,
        );
    }
    if message.contains("carbonyl")
        && (message.contains("unavailable")
            || message.contains("not found")
            || message.contains("not discovered"))
    {
        return failure(
            BrowserErrorCode::Unavailable,
            "Carbonyl is unavailable; run /browser doctor for local diagnostics",
            /*retryable*/ false,
        );
    }
    if message.contains("unknown field")
        || message.contains("missing field")
        || message.contains("invalid type")
        || message.contains("does not accept")
        || message.contains("must not")
        || message.contains("requires a url")
    {
        return failure(
            BrowserErrorCode::InvalidInput,
            "the terminal-browser tool arguments are invalid",
            /*retryable*/ false,
        );
    }
    failure(
        BrowserErrorCode::Internal,
        "the terminal-browser action failed; inspect local logs for details",
        /*retryable*/ false,
    )
}

fn failure(code: BrowserErrorCode, message: &'static str, retryable: bool) -> BrowserToolFailure {
    BrowserToolFailure {
        code,
        message,
        retryable,
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
