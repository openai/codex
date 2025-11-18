"""Utility for evaluating curriculum drafts and optionally logging MASTER runs.

This module is intentionally lightweight so it can run on mobile Python
launchers like Juno or PyDroid. It performs heuristic scoring of a curriculum
string, writes local ledgers, and—when Firebase credentials are provided—logs
MASTER-level evaluations to Firestore.
"""
from __future__ import annotations

import argparse
import importlib
import json
import statistics
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, Iterable, List


@dataclass
class EvaluationDimension:
    """Represents a scoring dimension for the curriculum."""

    name: str
    description: str
    weight: float


DIMENSIONS: List[EvaluationDimension] = [
    EvaluationDimension("clarity", "Clear learning goals and action verbs", 0.3),
    EvaluationDimension("coverage", "Concrete steps, examples, and checks", 0.3),
    EvaluationDimension("safety", "Avoids harmful instructions and keeps scope narrow", 0.2),
    EvaluationDimension("flow", "Logical ordering with short paragraphs", 0.2),
]


def _detect_keywords(text: str, keywords: Iterable[str]) -> int:
    lowered = text.lower()
    return sum(1 for keyword in keywords if keyword in lowered)


def _score_dimension(text: str, dimension: EvaluationDimension) -> float:
    word_count = len(text.split())
    paragraph_count = max(text.count("\n\n"), 1)
    checklist_hits = _detect_keywords(text, ("task", "check", "verify", "test"))
    example_hits = _detect_keywords(text, ("example", "sample", "snippet", "code"))

    if dimension.name == "clarity":
        base = min(word_count / 120, 1.2)
        clarity_bonus = _detect_keywords(text, ("objective", "goal", "outcome")) * 0.05
        return (base + clarity_bonus) * 100

    if dimension.name == "coverage":
        base = min((checklist_hits + example_hits) / 6, 1.0)
        return (0.6 + base * 0.4) * 100

    if dimension.name == "safety":
        safety_hits = _detect_keywords(text, ("avoid", "do not", "harm", "ethic"))
        return min(0.85 + safety_hits * 0.05, 1.0) * 100

    if dimension.name == "flow":
        base = min(paragraph_count / 4, 1.0)
        return (0.5 + base * 0.5) * 100

    return 70.0


def _normalize_score(raw_score: float) -> float:
    return max(0.0, min(round(raw_score, 1), 100.0))


def evaluate_text(text: str) -> Dict[str, float]:
    """Return weighted dimension scores for the curriculum text."""

    scores = {}
    for dimension in DIMENSIONS:
        scores[dimension.name] = _normalize_score(_score_dimension(text, dimension))
    return scores


def _overall_score(dimension_scores: Dict[str, float]) -> float:
    if not dimension_scores:
        return 0.0
    return _normalize_score(statistics.mean(dimension_scores.values()))


def _credential_level(overall_score: float) -> str:
    if overall_score >= 90:
        return "MASTER"
    if overall_score >= 75:
        return "JOURNEY"
    return "APPRENTICE"


def write_ledger(metadata: dict, ledger_path: str = "metadata/ledger.jsonl") -> None:
    """Append evaluation metadata to a local ledger file."""

    ledger_file = Path(ledger_path)
    ledger_file.parent.mkdir(parents=True, exist_ok=True)
    with ledger_file.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(metadata) + "\n")


def _load_firebase_clients(credentials_path: str):
    spec = importlib.util.find_spec("firebase_admin")
    if spec is None:
        return None

    firebase_admin = importlib.import_module("firebase_admin")
    firestore = importlib.import_module("firebase_admin.firestore")
    credentials = importlib.import_module("firebase_admin.credentials")

    if not firebase_admin._apps:
        cred = credentials.Certificate(credentials_path)
        firebase_admin.initialize_app(cred)
    return firestore.client()


def commit_to_evolution(report: dict, user_id: str = "dev-user", credentials_path: str | None = None) -> bool:
    """Log MASTER credential evaluations to Firestore when configured.

    Returns True if a log was written, False otherwise.
    """

    if report.get("credential_level") != "MASTER":
        return False
    if not credentials_path:
        return False

    firestore_client = _load_firebase_clients(credentials_path)
    if firestore_client is None:
        return False

    firestore_client.collection("evolution_logs").add(
        {
            "score": report.get("overall_score"),
            "dimensions": report.get("dimension_scores"),
            "timestamp": time.time(),
            "uid": user_id,
        }
    )
    return True


def validate(
    curriculum: str,
    *,
    ledger_path: str | None = None,
    log_master: bool = False,
    user_id: str = "dev-user",
    firebase_credentials: str | None = None,
) -> dict:
    """Validate a curriculum string and optionally log results."""

    dimension_scores = evaluate_text(curriculum)
    overall_score = _overall_score(dimension_scores)
    credential_level = _credential_level(overall_score)

    report = {
        "overall_score": overall_score,
        "dimension_scores": dimension_scores,
        "credential_level": credential_level,
        "timestamp": time.time(),
    }

    if ledger_path:
        write_ledger(report, ledger_path=ledger_path)

    if log_master and credential_level == "MASTER":
        commit_to_evolution(report, user_id=user_id, credentials_path=firebase_credentials)

    return report


def main() -> None:
    parser = argparse.ArgumentParser(description="Evaluate curriculum drafts for Stageport evolution workflows.")
    parser.add_argument("path", nargs="?", default="your_curriculum.txt", help="Path to the curriculum file to evaluate.")
    parser.add_argument("--ledger", help="Optional JSONL ledger path for saving evaluation history.")
    parser.add_argument("--log-master", action="store_true", help="Log MASTER-level runs to Firestore when credentials are provided.")
    parser.add_argument("--firebase-credentials", help="Path to a Firebase service account JSON file.")
    parser.add_argument("--user-id", default="dev-user", help="User ID recorded when logging to Firestore.")
    args = parser.parse_args()

    curriculum_path = Path(args.path)
    if not curriculum_path.exists():
        raise FileNotFoundError(f"Curriculum file not found: {curriculum_path}")

    curriculum = curriculum_path.read_text(encoding="utf-8")
    report = validate(
        curriculum,
        ledger_path=args.ledger,
        log_master=args.log_master,
        user_id=args.user_id,
        firebase_credentials=args.firebase_credentials,
    )

    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
