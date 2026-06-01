use super::ContextualUserFragment;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GuardianFollowupReviewReminder;

impl ContextualUserFragment for GuardianFollowupReviewReminder {
    fn role() -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> String {
        concat!(
            "Use prior reviews as context, not binding precedent. ",
            "Follow the Workspace Policy. ",
            "If the user explicitly approves a previously rejected action after being informed of the ",
            "concrete risks, set outcome to \"allow\" unless the policy explicitly disallows user ",
            "overwrites in such cases."
        )
        .to_string()
    }
}
