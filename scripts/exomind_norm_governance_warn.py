#!/usr/bin/env python3
"""Generate warn-only norm governance reports from a rule catalog template."""

from __future__ import annotations

import argparse
import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

VALID_LEVELS = {"L1", "L2", "L3"}
VALID_SEVERITIES = {"critical", "high", "medium", "low"}
VALID_ACTIONS = {"warn", "block", "autofix", "refactor_hint"}
REQUIRED_RULE_FIELDS = {
    "rule_id",
    "title",
    "rule_level",
    "severity",
    "scope",
    "owner",
    "version",
    "matcher",
    "action",
    "evidence",
}
REQUIRED_MATCHER_FIELDS = {"type", "value", "language"}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--catalog", required=True, help="Path to rule catalog JSON file.")
    parser.add_argument(
        "--markdown-out",
        required=True,
        help="Output markdown report path.",
    )
    parser.add_argument(
        "--json-out",
        required=True,
        help="Output machine-readable report path.",
    )
    return parser.parse_args()


def load_catalog(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as f:
        data = json.load(f)
    if not isinstance(data, dict):
        raise ValueError("catalog must be a JSON object")
    return data


def add_warning(
    warnings: list[dict[str, str]],
    code: str,
    message: str,
    rule_id: str | None = None,
) -> None:
    warning = {"code": code, "message": message}
    if rule_id:
        warning["rule_id"] = rule_id
    warnings.append(warning)


def validate_rules(rules: list[dict[str, Any]]) -> list[dict[str, str]]:
    warnings: list[dict[str, str]] = []
    seen_rule_ids: set[str] = set()

    for i, rule in enumerate(rules):
        if not isinstance(rule, dict):
            add_warning(
                warnings,
                "RULE_TYPE_INVALID",
                f"rules[{i}] is not an object",
            )
            continue

        missing_fields = sorted(REQUIRED_RULE_FIELDS - set(rule.keys()))
        rule_id = str(rule.get("rule_id", f"rules[{i}]"))
        if missing_fields:
            add_warning(
                warnings,
                "RULE_FIELD_MISSING",
                f"missing fields: {', '.join(missing_fields)}",
                rule_id,
            )

        if rule_id in seen_rule_ids:
            add_warning(
                warnings,
                "RULE_ID_DUPLICATED",
                "rule_id is duplicated in catalog",
                rule_id,
            )
        seen_rule_ids.add(rule_id)

        level = rule.get("rule_level")
        if level not in VALID_LEVELS:
            add_warning(
                warnings,
                "RULE_LEVEL_INVALID",
                f"invalid rule_level: {level}",
                rule_id,
            )

        severity = rule.get("severity")
        if severity not in VALID_SEVERITIES:
            add_warning(
                warnings,
                "RULE_SEVERITY_INVALID",
                f"invalid severity: {severity}",
                rule_id,
            )

        action = rule.get("action")
        if action not in VALID_ACTIONS:
            add_warning(
                warnings,
                "RULE_ACTION_INVALID",
                f"invalid action: {action}",
                rule_id,
            )

        matcher = rule.get("matcher")
        if not isinstance(matcher, dict):
            add_warning(
                warnings,
                "MATCHER_INVALID",
                "matcher must be an object",
                rule_id,
            )
        else:
            matcher_missing = sorted(REQUIRED_MATCHER_FIELDS - set(matcher.keys()))
            if matcher_missing:
                add_warning(
                    warnings,
                    "MATCHER_FIELD_MISSING",
                    f"matcher missing fields: {', '.join(matcher_missing)}",
                    rule_id,
                )

    return warnings


def detect_conflicts(rules: list[dict[str, Any]]) -> list[dict[str, str]]:
    conflicts: list[dict[str, str]] = []
    # Heuristic: same scope + same matcher(type,value,language) but different action.
    for i in range(len(rules)):
        left = rules[i]
        if not isinstance(left, dict):
            continue
        left_matcher = left.get("matcher")
        if not isinstance(left_matcher, dict):
            continue
        for j in range(i + 1, len(rules)):
            right = rules[j]
            if not isinstance(right, dict):
                continue
            right_matcher = right.get("matcher")
            if not isinstance(right_matcher, dict):
                continue

            same_scope = left.get("scope") == right.get("scope")
            same_matcher = (
                left_matcher.get("type") == right_matcher.get("type")
                and left_matcher.get("value") == right_matcher.get("value")
                and left_matcher.get("language") == right_matcher.get("language")
            )
            action_differs = left.get("action") != right.get("action")

            if same_scope and same_matcher and action_differs:
                conflicts.append(
                    {
                        "left_rule_id": str(left.get("rule_id", f"rules[{i}]")),
                        "right_rule_id": str(right.get("rule_id", f"rules[{j}]")),
                        "reason": "same_scope_and_matcher_but_action_differs",
                    }
                )
    return conflicts


def to_markdown(report: dict[str, Any]) -> str:
    lines: list[str] = []
    lines.append("# Norm Governance Warn Report")
    lines.append("")
    lines.append(f"- Generated at: `{report['generated_at']}`")
    lines.append(f"- Catalog path: `{report['catalog_path']}`")
    lines.append(f"- Rules total: `{report['rules_total']}`")
    lines.append(f"- Warnings total: `{report['warnings_total']}`")
    lines.append(f"- Potential conflicts: `{report['conflicts_total']}`")
    lines.append("")

    lines.append("## Warnings")
    if report["warnings"]:
        for warning in report["warnings"]:
            prefix = warning.get("rule_id", "catalog")
            lines.append(
                f"- `{warning['code']}` `{prefix}`: {warning['message']}"
            )
    else:
        lines.append("- None")
    lines.append("")

    lines.append("## Potential Conflicts")
    if report["conflicts"]:
        for conflict in report["conflicts"]:
            lines.append(
                "- "
                f"`{conflict['left_rule_id']}` vs `{conflict['right_rule_id']}`: "
                f"{conflict['reason']}"
            )
    else:
        lines.append("- None")
    lines.append("")
    lines.append("## Mode")
    lines.append("- warn-only: this report does not fail CI.")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    args = parse_args()

    catalog_path = Path(args.catalog)
    markdown_out = Path(args.markdown_out)
    json_out = Path(args.json_out)

    catalog = load_catalog(catalog_path)
    rules = catalog.get("rules", [])
    if not isinstance(rules, list):
        raise ValueError("catalog.rules must be an array")

    typed_rules = [r for r in rules if isinstance(r, dict)]
    warnings = validate_rules(typed_rules)
    conflicts = detect_conflicts(typed_rules)

    report = {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "catalog_path": str(catalog_path).replace("\\", "/"),
        "rules_total": len(typed_rules),
        "warnings_total": len(warnings),
        "conflicts_total": len(conflicts),
        "warnings": warnings,
        "conflicts": conflicts,
        "mode": "warn-only",
    }

    markdown_out.parent.mkdir(parents=True, exist_ok=True)
    json_out.parent.mkdir(parents=True, exist_ok=True)
    markdown_out.write_text(to_markdown(report), encoding="utf-8")
    json_out.write_text(json.dumps(report, indent=2), encoding="utf-8")

    print(
        "norm-governance warn report generated:",
        markdown_out.as_posix(),
        json_out.as_posix(),
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
