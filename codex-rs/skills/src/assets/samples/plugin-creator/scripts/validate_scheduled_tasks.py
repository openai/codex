#!/usr/bin/env python3
"""Validate plugin Scheduled task templates against the Codex JSON contract."""

from __future__ import annotations

import json
import math
import re
from pathlib import Path
from typing import Any


TEMPLATE_FILENAME_RE = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*\.json$")
TIME_RE = re.compile(r"^(?:[01]\d|2[0-3]):[0-5]\d$")
WEEKDAYS = {"MO", "TU", "WE", "TH", "FR", "SA", "SU"}
TEMPLATE_FIELDS = {"name", "prompt", "schedule"}


def validate_scheduled_task_templates(plugin_root: Path) -> list[str]:
    """Return author-facing errors for templates that Codex would silently omit."""
    scheduled_root = plugin_root / "scheduled"
    if not scheduled_root.exists():
        return []
    if not scheduled_root.is_dir():
        return ["`scheduled` must be a directory"]

    errors: list[str] = []
    for template_path in sorted(scheduled_root.iterdir()):
        if not template_path.is_file() or not template_path.name.lower().endswith(
            ".json"
        ):
            continue
        _validate_template_file(plugin_root, template_path, errors)
    return errors


def _validate_template_file(
    plugin_root: Path,
    template_path: Path,
    errors: list[str],
) -> None:
    relative_path = template_path.relative_to(plugin_root).as_posix()
    if TEMPLATE_FILENAME_RE.fullmatch(template_path.name) is None:
        errors.append(
            f"`{relative_path}` filename must be lowercase kebab-case ending in `.json`"
        )

    try:
        payload = json.loads(template_path.read_text(encoding="utf-8"))
    except OSError:
        errors.append(f"unable to read `{relative_path}`")
        return
    except json.JSONDecodeError:
        errors.append(f"`{relative_path}` must contain strict standard JSON")
        return

    if not isinstance(payload, dict):
        errors.append(f"`{relative_path}` must contain a JSON object")
        return

    _reject_unknown_fields(payload, TEMPLATE_FIELDS, relative_path, "$", errors)
    _require_non_empty_string(payload, "name", relative_path, errors)
    _require_non_empty_string(payload, "prompt", relative_path, errors)
    _validate_schedule(payload.get("schedule"), relative_path, errors)


def _validate_schedule(
    schedule: Any,
    relative_path: str,
    errors: list[str],
) -> None:
    if not isinstance(schedule, dict):
        errors.append(f"`{relative_path}` field `schedule` must be an object")
        return

    schedule_type = schedule.get("type")
    if schedule_type == "hourly":
        allowed_fields = {"type", "intervalHours", "days"}
        _reject_unknown_fields(
            schedule, allowed_fields, relative_path, "schedule", errors
        )
        interval = schedule.get("intervalHours")
        if not _is_positive_integer(interval):
            errors.append(
                f"`{relative_path}` field `schedule.intervalHours` must be a positive integer"
            )
        if "days" in schedule:
            _validate_days(schedule["days"], relative_path, errors)
        return

    if schedule_type in {"daily", "weekdays"}:
        _reject_unknown_fields(
            schedule, {"type", "time"}, relative_path, "schedule", errors
        )
        _validate_time(schedule.get("time"), relative_path, errors)
        return

    if schedule_type == "weekly":
        _reject_unknown_fields(
            schedule,
            {"type", "days", "time"},
            relative_path,
            "schedule",
            errors,
        )
        _validate_days(schedule.get("days"), relative_path, errors)
        _validate_time(schedule.get("time"), relative_path, errors)
        return

    errors.append(
        f"`{relative_path}` field `schedule.type` must be `hourly`, `daily`, "
        "`weekdays`, or `weekly`"
    )


def _validate_time(value: Any, relative_path: str, errors: list[str]) -> None:
    if not isinstance(value, str) or TIME_RE.fullmatch(value) is None:
        errors.append(
            f"`{relative_path}` field `schedule.time` must use 24-hour `HH:MM` format"
        )


def _is_positive_integer(value: Any) -> bool:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return False
    return math.isfinite(value) and value > 0 and float(value).is_integer()


def _validate_days(value: Any, relative_path: str, errors: list[str]) -> None:
    if (
        not isinstance(value, list)
        or not value
        or any(not isinstance(day, str) or day not in WEEKDAYS for day in value)
        or len(set(value)) != len(value)
    ):
        errors.append(
            f"`{relative_path}` field `schedule.days` must contain unique weekdays "
            "from `MO` through `SU`"
        )


def _require_non_empty_string(
    payload: dict[str, Any],
    field: str,
    relative_path: str,
    errors: list[str],
) -> None:
    value = payload.get(field)
    if not isinstance(value, str) or not value.strip():
        errors.append(f"`{relative_path}` field `{field}` must be a non-empty string")


def _reject_unknown_fields(
    payload: dict[str, Any],
    allowed_fields: set[str],
    relative_path: str,
    prefix: str,
    errors: list[str],
) -> None:
    for field in sorted(set(payload) - allowed_fields):
        field_path = field if prefix == "$" else f"{prefix}.{field}"
        errors.append(f"`{relative_path}` field `{field_path}` is not supported")
