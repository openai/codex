#!/usr/bin/env python3
"""Export Codex governance audit logs to CSV for compliance reporting.

Reads JSONL audit logs, validates the SHA-256 chain-of-integrity,
and exports to CSV compatible with SOC 2 / SOX reporting tools.

Usage:
    python audit-export.py audit.jsonl --output report.csv
    python audit-export.py audit.jsonl --validate-only
"""

import argparse
import csv
import hashlib
import json
import sys
from pathlib import Path

GENESIS_SEED = "CODEX_AUDIT_GENESIS"


def compute_genesis_hash() -> str:
    """Compute the well-known genesis hash for the first entry."""
    return hashlib.sha256(GENESIS_SEED.encode()).hexdigest()


def compute_entry_hash(entry: dict) -> str:
    """Compute SHA-256 of an entry excluding the chain_hash field."""
    entry_copy = {k: v for k, v in entry.items() if k != "chain_hash"}
    entry_bytes = json.dumps(entry_copy, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(entry_bytes).hexdigest()


def compute_chain_hash(previous_hash: str, entry: dict) -> str:
    """Compute chain hash: SHA-256(previous_hash + entry_hash)."""
    entry_hash = compute_entry_hash(entry)
    combined = f"{previous_hash}{entry_hash}"
    return hashlib.sha256(combined.encode()).hexdigest()


def validate_chain(entries: list[dict]) -> list[dict]:
    """Validate the hash chain and return entries with validation status."""
    results = []
    previous_hash = compute_genesis_hash()

    for i, entry in enumerate(entries):
        expected_hash = compute_chain_hash(previous_hash, entry)
        actual_hash = entry.get("chain_hash", "")

        is_valid = actual_hash == expected_hash
        results.append(
            {
                "index": i,
                "event_id": entry.get("event_id", "unknown"),
                "timestamp": entry.get("timestamp", ""),
                "valid": is_valid,
                "expected_hash": expected_hash,
                "actual_hash": actual_hash,
            }
        )

        # Use actual hash for next link (even if invalid, to detect
        # where the chain breaks vs. where it was tampered)
        previous_hash = actual_hash if actual_hash else expected_hash

    return results


def load_audit_log(path: Path) -> list[dict]:
    """Load JSONL audit log file."""
    entries = []
    with open(path, encoding="utf-8") as f:
        for line_num, line in enumerate(f, 1):
            line = line.strip()
            if not line:
                continue
            try:
                entries.append(json.loads(line))
            except json.JSONDecodeError as e:
                print(f"Warning: skipping invalid JSON on line {line_num}: {e}", file=sys.stderr)
    return entries


def export_csv(entries: list[dict], output_path: Path) -> None:
    """Export audit entries to CSV for compliance tools."""
    fieldnames = [
        "timestamp",
        "event_id",
        "user",
        "session_id",
        "command",
        "working_directory",
        "policy_files",
        "effective_decision",
        "matched_rule_count",
        "matched_decisions",
        "chain_hash",
        "chain_valid",
    ]

    # Validate chain for integrity column
    chain_results = validate_chain(entries)
    validity_map = {r["index"]: r["valid"] for r in chain_results}

    with open(output_path, "w", newline="", encoding="utf-8") as f:
        writer = csv.DictWriter(f, fieldnames=fieldnames)
        writer.writeheader()

        for i, entry in enumerate(entries):
            matched_rules = entry.get("matched_rules", [])
            writer.writerow(
                {
                    "timestamp": entry.get("timestamp", ""),
                    "event_id": entry.get("event_id", ""),
                    "user": entry.get("user", ""),
                    "session_id": entry.get("session_id", ""),
                    "command": " ".join(entry.get("command", [])),
                    "working_directory": entry.get("working_directory", ""),
                    "policy_files": "; ".join(entry.get("policy_files", [])),
                    "effective_decision": entry.get("effective_decision", ""),
                    "matched_rule_count": len(matched_rules),
                    "matched_decisions": "; ".join(
                        r.get("decision", "") for r in matched_rules
                    ),
                    "chain_hash": entry.get("chain_hash", ""),
                    "chain_valid": validity_map.get(i, False),
                }
            )


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Export Codex governance audit logs to CSV"
    )
    parser.add_argument("input", type=Path, help="Path to JSONL audit log file")
    parser.add_argument(
        "--output",
        "-o",
        type=Path,
        default=None,
        help="Output CSV path (default: <input>.csv)",
    )
    parser.add_argument(
        "--validate-only",
        action="store_true",
        help="Only validate the hash chain, do not export",
    )
    args = parser.parse_args()

    if not args.input.exists():
        print(f"Error: {args.input} not found", file=sys.stderr)
        sys.exit(1)

    entries = load_audit_log(args.input)
    print(f"Loaded {len(entries)} audit entries from {args.input}")

    # Validate chain integrity
    chain_results = validate_chain(entries)
    valid_count = sum(1 for r in chain_results if r["valid"])
    invalid_count = len(chain_results) - valid_count

    print(f"Chain validation: {valid_count} valid, {invalid_count} invalid")

    if invalid_count > 0:
        print("\nTamper-detected entries:", file=sys.stderr)
        for r in chain_results:
            if not r["valid"]:
                print(
                    f"  Entry {r['index']} (event_id={r['event_id']}, "
                    f"timestamp={r['timestamp']}): "
                    f"expected={r['expected_hash'][:16]}..., "
                    f"actual={r['actual_hash'][:16]}...",
                    file=sys.stderr,
                )

    if args.validate_only:
        sys.exit(1 if invalid_count > 0 else 0)

    # Export to CSV
    output_path = args.output or args.input.with_suffix(".csv")
    export_csv(entries, output_path)
    print(f"Exported to {output_path}")

    sys.exit(1 if invalid_count > 0 else 0)


if __name__ == "__main__":
    main()
