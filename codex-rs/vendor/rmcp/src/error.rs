use std::{borrow::Cow, fmt::Display};

pub use crate::model::ErrorData;
#[deprecated(
    note = "Use `rmcp::ErrorData` instead, `rmcp::ErrorData` could become `RmcpError` in the future."
)]
pub type Error = ErrorData;
impl Display for ErrorData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code.0, self.message)?;
        if let Some(data) = &self.data {
            write!(f, "({})", data)?;
        }
        Ok(())
    }
}

impl std::error::Error for ErrorData {}

/// This is an unified error type for the errors could be returned by the service.
#[derive(Debug, thiserror::Error)]
#[allow(clippy::large_enum_variant)]
pub enum RmcpError {
    #[cfg(any(feature = "client", feature = "server"))]
    #[error("Service error: {0}")]
    Service(#[from] crate::ServiceError),
    #[cfg(feature = "client")]
    #[error("Client initialization error: {0}")]
    ClientInitialize(#[from] crate::service::ClientInitializeError),
    #[cfg(feature = "server")]
    #[error("Server initialization error: {0}")]
    ServerInitialize(#[from] crate::service::ServerInitializeError),
    #[error("Runtime error: {0}")]
    Runtime(#[from] tokio::task::JoinError),
    #[error("Transport creation error: {error}")]
    // TODO: Maybe we can introduce something like `TryIntoTransport` to auto wrap transport type,
    // but it could be an breaking change, so we could do it in the future.
    TransportCreation {
        into_transport_type_name: Cow<'static, str>,
        into_transport_type_id: std::any::TypeId,
        #[source]
        error: Box<dyn std::error::Error + Send + Sync>,
    },
    // and cancellation shouldn't be an error?

    // TODO: add more error variants as needed
    #[error("Task error: {0}")]
    TaskError(String),
}

impl RmcpError {
    pub fn transport_creation<T: 'static>(
        error: impl Into<Box<dyn std::error::Error + Send + Sync>>,
    ) -> Self {
        RmcpError::TransportCreation {
            into_transport_type_id: std::any::TypeId::of::<T>(),
            into_transport_type_name: std::any::type_name::<T>().into(),
            error: error.into(),
        }
    }
}
