# Release Checklist (Python SDK)

## Pre-flight

- [ ] Clean working tree
- [ ] Version set in `pyproject.toml`
- [ ] `CHANGELOG.md` contains release notes for target version
- [ ] `README.md` examples and feature list reflect current surface

## Validation

- [ ] `python -m pip install -e '.[dev]'`
- [ ] `pytest`
- [ ] `RUN_REAL_CODEX_TESTS=1 pytest tests/test_real_app_server_integration.py` (when env supports `codex`)

## Build / packaging

- [ ] `python -m build`
- [ ] `python -m twine check dist/*`

## API sanity pass

- [ ] Sync/async parity maintained for all public high-level methods
- [ ] `*_typed` / `*_schema` parity maintained where expected
- [ ] Error mapping and retry helpers still covered by tests

## Publish

- [ ] Tag commit (`vX.Y.Z`)
- [ ] Publish package artifacts
- [ ] Announce highlights and migration notes
