//! Enhanced Call ID utilities for function call tracking.
//!
//! This module provides utilities for generating and parsing enhanced call IDs
//! that embed function names and indices for better tracking and debugging.
//!
//! ## Call ID Formats
//!
//! ### Client-Generated (when server doesn't provide ID)
//! Format: `cligen@<function_name>#<index>@<uuid>`
//! - `cligen@` prefix identifies client-generated IDs
//! - `<function_name>` allows extracting the function name later
//! - `#<index>` distinguishes multiple calls to the same function
//! - `@<uuid>` ensures uniqueness
//!
//! ### Server-Enhanced (when server provides ID)
//! Format: `srvgen@<function_name>@<original_call_id>`
//! - `srvgen@` prefix identifies server-generated (enhanced) IDs
//! - `<function_name>` allows extracting the function name
//! - `<original_call_id>` preserves the server's ID for sendback
//!
//! ## Example
//!
//! ```
//! use hyper_sdk::call_id::*;
//!
//! // Client-generated ID (server didn't provide one)
//! let client_id = generate_client_call_id("get_weather", 0);
//! assert!(is_client_generated_call_id(&client_id));
//! assert_eq!(parse_function_name_from_call_id(&client_id), Some("get_weather"));
//! assert_eq!(parse_call_index(&client_id), Some(0));
//!
//! // Server-enhanced ID (server provided call_abc123)
//! let server_id = enhance_server_call_id("call_abc123", "search_files");
//! assert!(!is_client_generated_call_id(&server_id));
//! assert_eq!(parse_function_name_from_call_id(&server_id), Some("search_files"));
//! assert_eq!(extract_original_call_id(&server_id), Some("call_abc123"));
//! ```

/// Prefix for client-generated call IDs (when server doesn't provide one).
pub const CLIENT_GEN_PREFIX: &str = "cligen@";

/// Prefix for server-generated (enhanced) call IDs.
pub const SERVER_GEN_PREFIX: &str = "srvgen@";

/// Generate a client-side call_id with embedded function name and index.
///
/// Format: `cligen@<function_name>#<index>@<uuid>`
///
/// Use when the server doesn't provide a call_id for function calls.
/// The index allows distinguishing multiple calls to the same function.
///
/// # Arguments
/// * `function_name` - The name of the function being called
/// * `index` - The index of this call (0-based, for multiple calls)
///
/// # Example
/// ```
/// use hyper_sdk::call_id::generate_client_call_id;
///
/// let id = generate_client_call_id("get_weather", 0);
/// assert!(id.starts_with("cligen@get_weather#0@"));
/// ```
pub fn generate_client_call_id(function_name: &str, index: i64) -> String {
    format!(
        "{}{function_name}#{index}@{}",
        CLIENT_GEN_PREFIX,
        uuid::Uuid::new_v4()
    )
}

/// Enhance a server-provided call_id with embedded function name.
///
/// Format: `srvgen@<function_name>@<original_call_id>`
///
/// Preserves the original call_id for later extraction when sending back to server.
///
/// # Arguments
/// * `original_id` - The server-provided call_id
/// * `function_name` - The name of the function being called
///
/// # Example
/// ```
/// use hyper_sdk::call_id::enhance_server_call_id;
///
/// let id = enhance_server_call_id("call_abc123", "search_files");
/// assert_eq!(id, "srvgen@search_files@call_abc123");
/// ```
pub fn enhance_server_call_id(original_id: &str, function_name: &str) -> String {
    format!("{SERVER_GEN_PREFIX}{function_name}@{original_id}")
}

/// Check if a call_id was generated/enhanced by us (has cligen@ or srvgen@ prefix).
///
/// # Example
/// ```
/// use hyper_sdk::call_id::{generate_client_call_id, is_enhanced_call_id};
///
/// let enhanced = generate_client_call_id("test", 0);
/// assert!(is_enhanced_call_id(&enhanced));
///
/// let plain = "some_random_id";
/// assert!(!is_enhanced_call_id(plain));
/// ```
pub fn is_enhanced_call_id(call_id: &str) -> bool {
    call_id.starts_with(CLIENT_GEN_PREFIX) || call_id.starts_with(SERVER_GEN_PREFIX)
}

/// Check if a call_id is client-generated (cligen@ prefix).
///
/// # Example
/// ```
/// use hyper_sdk::call_id::{generate_client_call_id, enhance_server_call_id, is_client_generated_call_id};
///
/// let client_id = generate_client_call_id("test", 0);
/// assert!(is_client_generated_call_id(&client_id));
///
/// let server_id = enhance_server_call_id("call_123", "test");
/// assert!(!is_client_generated_call_id(&server_id));
/// ```
pub fn is_client_generated_call_id(call_id: &str) -> bool {
    call_id.starts_with(CLIENT_GEN_PREFIX)
}

/// Parse function name from enhanced call_id (works for both cligen@ and srvgen@).
///
/// For client-generated: `cligen@<name>#<index>@<uuid>` → extracts `<name>`
/// For server-generated: `srvgen@<name>@<original_id>` → extracts `<name>`
///
/// Returns `None` for non-enhanced IDs.
///
/// # Example
/// ```
/// use hyper_sdk::call_id::{generate_client_call_id, enhance_server_call_id, parse_function_name_from_call_id};
///
/// let client_id = generate_client_call_id("get_weather", 0);
/// assert_eq!(parse_function_name_from_call_id(&client_id), Some("get_weather"));
///
/// let server_id = enhance_server_call_id("call_123", "search_files");
/// assert_eq!(parse_function_name_from_call_id(&server_id), Some("search_files"));
///
/// let plain_id = "some_random_id";
/// assert_eq!(parse_function_name_from_call_id(plain_id), None);
/// ```
pub fn parse_function_name_from_call_id(call_id: &str) -> Option<&str> {
    if let Some(rest) = call_id.strip_prefix(CLIENT_GEN_PREFIX) {
        // Client-generated format: <name>#<index>@<uuid>
        rest.split('#').next()
    } else if let Some(rest) = call_id.strip_prefix(SERVER_GEN_PREFIX) {
        // Server-generated format: <name>@<original_id>
        rest.split('@').next()
    } else {
        None
    }
}

