# Fixtures

Last updated: 2026-06-25

## Source-of-truth fixtures
- Path: existing tests and snapshots in affected crates.
- Notes: this behavior-neutral cleanup should require no fixture changes.

## Mapped fixtures
- [x] Existing connector snapshot tests cover live snapshot construction.
- [x] OAuth discovery coverage was retained after deleting its unused wrapper.
- [x] Existing app-instructions integration tests cover the behavior formerly duplicated by the dead test-only renderer.

## Coverage gaps
- [x] No generated fixtures or snapshots changed.
- [x] External Git consumers remain the only unobservable compatibility risk for removed public `0.0.0` crate APIs.
