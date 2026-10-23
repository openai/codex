//! API-key routing hints. Prefixes do not validate credentials or grant access.

/// Whether the key carries a FedRAMP issuance marker.
///
/// Unmarked keys may predate the marker, so `false` does not establish that a key
/// is commercial. Authentication must still use the complete, unchanged key.
pub fn is_fedramp_api_key(api_key: &str) -> bool {
    api_key.starts_with("sk-proj-fedramp-") || api_key.starts_with("sk-svcacct-fedramp-")
}
