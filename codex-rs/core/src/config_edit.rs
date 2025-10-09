use crate::config::CONFIG_TOML_FILE;
use anyhow::Result;
use std::path::Path;
use tempfile::NamedTempFile;
use toml_edit::DocumentMut;

pub const CONFIG_KEY_MODEL: &str = "model";
pub const CONFIG_KEY_EFFORT: &str = "model_reasoning_effort";

#[derive(Copy, Clone)]
enum NoneBehavior {
    Skip,
    Remove,
}

/// Persist overrides into `config.toml` using explicit key segments per
/// override. This avoids ambiguity with keys that contain dots or spaces.
pub async fn persist_overrides(
    codex_home: &Path,
    profile: Option<&str>,
    overrides: &[(&[&str], &str)],
) -> Result<()> {
    let with_options: Vec<(&[&str], Option<&str>)> = overrides
        .iter()
        .map(|(segments, value)| (*segments, Some(*value)))
        .collect();

    persist_overrides_with_behavior(codex_home, profile, &with_options, NoneBehavior::Skip).await
}

/// Persist overrides where values may be optional. Any entries with `None`
/// values are skipped. If all values are `None`, this becomes a no-op and
/// returns `Ok(())` without touching the file.
pub async fn persist_non_null_overrides(
    codex_home: &Path,
    profile: Option<&str>,
    overrides: &[(&[&str], Option<&str>)],
) -> Result<()> {
    persist_overrides_with_behavior(codex_home, profile, overrides, NoneBehavior::Skip).await
}

/// Persist overrides where `None` values clear any existing values from the
/// configuration file.
pub async fn persist_overrides_and_clear_if_none(
    codex_home: &Path,
    profile: Option<&str>,
    overrides: &[(&[&str], Option<&str>)],
) -> Result<()> {
    persist_overrides_with_behavior(codex_home, profile, overrides, NoneBehavior::Remove).await
}

/// Apply a single override onto a `toml_edit` document while preserving
/// existing formatting/comments.
/// The key is expressed as explicit segments to correctly handle keys that
/// contain dots or spaces.
fn apply_toml_edit_override_segments(
    doc: &mut DocumentMut,
    segments: &[&str],
    value: toml_edit::Item,
) {
    use toml_edit::Item;

    if segments.is_empty() {
        return;
    }

    let mut current = doc.as_table_mut();
    for seg in &segments[..segments.len() - 1] {
        if !current.contains_key(seg) {
            current[*seg] = Item::Table(toml_edit::Table::new());
            if let Some(t) = current[*seg].as_table_mut() {
                t.set_implicit(true);
            }
        }

        let maybe_item = current.get_mut(seg);
        let Some(item) = maybe_item else { return };

        if !item.is_table() {
            *item = Item::Table(toml_edit::Table::new());
            if let Some(t) = item.as_table_mut() {
                t.set_implicit(true);
            }
        }

        let Some(tbl) = item.as_table_mut() else {
            return;
        };
        current = tbl;
    }

    let last = segments[segments.len() - 1];
    current[last] = value;
}

async fn persist_overrides_with_behavior(
    codex_home: &Path,
    profile: Option<&str>,
    overrides: &[(&[&str], Option<&str>)],
    none_behavior: NoneBehavior,
) -> Result<()> {
    if overrides.is_empty() {
        return Ok(());
    }

    let should_skip = match none_behavior {
        NoneBehavior::Skip => overrides.iter().all(|(_, value)| value.is_none()),
        NoneBehavior::Remove => false,
    };

    if should_skip {
        return Ok(());
    }

    let config_path = codex_home.join(CONFIG_TOML_FILE);

    let read_result = tokio::fs::read_to_string(&config_path).await;
    let mut doc = match read_result {
        Ok(contents) => contents.parse::<DocumentMut>()?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if overrides
                .iter()
                .all(|(_, value)| value.is_none() && matches!(none_behavior, NoneBehavior::Remove))
            {
                return Ok(());
            }

            tokio::fs::create_dir_all(codex_home).await?;
            DocumentMut::new()
        }
        Err(e) => return Err(e.into()),
    };

    let effective_profile = if let Some(p) = profile {
        Some(p.to_owned())
    } else {
        doc.get("profile")
            .and_then(|i| i.as_str())
            .map(str::to_string)
    };

    let mut mutated = false;

    for (segments, value) in overrides.iter().copied() {
        let mut seg_buf: Vec<&str> = Vec::new();
        let segments_to_apply: &[&str];

        if let Some(ref name) = effective_profile {
            if segments.first().copied() == Some("profiles") {
                segments_to_apply = segments;
            } else {
                seg_buf.reserve(2 + segments.len());
                seg_buf.push("profiles");
                seg_buf.push(name.as_str());
                seg_buf.extend_from_slice(segments);
                segments_to_apply = seg_buf.as_slice();
            }
        } else {
            segments_to_apply = segments;
        }

        match value {
            Some(v) => {
                let item_value = toml_edit::value(v);
                apply_toml_edit_override_segments(&mut doc, segments_to_apply, item_value);
                mutated = true;
            }
            None => {
                if matches!(none_behavior, NoneBehavior::Remove)
                    && remove_toml_edit_segments(&mut doc, segments_to_apply)
                {
                    mutated = true;
                }
            }
        }
    }

    if !mutated {
        return Ok(());
    }

    let tmp_file = NamedTempFile::new_in(codex_home)?;
    tokio::fs::write(tmp_file.path(), doc.to_string()).await?;
    tmp_file.persist(config_path)?;

    Ok(())
}

