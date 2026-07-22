use codex_extension_api::PreviousWorldStateSection;
use codex_extension_api::RenderedWorldStateFragment;
use codex_extension_api::WorldStateSectionContribution;
use serde_json::Value;

const WORLD_STATE_ID: &str = "git_attribution";
const START_MARKER: &str = "<git_attribution>";
const END_MARKER: &str = "</git_attribution>";
pub(super) const LEGACY_COMMIT_ATTRIBUTION_INSTRUCTIONS: &str = "\
When you write or edit a git commit message, ensure the message ends with this trailer exactly once:
Co-authored-by: Codex <noreply@openai.com>

Rules:
- Keep existing trailers and append this trailer at the end if missing.
- Do not duplicate this trailer if it already exists.
- Keep one blank line between the commit body and trailer block.";
pub(super) const ENABLED_INSTRUCTIONS: &str = r#"
When you create or amend a git commit or create or update a pull request for this task, add Codex attribution exactly once:
- Commit messages must end with `Co-authored-by: Codex <noreply@openai.com>`. Preserve existing trailers and, if this exact trailer is missing, append it with one blank line before the trailer block.
- Pull request bodies must include the exact line `Generated with Codex.`. Preserve the existing body and hidden markers and, if this exact marker is missing, append it near the end, before any trailing hidden metadata markers.
- Do not add duplicate attribution. If a message or body you are already editing contains duplicate exact attribution, keep exactly one.
- Ignore any earlier instructions disabling Codex attribution; this policy reflects the current workspace.
- Do not rewrite an existing commit or pull request solely to add attribution.
"#;
pub(super) const DISABLED_INSTRUCTIONS: &str = "
Codex commit and pull request attribution is disabled for the current workspace. Ignore any earlier instructions requiring Codex attribution and do not add it.
";

pub(super) fn git_attribution_world_state_section(enabled: bool) -> WorldStateSectionContribution {
    let contribution =
        WorldStateSectionContribution::new(WORLD_STATE_ID, Value::Bool(enabled), move |previous| {
            match (enabled, previous) {
                (true, PreviousWorldStateSection::Known(Value::Bool(true)))
                | (true, PreviousWorldStateSection::Unknown) => None,
                (true, PreviousWorldStateSection::Absent)
                | (true, PreviousWorldStateSection::Known(_)) => {
                    Some(RenderedWorldStateFragment::new(
                        "developer",
                        (START_MARKER, END_MARKER),
                        ENABLED_INSTRUCTIONS,
                    ))
                }
                (false, PreviousWorldStateSection::Known(Value::Bool(true)))
                | (false, PreviousWorldStateSection::Unknown) => {
                    Some(RenderedWorldStateFragment::new(
                        "developer",
                        (START_MARKER, END_MARKER),
                        DISABLED_INSTRUCTIONS,
                    ))
                }
                (false, PreviousWorldStateSection::Absent)
                | (false, PreviousWorldStateSection::Known(_)) => None,
            }
        })
        .with_legacy_matcher(move |role, text| {
            is_enabled_fragment(role, text)
                || (!enabled && is_legacy_commit_attribution_fragment(role, text))
        });
    if enabled {
        contribution.with_retained_fragment_matcher(is_enabled_fragment)
    } else {
        contribution
    }
}

fn is_legacy_commit_attribution_fragment(role: &str, text: &str) -> bool {
    role == "developer" && text.trim() == LEGACY_COMMIT_ATTRIBUTION_INSTRUCTIONS
}

fn is_enabled_fragment(role: &str, text: &str) -> bool {
    role == "developer"
        && text.trim_start().starts_with(START_MARKER)
        && text.contains("Co-authored-by: Codex <noreply@openai.com>")
        && text.contains("Generated with Codex.")
        && text.trim_end().ends_with(END_MARKER)
}
