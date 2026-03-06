#!/usr/bin/env python3
"""Generate norm governance reports for warn/block modes with waiver support."""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from datetime import UTC, date, datetime
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


@dataclass
class Waiver:
    waiver_id: str
    owner: str
    expiry: date
    reason: str
    target: str


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--catalog", required=True, help="Path to rule catalog JSON file.")
    parser.add_argument("--markdown-out", required=True, help="Output markdown report path.")
    parser.add_argument("--json-out", required=True, help="Output machine-readable report path.")
    parser.add_argument(
        "--mode",
        choices=["warn", "block"],
        default="warn",
        help="warn: never fail CI, block: fail CI on unwaived L1 findings.",
    )
    parser.add_argument(
        "--waivers",
        help="Optional waiver JSON path.",
    )
    return parser.parse_args()


def load_catalog(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as f:
        data = json.load(f)
    if not isinstance(data, dict):
        raise ValueError("catalog must be a JSON object")
    return data


def load_waivers(path: str | None) -> tuple[list[Waiver], list[dict[str, str]]]:
    if not path:
        return [], []

    waivers_path = Path(path)
    if not waivers_path.exists():
        return [], [{"code": "WAIVER_FILE_MISSING", "message": f"{waivers_path} not found"}]

    with waivers_path.open("r", encoding="utf-8") as f:
        raw = json.load(f)

    entries: list[dict[str, Any]]
    if isinstance(raw, list):
        entries = [e for e in raw if isinstance(e, dict)]
    elif isinstance(raw, dict) and isinstance(raw.get("waivers"), list):
        entries = [e for e in raw["waivers"] if isinstance(e, dict)]
    else:
        return [], [{"code": "WAIVER_FORMAT_INVALID", "message": "expected list or {waivers: []}"}]

    parsed: list[Waiver] = []
    parse_errors: list[dict[str, str]] = []
    for i, entry in enumerate(entries):
        waiver_id = str(entry.get("waiver_id", f"waiver-{i}"))
        owner = str(entry.get("owner", "unknown"))
        reason = str(entry.get("reason", ""))
        target = entry.get("target")
        if not isinstance(target, str) or not target:
            # Backward-compatible shape: rule_id
            rule_id = entry.get("rule_id")
            if isinstance(rule_id, str) and rule_id:
                target = f"rule:{rule_id}"
            else:
                parse_errors.append(
                    {
                        "code": "WAIVER_TARGET_MISSING",
                        "message": f"{waiver_id} missing target or rule_id",
                    }
                )
                continue
        expiry_raw = entry.get("expiry")
        if not isinstance(expiry_raw, str):
            parse_errors.append(
                {
                    "code": "WAIVER_EXPIRY_INVALID",
                    "message": f"{waiver_id} missing expiry in YYYY-MM-DD",
                }
            )
            continue
        try:
            expiry = date.fromisoformat(expiry_raw)
        except ValueError:
            parse_errors.append(
                {
                    "code": "WAIVER_EXPIRY_INVALID",
                    "message": f"{waiver_id} has invalid expiry {expiry_raw}",
                }
            )
            continue
        parsed.append(
            Waiver(
                waiver_id=waiver_id,
                owner=owner,
                reason=reason,
                expiry=expiry,
                target=target,
            )
        )
    return parsed, parse_errors


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


def rule_level_index(rules: list[dict[str, Any]]) -> dict[str, str]:
    level_by_rule: dict[str, str] = {}
    for rule in rules:
        rule_id = rule.get("rule_id")
        level = rule.get("rule_level")
        if isinstance(rule_id, str) and isinstance(level, str):
            level_by_rule[rule_id] = level
    return level_by_rule


def waiver_target_for_warning(warning: dict[str, str]) -> str:
    rule_id = warning.get("rule_id", "catalog")
    return f"warning:{warning['code']}:{rule_id}"


def waiver_target_for_conflict(conflict: dict[str, str]) -> str:
    left = conflict["left_rule_id"]
    right = conflict["right_rule_id"]
    ordered = "|".join(sorted([left, right]))
    return f"conflict:{ordered}"


def annotate_waivers(
    warnings: list[dict[str, str]],
    conflicts: list[dict[str, str]],
    waivers: list[Waiver],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], list[dict[str, str]]]:
    today = date.today()
    active_by_target: dict[str, Waiver] = {}
    expired: list[dict[str, str]] = []
    for waiver in waivers:
        if waiver.expiry < today:
            expired.append(
                {
                    "waiver_id": waiver.waiver_id,
                    "target": waiver.target,
                    "owner": waiver.owner,
                    "expiry": waiver.expiry.isoformat(),
                }
            )
            continue
        active_by_target[waiver.target] = waiver

    annotated_warnings: list[dict[str, Any]] = []
    for warning in warnings:
        item = dict(warning)
        target = waiver_target_for_warning(warning)
        rule_target = f"rule:{warning['rule_id']}" if "rule_id" in warning else ""
        waiver = active_by_target.get(target) or (active_by_target.get(rule_target) if rule_target else None)
        item["waived"] = waiver is not None
        if waiver:
            item["waiver_id"] = waiver.waiver_id
            item["waiver_owner"] = waiver.owner
        annotated_warnings.append(item)

    annotated_conflicts: list[dict[str, Any]] = []
    for conflict in conflicts:
        item = dict(conflict)
        target = waiver_target_for_conflict(conflict)
        left_target = f"rule:{conflict['left_rule_id']}"
        right_target = f"rule:{conflict['right_rule_id']}"
        waiver = (
            active_by_target.get(target)
            or active_by_target.get(left_target)
            or active_by_target.get(right_target)
        )
        item["waived"] = waiver is not None
        if waiver:
            item["waiver_id"] = waiver.waiver_id
            item["waiver_owner"] = waiver.owner
        annotated_conflicts.append(item)

    return annotated_warnings, annotated_conflicts, expired


