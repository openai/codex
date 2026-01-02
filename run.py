#!/usr/bin/env python3
"""
Organization: Celaya Solutions
Project: LMU Curriculum Runtime
Version: 0.1.0
Generated: 2026-01-02T18:54:00Z
Purpose: Main entry point for LMU curriculum generation and execution
Status: Experimental

Usage:
    python run.py                    # Generate full curriculum
    python run.py --check            # Verify Ollama connectivity
    python run.py --lessons 0.1,0.2  # Generate specific lessons
    python run.py --phase foundations # Generate specific phase
"""

import sys
import argparse
import json
from pathlib import Path
from datetime import datetime, timezone

# Import LMU modules
from celaya.lmu.generator.pipeline import LessonPipeline
from celaya.lmu.runtime.runner import LMURunner, OllamaRunner
from celaya.lmu.runtime.receipts import get_receipt_writer
from celaya.lmu.grading.grader import LessonGrader


def check_ollama_connectivity(ollama_model: str = "llama2:latest") -> bool:
    """Verify Ollama running and accessible."""
    print("Checking Ollama...")

    ollama = OllamaRunner(model=ollama_model)

    if ollama.verify_connectivity():
        print(f"✓ Ollama accessible")
        print(f"  Model: {ollama_model}")
        print(f"  URL: {ollama.base_url}")
        return True
    else:
        print(f"✗ Ollama not accessible: {ollama.base_url}")
        print(f"  Run: ollama serve")
        print(f"  Pull: ollama pull {ollama_model}")
        return False


def generate_curriculum(
    lessons: list[str] | None = None,
    phase: str | None = None,
    max_retries: int = 3,
    ollama_model: str = "llama2:latest"
) -> dict:
    """Generate curriculum lessons."""
    print("=" * 60)
    print("LMU Curriculum Generation")
    print("=" * 60)
    print(f"Started: {datetime.now(timezone.utc).isoformat()}")
    print()

    # Initialize pipeline
    pipeline = LessonPipeline(max_retries=max_retries, ollama_model=ollama_model)

    # Determine lessons
    if phase:
        lessons = [
            lesson["id"]
            for lesson in pipeline.syllabus.get("lessons", [])
            if lesson.get("phase") == phase
        ]
        print(f"Phase: {phase}")
        print(f"Lessons: {', '.join(lessons)}")
    elif lessons:
        print(f"Lessons: {', '.join(lessons)}")
    else:
        print("All lessons")
        lessons = None

    print()

    # Generate
    summary = pipeline.generate_curriculum(lesson_ids=lessons)

    print()
    print("=" * 60)
    print("Complete")
    print("=" * 60)
    print(f"Total: {summary['total_lessons']}")
    print(f"Passed: {summary['lessons_passed']}")
    print(f"Failed: {summary['lessons_failed']}")
    print()

    # Write summary
    summary_file = Path("celaya/lmu/artifacts/generation_summary.json")
    summary_file.parent.mkdir(parents=True, exist_ok=True)

    with open(summary_file, 'w') as f:
        json.dump(summary, f, indent=2)

    print(f"Summary: {summary_file}")

    return summary


def grade_curriculum() -> dict:
    """
    Grade generated curriculum.

    Returns:
        Grading summary dict
    """
    print("=" * 60)
    print("LMU Curriculum Grading")
    print("=" * 60)
    print()

    grader = LessonGrader()

    # Load generated lessons (from artifacts)
    # For build-phase-1, this is placeholder
    lessons = []

    artifacts_dir = Path("celaya/lmu/artifacts/lessons")

    if artifacts_dir.exists():
        for lesson_dir in artifacts_dir.iterdir():
            if lesson_dir.is_dir():
                lesson_id = lesson_dir.name
                summary_file = lesson_dir / "summary.json"

                summary = None
                if summary_file.exists():
                    with open(summary_file, 'r') as f:
                        summary = json.load(f)

                lessons.append({
                    "id": lesson_id,
                    "dir": str(lesson_dir),
                    "summary": summary
                })

    if not lessons:
        print("No lessons found to grade. Generate curriculum first.")
        return {}

    # Grade curriculum
    grading_summary = grader.grade_curriculum(lessons)

    # Print results
    grader.print_summary(grading_summary)

    # Export summary
    grader.export_summary(grading_summary)

    return grading_summary


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(
        description="LMU Curriculum Runtime - Celaya Solutions",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  python run.py                    # Generate full curriculum
  python run.py --check            # Verify Ollama connectivity
  python run.py --lessons 0.1,0.2  # Generate specific lessons
  python run.py --phase foundations # Generate specific phase
  python run.py --grade            # Grade generated curriculum
        """
    )

    parser.add_argument(
        "--check",
        action="store_true",
        help="Verify Ollama connectivity and exit"
    )

    parser.add_argument(
        "--lessons",
        type=str,
        help="Comma-separated lesson IDs to generate (e.g., 0.1,0.2,1.0)"
    )

    parser.add_argument(
        "--phase",
        type=str,
        help="Phase name to generate (e.g., foundations, runtime)"
    )

    parser.add_argument(
        "--grade",
        action="store_true",
        help="Grade generated curriculum"
    )

    parser.add_argument(
        "--model",
        type=str,
        default="llama2:latest",
        help="Ollama model to use (default: llama2:latest)"
    )

    parser.add_argument(
        "--max-retries",
        type=int,
        default=3,
        help="Maximum retry attempts (default: 3)"
    )

    args = parser.parse_args()

    # Check Ollama connectivity
    if args.check:
        success = check_ollama_connectivity(args.model)
        sys.exit(0 if success else 1)

    # Grade curriculum
    if args.grade:
        grading_summary = grade_curriculum()
        sys.exit(0)

    # Parse lesson list
    lesson_list = None
    if args.lessons:
        lesson_list = [l.strip() for l in args.lessons.split(",")]

    # Generate curriculum
    try:
        summary = generate_curriculum(
            lessons=lesson_list,
            phase=args.phase,
            max_retries=args.max_retries,
            ollama_model=args.model
        )

        # Exit
        if summary.get("lessons_failed", 0) > 0:
            print()
            print("⚠ Failures. Check receipts.")
            print(f"  {Path('celaya/lmu/artifacts/receipts.jsonl').absolute()}")
            sys.exit(1)
        else:
            print()
            print("✓ Success")
            sys.exit(0)

    except KeyboardInterrupt:
        print()
        print("Interrupted by user")
        sys.exit(130)

    except Exception as e:
        print()
        print(f"✗ Error: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    main()
