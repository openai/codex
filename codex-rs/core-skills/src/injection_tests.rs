use super::*;
use pretty_assertions::assert_eq;
use std::collections::HashSet;

fn set<'a>(items: &'a [&'a str]) -> HashSet<&'a str> {
    items.iter().copied().collect()
}

fn assert_mentions(text: &str, expected_names: &[&str], expected_paths: &[&str]) {
    let mentions = extract_tool_mentions(text);
    assert_eq!(mentions.plain_names, set(expected_names));
    assert_eq!(mentions.paths, set(expected_paths));
}

#[test]
fn extract_tool_mentions_handles_plain_and_linked_mentions() {
    assert_mentions(
        "use $alpha and [$beta](/tmp/beta)",
        &["alpha"],
        &["/tmp/beta"],
    );
}

#[test]
fn extract_tool_mentions_skips_common_env_vars() {
    assert_mentions("use $PATH and $alpha", &["alpha"], &[]);
    assert_mentions("use [$HOME](/tmp/skill)", &[], &[]);
    assert_mentions("use $XDG_CONFIG_HOME and $beta", &["beta"], &[]);
}

#[test]
fn extract_tool_mentions_requires_link_syntax() {
    assert_mentions("[beta](/tmp/beta)", &[], &[]);
    assert_mentions("[$beta] /tmp/beta", &["beta"], &[]);
    assert_mentions("[$beta]()", &["beta"], &[]);
}

#[test]
fn extract_tool_mentions_trims_linked_paths_and_allows_spacing() {
    assert_mentions("use [$beta]   ( /tmp/beta )", &["beta"], &["/tmp/beta"]);
}

#[test]
fn extract_tool_mentions_stops_at_non_name_chars() {
    assert_mentions(
        "use $alpha.skill and $beta_extra",
        &["alpha", "beta_extra"],
        &[],
    );
}

#[test]
fn extract_tool_mentions_keeps_plugin_skill_namespaces() {
    assert_mentions(
        "use $slack:search and $alpha",
        &["alpha", "slack:search"],
        &[],
    );
}
