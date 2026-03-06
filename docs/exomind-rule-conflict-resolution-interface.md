# Exomind Rule Conflict Resolution Interface (M2)

## Goal

Define an implementation-ready interface for resolving rule conflicts before actions are applied in local runtime and CI checks.

## Integration Points

- After generation, before showing assistant output.
- Before write-to-disk operation.
- Before command execution.
- During CI batch evaluation for pull requests.

## Data Contracts

`RuleMatch`

- `rule_id: string`
- `rule_level: "L1" | "L2" | "L3"`
- `severity: "critical" | "high" | "medium" | "low"`
- `action: "warn" | "block" | "autofix" | "refactor_hint"`
- `scope: string`
- `version: string`
- `owner: string`
- `evidence: object`

`ResolutionDecision`

- `winner_rule_id: string`
- `suppressed_rule_ids: string[]`
- `final_action: "warn" | "block" | "autofix" | "refactor_hint"`
- `reason: string`

## Interface Draft

```text
resolve_conflicts(matches: RuleMatch[]) -> ResolutionDecision[]
```

Behavior:

1. Group matches by overlap key (`scope + matcher signature + evidence anchor`).
2. Inside each group, sort by precedence:
   - `rule_level`: L1 > L2 > L3
   - `severity`: critical > high > medium > low
   - `version`: latest semantic version
3. Emit one decision per overlap group, preserve suppressed rule ids for audit.
4. If top two candidates are still indistinguishable, mark `reason=owner_arbitration_required`.

## Pseudocode

```text
for group in group_by_overlap(matches):
  ranked = sort(group, by=[level_desc, severity_desc, version_desc])
  winner = ranked[0]
  losers = ranked[1:]
  decisions.append({
    winner_rule_id: winner.rule_id,
    suppressed_rule_ids: [x.rule_id for x in losers],
    final_action: winner.action,
    reason: build_reason(winner, losers),
  })
```

## Example Conflict Cases

1. Same matcher, L1 `block` vs L3 `autofix`:

- winner: L1 `block`.

2. Same level L2, severity `high` warn vs severity `medium` block:

- winner: severity `high` match.
- final action follows winning rule payload.

3. Same level and severity, different versions:

- winner: latest version.

4. Fully tied:

- winner chosen by deterministic order, decision reason marks owner arbitration required.
