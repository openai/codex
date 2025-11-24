//! In this module we only define constants for lsp specific error codes.
//! There are other error codes that are defined in the
//! [JSON RPC specification](https://www.jsonrpc.org/specification#error_object).

// This is the start range of LSP reserved error codes.
// It doesn't denote a real error code.
//
// @since 3.16.0
pub const LSP_RESERVED_ERROR_RANGE_START: i64 = -32899;

// The server cancelled the request. This error code should
// only be used for requests that explicitly support being
// server cancellable.
//
// @since 3.17.0
pub const SERVER_CANCELLED: i64 = -32802;

// The server detected that the content of a document got
// modified outside normal conditions. A server should
// NOT send this error code if it detects a content change
// in it unprocessed messages. The result even computed
// on an older state might still be useful for the client.
//
// If a client decides that a result is not of any use anymore
// the client should cancel the request.
pub const CONTENT_MODIFIED: i64 = -32801;

// The client has canceled a request and a server as detected
// the cancel.
pub const REQUEST_CANCELLED: i64 = -32800;

// This is the end range of LSP reserved error codes.
// It doesn't denote a real error code.
//
// @since 3.16.0
pub const LSP_RESERVED_ERROR_RANGE_END: i64 = -32800;
