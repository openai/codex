pub(crate) const VALIDATION_PLAN_SYSTEM_PROMPT: &str = "Validate that this bug exists.\n\n- Prefer validation against standard, shipped entrypoints (existing binaries/services/SDK-exposed surfaces), not synthetic harnesses.\n- For crash/memory-safety findings, validate by building an ASan-compiled version of the standard target and triggering the crash through a normal entrypoint, capturing the ASan stack trace.\n- For crypto/protocol/auth logic findings, validate by building/running a minimal, deterministic check that demonstrates the failure (ASan not required).\n\nPython script exit codes:\n- Exit 0 only when the bug is observed.\n- Exit 1 when the target runs but the bug is NOT observed (\"not validated\").\n- Exit 2 when validation cannot be completed due to environment/build/platform issues (\"not able to validate\").\n\nRespond ONLY with JSON Lines as requested; do not include markdown or prose.";
pub(crate) const VALIDATION_PLAN_PROMPT_TEMPLATE: &str = r#"
Validate that this bug exists.

- If the finding is a crash/memory-safety issue (e.g., `verification_types` includes `crash_poc_release_bin` or `crash_poc_func`), validate it against an ASan-compiled build and capture the ASan stack trace.
- If the finding is a crypto/protocol/auth logic issue, validate it with a minimal, deterministic harness (no ASan required).

Shared TESTING.md (read this first):
- The worker will follow these shared build/install/run instructions before running any per-bug PoC scripts.
- Do NOT repeat shared setup steps inside the python script.
- If you discover missing prerequisites or better shared setup, include them in `testing_md_additions` (markdown bullets/commands, no heading).

Shared TESTING.md (may be truncated):
{testing_md}

For each finding listed in Context, emit exactly one JSON line keyed by its `id_kind`/`id_value`:
- If you can provide a safe, local reproduction, emit `tool:"python"` with an inline script in `script`.
- If you cannot validate safely (missing build instructions, unclear harness, requires complex dependencies), emit `tool:"none"` with a short `reason`.

For python validations, the script must include both a CONTROL case and a TRIGGER case, and print the exact commands/inputs used with clear section headers.

Exit codes for python validations:
- Exit 0 only when the bug is observed.
- Exit 1 when the target runs but the bug is NOT observed ("not validated").
- Exit 2 when you cannot validate due to environment/build/platform issues ("not able to validate").

Crash/memory-safety findings:
- Use `crash_poc_category` from Context when present:
  - `crash_poc_release_bin`: proceed with ASan validation via a standard shipped target/entrypoint.
  - `crash_poc_func`: do NOT create a synthetic harness just to call the function; emit `tool:"none"` unless you can reproduce via a standard shipped entrypoint without adding code.
- Validate against a standard, shipped target (existing binary/service entrypoint) rather than a synthetic harness that calls an internal function.
- Use only “real” entrypoints (CLI args, config, input files, HTTP requests, etc.) that exercise the same surface area as typical releases.
- Do not create a new harness/test binary solely to call a vulnerable function; if you cannot plausibly reach the crash from a standard target, emit `tool:"none"` with a short reason.
- Build an ASan-instrumented, ASan-compiled version of that standard target locally (if feasible).
- Trigger the crash through that standard entrypoint against the ASan build.
- Print the ASan stack trace.
- Exit 0 only when an ASan signature is observed; otherwise exit non-zero.

Crypto/protocol/auth logic findings:
- Build/run a minimal harness or test that deterministically demonstrates the bug (no ASan required).
- Print the observed behavior for both CONTROL and TRIGGER.
- Exit 0 only when the bug is observed; otherwise exit non-zero.

Context (findings):
{findings}

Output format (one JSON object per line, no fences):
- For validations: {"id_kind":"risk_rank|summary_id","id_value":<int>,"tool":"python|none","script":"<string, optional>","reason":"<string, optional>","testing_md_additions":"<string, optional>"}
"#;

pub(crate) const VALIDATION_REFINE_SYSTEM_PROMPT: &str = "You are an application security engineer doing post-validation refinement of a proof-of-concept (PoC). You may use tools to inspect files and run local commands. Do NOT modify the target repository. Respond ONLY with a single JSON object (no markdown, no prose).";

pub(crate) const VALIDATION_REFINE_PROMPT_TEMPLATE: &str = r#"
You are doing post-validation refinement for a security finding.

Goals:
- If possible, produce a standalone Dockerfile that reproduces the finding in a clean environment.
  - Prefer an ASan-compiled build for crash PoCs.
  - For crash PoCs, reproduce via a standard, shipped target/entrypoint (existing binary/service) rather than adding a synthetic harness that calls internal functions.
  - For crypto/protocol logic bugs, build/run a minimal harness that demonstrates the failure (no ASan required).
- If you cannot produce a Dockerfile, summarize what you tried and why it failed.

Shared TESTING.md (read first):
- The worker will follow these shared build/install/run instructions before running per-bug Dockerfiles/PoCs.
- If you discover missing prerequisites or better shared setup, include them in `testing_md_additions` (markdown bullets/commands, no heading).

Shared TESTING.md (may be truncated):
{testing_md}

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
- testing_md_additions: string|null — optional shared prerequisites to append to TESTING.md (no heading)
- files: [{"path": "...", "contents": "..."}] — optional extra files that should live next to the Dockerfile
"#;
