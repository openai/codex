use super::ContextualUserFragment;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::truncate_text;

const MAX_NETWORK_TARGET_TOKENS: usize = 128;
const MAX_GUARDIAN_REJECTION_TOKENS: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GuardianNetworkAccessDenied {
    target: String,
    rejection: String,
}

impl GuardianNetworkAccessDenied {
    pub(crate) fn new(target: &str, rejection: &str) -> Self {
        Self {
            target: truncate_text(target, TruncationPolicy::Tokens(MAX_NETWORK_TARGET_TOKENS)),
            rejection: truncate_text(
                rejection,
                TruncationPolicy::Tokens(MAX_GUARDIAN_REJECTION_TOKENS),
            ),
        }
    }
}

impl ContextualUserFragment for GuardianNetworkAccessDenied {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (
            "<guardian_network_access_denied>",
            "</guardian_network_access_denied>",
        )
    }

    fn body(&self) -> String {
        format!(
            "\nAutomatic approval review denied network access to {:?}.\n{}\n",
            self.target, self.rejection
        )
    }
}
