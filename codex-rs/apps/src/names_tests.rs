use pretty_assertions::assert_eq;

use super::*;

#[test]
fn natural_names_are_reserved_without_reordering_results() {
    let first_identity = "first";
    let second_identity = "second";
    let natural_name = format!("tool{}", sha1_12_hex_suffix(first_identity));
    let allocated = allocate_deterministic_names([
        ("tool", first_identity),
        (natural_name.as_str(), "natural"),
        ("tool", second_identity),
    ]);

    assert_eq!(
        allocated,
        vec![
            format!(
                "tool{}",
                sha1_12_hex_suffix(&format!("{first_identity}\0{}", 1))
            ),
            natural_name,
            format!("tool{}", sha1_12_hex_suffix(second_identity)),
        ]
    );
}

#[test]
fn overlong_names_are_bounded_with_a_stable_identity_hash() {
    let base = "connector_".repeat(16);
    let identity = "连接器🙂identity";
    let allocated = allocate_deterministic_names([(base.as_str(), identity)]);
    let suffix = sha1_12_hex_suffix(identity);

    assert_eq!(allocated.len(), 1);
    assert!(allocated[0].len() <= MAX_VIRTUAL_MCP_IDENTIFIER_BYTES);
    assert!(allocated[0].ends_with(&suffix));
    assert_eq!(
        allocated[0],
        format!(
            "{}{}",
            take_bytes_at_char_boundary(&base, MAX_VIRTUAL_MCP_IDENTIFIER_BYTES - suffix.len()),
            suffix
        )
    );
}

#[test]
fn bounded_hash_collisions_are_salted_without_renaming_natural_names() {
    let base = "connector_".repeat(16);
    let first_identity = "first";
    let second_identity = "second";
    let preferred_suffix = sha1_12_hex_suffix(first_identity);
    let preferred_name = format!(
        "{}{}",
        take_bytes_at_char_boundary(
            &base,
            MAX_VIRTUAL_MCP_IDENTIFIER_BYTES - preferred_suffix.len()
        ),
        preferred_suffix
    );
    let allocated = allocate_deterministic_names([
        (base.as_str(), first_identity),
        (preferred_name.as_str(), "natural"),
        (base.as_str(), second_identity),
    ]);

    assert_eq!(allocated[1], preferred_name);
    assert_eq!(
        allocated
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        3
    );
    assert!(
        allocated
            .iter()
            .all(|name| name.len() <= MAX_VIRTUAL_MCP_IDENTIFIER_BYTES)
    );
    assert!(allocated[0].ends_with(&sha1_12_hex_suffix(&format!("{first_identity}\0{}", 1))));
    assert!(allocated[2].ends_with(&sha1_12_hex_suffix(second_identity)));
}

#[test]
fn truncation_never_splits_utf8() {
    let base = format!("{}{}", "a".repeat(50), "é".repeat(10));
    let allocated = allocate_deterministic_names([(base.as_str(), "utf8")]);
    let suffix = sha1_12_hex_suffix("utf8");

    assert_eq!(allocated[0], format!("{}{}", "a".repeat(50), suffix));
    assert!(allocated[0].len() < MAX_VIRTUAL_MCP_IDENTIFIER_BYTES);
}