def blocking_findings(
    warnings: list[dict[str, Any]],
    conflicts: list[dict[str, Any]],
    level_by_rule: dict[str, str],
) -> list[dict[str, str]]:
    findings: list[dict[str, str]] = []
    for warning in warnings:
        if warning.get("waived"):
            continue
        rule_id = warning.get("rule_id")
        if isinstance(rule_id, str) and level_by_rule.get(rule_id) == "L1":
            findings.append(
                {
                    "kind": "warning",
                    "target": waiver_target_for_warning(warning),
                    "rule_id": rule_id,
                    "code": warning["code"],
                }
            )

    for conflict in conflicts:
        if conflict.get("waived"):
            continue
        left = conflict["left_rule_id"]
        right = conflict["right_rule_id"]
        if level_by_rule.get(left) == "L1" or level_by_rule.get(right) == "L1":
            findings.append(
                {
                    "kind": "conflict",
                    "target": waiver_target_for_conflict(conflict),
                    "rule_id": f"{left}|{right}",
                    "code": "RULE_CONFLICT_L1",
                }
            )

    return findings


def to_markdown(report: dict[str, Any]) -> str:
    lines: list[str] = []
    lines.append("# Norm Governance Report")
    lines.append("")
    lines.append(f"- Generated at: `{report['generated_at']}`")
    lines.append(f"- Catalog path: `{report['catalog_path']}`")
    lines.append(f"- Mode: `{report['mode']}`")
    lines.append(f"- Rules total: `{report['rules_total']}`")
    lines.append(f"- Warnings total: `{report['warnings_total']}`")
    lines.append(f"- Potential conflicts: `{report['conflicts_total']}`")
    lines.append(f"- Blocking findings: `{report['blocking_findings_total']}`")
    lines.append("")

    lines.append("## Warnings")
    if report["warnings"]:
        for warning in report["warnings"]:
            prefix = warning.get("rule_id", "catalog")
            waiver_suffix = (
                f" (waived by {warning['waiver_id']})" if warning.get("waived") else ""
            )
            lines.append(
                f"- `{warning['code']}` `{prefix}`: {warning['message']}{waiver_suffix}"
            )
    else:
        lines.append("- None")
    lines.append("")

    lines.append("## Potential Conflicts")
    if report["conflicts"]:
        for conflict in report["conflicts"]:
            waiver_suffix = (
                f" (waived by {conflict['waiver_id']})" if conflict.get("waived") else ""
            )
            lines.append(
                "- "
                f"`{conflict['left_rule_id']}` vs `{conflict['right_rule_id']}`: "
                f"{conflict['reason']}{waiver_suffix}"
            )
    else:
        lines.append("- None")
    lines.append("")

    lines.append("## Waivers")
    lines.append(f"- Active waivers loaded: `{report['active_waivers_total']}`")
    lines.append(f"- Expired waivers: `{report['expired_waivers_total']}`")
    if report["expired_waivers"]:
        for item in report["expired_waivers"]:
            lines.append(
                f"- expired `{item['waiver_id']}` target `{item['target']}` owner `{item['owner']}` expiry `{item['expiry']}`"
            )
    lines.append("")

    lines.append("## Result")
    if report["blocked"]:
        lines.append("- `blocked`: block mode found unwaived L1 findings.")
    else:
        lines.append("- `pass`: no unwaived blocking findings.")
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
    waivers, waiver_parse_errors = load_waivers(args.waivers)

    warnings.extend(waiver_parse_errors)
    level_by_rule = rule_level_index(typed_rules)
    annotated_warnings, annotated_conflicts, expired_waivers = annotate_waivers(
        warnings,
        conflicts,
        waivers,
    )
    blocks = blocking_findings(annotated_warnings, annotated_conflicts, level_by_rule)
    blocked = args.mode == "block" and len(blocks) > 0

    report = {
        "generated_at": datetime.now(UTC).isoformat(),
        "catalog_path": str(catalog_path).replace("\\", "/"),
        "rules_total": len(typed_rules),
        "warnings_total": len(annotated_warnings),
        "conflicts_total": len(annotated_conflicts),
        "warnings": annotated_warnings,
        "conflicts": annotated_conflicts,
        "mode": args.mode,
        "active_waivers_total": len([w for w in waivers if w.expiry >= date.today()]),
        "expired_waivers_total": len(expired_waivers),
        "expired_waivers": expired_waivers,
        "blocking_findings_total": len(blocks),
        "blocking_findings": blocks,
        "blocked": blocked,
    }

    markdown_out.parent.mkdir(parents=True, exist_ok=True)
    json_out.parent.mkdir(parents=True, exist_ok=True)
    markdown_out.write_text(to_markdown(report), encoding="utf-8")
    json_out.write_text(json.dumps(report, indent=2), encoding="utf-8")

    print(
        "norm-governance report generated:",
        markdown_out.as_posix(),
        json_out.as_posix(),
        f"(mode={args.mode}, blocked={blocked})",
    )
    if blocked:
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
