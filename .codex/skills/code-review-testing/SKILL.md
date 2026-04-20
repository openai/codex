---
name: code-review-testing
description: Test authoring guidance
---

For agent changes prefer integration tests over unit tests. Integration tests are under `core/suite` and use `test_codex` to set up a test instance of codex.

Features change change the agent logic MUST add an integration test:
- Provide a list of major logic changes and user-facing behaviors that need to be tested.

Check whether there are existing helpers to make tests more streamlined and readable.