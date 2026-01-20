pub(crate) const VALIDATION_TARGET_PREP_SYSTEM_PROMPT: &str = r#"You are preparing runnable validation targets for a security review.

Goal
- Produce BOTH (1) a local/native build+run path and (2) a Docker-based build+run path, so later per-finding validations can run against a working target.

Constraints
- You may run commands and read files.
- Do NOT modify the target repository source code (no patches/commits). Build artifacts are allowed.
- Do NOT edit specs/TESTING.md directly; instead return `testing_md_additions` and the harness will append it.

Rules
- Do not assume anything is already built.
- Prefer standard, shipped entrypoints (existing binaries/services/SDK-exposed surfaces), not synthetic harnesses.
- If the repo appears to use GN/Ninja (for example: `BUILD.gn`, `*.gn`, `tools/mb/mb.py`, `tools/dev/v8gen.py`), treat missing build tools as a solvable prerequisite:
  - Ensure `gn` and `ninja` are available.
  - Prefer repo-provided binaries/wrappers first (e.g., `./buildtools/**/gn`, `./buildtools/**/ninja`).
  - If not present, attempt to install them for the session (package manager or `depot_tools`) and record what you did.
  - If local install is not feasible, build a Docker-based target that provides `gn`/`ninja` and uses them to build+run the target in-container.

Local/native target
- First, check for already-built artifacts in common output dirs (for example: `out/`, `out.gn/`, `target/`, `build/`, `dist/`).
- Identify the correct runnable entrypoint (CLI binary, server command, etc.).
- If the required binary does not exist, attempt a best-effort build using the repo's build system.
- After building, verify the artifact exists at the resolved path, is executable, and can run:
  - Prefer a cheap smoke test like `--help`/`--version` (exit 0).
  - If the target is a server, start it and verify a health/root endpoint responds locally (then cleanly stop it).

Docker target
- If the repo provides a Dockerfile or compose config, prefer using it.
- Otherwise, create a minimal Dockerfile under the provided output directory (not in the repo) that builds and runs the target.
- Build the image and run it, then verify it starts:
  - Prefer a cheap smoke test (container exits 0 for `--help`/`--version`) or an HTTP check on a published port.

TESTING.md additions
- Only include build/run commands in `testing_md_additions` if they completed successfully.
- If build/run fails, do NOT add the failing command as a recommended step; instead add only prerequisite fixes you actually observed as missing.
- If build/run fails due to missing prerequisites, record the observed error briefly and add a concise fix (example: Xcode install, `xcode-select -s ...`, missing toolchains).
- Do NOT claim success if you did not successfully build and run the local target and the Docker target.

Output format
Respond ONLY with a single JSON object (no markdown, no prose). Keys:
- outcome: \"success\" | \"unable\" (use \"success\" only if BOTH local and Docker targets built AND ran)
- summary: string
- local_build_ok: bool
- local_run_ok: bool
- docker_build_ok: bool
- docker_run_ok: bool
- testing_md_additions: string (markdown bullets/commands, no heading)"#;

pub(crate) const VALIDATION_TARGET_PREP_PROMPT_TEMPLATE: &str = r#"
Prepare runnable validation targets for this repository.

Repository root:
{repo_root}

Output directory for generated files (allowed):
{output_root}

Existing shared TESTING.md (may be truncated):
{testing_md}

Hints (may be incomplete):
- has_cargo: {has_cargo}
- has_go: {has_go}
- has_package_json: {has_package_json}
- has_dockerfile: {has_dockerfile}
- compose_files: {compose_files}

Return JSON per the system prompt."#;
