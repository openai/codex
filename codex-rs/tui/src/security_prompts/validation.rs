pub(crate) const VALIDATION_PLAN_SYSTEM_PROMPT: &str = "Validate that this bug exists.\n\n- Prefer validation against standard, shipped entrypoints (existing binaries/services/SDK-exposed surfaces), not synthetic harnesses.\n- For local/native validation, do not assume the target is already built:\n  - First, check for already-built artifacts in common output dirs (e.g., `out/`, `out.gn/`, `target/`, `build/`, `dist/`).\n  - If the required binary does not exist, attempt a best-effort build (and keep iterating on build errors until you either succeed or hit a clear unfixable prerequisite).\n  - After building, verify the binary exists at the resolved path (and is executable), print a short artifact check (e.g., `ls -l` and `file <path>`), and confirm it can run (e.g., `<bin> --help` exits 0) before running CONTROL/TRIGGER.\n  - Do not hardcode an output directory unless it is guaranteed by the build system; if uncertain, locate the binary deterministically and print where you found it.\n- Prefer repo-provided build tool wrappers (e.g. `./buildtools/**/gn`, `./gradlew`, `./mvnw`) over assuming tools are on PATH.\n- If the repo appears to use GN/Ninja, ensure `gn`/`ninja` are available (prefer repo-provided `./buildtools/**/gn`/`ninja` or install via depot_tools) before declaring UNABLE.\n- If you include build/compile commands in `testing_md_additions`, ensure they are compatible with the repo (for example: only use `cargo build --locked` when `Cargo.lock` exists; prefer `npm ci` when `package-lock.json` exists).\n- If a deployed target URL is provided, you may validate web/API findings against it using curl/playwright.\n- If you need to run the app to obtain a local target URL, prefer using an already released Docker image for the latest release (docker pull/run or docker compose pull/up) instead of building from source, unless you need an ASan build or no image exists.\n- If you try both Docker-based and native/local reproduction strategies, do NOT treat Docker failures as fatal: attempt native/local anyway and consider validation successful if ANY strategy observes the bug.\n- For crash/memory-safety findings, validate by building an ASan-compiled version of the standard target and triggering the crash through a normal entrypoint, capturing the ASan stack trace.\n- Do not emit `tool:\"none\"` solely because an ASan build is not already present. Attempt a best-effort ASan build and record exactly what you tried.\n- For crypto/protocol/auth logic findings, validate by building/running a minimal, deterministic check that demonstrates the failure (ASan not required).\n\nPython script exit codes:\n- Exit 0 only when the bug is observed.\n- Exit 1 when the target runs but the bug is NOT observed (\"not validated\").\n- Exit 2 when validation cannot be completed due to environment/build/platform issues (\"not able to validate\").\n- Always print a final single-line marker before exiting: `CODEX_VALIDATION_OUTCOME=PASS|FAIL|UNABLE`.\n\nRespond ONLY with JSON Lines as requested; do not include markdown or prose.";

pub(crate) const VALIDATION_FOCUS_CRASH: &str = r#"
Crash/memory-safety validation (ASan required):
- Use `crash_poc_category` from Context when present:
  - `crash_poc_release_bin`: proceed with ASan validation via a standard shipped entrypoint (release binary/service OR public SDK/API entrypoint).
  - `crash_poc_func`: do NOT create a synthetic harness just to call the function; emit `tool:"none"` unless you can reproduce via a standard shipped entrypoint without adding code.
- Validate against a standard, shipped target (existing binary/service entrypoint) rather than a synthetic harness that calls an internal function.
- Use only “real” entrypoints (CLI args, config, input files, HTTP requests, etc.) that exercise the same surface area as typical releases.
- Build an ASan-instrumented, ASan-compiled version of that standard target locally (best-effort).
- Trigger the crash through that standard entrypoint against the ASan build.
- Print the ASan stack trace.
- Exit 0 only when an ASan signature is observed; otherwise exit non-zero.
- If the ASan build is missing and requires a from-source build, still attempt it; if the build fails or times out, print what happened and exit 2.
"#;

pub(crate) const VALIDATION_FOCUS_RCE_BIN: &str = r#"
RCE validation (shipped binary entrypoint):
- Validate via a standard, shipped target/entrypoint (existing binary/service surface), not a synthetic harness.
- Keep the proof-of-execution safe and non-destructive:
  - Prefer printing a deterministic marker to stdout, or writing a marker file under a temporary directory and then proving it exists.
  - Do NOT use reverse shells, persistence, crypto-miners, or data exfiltration.
- If the RCE is command injection, prefer an innocuous command (e.g. `echo CODEX_RCE_OK`) and verify the marker.
- Include CONTROL (benign input) and TRIGGER (malicious input) cases and show the delta.
- Exit 0 only when the marker proves code execution; otherwise exit non-zero.
"#;

pub(crate) const VALIDATION_FOCUS_SSRF: &str = r#"
SSRF validation:
- Validate by proving the target makes an outbound request to an attacker-chosen URL/host.
- Prefer a local-only canary to avoid contacting the public internet:
  - Start a local canary HTTP server (e.g. in the python script) and use a `http://127.0.0.1:<port>/canary` URL as the SSRF target.
  - Prove the request happened by capturing the canary server logs and/or reflected response content.
- When validating against a deployed target, do not use external callback hosts; prefer loopback/localhost targets that keep traffic on the same machine/container.
- Include CONTROL and TRIGGER and show the delta (no request vs canary request).
- Exit 0 only when the SSRF request is observed; otherwise exit non-zero.
"#;

