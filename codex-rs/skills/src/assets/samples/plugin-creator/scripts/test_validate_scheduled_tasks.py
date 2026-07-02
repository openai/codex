#!/usr/bin/env python3
"""Tests for plugin Scheduled task scaffolding and validation."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from create_basic_plugin import build_plugin_json  # noqa: E402
from validate_plugin import validate_plugin  # noqa: E402
from validate_scheduled_tasks import validate_scheduled_task_templates  # noqa: E402


class ScheduledTaskTemplateValidationTest(unittest.TestCase):
    def test_accepts_every_schedule_shape(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            plugin_root = Path(temp_dir)
            templates = {
                "hourly.json": {
                    "type": "hourly",
                    "intervalHours": 2.0,
                    "days": ["MO", "WE", "FR"],
                },
                "daily.json": {"type": "daily", "time": "07:05"},
                "weekdays.json": {"type": "weekdays", "time": "09:00"},
                "weekly.json": {
                    "type": "weekly",
                    "days": ["TU", "SU"],
                    "time": "23:59",
                },
            }
            for filename, schedule in templates.items():
                self._write_template(plugin_root, filename, schedule=schedule)

            self.assertEqual(validate_scheduled_task_templates(plugin_root), [])

    def test_reports_templates_codex_would_silently_omit(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            plugin_root = Path(temp_dir)
            self._write_template(
                plugin_root,
                "Daily.JSON",
                schedule={"type": "daily", "time": "09:00"},
            )
            self._write_template(
                plugin_root,
                "bad-hourly.json",
                name="",
                prompt=" ",
                extra=True,
                schedule={
                    "type": "hourly",
                    "intervalHours": True,
                    "days": ["MO", "MO"],
                    "time": "09:00",
                },
            )
            broken_path = plugin_root / "scheduled" / "broken.json"
            broken_path.write_text("not-json", encoding="utf-8")

            self.assertEqual(
                validate_scheduled_task_templates(plugin_root),
                [
                    "`scheduled/Daily.JSON` filename must be lowercase kebab-case ending in `.json`",
                    "`scheduled/bad-hourly.json` field `extra` is not supported",
                    "`scheduled/bad-hourly.json` field `name` must be a non-empty string",
                    "`scheduled/bad-hourly.json` field `prompt` must be a non-empty string",
                    "`scheduled/bad-hourly.json` field `schedule.time` is not supported",
                    "`scheduled/bad-hourly.json` field `schedule.intervalHours` must be a positive integer",
                    "`scheduled/bad-hourly.json` field `schedule.days` must contain unique weekdays from `MO` through `SU`",
                    "`scheduled/broken.json` must contain strict standard JSON",
                ],
            )

    def test_plugin_validator_includes_scheduled_task_errors(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            plugin_root = Path(temp_dir) / "reporting"
            manifest_path = plugin_root / ".codex-plugin" / "plugin.json"
            manifest_path.parent.mkdir(parents=True)
            manifest_path.write_text(
                json.dumps(
                    build_plugin_json(
                        "reporting",
                        with_mcp=False,
                        with_apps=False,
                    )
                ),
                encoding="utf-8",
            )
            self._write_template(
                plugin_root,
                "report.json",
                schedule={"type": "daily", "time": "9:00"},
            )

            self.assertEqual(
                validate_plugin(plugin_root),
                [
                    "`scheduled/report.json` field `schedule.time` must use 24-hour `HH:MM` format"
                ],
            )

    def test_scaffold_can_create_an_empty_scheduled_directory(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            script_path = Path(__file__).with_name("create_basic_plugin.py")
            subprocess.run(
                [
                    sys.executable,
                    str(script_path),
                    "reporting",
                    "--path",
                    temp_dir,
                    "--with-scheduled",
                ],
                check=True,
                capture_output=True,
                text=True,
            )

            scheduled_root = Path(temp_dir) / "reporting" / "scheduled"
            self.assertTrue(scheduled_root.is_dir())
            self.assertEqual(list(scheduled_root.iterdir()), [])

    def _write_template(
        self,
        plugin_root: Path,
        filename: str,
        *,
        name: str = "Report",
        prompt: str = "Summarize the queue.",
        schedule: dict[str, object],
        **extra: object,
    ) -> None:
        scheduled_root = plugin_root / "scheduled"
        scheduled_root.mkdir(parents=True, exist_ok=True)
        payload = {
            "name": name,
            "prompt": prompt,
            "schedule": schedule,
            **extra,
        }
        (scheduled_root / filename).write_text(
            json.dumps(payload),
            encoding="utf-8",
        )


if __name__ == "__main__":
    unittest.main()
