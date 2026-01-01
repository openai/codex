# Eval Capture Bundles

Codex can capture "eval case" bundles from the `/feedback` flow (bad result -> capture eval sample).
These bundles are meant to turn real failures into reproducible, local-first artifacts.

## Where Bundles Are Written

Bundles are stored under:

`$CODEX_HOME/eval-case/<case-id>/`

## Bundle Contents

Each bundle contains:

- `manifest.json` - metadata about the capture (schema version, start marker, notes, repo base).
- `rollout.jsonl` - the full session rollout (multi-turn trajectory).
- `repo.patch` - a git patch representing the repository state at the chosen start marker.
- `codex-logs.log` - tracing logs to help maintainers debug the session.

## Start Marker And Repo State

Bundles include the entire rollout, but also record a start marker to indicate where an eval
harness (or a human) should begin replaying/interpreting the trajectory.

The repository patch must match that chosen start marker:

- If the session has repo snapshots available, `repo.patch` is derived from the ghost snapshot
  commit associated with the selected user turn (diff from the snapshot's base commit to the
  snapshot commit).
- If no snapshot is available for a given start marker, the TUI disables that option (and may
  fall back to the basic feedback flow instead).

For reproducibility outside your machine, the base commit recorded in `manifest.json` should be
reachable by maintainers (for example, pushed and available on the default branch).

## App-Server API (For Integrations)

Non-TUI clients can create bundles via the app-server JSON-RPC method:

- `evalCase/create`

The handler copies the rollout into the bundle and derives `repo.patch` based on the selected start
marker when repo snapshots are available.
