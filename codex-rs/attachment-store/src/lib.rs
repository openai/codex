//! Storage-neutral attachment persistence interfaces.

use std::error::Error;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use serde::Deserialize;
use serde::Serialize;

/// Result returned by [`AttachmentStore`] operations.
pub type AttachmentStoreResult<T> = Result<T, AttachmentStoreError>;

/// Future returned by [`AttachmentStore::upload`].
pub type UploadFuture<'a> =
    Pin<Box<dyn Future<Output = AttachmentStoreResult<UploadResult>> + Send + 'a>>;

/// Future returned by [`AttachmentStore::resolve`].
pub type ResolveFuture<'a> =
    Pin<Box<dyn Future<Output = AttachmentStoreResult<AttachmentMetadata>> + Send + 'a>>;

/// Uploads attachments and resolves metadata for stored files.
///
/// Implementations choose whether uploaded attachments remain inline or become file references.
/// File references returned by [`AttachmentStore::upload`] must be resolvable by
/// [`AttachmentStore::resolve`].
pub trait AttachmentStore: Send + Sync {
    /// Uploads an attachment and returns the representation callers should retain.
    fn upload(&self, request: UploadRequest) -> UploadFuture<'_>;

    /// Resolves metadata and, when requested, a fresh URL for a stored attachment.
    fn resolve<'a>(&'a self, request: ResolveRequest<'a>) -> ResolveFuture<'a>;
}

/// Attachment data supplied to [`AttachmentStore::upload`].
#[derive(Clone, Eq, PartialEq)]
pub struct UploadRequest {
    /// Thread receiving this attachment; storage backends may use it for placement.
    pub thread_id: String,
    /// Optional name associated with the attachment.
    pub file_name: Option<String>,
    /// Attachment bytes to persist.
    pub data: Vec<u8>,
}

impl fmt::Debug for UploadRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UploadRequest")
            .field("thread_id", &self.thread_id)
            .field("file_name", &self.file_name)
            .field("data", &"<redacted>")
            .finish()
    }
}

/// Parameters supplied to [`AttachmentStore::resolve`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolveRequest<'a> {
    /// Identifier assigned by the attachment store.
    pub file_id: &'a str,
    /// Minimum remaining lifetime required for the returned file URL.
    ///
    /// When absent, implementations must omit the file URL and may return cached metadata. When
    /// present, implementations must return a URL that remains valid for at least this duration
    /// after the resolve operation completes.
    pub download_url_ttl: Option<Duration>,
}

/// Metadata used to validate and interpret an attachment.
#[derive(Clone, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AttachmentMetadata {
    /// Name associated with the attachment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    /// MD5 digest of the attachment bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    /// Length of the attachment byte payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    /// IANA media type used to interpret the attachment bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Metadata specific to the attachment's format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format_specific: Option<FormatSpecificMetadata>,
    /// URL for reading the attachment bytes when requested during resolution.
    ///
    /// Implementations must omit this field when [`ResolveRequest::download_url_ttl`] is absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_url: Option<String>,
}

impl fmt::Debug for AttachmentMetadata {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let file_url = self.file_url.as_ref().map(|_| "<redacted>");
        formatter
            .debug_struct("AttachmentMetadata")
            .field("file_name", &self.file_name)
            .field("digest", &self.digest)
            .field("size_bytes", &self.size_bytes)
            .field("mime_type", &self.mime_type)
            .field("format_specific", &self.format_specific)
            .field("file_url", &file_url)
            .finish()
    }
}

/// Metadata specific to an attachment format.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum FormatSpecificMetadata {
    /// Metadata for an image attachment.
    Image(ImageMetadata),
}

/// Metadata specific to an image attachment.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ImageMetadata {
    /// Image width in pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// Image height in pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

/// Representation callers should retain after uploading an attachment.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub enum UploadResult {
    /// Attachment data remains inline.
    Inline {
        /// Original attachment bytes passed to [`AttachmentStore::upload`].
        bytes: Vec<u8>,
    },
    /// Attachment data is available through an external file reference.
    File {
        /// Identifier assigned by the attachment store.
        file_id: String,
    },
}

impl fmt::Debug for UploadResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inline { .. } => formatter
                .debug_struct("Inline")
                .field("bytes", &"<redacted>")
                .finish(),
            Self::File { file_id } => formatter
                .debug_struct("File")
                .field("file_id", file_id)
                .finish(),
        }
    }
}

/// Leaves attachments inline instead of storing them.
pub struct InlineAttachmentStore;

impl AttachmentStore for InlineAttachmentStore {
    #[tracing::instrument(level = "trace", skip_all)]
    fn upload(&self, request: UploadRequest) -> UploadFuture<'_> {
        Box::pin(async move {
            Ok(UploadResult::Inline {
                bytes: request.data,
            })
        })
    }

    #[tracing::instrument(level = "trace", skip_all)]
    fn resolve<'a>(&'a self, request: ResolveRequest<'a>) -> ResolveFuture<'a> {
        let file_id = request.file_id;
        Box::pin(async move {
            Err(AttachmentStoreError::new(
                AttachmentStoreErrorKind::NotFound,
                format!("attachment `{file_id}` was not found"),
            ))
        })
    }
}

/// Category of failure returned by an [`AttachmentStore`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttachmentStoreErrorKind {
    /// The requested attachment could not be resolved.
    NotFound,
    /// The attachment data or metadata is invalid.
    InvalidAttachment,
    /// The backing store could not complete the operation.
    Backend,
}

/// Error returned by an [`AttachmentStore`] implementation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachmentStoreError {
    kind: AttachmentStoreErrorKind,
    message: String,
}

impl AttachmentStoreError {
    /// Creates an error without exposing backend-specific error types.
    pub fn new(kind: AttachmentStoreErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// Returns the category callers can use to handle this error.
    #[must_use]
    pub const fn kind(&self) -> AttachmentStoreErrorKind {
        self.kind
    }
}

impl fmt::Display for AttachmentStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(formatter)
    }
}

impl Error for AttachmentStoreError {}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
