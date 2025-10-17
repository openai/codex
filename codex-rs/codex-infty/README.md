# Codex Infty

Codex Infty is a small orchestration layer that coordinates multiple Codex roles (Solver, Director, Verifier(s)) to drive longer, multi‑step objectives with minimal human intervention. It provides:

- A run orchestrator that routes messages between roles and advances the workflow.
- A durable run store on disk with metadata and standard subfolders.
- Default role prompts for Solver/Director/Verifier.
- A lightweight progress reporting hook for UIs/CLIs.

The crate is designed to be embedded (via the library API) and also powers the `codex infty` CLI commands.

## High‑Level Flow

```
objective → Solver
  Solver → direction_request → Director → directive → Solver
  … (iterate) …
  Solver → final_delivery → Orchestrator returns RunOutcome
```

- The Solver always speaks structured JSON. The orchestrator parses those messages and decides the next hop.
- The Director provides crisp guidance (also JSON) that is forwarded back to the Solver.
- One or more Verifiers may assess the final deliverable; the orchestrator aggregates results and reports a summary to the Solver.
- On final_delivery, the orchestrator resolves and validates the deliverable path and returns the `RunOutcome`.

## Directory Layout (Run Store)

When a run is created, a directory is initialized with this structure:

```
<runs_root>/<run_id>/
  artifacts/      # long‑lived artifacts produced by the Solver
  memory/         # durable notes, claims, context
  index/          # indexes and caches
  deliverable/    # final output(s) assembled by the Solver
  run.json        # run metadata (id, timestamps, roles)
```

See: `codex-infty/src/run_store.rs`.

- The orchestrator persists rollout paths and optional config paths for each role into `run.json`.
- Metadata timestamps are updated on significant events (role spawns, handoffs, final delivery).
- Final deliverables must remain within the run directory. Paths are canonicalized and validated.

## Roles and Prompts

Default base instructions are injected per role if the provided `Config` has none:

- Solver: `codex-infty/src/prompts/solver.md`
- Director: `codex-infty/src/prompts/director.md`
- Verifier: `codex-infty/src/prompts/verifier.md`

You can provide your own instructions by pre‑populating `Config.base_instructions`.

## Solver Signal Contract

The Solver communicates intent using JSON messages (possibly wrapped in a fenced block). The orchestrator accepts two shapes:

- Direction request (sent to Director):

```json
{"type":"direction_request","prompt":"<question or decision>"}
```

- Final delivery (completes the run):

```json
{"type":"final_delivery","deliverable_path":"deliverable/summary.txt","summary":"<short text>"}
```

JSON may be fenced as ```json … ```; the orchestrator will strip the fence.

## Key Types and Modules

- Orchestrator: `codex-infty/src/orchestrator.rs`
  - `InftyOrchestrator`: spawns/resumes role sessions, drives the event loop, and routes signals.
  - `execute_new_run`: one‑shot helper that spawns and then drives.
  - `spawn_run`: set up sessions and the run store.
  - `call_role`, `relay_assistant_to_role`, `post_to_role`, `await_first_assistant`, `stream_events`: utilities when integrating custom flows.

- Run store: `codex-infty/src/run_store.rs`
  - `RunStore`, `RunMetadata`, `RoleMetadata`: metadata and persistence helpers.

- Types: `codex-infty/src/types.rs`
  - `RoleConfig`: wraps a `Config` and sets sensible defaults for autonomous flows (no approvals, full sandbox access). Also used to persist optional config paths.
  - `RunParams`: input to spawn runs.
  - `RunExecutionOptions`: per‑run options (objective, timeouts).
  - `RunOutcome`: returned on successful final delivery.

- Signals: `codex-infty/src/signals.rs`
  - DTOs for director responses and verifier verdicts, and the aggregated summary type.

- Progress: `codex-infty/src/progress.rs`
  - `ProgressReporter` trait: hook for UIs/CLIs to observe solver/director/verifier activity.

## Orchestrator Workflow (Details)

1. Spawn or resume role sessions (Solver, Director, and zero or more Verifiers). Default prompts are applied if the role’s `Config` has no base instructions.
2. Optionally post an `objective` to the Solver. The progress reporter is notified and the orchestrator waits for the first Solver signal.
3. On `direction_request`:
   - Post a structured request to the Director and await the first assistant message.
   - Parse it into a `DirectiveResponse` and forward the normalized JSON to the Solver.
4. On `final_delivery`:
   - Canonicalize and validate that `deliverable_path` stays within the run directory.
   - Optionally run a verification pass using configured Verifier(s), aggregate results, and post a summary back to the Solver.
   - Notify the progress reporter, touch the run store, and return `RunOutcome`.

## Library Usage

```rust
use std::sync::Arc;
use codex_core::{CodexAuth, config::Config};
use codex_infty::{InftyOrchestrator, RoleConfig, RunParams, RunExecutionOptions};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1) Load or build a Config for each role
    let solver_cfg: Config = load_config();
    let mut director_cfg = solver_cfg.clone();
    director_cfg.model = "o4-mini".into();

    // 2) Build role configs
    let solver = RoleConfig::new("solver", solver_cfg.clone());
    let director = RoleConfig::new("director", director_cfg);
    let verifiers = vec![RoleConfig::new("verifier-alpha", solver_cfg.clone())];

    // 3) Create an orchestrator (using default runs root)
    let auth = CodexAuth::from_api_key("sk-…");
    let orchestrator = InftyOrchestrator::new(auth)?;

    // 4) Execute a new run with an objective
    let params = RunParams {
        run_id: "my-run".into(),
        run_root: None, // use default ~/.codex/infty/<run_id>
        solver,
        director,
        verifiers,
    };
    let mut opts = RunExecutionOptions::default();
    opts.objective = Some("Implement feature X".into());

    let outcome = orchestrator.execute_new_run(params, opts).await?;
    println!("deliverable: {}", outcome.deliverable_path.display());
    Ok(())
}
# fn load_config() -> codex_core::config::Config { codex_core::config::Config::default() }
```

Note: Resuming runs is currently disabled.

## CLI Quickstart

The CLI (`codex`) exposes Infty helpers under the `infty` subcommand. Examples:

```bash
# Create a run and immediately drive toward completion
codex infty create --run-id demo --objective "Build and test feature"

# Inspect runs
codex infty list
codex infty show demo

# Sending one-off messages to stored runs is currently disabled
```

Flags allow customizing the Director’s model and reasoning effort; see `codex infty create --help`.

## Progress Reporting

Integrate your UI by implementing `ProgressReporter` and attaching it with `InftyOrchestrator::with_progress(...)`. You’ll receive callbacks on key milestones (objective posted, solver messages, director response, verification summaries, final delivery, etc.).

## Safety and Guardrails

- `RoleConfig::new` sets `SandboxPolicy::DangerFullAccess` and `AskForApproval::Never` to support autonomous flows. Adjust if your environment requires stricter policies.
- Deliverable paths are validated to stay inside the run directory and are fully canonicalized.
- JSON payloads are schema‑checked where applicable (e.g., solver signals and final delivery shape).

## Tests

Run the crate’s tests:

```bash
cargo test -p codex-infty
```

Many tests rely on mocked SSE streams and will auto‑skip in sandboxes where network is disabled.

## When to Use This Crate

Use `codex-infty` when you want a minimal, pragmatic multi‑role loop with:

- Clear role separation and routing.
- Durable, restart‑resilient state on disk.
- Simple integration points (progress hooks and helper APIs).

It’s intentionally small and focused so it can be embedded into larger tools or extended to meet your workflows.