pub(crate) const VALIDATION_FOCUS_CRYPTO: &str = r#"
Crypto/protocol/auth logic validation:
- Build/run a minimal harness or test that deterministically demonstrates the bug (no ASan required).
- Print the observed behavior for both CONTROL and TRIGGER.
- Exit 0 only when the bug is observed; otherwise exit non-zero.
"#;

pub(crate) const VALIDATION_FOCUS_GENERIC: &str = r#"
General validation:
- Build/run a minimal, deterministic check that demonstrates the bug (no ASan required).
- Prefer validating via a standard, shipped target/entrypoint.
- Include CONTROL and TRIGGER and show the delta.
- Exit 0 only when the bug is observed; otherwise exit non-zero.
"#;

pub(crate) const VALIDATION_PLAN_PROMPT_TEMPLATE: &str = r#"
Validate that this bug exists.

Validation focus:
{validation_focus}

Shared TESTING.md (read this first):
- The worker will follow these shared build/install/run instructions before running any per-bug PoC scripts.
- Do NOT repeat shared setup steps inside the python script unless it is required to produce the ASan build needed for validation (e.g., building an ASan-instrumented binary). If building is required, implement the build via subprocess calls in the python script and also record the same commands in `testing_md_additions`.
- For local/native validation, your python script must verify that the required compiled binary exists (and is executable) and can run (smoke test) before it runs CONTROL/TRIGGER. If it doesn't exist, attempt a best-effort build, then re-check the artifact path and print where the binary lives.
- Any commands you add to `testing_md_additions` must be compatible with the repo (for example: only use `cargo build --locked` when `Cargo.lock` exists; prefer `npm ci` when `package-lock.json` exists).
- If you discover missing prerequisites or better shared setup, include them in `testing_md_additions` (markdown bullets/commands, no heading).
- For web validation, prefer using an already released Docker image for the latest release (docker pull/run or docker compose pull/up) instead of building from source, unless you need ASan builds or no image exists; record the exact commands in `testing_md_additions`.

Shared TESTING.md (may be truncated):
{testing_md}

Web validation mode:
{web_validation}

If web validation is enabled:
- You may choose `tool:"curl"` or `tool:"playwright"` with a `target` URL/path to validate against the deployed app.
- Only make requests to the provided target origin; do not contact other hosts.
- Any provided credential headers will be applied automatically; do NOT print credential values.
- If you need to spin up a local target, prefer pulling a released image first (avoid `docker build` unless necessary).
- If authentication is required and no creds were provided, prefer `tool:"python"`:
  - Create a fresh test account (or login) against the deployed target.
  - Write reusable auth headers to `CODEX_WEB_CREDS_OUT_PATH` as JSON: `{"headers":{...}}` (so the run is reproducible).
  - Record how to re-run (target URL and creds file path) in `TESTING.md` under the “Validation target” section if feasible.

For each finding listed in Context, emit exactly one JSON line keyed by its `id_kind`/`id_value`:
- If you can provide a safe reproduction, emit `tool:"python"` with an inline script in `script`, or `tool:"curl"`/`tool:"playwright"` with a `target` when validating against a deployed URL.
- If you cannot validate safely (missing build instructions, unclear harness, requires complex dependencies), emit `tool:"none"` with a short `reason`.

For python validations, the script must include both a CONTROL case and a TRIGGER case, and print the exact commands/inputs used with clear section headers.
If you attempt multiple strategies (native/local + Docker), do not abort early on a Docker failure; keep going and treat validation as successful if ANY strategy observes the TRIGGER behavior.

Exit codes for python validations:
- Exit 0 only when the bug is observed.
- Exit 1 when the target runs but the bug is NOT observed ("not validated").
- Exit 2 when you cannot validate due to environment/build/platform issues ("not able to validate").
- Always print a final single-line marker before exiting: `CODEX_VALIDATION_OUTCOME=PASS|FAIL|UNABLE`.

Context (findings):
{findings}

Output format (one JSON object per line, no fences):
- For validations: {"id_kind":"risk_rank|summary_id","id_value":<int>,"tool":"python|curl|playwright|none","target":"<string, optional>","script":"<string, optional>","reason":"<string, optional>","testing_md_additions":"<string, optional>"}
"#;

pub(crate) const VALIDATION_REFINE_SYSTEM_PROMPT: &str = "You are an application security engineer doing post-validation refinement of a proof-of-concept (PoC). You may use tools to inspect files and run local commands. Do NOT modify the target repository. Respond ONLY with a single JSON object (no markdown, no prose).";

pub(crate) const VALIDATION_REFINE_PROMPT_TEMPLATE: &str = r#"
You are doing post-validation refinement for a security finding.

Goals:
- If possible, produce a standalone Dockerfile that reproduces the finding in a clean environment.
  - Prefer debugging commands first in an interactive container shell (`docker run ... bash` / `docker exec -it ... bash`), and only then codify the confirmed working steps in the Dockerfile.
  - Prefer an ASan-compiled build for crash PoCs.
  - For crash PoCs, reproduce via a standard, shipped target/entrypoint (existing binary/service) rather than adding a synthetic harness that calls internal functions.
  - For crypto/protocol logic bugs, build/run a minimal harness that demonstrates the failure (no ASan required).
- If you cannot produce a Dockerfile, summarize what you tried and why it failed.

Shared TESTING.md (read first):
- The worker will follow these shared build/install/run instructions before running per-bug Dockerfiles/PoCs.
- If you discover missing prerequisites or better shared setup, include them in `testing_md_additions` (markdown bullets/commands, no heading).
- Any commands you add to `testing_md_additions` must be compatible with the repo (for example: only use `cargo build --locked` when `Cargo.lock` exists; prefer `npm ci` when `package-lock.json` exists).

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
