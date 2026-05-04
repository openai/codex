use super::ContextualUserFragment;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GuardianFollowupReviewReminder;

impl ContextualUserFragment for GuardianFollowupReviewReminder {
    const ROLE: &'static str = "developer";
    const START_MARKER: &'static str = "";
    const END_MARKER: &'static str = "";

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
