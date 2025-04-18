# Deploying Codex CLI in a Locked-Down Enterprise Environment

This document outlines the steps and considerations for packaging and running the Codex CLI in an environment with restricted access (no Docker, no external services calls aside from OpenAI API).

1. Runtime Requirements
   - Node.js v22 or later installed on target machines.
   - No Docker or additional runtime services required—Codex bundles into a single executable script.

2. Installation / Packaging
   - Build upstream (outside the locked-down env) with:
     ```bash
     npm install && npm run build
     ```
   - From the build output, ship these files into your environment:
     - `bin/codex.js` (loader script)
     - `dist/cli.js` (self‑contained bundle)
     - `require-shim.js` (injection for CommonJS shimming)
   - Optionally archive them together (e.g. `codex.tar.gz`) and unpack on each host.
   - No need to install `node_modules` at runtime.

3. Configuration
   - Set `OPENAI_API_KEY` in the environment or via `codex config set apiKey ...`.
   - If using a custom proxy or private OpenAI endpoint, set `OPENAI_BASE_URL`.
   - Codex persists user config and history under `~/.codex`.

4. Network Egress
   - Codex only makes HTTPS calls to OpenAI (`openai` SDK) on port 443.
   - No other external API calls or telemetry uploads.
   - Firewall rules can restrict egress to `*.openai.com` (and custom base URL if used).

5. Enterprise-Friendly Details
   - Offline installs: mirror the npm package in an internal registry or ship the built bundle directly.
   - No privileged operations—runs entirely as the invoking user.
   - Local logging is used (disabled by default unless `DEBUG` is set).

6. Optional Tightening
   - Disable file‑system logging by leaving `DEBUG` unset (no-op logger).
   - Avoid `open` calls by not using any `--open` flags or removing that behavior if necessary.

Bottom line: Codex CLI is a self-contained Node.js executable that only requires egress to the OpenAI API. You can pre-build it once, ship the `bin/` and `dist/` files into your locked-down environment, configure your API key/base URL, and run it without Docker or additional services.