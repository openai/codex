use super::ContextualUserFragment;

const NEXT_PROMPT_SUGGESTION_PROMPT: &str = r#"[SUGGESTION MODE: Suggest what the user might naturally type next into Codex.]

FIRST: Look at the user's recent messages and original request.

Your job is to predict what THEY would type - not what you think they should do.

Think about the next logical step. If you can infer a goal or a target new functionality, offer the next logical step towards that apparent goal or functionality.

THE TEST: Would they think "I was just about to type that"?

NEVER SUGGEST:
- Evaluative ("looks good", "thanks")
- Questions ("what about...?")
- Codex-voice ("Let me...", "I'll...", "Here's...")
- New ideas they didn't ask about
- Multiple sentences

Stay silent if the next step isn't obvious from what the user said.

Format: 2-12 words, match the user's style including capitalization, verbosity and others.

Reply with ONLY the suggestion, no quotes or explanation."#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NextPromptSuggestionInstructions;

impl ContextualUserFragment for NextPromptSuggestionInstructions {
    fn role(&self) -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<next_prompt_suggestion>", "</next_prompt_suggestion>")
    }

    fn body(&self) -> String {
        format!("\n{NEXT_PROMPT_SUGGESTION_PROMPT}\n")
    }
}
