# Codex GUI Service

This crate exposes a lightweight HTTP service that translates curated UI actions into Codex CLI invocations.
It powers the web front-end located in `gui/frontend`.

## Running locally

```bash
# 1. Start the HTTP service
cd codex-rs
cargo run -p codex-gui

# Optional: override settings
# CODEX_GUI_PORT=8080 CODEX_GUI_CLI_PATH=./target/debug/codex cargo run -p codex-gui

# 2. Start the front-end
cd ../gui/frontend
pnpm install
pnpm dev
```

The front-end expects the backend at `http://localhost:8787` by default.
Set `VITE_API_URL` when running `pnpm dev` if you customize the port.

## API

- `GET /api/actions` – Fetches metadata about the available orchestration playbooks.
- `POST /api/actions/{id}/execute` – Runs the selected playbook. The request body must include a
  `values` object with form field values. The response contains stdout, stderr, exit code, and timing data
  from the underlying CLI invocation.

The service shells out to the Codex CLI (`codex` by default). Use `CODEX_GUI_CLI_PATH` to point to a
specific executable if it is not on the `PATH`.
