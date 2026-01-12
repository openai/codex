pub(crate) const VALIDATION_PLAN_SYSTEM_PROMPT: &str = "Validate that this bug exists.\n\n- For crash/memory-safety findings, validate by building an ASan-compiled version of the program (or harness) and triggering the crash, capturing the ASan stack trace.\n- For crypto/protocol/auth logic findings, validate by building/running a minimal, deterministic harness that demonstrates the failure (ASan not required).\n\nRespond ONLY with JSON Lines as requested; do not include markdown or prose.";
pub(crate) const VALIDATION_PLAN_PROMPT_TEMPLATE: &str = r#"
Validate that this bug exists.

- If the finding is a crash/memory-safety issue (e.g., `verification_types` includes `crash_poc`), validate it against an ASan-compiled build and capture the ASan stack trace.
- If the finding is a crypto/protocol/auth logic issue, validate it with a minimal, deterministic harness (no ASan required).

For each finding listed in Context, emit exactly one JSON line keyed by its `id_kind`/`id_value`:
- If you can provide a safe, local reproduction, emit `tool:"python"` with an inline script in `script`.
- If you cannot validate safely (missing build instructions, unclear harness, requires complex dependencies), emit `tool:"none"` with a short `reason`.

For python validations, the script must include both a CONTROL case and a TRIGGER case, and print the exact commands/inputs used with clear section headers.

Crash/memory-safety findings:
- Build an ASan-instrumented, ASan-compiled version of the target (binary or library + harness) locally.
- Trigger the crash against that ASan build.
- Print the ASan stack trace.
- Exit 0 only when an ASan signature is observed; otherwise exit non-zero.

Crypto/protocol/auth logic findings:
- Build/run a minimal harness or test that deterministically demonstrates the bug (no ASan required).
- Print the observed behavior for both CONTROL and TRIGGER.
- Exit 0 only when the bug is observed; otherwise exit non-zero.

Context (findings):
{findings}

Output format (one JSON object per line, no fences):
- For validations: {"id_kind":"risk_rank|summary_id","id_value":<int>,"tool":"python|none","script":"<string, optional>","reason":"<string, optional>"}
"#;

pub(crate) const VALIDATION_REFINE_SYSTEM_PROMPT: &str = "You are an application security engineer doing post-validation refinement of a proof-of-concept (PoC). You may use tools to inspect files and run local commands. Do NOT modify the target repository. Respond ONLY with a single JSON object (no markdown, no prose).";

pub(crate) const VALIDATION_REFINE_PROMPT_TEMPLATE: &str = r#"
You are doing post-validation refinement for a security finding.

Goals:
- If possible, produce a standalone Dockerfile that reproduces the finding in a clean environment.
  - Prefer an ASan-compiled build for crash PoCs.
  - For crypto/protocol logic bugs, build/run a minimal harness that demonstrates the failure (no ASan required).
- If you cannot produce a Dockerfile, summarize what you tried and why it failed.

Constraints:
- You may use tools (read files, run commands) to refine the PoC.
- Do not edit or patch the target repository.
- Keep the output concise and reproducible.

Finding:
{finding}

Current validation state:
{validation_state}

Python PoC script (if any):
{python_script}

Output JSON (single object, no fences). Keys:
- summary: string (required) — what you did and why it succeeded/failed
- dockerfile: string|null — Dockerfile contents if you can produce one
- docker_build: string|null — exact docker build command to run
- docker_run: string|null — exact docker run command to run
- files: [{"path": "...", "contents": "..."}] — optional extra files that should live next to the Dockerfile
"#;
