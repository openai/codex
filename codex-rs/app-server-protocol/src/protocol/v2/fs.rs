use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

/// Read a file from the host filesystem.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsReadFileParams {
    /// Absolute path to read.
    pub path: AbsolutePathBuf,
}

/// Base64-encoded file contents returned by `fs/readFile`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsReadFileResponse {
    /// File contents encoded as base64.
    pub data_base64: String,
}

/// Write a file on the host filesystem.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsWriteFileParams {
    /// Absolute path to write.
    pub path: AbsolutePathBuf,
    /// File contents encoded as base64.
    pub data_base64: String,
}

/// Successful response for `fs/writeFile`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsWriteFileResponse {}

/// Open a file for bounded positional reads.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsReadFileOpenParams {
    /// Client-supplied handle identifier scoped to this connection.
    pub handle_id: String,
    /// Absolute path to open.
    pub path: AbsolutePathBuf,
}

/// Successful response for `fs/readFile/open`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsReadFileOpenResponse {
    /// Maximum decoded bytes returned by one read.
    ///
    /// Clients may use this value to size reusable read buffers.
    pub max_chunk_bytes: u32,
}

/// Read a bounded chunk from an open file at an absolute byte offset.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsReadFileReadParams {
    pub handle_id: String,
    #[ts(type = "number")]
    pub offset: u64,
    /// Requested upper bound. Values above `maxChunkBytes` are capped.
    #[ts(optional = nullable)]
    pub max_bytes: Option<u32>,
}

/// Bounded bytes returned by `fs/readFile/read`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsReadFileReadResponse {
    pub data_base64: String,
    /// No bytes beyond this response were available at the time of the read.
    pub eof: bool,
}

/// Request metadata for an open read handle.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsReadFileStatParams {
    pub handle_id: String,
}

/// Metadata for the opened file object.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsReadFileStatResponse {
    #[ts(type = "number")]
    pub size_bytes: u64,
    #[ts(type = "number")]
    pub created_at_ms: i64,
    #[ts(type = "number")]
    pub modified_at_ms: i64,
}

/// Close an open read handle.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsReadFileCloseParams {
    pub handle_id: String,
}

/// Successful response for `fs/readFile/close`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsReadFileCloseResponse {}

/// Create or truncate a file for streamed writes.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsWriteFileOpenParams {
    /// Client-supplied handle identifier scoped to this connection.
    pub handle_id: String,
    /// Absolute destination path.
    pub path: AbsolutePathBuf,
}

/// Successful response for `fs/writeFile/open`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsWriteFileOpenResponse {
    /// Maximum decoded bytes accepted by one write.
    pub max_chunk_bytes: u32,
}

/// Append one complete bounded chunk to an open write handle.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsWriteFileWriteParams {
    pub handle_id: String,
    pub data_base64: String,
}

/// Successful response for `fs/writeFile/write`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsWriteFileWriteResponse {}

/// Close an open write handle.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsWriteFileCloseParams {
    pub handle_id: String,
}

/// Successful response for `fs/writeFile/close`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsWriteFileCloseResponse {}

/// Create a directory on the host filesystem.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsCreateDirectoryParams {
    /// Absolute directory path to create.
    pub path: AbsolutePathBuf,
    /// Whether parent directories should also be created. Defaults to `true`.
    #[ts(optional = nullable)]
    pub recursive: Option<bool>,
}

/// Successful response for `fs/createDirectory`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsCreateDirectoryResponse {}

/// Request metadata for an absolute path.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsGetMetadataParams {
    /// Absolute path to inspect.
    pub path: AbsolutePathBuf,
}

/// Metadata returned by `fs/getMetadata`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsGetMetadataResponse {
    /// Whether the path resolves to a directory.
    pub is_directory: bool,
    /// Whether the path resolves to a regular file.
    pub is_file: bool,
    /// Whether the path itself is a symbolic link.
    pub is_symlink: bool,
    /// File creation time in Unix milliseconds when available, otherwise `0`.
    #[ts(type = "number")]
    pub created_at_ms: i64,
    /// File modification time in Unix milliseconds when available, otherwise `0`.
    #[ts(type = "number")]
    pub modified_at_ms: i64,
}

/// List direct child names for a directory.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsReadDirectoryParams {
    /// Absolute directory path to read.
    pub path: AbsolutePathBuf,
}

/// A directory entry returned by `fs/readDirectory`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsReadDirectoryEntry {
    /// Direct child entry name only, not an absolute or relative path.
    pub file_name: String,
    /// Whether this entry resolves to a directory.
    pub is_directory: bool,
    /// Whether this entry resolves to a regular file.
    pub is_file: bool,
}

/// Directory entries returned by `fs/readDirectory`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsReadDirectoryResponse {
    /// Direct child entries in the requested directory.
    pub entries: Vec<FsReadDirectoryEntry>,
}

/// Remove a file or directory tree from the host filesystem.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsRemoveParams {
    /// Absolute path to remove.
    pub path: AbsolutePathBuf,
    /// Whether directory removal should recurse. Defaults to `true`.
    #[ts(optional = nullable)]
    pub recursive: Option<bool>,
    /// Whether missing paths should be ignored. Defaults to `true`.
    #[ts(optional = nullable)]
    pub force: Option<bool>,
}

/// Successful response for `fs/remove`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsRemoveResponse {}

/// Copy a file or directory tree on the host filesystem.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsCopyParams {
    /// Absolute source path.
    pub source_path: AbsolutePathBuf,
    /// Absolute destination path.
    pub destination_path: AbsolutePathBuf,
    /// Required for directory copies; ignored for file copies.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub recursive: bool,
}

/// Successful response for `fs/copy`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsCopyResponse {}

/// Start filesystem watch notifications for an absolute path.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsWatchParams {
    /// Connection-scoped watch identifier used for `fs/unwatch` and `fs/changed`.
    pub watch_id: String,
    /// Absolute file or directory path to watch.
    pub path: AbsolutePathBuf,
}

/// Successful response for `fs/watch`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsWatchResponse {
    /// Canonicalized path associated with the watch.
    pub path: AbsolutePathBuf,
}

/// Stop filesystem watch notifications for a prior `fs/watch`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsUnwatchParams {
    /// Watch identifier previously provided to `fs/watch`.
    pub watch_id: String,
}

/// Successful response for `fs/unwatch`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsUnwatchResponse {}

/// Filesystem watch notification emitted for `fs/watch` subscribers.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct FsChangedNotification {
    /// Watch identifier previously provided to `fs/watch`.
    pub watch_id: String,
    /// File or directory paths associated with this event.
    pub changed_paths: Vec<AbsolutePathBuf>,
}
