//! Access structured MCP errors without dropping their original protocol data.

use anyhow::Error;
use rmcp::ErrorData;
use rmcp::service::ServiceError;
use std::time::Duration;

use crate::rmcp_client::ClientOperationError;

/// Returns the server's structured protocol error, including its original data.
pub fn mcp_error(error: &Error) -> Option<&ErrorData> {
    let ClientOperationError::Service(ServiceError::McpError(error)) = error.downcast_ref()? else {
        return None;
    };
    Some(error)
}

/// Preserves a request deadline as a typed error for caller-side telemetry.
pub fn request_timeout_error(label: &str, duration: Duration) -> Error {
    ClientOperationError::Timeout {
        label: label.to_string(),
        duration,
    }
    .into()
}

/// Returns a bounded caller-side failure category without parsing error text.
pub fn request_error_code(error: &Error) -> Option<&'static str> {
    let error = error.downcast_ref::<ClientOperationError>()?;
    Some(match error {
        ClientOperationError::Timeout { .. } => "request_timeout",
        ClientOperationError::Service(error) => match error {
            ServiceError::Timeout { .. } => "request_timeout",
            ServiceError::TransportClosed => "transport_closed",
            ServiceError::TransportSend(_) => "transport_error",
            ServiceError::McpError(_) => "protocol_error",
            ServiceError::UnexpectedResponse => "unexpected_response",
            ServiceError::Cancelled { .. } => "cancelled",
            ServiceError::SubscriptionLagged { .. } => "subscription_lagged",
            ServiceError::InputRequiredRoundsExceeded { .. } => "input_required_rounds_exceeded",
            _ => return None,
        },
    })
}

#[cfg(test)]
#[path = "service_error_tests.rs"]
mod tests;
