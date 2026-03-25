//! User and skill instruction payloads and contextual user fragment markers for Codex prompts.

mod fragment;
mod user_instructions;

pub use fragment::AGENTS_MD_END_MARKER;
pub use fragment::AGENTS_MD_FRAGMENT;
pub use fragment::AGENTS_MD_START_MARKER;
pub use fragment::ContextualUserFragmentDefinition;
pub use fragment::SKILL_CLOSE_TAG;
pub use fragment::SKILL_FRAGMENT;
pub use fragment::SKILL_OPEN_TAG;
pub use user_instructions::SkillInstructions;
pub use user_instructions::USER_INSTRUCTIONS_PREFIX;
pub use user_instructions::UserInstructions;