fn remove_toml_edit_segments(doc: &mut DocumentMut, segments: &[&str]) -> bool {
    use toml_edit::Item;

    if segments.is_empty() {
        return false;
    }

    let mut current = doc.as_table_mut();
    for seg in &segments[..segments.len() - 1] {
        let Some(item) = current.get_mut(seg) else {
            return false;
        };

        match item {
            Item::Table(table) => {
                current = table;
            }
            _ => {
                return false;
            }
        }
    }

    current.remove(segments[segments.len() - 1]).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    /// Verifies model and effort are written at top-level when no profile is set.
    #[tokio::test]
    async fn set_default_model_and_effort_top_level_when_no_profile() {
        let tmpdir = tempdir().expect("tmp");
        let codex_home = tmpdir.path();

        persist_overrides(
            codex_home,
            None,
            &[
                (&[CONFIG_KEY_MODEL], "gpt-5-codex"),
                (&[CONFIG_KEY_EFFORT], "high"),
            ],
        )
        .await
        .expect("persist");

        let contents = read_config(codex_home).await;
        let expected = r#"model = "gpt-5-codex"
model_reasoning_effort = "high"
"#;
        assert_eq!(contents, expected);
    }

    /// Verifies values are written under the active profile when `profile` is set.
    #[tokio::test]
    async fn set_defaults_update_profile_when_profile_set() {
        let tmpdir = tempdir().expect("tmp");
        let codex_home = tmpdir.path();

        // Seed config with a profile selection but without profiles table
        let seed = "profile = \"o3\"\n";
        tokio::fs::write(codex_home.join(CONFIG_TOML_FILE), seed)
            .await
            .expect("seed write");

        persist_overrides(
            codex_home,
            None,
            &[
                (&[CONFIG_KEY_MODEL], "o3"),
                (&[CONFIG_KEY_EFFORT], "minimal"),
            ],
        )
        .await
        .expect("persist");

        let contents = read_config(codex_home).await;
        let expected = r#"profile = "o3"

[profiles.o3]
model = "o3"
model_reasoning_effort = "minimal"
"#;
        assert_eq!(contents, expected);
    }

    /// Verifies profile names with dots/spaces are preserved via explicit segments.
    #[tokio::test]
    async fn set_defaults_update_profile_with_dot_and_space() {
        let tmpdir = tempdir().expect("tmp");
        let codex_home = tmpdir.path();

        // Seed config with a profile name that contains a dot and a space
        let seed = "profile = \"my.team name\"\n";
        tokio::fs::write(codex_home.join(CONFIG_TOML_FILE), seed)
            .await
            .expect("seed write");

        persist_overrides(
            codex_home,
            None,
            &[
                (&[CONFIG_KEY_MODEL], "o3"),
                (&[CONFIG_KEY_EFFORT], "minimal"),
            ],
        )
        .await
        .expect("persist");

        let contents = read_config(codex_home).await;
        let expected = r#"profile = "my.team name"

[profiles."my.team name"]
model = "o3"
model_reasoning_effort = "minimal"
"#;
        assert_eq!(contents, expected);
    }

    /// Verifies explicit profile override writes under that profile even without active profile.
    #[tokio::test]
    async fn set_defaults_update_when_profile_override_supplied() {
        let tmpdir = tempdir().expect("tmp");
        let codex_home = tmpdir.path();

        // No profile key in config.toml
        tokio::fs::write(codex_home.join(CONFIG_TOML_FILE), "")
            .await
            .expect("seed write");

        // Persist with an explicit profile override
        persist_overrides(
            codex_home,
            Some("o3"),
            &[(&[CONFIG_KEY_MODEL], "o3"), (&[CONFIG_KEY_EFFORT], "high")],
        )
        .await
        .expect("persist");

        let contents = read_config(codex_home).await;
        let expected = r#"[profiles.o3]
model = "o3"
model_reasoning_effort = "high"
"#;
        assert_eq!(contents, expected);
    }

    /// Verifies nested tables are created as needed when applying overrides.
    #[tokio::test]
    async fn persist_overrides_creates_nested_tables() {
        let tmpdir = tempdir().expect("tmp");
        let codex_home = tmpdir.path();

        persist_overrides(
            codex_home,
            None,
            &[
                (&["a", "b", "c"], "v"),
                (&["x"], "y"),
                (&["profiles", "p1", CONFIG_KEY_MODEL], "gpt-5-codex"),
            ],
        )
        .await
        .expect("persist");

        let contents = read_config(codex_home).await;
        let expected = r#"x = "y"

[a.b]
c = "v"

[profiles.p1]
model = "gpt-5-codex"
"#;
        assert_eq!(contents, expected);
    }

    /// Verifies a scalar key becomes a table when nested keys are written.
    #[tokio::test]
    async fn persist_overrides_replaces_scalar_with_table() {
        let tmpdir = tempdir().expect("tmp");
        let codex_home = tmpdir.path();
        let seed = "foo = \"bar\"\n";
        tokio::fs::write(codex_home.join(CONFIG_TOML_FILE), seed)
            .await
            .expect("seed write");

        persist_overrides(codex_home, None, &[(&["foo", "bar", "baz"], "ok")])
            .await
            .expect("persist");

        let contents = read_config(codex_home).await;
        let expected = r#"[foo.bar]
baz = "ok"
"#;
        assert_eq!(contents, expected);
    }

    /// Verifies comments and spacing are preserved when writing under active profile.
    #[tokio::test]
    async fn set_defaults_preserve_comments() {
        let tmpdir = tempdir().expect("tmp");
        let codex_home = tmpdir.path();

        // Seed a config with comments and spacing we expect to preserve
        let seed = r#"# Global comment
# Another line

profile = "o3"

# Profile settings
[profiles.o3]
# keep me
existing = "keep"
"#;
        tokio::fs::write(codex_home.join(CONFIG_TOML_FILE), seed)
            .await
            .expect("seed write");

        // Apply defaults; since profile is set, it should write under [profiles.o3]
        persist_overrides(
            codex_home,
            None,
            &[(&[CONFIG_KEY_MODEL], "o3"), (&[CONFIG_KEY_EFFORT], "high")],
        )
        .await
        .expect("persist");

        let contents = read_config(codex_home).await;
        let expected = r#"# Global comment
# Another line

profile = "o3"

# Profile settings
[profiles.o3]
# keep me
existing = "keep"
model = "o3"
model_reasoning_effort = "high"
"#;
        assert_eq!(contents, expected);
    }

    /// Verifies comments and spacing are preserved when writing at top level.
    #[tokio::test]
    async fn set_defaults_preserve_global_comments() {
        let tmpdir = tempdir().expect("tmp");
        let codex_home = tmpdir.path();

        // Seed a config WITHOUT a profile, containing comments and spacing
        let seed = r#"# Top-level comments
# should be preserved

existing = "keep"
"#;
        tokio::fs::write(codex_home.join(CONFIG_TOML_FILE), seed)
            .await
            .expect("seed write");

        // Since there is no profile, the defaults should be written at top-level
        persist_overrides(
            codex_home,
            None,
            &[
                (&[CONFIG_KEY_MODEL], "gpt-5-codex"),
                (&[CONFIG_KEY_EFFORT], "minimal"),
            ],
        )
        .await
        .expect("persist");

        let contents = read_config(codex_home).await;
        let expected = r#"# Top-level comments
# should be preserved

existing = "keep"
model = "gpt-5-codex"
model_reasoning_effort = "minimal"
"#;
        assert_eq!(contents, expected);
    }

    /// Verifies errors on invalid TOML propagate and file is not clobbered.
    #[tokio::test]
    async fn persist_overrides_errors_on_parse_failure() {
        let tmpdir = tempdir().expect("tmp");
        let codex_home = tmpdir.path();

        // Write an intentionally invalid TOML file
        let invalid = "invalid = [unclosed";
        tokio::fs::write(codex_home.join(CONFIG_TOML_FILE), invalid)
            .await
            .expect("seed write");

        // Attempting to persist should return an error and must not clobber the file.
        let res = persist_overrides(codex_home, None, &[(&["x"], "y")]).await;
        assert!(res.is_err(), "expected parse error to propagate");

        // File should be unchanged
        let contents = read_config(codex_home).await;
        assert_eq!(contents, invalid);
    }

    /// Verifies changing model only preserves existing effort at top-level.
    #[tokio::test]
    async fn changing_only_model_preserves_existing_effort_top_level() {
        let tmpdir = tempdir().expect("tmp");
        let codex_home = tmpdir.path();

        // Seed with an effort value only
        let seed = "model_reasoning_effort = \"minimal\"\n";
        tokio::fs::write(codex_home.join(CONFIG_TOML_FILE), seed)
            .await
            .expect("seed write");

        // Change only the model
        persist_overrides(codex_home, None, &[(&[CONFIG_KEY_MODEL], "o3")])
            .await
            .expect("persist");

        let contents = read_config(codex_home).await;
        let expected = r#"model_reasoning_effort = "minimal"
model = "o3"
"#;
        assert_eq!(contents, expected);
    }

    /// Verifies changing effort only preserves existing model at top-level.
    #[tokio::test]
    async fn changing_only_effort_preserves_existing_model_top_level() {
        let tmpdir = tempdir().expect("tmp");
        let codex_home = tmpdir.path();

        // Seed with a model value only
        let seed = "model = \"gpt-5-codex\"\n";
        tokio::fs::write(codex_home.join(CONFIG_TOML_FILE), seed)
            .await
            .expect("seed write");

        // Change only the effort
        persist_overrides(codex_home, None, &[(&[CONFIG_KEY_EFFORT], "high")])
            .await
            .expect("persist");

        let contents = read_config(codex_home).await;
        let expected = r#"model = "gpt-5-codex"
model_reasoning_effort = "high"
"#;
        assert_eq!(contents, expected);
    }

    /// Verifies changing model only preserves existing effort in active profile.
    #[tokio::test]
    async fn changing_only_model_preserves_effort_in_active_profile() {
        let tmpdir = tempdir().expect("tmp");
        let codex_home = tmpdir.path();

        // Seed with an active profile and an existing effort under that profile
        let seed = r#"profile = "p1"

[profiles.p1]
model_reasoning_effort = "low"
"#;
        tokio::fs::write(codex_home.join(CONFIG_TOML_FILE), seed)
            .await
            .expect("seed write");

        persist_overrides(codex_home, None, &[(&[CONFIG_KEY_MODEL], "o4-mini")])
            .await
            .expect("persist");

        let contents = read_config(codex_home).await;
        let expected = r#"profile = "p1"

[profiles.p1]
model_reasoning_effort = "low"
model = "o4-mini"
"#;
        assert_eq!(contents, expected);
    }

    /// Verifies changing effort only preserves existing model in a profile override.
    #[tokio::test]
    async fn changing_only_effort_preserves_model_in_profile_override() {
        let tmpdir = tempdir().expect("tmp");
        let codex_home = tmpdir.path();

        // No active profile key; we'll target an explicit override
        let seed = r#"[profiles.team]
model = "gpt-5-codex"
"#;
        tokio::fs::write(codex_home.join(CONFIG_TOML_FILE), seed)
            .await
            .expect("seed write");

        persist_overrides(
            codex_home,
            Some("team"),
            &[(&[CONFIG_KEY_EFFORT], "minimal")],
        )
        .await
        .expect("persist");

        let contents = read_config(codex_home).await;
        let expected = r#"[profiles.team]
model = "gpt-5-codex"
model_reasoning_effort = "minimal"
"#;
        assert_eq!(contents, expected);
    }

    /// Verifies `persist_non_null_overrides` skips `None` entries and writes only present values at top-level.
    #[tokio::test]
    async fn persist_non_null_skips_none_top_level() {
        let tmpdir = tempdir().expect("tmp");
        let codex_home = tmpdir.path();

        persist_non_null_overrides(
            codex_home,
            None,
            &[
                (&[CONFIG_KEY_MODEL], Some("gpt-5-codex")),
                (&[CONFIG_KEY_EFFORT], None),
            ],
        )
        .await
        .expect("persist");

        let contents = read_config(codex_home).await;
        let expected = "model = \"gpt-5-codex\"\n";
        assert_eq!(contents, expected);
    }

    /// Verifies no-op behavior when all provided overrides are `None` (no file created/modified).
    #[tokio::test]
    async fn persist_non_null_noop_when_all_none() {
        let tmpdir = tempdir().expect("tmp");
        let codex_home = tmpdir.path();

        persist_non_null_overrides(
            codex_home,
            None,
            &[(&["a"], None), (&["profiles", "p", "x"], None)],
        )
        .await
        .expect("persist");

        // Should not create config.toml on a pure no-op
        assert!(!codex_home.join(CONFIG_TOML_FILE).exists());
    }

    /// Verifies entries are written under the specified profile and `None` entries are skipped.
    #[tokio::test]
    async fn persist_non_null_respects_profile_override() {
        let tmpdir = tempdir().expect("tmp");
        let codex_home = tmpdir.path();

        persist_non_null_overrides(
            codex_home,
            Some("team"),
            &[
                (&[CONFIG_KEY_MODEL], Some("o3")),
                (&[CONFIG_KEY_EFFORT], None),
            ],
        )
        .await
        .expect("persist");

        let contents = read_config(codex_home).await;
        let expected = r#"[profiles.team]
model = "o3"
"#;
        assert_eq!(contents, expected);
    }

    #[tokio::test]
    async fn persist_clear_none_removes_top_level_value() {
        let tmpdir = tempdir().expect("tmp");
        let codex_home = tmpdir.path();

        let seed = r#"model = "gpt-5-codex"
model_reasoning_effort = "medium"
"#;
        tokio::fs::write(codex_home.join(CONFIG_TOML_FILE), seed)
            .await
            .expect("seed write");

        persist_overrides_and_clear_if_none(
            codex_home,
            None,
            &[
                (&[CONFIG_KEY_MODEL], None),
                (&[CONFIG_KEY_EFFORT], Some("high")),
            ],
        )
        .await
        .expect("persist");

        let contents = read_config(codex_home).await;
        let expected = "model_reasoning_effort = \"high\"\n";
        assert_eq!(contents, expected);
    }

    #[tokio::test]
    async fn persist_clear_none_respects_active_profile() {
        let tmpdir = tempdir().expect("tmp");
        let codex_home = tmpdir.path();

        let seed = r#"profile = "team"

[profiles.team]
model = "gpt-4"
model_reasoning_effort = "minimal"
"#;
        tokio::fs::write(codex_home.join(CONFIG_TOML_FILE), seed)
            .await
            .expect("seed write");

        persist_overrides_and_clear_if_none(
            codex_home,
            None,
            &[
                (&[CONFIG_KEY_MODEL], None),
                (&[CONFIG_KEY_EFFORT], Some("high")),
            ],
        )
        .await
        .expect("persist");

        let contents = read_config(codex_home).await;
        let expected = r#"profile = "team"

[profiles.team]
model_reasoning_effort = "high"
"#;
        assert_eq!(contents, expected);
    }

    #[tokio::test]
    async fn persist_clear_none_noop_when_file_missing() {
        let tmpdir = tempdir().expect("tmp");
        let codex_home = tmpdir.path();

        persist_overrides_and_clear_if_none(codex_home, None, &[(&[CONFIG_KEY_MODEL], None)])
            .await
            .expect("persist");

        assert!(!codex_home.join(CONFIG_TOML_FILE).exists());
    }

    // Test helper moved to bottom per review guidance.
    async fn read_config(codex_home: &Path) -> String {
        let p = codex_home.join(CONFIG_TOML_FILE);
        tokio::fs::read_to_string(p).await.unwrap_or_default()
    }
}
