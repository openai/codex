#!/usr/bin/env python3
from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import pathlib
import subprocess
import sys
from dataclasses import dataclass


@dataclass(frozen=True)
class Shard:
    shard_id: str
    since: str
    until: str


def _parse_date(s: str) -> dt.date:
    try:
        return dt.date.fromisoformat(s)
    except Exception as e:
        raise ValueError(f"invalid date: {s} (expected YYYY-MM-DD)") from e


def _fmt(d: dt.date) -> str:
    return d.isoformat()


def _split_into_shards(since: dt.date, until: dt.date, num_shards: int) -> list[Shard]:
    if until < since:
        raise ValueError("until must be >= since")
    total_days = (until - since).days + 1
    step = max(1, total_days // num_shards)

    shards: list[Shard] = []
    start = since
    for i in range(num_shards):
        end = start + dt.timedelta(days=step - 1)
        if i == num_shards - 1 or end > until:
            end = until
        shard_id = f"{_fmt(start)}..{_fmt(end)}"
        shards.append(Shard(shard_id=shard_id, since=_fmt(start), until=_fmt(end)))
        if end == until:
            break
        start = end + dt.timedelta(days=1)
    return shards


def _run_one(
    repo_root: pathlib.Path,
    schema_path: pathlib.Path,
    out_path: pathlib.Path,
    query: str,
    scope: dict,
    model: str | None,
) -> None:
    prompt = "\n".join(
        [
            "$history-search",
            "",
            "query:",
            query.strip(),
            "",
            "scope (JSON):",
            json.dumps(scope, ensure_ascii=False, separators=(",", ":")),
            "",
        ]
    )

    cmd: list[str] = ["codex", "--ask-for-approval", "never"]
    if model:
        cmd += ["--model", model]
    cmd += [
        "exec",
        "--cd",
        str(repo_root),
        "--sandbox",
        "read-only",
        "--output-schema",
        str(schema_path),
        "--output-last-message",
        str(out_path),
        "-",
    ]

    subprocess.run(cmd, input=prompt, text=True, check=True)


def main() -> int:
    parser = argparse.ArgumentParser(description="history-search を shard 分割で実行してJSONを回収する")
    parser.add_argument("--repo-root", default=str(pathlib.Path.cwd()))
    parser.add_argument("--since", required=True)
    parser.add_argument("--until", required=True)
    parser.add_argument("--workers", type=int, default=4, help="shard数")
    parser.add_argument("--out-dir", default=str(pathlib.Path.cwd() / ".memo/history-search/shards"))
    parser.add_argument("--recent-files", type=int, default=300)
    parser.add_argument("--include-archived", action="store_true", default=False)
    parser.add_argument("--max-hits", type=int, default=60)
    parser.add_argument("--expand-window", type=int, default=2)
    parser.add_argument("--model", default=None)
    parser.add_argument("--project-root", default=".", help='scope に入れる project_root（通常 "."）')
    parser.add_argument("--query", required=True)
    args = parser.parse_args()

    repo_root = pathlib.Path(args.repo_root).resolve()
    out_dir = pathlib.Path(args.out_dir).resolve()
    out_dir.mkdir(parents=True, exist_ok=True)

    schema_path = repo_root / ".codex/skills/history-search/references/WORKER_OUTPUT_SCHEMA.json"
    if not schema_path.exists():
        raise RuntimeError(f"missing schema: {schema_path}")

    since = _parse_date(args.since)
    until = _parse_date(args.until)
    shards = _split_into_shards(since, until, args.workers)

    for shard in shards:
        scope = {
            "shard_id": shard.shard_id,
            "since": shard.since,
            "until": shard.until,
            "recent_files": args.recent_files,
            "include_archived": bool(args.include_archived),
            "project_root": str(args.project_root),
            "kinds": ["command", "file", "error", "message", "tool"],
            "max_hits": args.max_hits,
            "expand_window": args.expand_window,
        }
        out_path = out_dir / f"{shard.shard_id}.json"
        _run_one(repo_root, schema_path, out_path, args.query, scope, args.model)
        print(f"wrote: {out_path}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
