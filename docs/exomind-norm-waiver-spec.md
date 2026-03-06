# Exomind Norm Waiver Spec (M6)

Waivers are used to suppress block/warn findings for known exceptions with expiry.

## File Location
- Default CI example: `docs/exomind-norm-waivers.json`

## Supported Shapes
1. Wrapped form:
```json
{
  "waivers": [
    {
      "waiver_id": "WAIVER-001",
      "owner": "security-architecture",
      "expiry": "2026-12-31",
      "reason": "Temporary migration window",
      "target": "rule:L1-SEC-NO-SHELL-UNSAFE"
    }
  ]
}
```

2. List form:
```json
[
  {
    "waiver_id": "WAIVER-002",
    "owner": "runtime-quality",
    "expiry": "2026-10-01",
    "reason": "Legacy PR in transition",
    "target": "warning:RULE_FIELD_MISSING:L2-TEST-CHANGED-CODE-HAS-TEST"
  }
]
```

## Target Values
- Rule-scoped:
  - `rule:<rule_id>`
- Warning-scoped:
  - `warning:<code>:<rule_id-or-catalog>`
- Conflict-scoped:
  - `conflict:<ruleA>|<ruleB>` (sorted pair)

## Required Fields
- `waiver_id`
- `owner`
- `expiry` (ISO date, `YYYY-MM-DD`)
- `reason`
- `target` (or backward-compatible `rule_id`)

## Expiry Semantics
- Expired waivers are ignored and listed in governance reports.
- Active waivers are applied at report-time and runtime checks.
