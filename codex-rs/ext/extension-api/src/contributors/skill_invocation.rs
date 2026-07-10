use crate::ExtensionData;

/// Input supplied when the host or an extension observes one skill invocation.
pub struct SkillInvocationInput<'a> {
    /// Store scoped to the host session runtime.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
    /// Store scoped to this turn runtime.
    pub turn_store: &'a ExtensionData,
    /// Current turn submission id.
    pub turn_id: &'a str,
    /// Main prompt path or opaque resource id for the invoked skill.
    pub skill_resource: &'a str,
    /// How the skill invocation was initiated.
    pub kind: SkillInvocationKind,
}

/// How an observed skill invocation was initiated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkillInvocationKind {
    /// The user explicitly mentioned the skill.
    Explicit,
    /// The model read the skill instructions or ran one of its scripts.
    Implicit,
}
