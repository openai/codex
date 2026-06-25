# Decisions

## 2026-06-25: Require positive deadness evidence
- Context: integration code crosses feature, protocol, and external-consumer boundaries.
- Decision: delete only items supported by compiler/linter findings or exhaustive reference and construction analysis.
- Consequences: some suspicious unused public surfaces will remain if external compatibility cannot be disproven.
