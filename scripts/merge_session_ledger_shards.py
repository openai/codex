#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import pathlib
from dataclasses import dataclass
from typing import Any


@dataclass
class Hit:
    title: str
    type: str
    statement: str
    action: str
    utility_score: int
    sources: list[tuple[str, int]]
    shards: set[str]


def _read_json(path: pathlib.Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception as e:
        raise RuntimeError(f"failed to read json: {path}") from e


def main() -> int:
    parser = argparse.ArgumentParser(description="history-search の出力JSONをまとめる（決定はしない）")
    parser.add_argument("--in-dir", default=str(pathlib.Path.cwd() / ".memo/history-search/shards"))
    parser.add_argument("--out-json", default=str(pathlib.Path.cwd() / ".memo/history-search/merged.json"))
    parser.add_argument("--out-md", default=str(pathlib.Path.cwd() / ".memo/history-search/merged.md"))
    args = parser.parse_args()

    in_dir = pathlib.Path(args.in_dir).resolve()
    out_json = pathlib.Path(args.out_json).resolve()
    out_md = pathlib.Path(args.out_md).resolve()

    files = sorted(in_dir.glob("*.json"))
    if not files:
        raise RuntimeError(f"no shard json found in: {in_dir}")

    merged: dict[tuple[str, str], Hit] = {}

    for p in files:
        data = _read_json(p)
        scope = data.get("scope") or {}
        shard_id = scope.get("shard_id")
        shard_id = str(shard_id) if isinstance(shard_id, str) else p.stem

        for hit in data.get("top_hits", []):
            knowledge = hit.get("knowledge") or {}
            utility = hit.get("utility") or {}

            title = str(knowledge.get("title") or "").strip()
            action = str(knowledge.get("action") or "").strip()
            if not title:
                continue

            key = (title, action)
            if key not in merged:
                merged[key] = Hit(
                    title=title,
                    type=str(knowledge.get("type") or ""),
                    statement=str(knowledge.get("statement") or ""),
                    action=action,
                    utility_score=int(utility.get("score") or 0),
                    sources=[],
                    shards=set(),
                )

            m = merged[key]
            m.shards.add(shard_id)
            m.utility_score = max(m.utility_score, int(utility.get("score") or 0))

            for ev in utility.get("evidence", []) or []:
                rp = str(ev.get("rollout_path") or "")
                ln = int(ev.get("line_no") or 0)
                if rp and ln > 0:
                    pair = (rp, ln)
                    if pair not in m.sources:
                        m.sources.append(pair)

    hits = sorted(
        merged.values(),
        key=lambda h: (-h.utility_score, -len(h.shards), -len(h.sources), h.title),
    )

    out_json.parent.mkdir(parents=True, exist_ok=True)
    out_md.parent.mkdir(parents=True, exist_ok=True)

    out_json.write_text(
        json.dumps(
            {
                "input_dir": str(in_dir),
                "shards": [p.name for p in files],
                "hits": [
                    {
                        "title": h.title,
                        "type": h.type,
                        "statement": h.statement,
                        "action": h.action,
                        "utility_score_max": h.utility_score,
                        "shards": sorted(h.shards),
                        "evidence": [{"rollout_path": rp, "line_no": ln} for rp, ln in h.sources[:10]],
                    }
                    for h in hits
                ],
            },
            ensure_ascii=False,
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )

    lines: list[str] = []
    lines.append("# セッション履歴ナレッジ台帳（history-search shard出力の統合）")
    lines.append("")
    lines.append(f"- input: `{in_dir}`")
    lines.append(f"- shards: {len(files)}")
    lines.append(f"- hits: {len(hits)}")
    lines.append("")

    for h in hits[:50]:
        lines.append(f"## {h.title}")
        lines.append("")
        lines.append(f"- type: `{h.type}`")
        lines.append(f"- utility_score_max: {h.utility_score}")
        lines.append(f"- shards: {', '.join(sorted(h.shards))}")
        lines.append(f"- statement: {h.statement}")
        lines.append(f"- action: {h.action}")
        lines.append("- evidence:")
        for rp, ln in h.sources[:5]:
            lines.append(f"  - `{rp}:{ln}`")
        lines.append("")

    out_md.write_text("\n".join(lines).rstrip() + "\n", encoding="utf-8")

    print(f"wrote: {out_json}")
    print(f"wrote: {out_md}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
