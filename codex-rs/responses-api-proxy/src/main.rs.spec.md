## Overview
`responses-api-proxy::main` is the entrypoint for the Codex Responses API proxy. It applies process hardening before parsing CLI arguments and delegates to `run_main` in the library crate.

## Detailed Behavior
- Uses `#[ctor::ctor]` to run `codex_process_hardening::pre_main_hardening` before `main`, ensuring core dumps/ptrace are disabled.
- `main`:
  - Parses CLI arguments (`ResponsesApiProxyArgs`) with Clap.
  - Calls `codex_responses_api_proxy::run_main(args)` and returns its `anyhow::Result`.

## Broader Context
- The proxy forwards OpenAI Responses API requests through Codex-controlled infrastructure. The heavy lifting is handled by the library; the binary is a thin wrapper.

## Technical Debt
- None; the binary intentionally keeps no logic.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../process-hardening/src/lib.rs.spec.md
  - codex-responses-api-proxy library spec (future)
