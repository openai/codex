# Exomind Rule Catalog Spec (M1)

## Purpose

Define a machine-readable rule catalog schema that can be consumed by Codex CLI policy runtime and CI warn checks.

## Minimal Schema

Top-level fields:

- `catalog_version`: semantic version for schema payload.
- `updated_at`: catalog update date in `YYYY-MM-DD`.
- `owner`: catalog owner group.
- `rules`: array of rule objects.

Rule object required fields:

- `rule_id`: stable unique identifier.
- `title`: concise human-readable rule name.
- `rule_level`: one of `L1`, `L2`, `L3`.
- `severity`: one of `critical`, `high`, `medium`, `low`.
- `scope`: repository path scope pattern.
- `owner`: rule owner team or role.
- `version`: rule semantic version.
- `matcher`: object with `type`, `value`, `language`.
- `action`: one of `warn`, `block`, `autofix`, `refactor_hint`.
- `evidence`: object with `required_fields` array and optional `note`.

## Priority Mapping

- `L1`: architecture/security constitutional rules. Default `block`.
- `L2`: domain/testing rules. Default `warn` or `block`.
- `L3`: style/convention rules. Default `warn` or `autofix`.

Conflict precedence:

1. `rule_level` (`L1 > L2 > L3`)
2. `severity` (`critical > high > medium > low`)
3. `version` (latest wins) and owner arbitration fallback

## Example Catalog

- Template file: `docs/exomind-rule-catalog-template.json`
- Included examples:
  - `L1-SEC-NO-SHELL-UNSAFE`
  - `L2-TEST-CHANGED-CODE-HAS-TEST`
  - `L3-STYLE-IMPORT-ORDER`
