pub(crate) const VALIDATION_PLAN_SYSTEM_PROMPT: &str = "Validate that this bug exists by testing it against an ASan-compiled version of the program. If it reproduces, record the PoC and ASan stack trace in the validation section. Respond ONLY with JSON Lines as requested; do not include markdown or prose.";
pub(crate) const VALIDATION_PLAN_PROMPT_TEMPLATE: &str = r#"
Validate that this bug exists by testing it against an ASan-compiled version of the program. If it reproduces, record the PoC and ASan stack trace in the validation section.

For each finding listed in Context, emit exactly one JSON line keyed by its `id_kind`/`id_value`:
- If you can provide a safe, local ASan reproduction, emit `tool:"python"` with an inline script in `script`.
- If you cannot validate safely (missing build instructions, unclear harness, requires complex dependencies), emit `tool:"none"` with a short `reason`.

For python validations, the script must:
- Build an ASan-instrumented, ASan-compiled version of the target (binary or library + harness) locally.
- Create a minimal PoC that triggers the crash against that ASan build.
- Run a CONTROL execution that should not crash.
- Run a TRIGGER execution expected to crash under ASan using that PoC.
- Print the exact commands/inputs (control + trigger/PoC) and the ASan stack trace with clear section headers.
- Exit 0 only when an ASan signature is observed; otherwise exit non-zero.

Context (findings):
{findings}

Output format (one JSON object per line, no fences):
- For validations: {"id_kind":"risk_rank|summary_id","id_value":<int>,"tool":"python|none","script":"<string, optional>","reason":"<string, optional>"}
"#;