/// Parse the index from a client-generated call_id.
///
/// Format: `cligen@<function_name>#<index>@<uuid>`
///
/// Returns `None` for server-generated or non-enhanced IDs.
///
/// # Example
/// ```
/// use hyper_sdk::call_id::{generate_client_call_id, enhance_server_call_id, parse_call_index};
///
/// let client_id = generate_client_call_id("get_weather", 2);
/// assert_eq!(parse_call_index(&client_id), Some(2));
///
/// let server_id = enhance_server_call_id("call_123", "test");
/// assert_eq!(parse_call_index(&server_id), None);
/// ```
pub fn parse_call_index(call_id: &str) -> Option<i64> {
    let rest = call_id.strip_prefix(CLIENT_GEN_PREFIX)?;
    // Format: <function_name>#<index>@<uuid>
    let hash_pos = rest.find('#')?;
    let after_hash = &rest[hash_pos + 1..];
    let at_pos = after_hash.find('@')?;
    after_hash[..at_pos].parse().ok()
}

/// Extract original call_id from server-enhanced format.
///
/// Format: `srvgen@<function_name>@<original_call_id>`
///
/// Returns `None` for client-generated or non-enhanced IDs.
///
/// # Example
/// ```
/// use hyper_sdk::call_id::{generate_client_call_id, enhance_server_call_id, extract_original_call_id};
///
/// let server_id = enhance_server_call_id("call_abc123", "search_files");
/// assert_eq!(extract_original_call_id(&server_id), Some("call_abc123"));
///
/// let client_id = generate_client_call_id("test", 0);
/// assert_eq!(extract_original_call_id(&client_id), None);
/// ```
pub fn extract_original_call_id(call_id: &str) -> Option<&str> {
    let rest = call_id.strip_prefix(SERVER_GEN_PREFIX)?;
    // Format: <function_name>@<original_id>
    rest.split('@').nth(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_client_call_id() {
        let call_id = generate_client_call_id("get_weather", 0);
        assert!(call_id.starts_with("cligen@get_weather#0@"));
        assert!(is_enhanced_call_id(&call_id));
        assert!(is_client_generated_call_id(&call_id));
        assert_eq!(
            parse_function_name_from_call_id(&call_id),
            Some("get_weather")
        );
        assert_eq!(parse_call_index(&call_id), Some(0));
        assert_eq!(extract_original_call_id(&call_id), None);
    }

    #[test]
    fn test_generate_client_call_id_with_index() {
        let call_id_0 = generate_client_call_id("get_weather", 0);
        let call_id_1 = generate_client_call_id("get_weather", 1);
        let call_id_5 = generate_client_call_id("get_weather", 5);

        assert!(call_id_0.starts_with("cligen@get_weather#0@"));
        assert!(call_id_1.starts_with("cligen@get_weather#1@"));
        assert!(call_id_5.starts_with("cligen@get_weather#5@"));

        assert_eq!(parse_call_index(&call_id_0), Some(0));
        assert_eq!(parse_call_index(&call_id_1), Some(1));
        assert_eq!(parse_call_index(&call_id_5), Some(5));

        // All should have the same function name
        assert_eq!(
            parse_function_name_from_call_id(&call_id_0),
            Some("get_weather")
        );
        assert_eq!(
            parse_function_name_from_call_id(&call_id_1),
            Some("get_weather")
        );
        assert_eq!(
            parse_function_name_from_call_id(&call_id_5),
            Some("get_weather")
        );
    }

    #[test]
    fn test_enhance_server_call_id() {
        let call_id = enhance_server_call_id("call_abc123", "search_files");
        assert_eq!(call_id, "srvgen@search_files@call_abc123");
        assert!(is_enhanced_call_id(&call_id));
        assert!(!is_client_generated_call_id(&call_id));
        assert_eq!(
            parse_function_name_from_call_id(&call_id),
            Some("search_files")
        );
        assert_eq!(extract_original_call_id(&call_id), Some("call_abc123"));
        assert_eq!(parse_call_index(&call_id), None);
    }

    #[test]
    fn test_non_enhanced_call_id() {
        let call_id = "some_random_call_id";
        assert!(!is_enhanced_call_id(call_id));
        assert!(!is_client_generated_call_id(call_id));
        assert_eq!(parse_function_name_from_call_id(call_id), None);
        assert_eq!(extract_original_call_id(call_id), None);
        assert_eq!(parse_call_index(call_id), None);
    }

    #[test]
    fn test_function_name_with_underscores() {
        // Function names with underscores should work correctly
        let client_id = generate_client_call_id("read_file_contents", 0);
        assert_eq!(
            parse_function_name_from_call_id(&client_id),
            Some("read_file_contents")
        );
        assert_eq!(parse_call_index(&client_id), Some(0));

        let server_id = enhance_server_call_id("srv_123", "write_to_database");
        assert_eq!(
            parse_function_name_from_call_id(&server_id),
            Some("write_to_database")
        );
        assert_eq!(extract_original_call_id(&server_id), Some("srv_123"));
    }

    #[test]
    fn test_uuid_uniqueness() {
        // Generate multiple IDs with same name/index - should be unique
        let id1 = generate_client_call_id("test", 0);
        let id2 = generate_client_call_id("test", 0);
        assert_ne!(id1, id2);
    }
}
