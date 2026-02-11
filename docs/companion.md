# Companion Web UI

Codex includes an experimental, local web UI backed by `codex app-server`.

## Usage

Run:

```sh
codex --companion
```

Codex will:

- Start a local HTTP server bound to `127.0.0.1`
- Print a URL containing a per-run session token
- Open that URL in your browser (disable with `--companion-no-open`)

Optional flags:

- `--companion-port <PORT>`: bind a specific port (default `0` picks a free port)
- `--companion-no-open`: don’t auto-open the browser
- `--companion-ui-dev-url <URL>`: use an external frontend URL (for example Vite) instead of embedded UI assets

## Security Model

- The server binds only to `127.0.0.1` (localhost).
- The UI entrypoint (`/`) and transport websocket (`/ws`) require a per-run random `token` included in the URL.

## Frontend Development

The Companion frontend is a React app in `codex-rs/companion/ui`.

```sh
cd codex-rs/companion/ui
pnpm install --ignore-workspace
pnpm build
```

This writes production assets to `codex-rs/companion/ui/dist`, which are embedded into the Rust binary.

### Live Reload (Vite HMR)

For UI development, run Companion against a Vite dev server so frontend edits live-reload without restarting Codex:

Terminal 1:

```sh
cd codex-rs/companion/ui
pnpm install --ignore-workspace
pnpm dev --host 127.0.0.1 --port 5173
```

Terminal 2:

```sh
cd codex-rs
RUSTC="$(rustup which rustc --toolchain 1.93.0)" \
"$(rustup which cargo --toolchain 1.93.0)" run -p codex-cli -- \
  --companion \
  --companion-no-open \
  --companion-port 4321 \
  --companion-ui-dev-url http://127.0.0.1:5173
```

Open the printed `Companion:` URL. The React app will come from Vite (`5173`) while WebSocket/API traffic is routed to Companion (`4321`).

## Troubleshooting

- If the browser opens but shows “Missing token”, use the exact URL printed in your terminal.
- If you’re running over SSH, use `--companion-no-open` and copy the printed URL into a browser on the same machine.
