use super::definition::SubagentDefinition;
use super::definition::SubagentSource;
use include_dir::Dir;
use include_dir::include_dir;

/// Load embedded default subagents shipped with the binary.
///
/// Project (~/.codex/agents) and user (.codex/agents) definitions override these.
pub(crate) fn embedded_defs() -> Vec<SubagentDefinition> {
    static DEFAULTS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/src/subagents/defaults");

    let mut defs: Vec<SubagentDefinition> = Vec::new();

    for file in DEFAULTS_DIR.files() {
        if file
            .path()
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("json"))
            .unwrap_or(false)
        {
            match file.contents_utf8() {
                Some(contents) => match SubagentDefinition::from_json_str(contents) {
                    Ok(mut def) => {
                        def.source = SubagentSource::EmbeddedDefault;
                        defs.push(def)
                    }
                    Err(e) => {
                        tracing::warn!(
                            "failed to parse embedded default subagent '{}': {}",
                            file.path().display(),
                            e
                        );
                    }
                },
                None => {
                    tracing::warn!(
                        "embedded defaults file is not valid UTF-8: {}",
                        file.path().display()
                    );
                }
            }
        }
    }

    defs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_defaults_are_valid_subagents() {
        // Ensure we can load all embedded defaults and that the list is non-empty
        let defs = embedded_defs();
        assert!(
            !defs.is_empty(),
            "expected at least one default subagent in src/subagents/defaults"
        );

        // Basic sanity checks on each definition
        for def in defs {
            assert!(
                !def.name.trim().is_empty(),
                "subagent name must not be empty"
            );
            assert!(
                !def.instructions.trim().is_empty(),
                "subagent '{}' must have instructions",
                def.name
            );
        }
    }
}
