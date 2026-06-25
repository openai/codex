# Parity Checklist

Last updated: 2026-06-25

## MCP
- [x] MCP cleanup preserves client/server behavior and schemas.

## Apps / connectors
- [x] App and connector cleanup preserves listings, auth, invocation, and protocol behavior.

## Plugins / skills
- [x] Plugin and skill cleanup preserves discovery, loading, configuration, and invocation.

## Behavior / platform
- [x] Feature-gated, test-only, and platform-specific behavior remains intact.

## Verification
- [x] Formatting passes.
- [x] Targeted checks/tests pass, apart from two unrelated environment-sensitive skill-root assertions.
- [x] Independent diff review finds no live-surface deletion.
