use std::collections::HashMap;
use std::collections::HashSet;

use codex_utils_string::sha1_12_hex_suffix;
use codex_utils_string::take_bytes_at_char_boundary;

/// Bounds connector server names and raw tool names before they become HTTP routes or MCP
/// identifiers. This matches Codex's model-visible MCP tool-name budget while leaving room for a
/// stable 12-hex identity suffix when truncation is required.
pub(super) const MAX_VIRTUAL_MCP_IDENTIFIER_BYTES: usize = 64;

/// Allocates deterministic unique names for one inventory snapshot.
///
/// Colliding base names retain their legacy identity hash whenever it is available. If that name
/// is already reserved by a natural name or another hash, a deterministic salted identity is used
/// instead. Returned names remain aligned with the input order. Names can change across snapshots
/// when the set of colliding identities changes.
pub(super) fn allocate_deterministic_names<'a>(
    candidates: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Vec<String> {
    let candidates = candidates.into_iter().collect::<Vec<_>>();
    let mut counts_by_base = HashMap::<&str, usize>::new();
    for (base, _) in &candidates {
        *counts_by_base.entry(base).or_default() += 1;
    }

    let mut allocated = vec![String::new(); candidates.len()];
    let mut used = HashSet::with_capacity(candidates.len());
    let mut generated_indices = Vec::new();
    for (index, (base, _)) in candidates.iter().enumerate() {
        if counts_by_base[base] == 1 && base.len() <= MAX_VIRTUAL_MCP_IDENTIFIER_BYTES {
            allocated[index] = (*base).to_string();
            used.insert((*base).to_string());
        } else {
            generated_indices.push(index);
        }
    }

    generated_indices.sort_by_key(|&index| candidates[index]);
    for index in generated_indices {
        let (base, identity) = candidates[index];
        let mut salt = 0_u64;
        loop {
            let suffix = if salt == 0 {
                sha1_12_hex_suffix(identity)
            } else {
                sha1_12_hex_suffix(&format!("{identity}\0{salt}"))
            };
            let max_base_bytes = MAX_VIRTUAL_MCP_IDENTIFIER_BYTES - suffix.len();
            let bounded_base = take_bytes_at_char_boundary(base, max_base_bytes);
            let name = format!("{bounded_base}{suffix}");
            if used.insert(name.clone()) {
                allocated[index] = name;
                break;
            }
            salt += 1;
        }
    }

    allocated
}

#[cfg(test)]
#[path = "names_tests.rs"]
mod tests;
