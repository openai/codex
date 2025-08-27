# macOS Native Browser Login (WKWebView)

Codex can complete ChatGPT authentication using a tiny macOS helper that opens a real browser context (WebKit/WKWebView) and intercepts the final redirect to `http://localhost/.../auth/callback`. This avoids issues where desktop browsers refuse to follow an `https` → `http` redirect for localhost, and removes the need to run a local HTTP server.

## When to use it

- You’re on macOS and your default browser blocks the `http://localhost` callback.
- You want a seamless login from the CLI without copying URLs or running a local login server.

## Usage

```bash
codex login --browser
```

This opens a small window titled “Codex – Sign in to OpenAI.” After you complete login, the window closes automatically and Codex persists your credentials to `$CODEX_HOME/auth.json` (defaults to `~/.codex/auth.json`).

Notes:
- This feature is currently macOS-only. On other platforms, `codex login --browser` will print a friendly “not supported” message.
- The helper uses a non‑persistent `WKWebsiteDataStore` and only returns the OAuth `code` and `state` to the CLI. The CLI exchanges tokens and performs secure persistence.
- Basic macOS menus are available (About, Quit, Cut/Copy/Paste/Select All). Copy/paste (⌘C/⌘V) works in all form fields.
- Closing the window aborts login (non-zero exit). The CLI prints a neutral “Login aborted…” message and does not reopen the helper.

## Requirements (macOS)

- macOS 12+
- Xcode Command Line Tools (for `swiftc`) — run: `xcode-select --install`

Build details:
- During `cargo build` on macOS, the tiny Swift helper (`WKWebView`) is compiled and embedded into the Rust binary. Codex prefers this embedded helper at runtime.
- The embedded helper is built as a universal binary (arm64 + x86_64), so a single build runs on both Apple Silicon and Intel Macs.
- On non‑macOS targets, the helper is not compiled/embedded.
- If `swiftc` was not available at build time, the build script will emit a warning and embed an empty placeholder. At runtime, Codex will attempt to compile the helper on‑demand the first time you run `codex login --browser` (requires `swiftc` on that machine).

## How it works (high level)

1. The CLI generates PKCE + state and constructs the regular authorize URL with `redirect_uri=http://localhost:1455/auth/callback`.
2. A tiny helper app opens that URL in `WKWebView`.
3. When the page tries to navigate to `http://localhost/.../auth/callback`, the helper intercepts the navigation (via `WKNavigationDelegate`), extracts `code` and `state`, cancels the load, prints JSON to stdout, and exits.
4. The CLI validates `state`, exchanges the `code` for tokens, and optionally performs a token exchange for an API Key (when available). Finally, it persists tokens to `auth.json` and prints a success message that includes basic account details (e.g., email, plan) when available.

Exit codes and messages:
- Success: prints “✅ Successfully logged in using native browser …” and exits 0.
- Aborted by user (window closed): prints “Login aborted (native browser window closed)” and exits 2.
- Other failures: prints a concise error and exits 1.

## Troubleshooting

- “swiftc not found” during `cargo build`:
  - Install Xcode Command Line Tools: `xcode-select --install`. Rebuild.
  - If build still cannot compile the helper, the runtime will try an on‑demand compile the first time you run `codex login --browser`.
- The window opens but then closes without logging in:
  - Ensure the login completed and no network policy blocked the flow. Run again and check terminal output for errors.
- Using a non‑macOS platform:
  - This feature is macOS‑only for now; use the standard `codex login` flow or the “device authorization” path.

## Security considerations

- The helper runs with an ephemeral (non‑persistent) WebKit data store.
- The helper returns only `code`/`state` to the CLI. Token exchange and persistence happen in the CLI process.
- The CLI writes `auth.json` with mode `0600` on Unix systems.

## Implementation overview

- CLI flag: `codex login --browser` (in `codex-rs/cli`).
- Helper build: `codex-rs/login/build.rs` compiles `src/native_browser_helper.swift` on macOS and embeds a universal (arm64 + x86_64) helper at `OUT_DIR/codex-auth-helper`.
- Runtime: `codex-rs/login/src/native_browser.rs` prefers the embedded helper; otherwise falls back to compiling the same Swift source on-demand.
