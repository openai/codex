# Contributing (Python SDK)

Thanks for improving the Codex app-server Python SDK.

## Development setup

```bash
cd sdk/python
python3 -m venv .venv
source .venv/bin/activate
python -m pip install -U pip
python -m pip install -e '.[dev]'
```

## Project principles

- Keep dict-native APIs stable and backward compatible.
- Add ergonomic helpers as additive APIs (`*_typed`, `*_schema`, convenience helpers).
- Preserve sync/async parity where feasible.
- Favor explicit typing and accurate return annotations.
- Keep docs and tests updated in the same change.

## Typical workflow

1. Make code changes in `src/codex_app_server/`.
2. If protocol/schema contracts changed, regenerate derived files:

```bash
python scripts/generate_protocol_typed_dicts.py
python scripts/generate_types_from_schema.py
```

3. Run tests:

```bash
pytest
```

4. If your environment has `codex` configured, run real integration smoke tests:

```bash
RUN_REAL_CODEX_TESTS=1 pytest tests/test_real_app_server_integration.py
```

5. Update docs/changelog for user-visible changes.

## Testing expectations

- New features should include unit/flow coverage in `tests/test_sdk_flow.py`.
- Transport/protocol behavior should be validated against `tests/fake_app_server.py`.
- Real integration tests are optional locally, but should pass in environments where `codex` is available.

## Commit guidance

- Prefer focused commits by concern (e.g., API parity, tests, docs).
- Use clear messages describing user-visible impact.
- Avoid bundling unrelated refactors.

## Release checklist

- [ ] All tests pass (`pytest`)
- [ ] Real integration smoke tests pass when env allows
- [ ] Typing/doc parity verified for changed API surface
- [ ] `CHANGELOG.md` updated
- [ ] `README.md` updated when behavior or recommended usage changes
