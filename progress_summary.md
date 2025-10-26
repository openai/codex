## Documentation Progress Snapshot

- **Phase 3A – File & Git Tooling**  
  Completed specs for `file-search`, `git-tooling`, `git-apply`, and `apply-patch`, covering search pipelines, ghost commits, git apply semantics, and the Codex patch engine.

- **Phase 3B – Client Libraries & Proxies**  
  Documented `backend-client`, `cloud-tasks`, `cloud-tasks-client`, `responses-api-proxy`, `rmcp-client`, and `ollama`, including OAuth flows, MCP transport, OSS model handling, and proxy behavior.

- **Phase 3C – Feedback & Telemetry**  
  Added specs for `feedback`, `otel` (config/provider/event manager), and `ansi-escape`.

- **Phase 3D – Async & IPC Utilities**  
  Covered `async-utils`, `arg0`, and `stdio-to-uds`.

- **Phase 4 – Utilities, Harnesses & Scripts**  
  Documented `utils/json-to-toml`, `utils/string`, `utils/tokenizer`, `utils/pty`, `utils/readiness`, the `chatgpt` crate, shared test harnesses (`app-server/tests/common`, `core/tests/common`, `mcp-server/tests/common`), and the `scripts/create_github_release` automation.

- **Post-Phase Sweep**  
  Added crate + module specs for `codex-login` (`mod.rs`, `lib.rs`, `server.rs`, `device_code_auth.rs`, `pkce.rs`) and its integration test harness (`tests/all.rs`, `tests/suite/*`), covering CLI login flows and PKCE helpers.
  Documented exec crate tests (`event_processor_with_json_output.rs`, `tests/suite/*`), chatgpt integration tests, linux-sandbox tests, stdio-to-uds/cloud-tasks test modules, and supporting binaries/build scripts.
  Captured the app-server protocol surface (`app-server-protocol/src/{lib,protocol,export,jsonrpc_lite}.rs`, `src/bin/export.rs`) including TypeScript/JSON schema generation workflows.
  Covered the TUI crate’s infrastructure modules (entrypoint, bottom pane/composer, streaming controller, chat widget runtime, exec cells, clipboard utilities, animations, terminal runtime) and summarized RMCP helper binaries.

- **Plan Updates**  
  `plan/codex-rs-documentation-plan.md` now marks Phase 4 as complete and narrows outstanding work to the Phase 1/2 straggler sweep plus global index updates.

### Next Focus
- Finish documenting the remaining TUI component modules (public widgets, rendering utilities, file search helpers, status/onboarding, palette/style).
- Tackle the remaining Phase 1/2 stragglers (core and MCP configuration/auth modules).
- Update `docs/code-specs/README.md`, then retire or merge `progress_summary.md` into the long-term index.
