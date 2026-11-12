# Rollout trace

Only the exact Codex binary that wrote a diagnostic trace bundle runs its
reducer. There is no cross-binary replay compatibility, even between builds
with identical version numbers.

- When changing a raw event or payload, update its writer, reducer, and affected
  fixtures together. Do not add migrations, legacy parsers, or tests for another
  binary's traces. A fallback needs a case the current writer can produce, such
  as a recording interrupted between two events.
- Treat `state.json` as the consumer API and `manifest.json` identity metadata
  as the bundle discovery API. For every `state.json` schema change, including
  changes to types serialized in both raw events and reduced state, bump
  `REDUCED_TRACE_SCHEMA_VERSION` and consider backward compatibility for
  consumers. Breaking changes are allowed. Raw-only changes do not require a
  reduced-state version bump.
