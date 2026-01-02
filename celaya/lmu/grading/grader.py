"""
Organization: Celaya Solutions
Project: LMU Curriculum Runtime
Version: 0.1.0
Generated: 2026-01-02T18:54:00Z
Purpose: Partial credit calculation and lesson grading
Status: Experimental
"""

import json
from pathlib import Path
from typing import Dict, Any, List, Optional, Tuple


class LessonGrader:
    """
    Score lessons with partial credit based on artifact weights.

    Grading philosophy:
    - Missing artifacts score 0 (not errors)
    - Partial success preferred over total failure
    - Weights defined per lesson in weights.json
    - Passing threshold: 70% by default
    """

    def __init__(self, weights_file: str = "celaya/lmu/grading/weights.json"):
        """
        Initialize grader.

        Args:
            weights_file: Path to weights.json configuration
        """
        self.weights_file = Path(weights_file)

        with open(self.weights_file, 'r') as f:
            self.weights_config = json.load(f)

        self.passing_threshold = self.weights_config.get("passing_threshold", 0.70)

    def get_lesson_weights(self, lesson_id: str) -> Dict[str, float]:
        """
        Get artifact weights for a specific lesson.

        Args:
            lesson_id: Lesson identifier

        Returns:
            Dict mapping artifact name to weight
        """
        # Check for lesson-specific overrides
        if lesson_id in self.weights_config.get("lesson_overrides", {}):
            return self.weights_config["lesson_overrides"][lesson_id]

        # Use default weights
        return self.weights_config.get("default_weights", {})

    def check_artifact_exists(self, artifact_path: Path) -> bool:
        """
        Check if artifact file exists and is non-empty.

        Args:
            artifact_path: Path to artifact file

        Returns:
            True if exists and non-empty
        """
        if not artifact_path.exists():
            return False

        if artifact_path.stat().st_size == 0:
            return False

        return True

    def grade_lesson(
        self,
        lesson_id: str,
        lesson_dir: str,
        summary: Optional[Dict[str, Any]] = None
    ) -> Dict[str, Any]:
        """
        Grade a lesson based on artifacts and summary.

        Args:
            lesson_id: Lesson identifier
            lesson_dir: Directory containing lesson artifacts
            summary: Optional summary.json data

        Returns:
            Grading result dict with score, breakdown, pass/fail
        """
        lesson_path = Path(lesson_dir)
        weights = self.get_lesson_weights(lesson_id)

        result = {
            "lesson_id": lesson_id,
            "score": 0.0,
            "weighted_score": 0.0,
            "passing_threshold": self.passing_threshold,
            "passed": False,
            "artifacts_found": [],
            "artifacts_missing": [],
            "breakdown": {},
            "bonus_points": 0.0
        }

        total_weight = 0.0

        # Grade each expected artifact
        for artifact_name, weight in weights.items():
            # Handle run.sh_or_run.py special case
            if artifact_name == "run.sh_or_run.py":
                artifact_exists = (
                    self.check_artifact_exists(lesson_path / "run.sh") or
                    self.check_artifact_exists(lesson_path / "run.py")
                )
                actual_artifact = "run.sh" if (lesson_path / "run.sh").exists() else "run.py"
            else:
                artifact_exists = self.check_artifact_exists(lesson_path / artifact_name)
                actual_artifact = artifact_name

            if artifact_exists:
                result["artifacts_found"].append(actual_artifact)
                result["breakdown"][artifact_name] = {
                    "weight": weight,
                    "earned": weight,
                    "status": "present"
                }
                result["weighted_score"] += weight
            else:
                result["artifacts_missing"].append(artifact_name)
                result["breakdown"][artifact_name] = {
                    "weight": weight,
                    "earned": 0.0,
                    "status": "missing"
                }

            total_weight += weight

        # Normalize score to 0-1 range
        if total_weight > 0:
            result["score"] = result["weighted_score"] / total_weight
        else:
            result["score"] = 0.0

        # Apply bonus criteria
        bonus_config = self.weights_config.get("bonus_criteria", {})

        # Bonus: All artifacts present
        if len(result["artifacts_missing"]) == 0:
            bonus = bonus_config.get("all_artifacts_present", 0.0)
            result["bonus_points"] += bonus
            result["score"] += bonus

        # Bonus: No retries (from summary)
        if summary and summary.get("retry_count", 0) == 0:
            bonus = bonus_config.get("no_retries_needed", 0.0)
            result["bonus_points"] += bonus
            result["score"] += bonus

        # Bonus: Fast execution (from summary)
        if summary:
            estimated_time = summary.get("estimated_duration_ms", float('inf'))
            actual_time = summary.get("actual_duration_ms", 0)

            if actual_time < estimated_time:
                bonus = bonus_config.get("execution_under_estimated_time", 0.0)
                result["bonus_points"] += bonus
                result["score"] += bonus

        # Cap score at 1.0
        result["score"] = min(result["score"], 1.0)

        # Determine pass/fail
        result["passed"] = result["score"] >= self.passing_threshold

        return result

    def grade_curriculum(
        self,
        lessons: List[Dict[str, Any]]
    ) -> Dict[str, Any]:
        """
        Grade entire curriculum.

        Args:
            lessons: List of lesson dicts with id, dir, summary

        Returns:
            Curriculum grading summary
        """
        summary = {
            "total_lessons": len(lessons),
            "lessons_passed": 0,
            "lessons_failed": 0,
            "overall_score": 0.0,
            "pass_rate": 0.0,
            "results": {}
        }

        total_score = 0.0

        for lesson in lessons:
            lesson_id = lesson["id"]
            lesson_dir = lesson["dir"]
            lesson_summary = lesson.get("summary")

            result = self.grade_lesson(lesson_id, lesson_dir, lesson_summary)

            summary["results"][lesson_id] = result
            total_score += result["score"]

            if result["passed"]:
                summary["lessons_passed"] += 1
            else:
                summary["lessons_failed"] += 1

        # Calculate overall metrics
        if len(lessons) > 0:
            summary["overall_score"] = total_score / len(lessons)
            summary["pass_rate"] = summary["lessons_passed"] / len(lessons)

        return summary

    def export_summary(
        self,
        grading_summary: Dict[str, Any],
        output_file: str = "celaya/lmu/artifacts/generation_summary.json"
    ) -> None:
        """
        Export grading summary to JSON file.

        Args:
            grading_summary: Grading summary dict
            output_file: Output file path
        """
        output_path = Path(output_file)
        output_path.parent.mkdir(parents=True, exist_ok=True)

        with open(output_path, 'w') as f:
            json.dumps(grading_summary, indent=2)

    def print_summary(self, grading_summary: Dict[str, Any]) -> None:
        """
        Print grading summary to console.

        Args:
            grading_summary: Grading summary dict
        """
        print("=" * 60)
        print("LMU Curriculum Grading Summary")
        print("=" * 60)
        print(f"Total Lessons: {grading_summary['total_lessons']}")
        print(f"Passed: {grading_summary['lessons_passed']}")
        print(f"Failed: {grading_summary['lessons_failed']}")
        print(f"Pass Rate: {grading_summary['pass_rate']:.1%}")
        print(f"Overall Score: {grading_summary['overall_score']:.1%}")
        print()

        print("Per-Lesson Results:")
        print("-" * 60)

        for lesson_id, result in grading_summary["results"].items():
            status = "✓ PASS" if result["passed"] else "✗ FAIL"
            print(f"{lesson_id}: {status} ({result['score']:.1%})")

            print(f"  Artifacts found: {len(result['artifacts_found'])}/{len(result['breakdown'])}")

            if result["artifacts_missing"]:
                print(f"  Missing: {', '.join(result['artifacts_missing'])}")

            if result["bonus_points"] > 0:
                print(f"  Bonus: +{result['bonus_points']:.1%}")

            print()

        print("=" * 60)


# Example usage
if __name__ == "__main__":
    # Initialize grader
    grader = LessonGrader()

    # Grade single lesson (example)
    # result = grader.grade_lesson(
    #     lesson_id="0.1",
    #     lesson_dir="celaya/lmu/artifacts/lessons/0.1",
    #     summary={"retry_count": 0, "actual_duration_ms": 1500}
    # )
    # print(json.dumps(result, indent=2))

    # Example curriculum grading
    lessons = [
        {
            "id": "0.1",
            "dir": "celaya/lmu/artifacts/lessons/0.1",
            "summary": {"retry_count": 0, "actual_duration_ms": 1800}
        },
        {
            "id": "0.2",
            "dir": "celaya/lmu/artifacts/lessons/0.2",
            "summary": {"retry_count": 1, "actual_duration_ms": 3200}
        }
    ]

    # curriculum_summary = grader.grade_curriculum(lessons)
    # grader.print_summary(curriculum_summary)

    print("Grader module loaded successfully")
