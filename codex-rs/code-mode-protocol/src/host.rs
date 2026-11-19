//! Resource limits shared by the code-mode host, client, and runtime.

/// Maximum encoded gRPC message size.
pub const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;

/// Maximum number of unresolved delegate callbacks allowed per host connection.
pub const MAX_PENDING_DELEGATE_CALLS: usize = 4_096;

/// Callback capacity of clients that do not advertise the larger budget.
pub const LEGACY_MAX_PENDING_DELEGATE_CALLS: usize = 1_024;

/// Maximum aggregate text bytes retained per server-truncated observation.
pub const BOUNDED_OUTPUT_MAX_BYTES: usize = 1024 * 1024;

/// Maximum aggregate inline image bytes retained per server-truncated observation.
pub const BOUNDED_OUTPUT_MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;

/// Maximum aggregate encoded audio URL bytes retained per server-truncated observation.
/// Count the whole URL, including the data URL prefix and any base64 payload.
/// This leaves heuristic transport headroom but does not bound the complete response.
pub const BOUNDED_OUTPUT_MAX_AUDIO_BYTES: usize = 62 * 1024 * 1024;
