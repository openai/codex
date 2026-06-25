# Decisions

## 2026-06-25: Require positive deadness evidence
- Context: integration code crosses feature, protocol, and external-consumer boundaries.
- Decision: delete only items supported by compiler/linter findings or exhaustive reference and construction analysis.
- Consequences: some suspicious unused public surfaces will remain if external compatibility cannot be disproven.

## 2026-06-25: Remove workspace-unreferenced `0.0.0` APIs
- Context: several dead islands were publicly reachable but had no workspace implementation, construction, or consumer.
- Decision: remove them after exhaustive reference searches and all-target compilation, while preserving serialized and protocol surfaces.
- Consequences: external Git consumers of unpublished workspace crates may need to migrate; the draft PR calls out this bounded risk.
