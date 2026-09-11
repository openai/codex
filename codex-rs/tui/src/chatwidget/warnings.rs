use std::collections::HashSet;

const FALLBACK_MODEL_METADATA_WARNING_PREFIX: &str = "Model metadata for `";
const FALLBACK_MODEL_METADATA_WARNING_SUFFIX: &str =
    "` not found. Defaulting to fallback metadata; this can degrade performance and cause issues.";

#[derive(Default)]
pub(super) struct WarningDisplayState {
    /// Completed resume history does not end startup; active work does.
    pub(super) startup_complete: bool,
    /// Initialization config warnings may also arrive as ordinary thread warnings.
    pub(super) startup_config_warnings: HashSet<String>,
    fallback_model_metadata_slugs: HashSet<String>,
}

impl WarningDisplayState {
    pub(super) fn should_display(&mut self, message: &str) -> bool {
        !self.startup_config_warnings.contains(message)
            && fallback_model_metadata_warning_slug(message)
                .is_none_or(|slug| self.fallback_model_metadata_slugs.insert(slug.to_string()))
    }
}

fn fallback_model_metadata_warning_slug(message: &str) -> Option<&str> {
    message
        .strip_prefix(FALLBACK_MODEL_METADATA_WARNING_PREFIX)?
        .strip_suffix(FALLBACK_MODEL_METADATA_WARNING_SUFFIX)
}
