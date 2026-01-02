"""
Organization: Celaya Solutions
Project: LMU Curriculum Runtime
Version: 0.1.0
Generated: 2026-01-02T18:54:00Z
Purpose: Lesson generation orchestration and pipeline
Status: Experimental
"""

import yaml
import json
from pathlib import Path
from typing import Dict, Any, List, Optional, Tuple
from datetime import datetime, timezone

# Import sibling modules
from . import prompts
from . import extract
from . import validators

# Import runtime for Ollama
import sys
sys.path.insert(0, str(Path(__file__).parent.parent))
from runtime.runner import OllamaRunner


class LessonPipeline:
    """
    Orchestrates lesson generation from syllabus to artifacts.

    Stages:
    1. Plan: Generate lesson structure
    2. Generate: Create spec.md, tasks.json, etc.
    3. Validate: Check schemas and contracts
    4. Retry: On failure, tighten constraints and retry
    """

    def __init__(
        self,
        syllabus_path: str = "celaya/lmu/syllabus/syllabus.yaml",
        artifact_dir: str = "celaya/lmu/artifacts",
        max_retries: int = 3,
        ollama_model: str = "llama2:latest"
    ):
        """
        Initialize pipeline.

        Args:
            syllabus_path: Path to syllabus.yaml
            artifact_dir: Output directory for artifacts
            max_retries: Maximum retry attempts per operation
            ollama_model: Ollama model name
        """
        self.syllabus_path = Path(syllabus_path)
        self.artifact_dir = Path(artifact_dir)
        self.max_retries = max_retries

        # Load syllabus
        with open(self.syllabus_path, 'r') as f:
            self.syllabus = yaml.safe_load(f)

        # Create artifact directory
        self.artifact_dir.mkdir(parents=True, exist_ok=True)

        # Initialize Ollama
        self.ollama = OllamaRunner(model=ollama_model)

    def load_lesson_config(self, lesson_id: str) -> Optional[Dict[str, Any]]:
        """
        Load lesson configuration from syllabus.

        Args:
            lesson_id: Lesson identifier (e.g., "0.1")

        Returns:
            Lesson config dict or None if not found
        """
        for lesson in self.syllabus.get("lessons", []):
            if lesson.get("id") == lesson_id:
                return lesson
        return None

    def plan_lesson(self, lesson_config: Dict[str, Any]) -> Tuple[Optional[Dict], Optional[str]]:
        """
        Stage 1: Plan lesson structure.

        Args:
            lesson_config: Lesson configuration from syllabus

        Returns:
            Tuple of (lesson_plan, error_message)
        """
        prompt = prompts.format_prompt(
            prompts.PLAN_LESSON_PROMPT,
            lesson_id=lesson_config["id"],
            lesson_name=lesson_config["name"],
            lesson_description=lesson_config["description"],
            phase=lesson_config["phase"],
            dependencies=", ".join(lesson_config.get("dependencies", [])),
            cuda_analogy=lesson_config.get("cuda_analogy", "")
        )

        # Call Ollama
        try:
            response = self.ollama.generate(prompt, max_tokens=500, temperature=0.3)
        except Exception as e:
            return None, f"Ollama call failed: {e}"

        # Extract and validate
        plan, extract_error = extract.extract_json_from_text(response)

        if plan is None:
            return None, f"Failed to extract lesson plan: {extract_error}"

        is_valid, errors = validators.validate_lesson_plan(plan)

        if not is_valid:
            return None, f"Invalid lesson plan: {'; '.join(errors)}"

        return plan, None

    def generate_spec(
        self,
        lesson_plan: Dict[str, Any],
        lesson_config: Dict[str, Any]
    ) -> Tuple[Optional[str], Optional[str]]:
        """
        Stage 2: Generate spec.md content.

        Args:
            lesson_plan: Lesson plan from planning stage
            lesson_config: Lesson config from syllabus

        Returns:
            Tuple of (spec_content, error_message)
        """
        header = prompts.get_header().format(purpose="Lesson specification")

        prompt = prompts.format_prompt(
            prompts.GENERATE_SPEC_PROMPT,
            lesson_plan=json.dumps(lesson_plan, indent=2),
            header=header
        )

        # Call Ollama
        try:
            response = self.ollama.generate(prompt, max_tokens=800, temperature=0.3)
        except Exception as e:
            return None, f"Ollama call failed: {e}"

        # Validate CUDA analogy
        is_valid, warnings = validators.validate_cuda_analogy(response)

        if not is_valid:
            return None, f"Missing CUDA analogy: {'; '.join(warnings)}"

        # Check for hype
        is_valid, violations = validators.validate_no_hype_language(response)

        if not is_valid:
            return None, f"Contains hype language: {'; '.join(violations)}"

        return response, None

    def generate_tasks(
        self,
        lesson_plan: Dict[str, Any]
    ) -> Tuple[Optional[Dict], Optional[str]]:
        """
        Stage 2: Generate tasks.json.

        Args:
            lesson_plan: Lesson plan from planning stage

        Returns:
            Tuple of (tasks_dict, error_message)
        """
        prompt = prompts.format_prompt(
            prompts.GENERATE_TASKS_PROMPT,
            lesson_plan=json.dumps(lesson_plan, indent=2)
        )

        # Call Ollama
        try:
            response = self.ollama.generate(prompt, max_tokens=400, temperature=0.3)
        except Exception as e:
            return None, f"Ollama call failed: {e}"

        tasks, extract_error = extract.extract_json_from_text(response)

        if tasks is None:
            return None, f"Failed to extract tasks: {extract_error}"

        is_valid, errors = validators.validate_tasks_json(tasks)

        if not is_valid:
            return None, f"Invalid tasks.json: {'; '.join(errors)}"

        return tasks, None

    def generate_lesson(self, lesson_id: str) -> Dict[str, Any]:
        """
        Full pipeline: plan → generate → validate.

        Args:
            lesson_id: Lesson identifier

        Returns:
            Result dict with status, artifacts, errors
        """
        result = {
            "lesson_id": lesson_id,
            "status": "pending",
            "artifacts": [],
            "errors": [],
            "timestamp": datetime.now(timezone.utc).isoformat()
        }

        # Load lesson config
        lesson_config = self.load_lesson_config(lesson_id)

        if not lesson_config:
            result["status"] = "failed"
            result["errors"].append(f"Lesson {lesson_id} not found in syllabus")
            return result

        # Stage 1: Plan
        lesson_plan, error = self.plan_lesson(lesson_config)

        if error:
            result["status"] = "failed"
            result["errors"].append(f"Planning failed: {error}")
            return result

        # Stage 2: Generate spec.md
        spec_content, error = self.generate_spec(lesson_plan, lesson_config)

        if error:
            result["status"] = "failed"
            result["errors"].append(f"Spec generation failed: {error}")
            return result

        result["artifacts"].append("spec.md")

        # Stage 2: Generate tasks.json
        tasks, error = self.generate_tasks(lesson_plan)

        if error:
            result["status"] = "failed"
            result["errors"].append(f"Tasks generation failed: {error}")
            return result

        result["artifacts"].append("tasks.json")

        # TODO: Generate other artifacts (expected_artifacts.json, run.sh, grader.md)

        result["status"] = "success"
        return result

    def generate_curriculum(
        self,
        lesson_ids: Optional[List[str]] = None
    ) -> Dict[str, Any]:
        """
        Generate full curriculum or specific lessons.

        Args:
            lesson_ids: List of lesson IDs to generate, or None for all

        Returns:
            Summary dict with results for each lesson
        """
        if lesson_ids is None:
            # Generate all lessons
            lesson_ids = [
                lesson["id"]
                for lesson in self.syllabus.get("lessons", [])
            ]

        summary = {
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "total_lessons": len(lesson_ids),
            "lessons_passed": 0,
            "lessons_failed": 0,
            "results": {}
        }

        for lesson_id in lesson_ids:
            print(f"Generating lesson {lesson_id}...")

            result = self.generate_lesson(lesson_id)

            summary["results"][lesson_id] = result

            if result["status"] == "success":
                summary["lessons_passed"] += 1
            else:
                summary["lessons_failed"] += 1

                # Check fail_fast config
                if not self.syllabus.get("config", {}).get("fail_fast", False):
                    print(f"  Warning: Lesson {lesson_id} failed, continuing...")
                else:
                    print(f"  Error: Lesson {lesson_id} failed, stopping (fail_fast=true)")
                    break

        return summary


# Example usage
if __name__ == "__main__":
    # Initialize pipeline
    pipeline = LessonPipeline()

    # Generate single lesson
    result = pipeline.generate_lesson("0.1")
    print(json.dumps(result, indent=2))

    # Generate full curriculum
    # summary = pipeline.generate_curriculum()
    # print(json.dumps(summary, indent=2))
