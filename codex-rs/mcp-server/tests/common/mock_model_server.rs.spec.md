## Overview
`mock_model_server` starts a WireMock server that returns scripted SSE responses for `/v1/chat/completions`, ensuring MCP integration tests receive deterministic model output.

## Detailed Behavior
- `create_mock_chat_completions_server(responses)` launches a mock server, registers a sequential responder, and asserts the number of expected calls.
- `SeqResponder` tracks the current call index with an `AtomicUsize`, returning each provided SSE payload in order and panicking if the test issues extra requests.

## Broader Context
- Shared with app-server test fixtures; MCP suites reuse it to stream apply_patch or shell tool calls via SSE.

## Technical Debt
- Only supports happy-path 200 responses; adding support for error injection would broaden test coverage.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./responses.rs.spec.md
