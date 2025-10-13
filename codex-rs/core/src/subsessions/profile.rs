use crate::config::GPT_5_CODEX_MEDIUM_MODEL;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionType {
    Tester,
    Mathematician,
    LinterFixer,
    Default,
}

impl SessionType {
    pub fn as_str(self) -> &'static str {
        match self {
            SessionType::Tester => "tester",
            SessionType::Mathematician => "mathematician",
            SessionType::LinterFixer => "linter_fixer",
            SessionType::Default => "default",
        }
    }
}

impl std::fmt::Display for SessionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str((*self).as_str())
    }
}

impl std::str::FromStr for SessionType {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "tester" => Ok(SessionType::Tester),
            "mathematician" => Ok(SessionType::Mathematician),
            "linter_fixer" => Ok(SessionType::LinterFixer),
            "default" => Ok(SessionType::Default),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubsessionProfile {
    pub session_type: SessionType,
    pub developer_instructions: &'static str,
    pub model_name: Option<&'static str>,
}

impl SubsessionProfile {
    pub fn for_session_type(session_type: SessionType) -> Self {
        match session_type {
            SessionType::Tester => Self {
                session_type,
                developer_instructions: LINTER_FIXER_PROMPT,
                model_name: Some(GPT_5_CODEX_MEDIUM_MODEL),
            },
            SessionType::Mathematician => Self {
                session_type,
                developer_instructions: LINTER_FIXER_PROMPT,
                model_name: Some(GPT_5_CODEX_MEDIUM_MODEL),
            },
            SessionType::LinterFixer => Self {
                session_type,
                developer_instructions: LINTER_FIXER_PROMPT,
                model_name: Some(GPT_5_CODEX_MEDIUM_MODEL),
            },
            SessionType::Default => Self {
                session_type,
                developer_instructions: DEFAULT_PROMPT,
                model_name: None,
            },
        }
    }
}
const MAIN_PROMPT: &str  = include_str!("../../gpt_5_codex_prompt.md");

const TESTER_PROMPT: &str = "\
You are a focused software testing assistant. Generate precise, minimal, and \
actionable tests that directly validate the described behavior. When clarifying \
requirements, ask only what is necessary.";

const MATHEMATICIAN_PROMPT: &str = "\
You are a detail-oriented mathematical reasoning assistant. Solve problems with \
clear derivations, keep intermediate notes concise, and prefer exact symbolic \
results when practical.";

const LINTER_FIXER_PROMPT: &str = include_str!("profiles/linter.md");

const DEFAULT_PROMPT: &str = "\
You are a compact subsession assistant. Provide direct, implementation-ready \
answers for the given request without rehashing unrelated project context.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_type_roundtrips() {
        for ty in [
            SessionType::Tester,
            SessionType::Mathematician,
            SessionType::LinterFixer,
            SessionType::Default,
        ] {
            let parsed: SessionType = ty.as_str().parse().expect("parse");
            assert_eq!(parsed, ty);
        }
    }
}
