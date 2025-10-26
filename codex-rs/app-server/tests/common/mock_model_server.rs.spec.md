## Overview
`mock_model_server` spins up a WireMock-backed SSE server that mimics the OpenAI `/v1/chat/completions` endpoint for integration tests.

## Detailed Behavior
- `create_mock_chat_completions_server(responses)`:
  - Starts a `wiremock::MockServer`.
  - Wraps the supplied SSE payloads in a `SeqResponder` that tracks how many times the endpoint has been called.
  - Registers a mock matching `POST /v1/chat/completions`, returning each response in order and asserting the number of expected calls.
- `SeqResponder` implements `wiremock::Respond`, constructing 200 responses with the `text/event-stream` content type and panicking when more requests arrive than responses provided.

## Broader Context
- Used alongside `McpProcess` to supply deterministic model outputs while exercising streaming behavior end-to-end.
- Supports SSE payloads constructed by helpers in `responses.rs`.

## Technical Debt
- Mock server only supports sequential playback; tests needing branching or error status codes must reimplement their own responder.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./responses.rs.spec.md
