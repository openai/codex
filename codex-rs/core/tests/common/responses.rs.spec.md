## Overview
`core::tests::common::responses` builds WireMock responders and SSE utilities tailored to the `/v1/responses` endpoint Codex uses for model streaming.

## Detailed Behavior
- `ResponseMock` captures every request hitting the mounted mock, storing them for later inspection. Methods (`single_request`, `requests`) expose captured payloads while enforcing expectations on counts.
- `ResponsesRequest` wraps `wiremock::Request` with helpers to access JSON bodies, tool call outputs, headers, paths, and query params.
- `Match` implementation records requests and enforces invariants via `validate_request_body_invariants`, ensuring every tool call output corresponds to an input (and vice versa) before tests proceed.
- SSE helpers (`sse`, `ev_completed`, `ev_response_created`, etc.) provide building blocks for constructing realistic streaming sequences.
- `mount_sse_once`, `mount_sse_sequence`, `mount_json_response` attach mocks to a `MockServer`, returning `ResponseMock` handles for assertions.
- Invariant validation checks for missing or empty `call_id`s, mismatched tool outputs, and ensures symmetry between calls and outputs.

## Broader Context
- Powers end-to-end tests verifying Codex’s tool orchestration, ensuring responses posted back to `/v1/responses` remain well-formed.
- Aligns with production expectations documented in `codex-core` tool specs, giving the suite tight feedback on regressions.

## Technical Debt
- Helper currently assumes sequential request ordering; parallel test scenarios would need enhancements to differentiate concurrent streams.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Enhance mocks to handle concurrent conversations or annotate request contexts for parallel test support.
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
