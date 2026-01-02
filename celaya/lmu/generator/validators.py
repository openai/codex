"""
Organization: Celaya Solutions
Project: LMU Curriculum Runtime
Version: 0.1.0
Generated: 2026-01-02T18:54:00Z
Purpose: Schema and contract validation for generated artifacts
Status: Experimental
"""

from typing import Dict, Any, List, Tuple, Optional
import re


def validate_lesson_plan(data: Dict[str, Any]) -> Tuple[bool, List[str]]:
    """
    Validate lesson plan JSON structure.

    Required keys:
    - objective (string)
    - constraints (list)
    - success_criteria (list)
    - cuda_analogy_explanation (string)

    Args:
        data: Parsed lesson plan JSON

    Returns:
        Tuple of (is_valid, list_of_errors)
    """
    errors = []

    # Check required keys
    required_keys = [
        "objective",
        "constraints",
        "success_criteria",
        "cuda_analogy_explanation"
    ]

    for key in required_keys:
        if key not in data:
            errors.append(f"Missing required key: {key}")

    # Validate types
    if "objective" in data and not isinstance(data["objective"], str):
        errors.append("'objective' must be a string")

    if "constraints" in data:
        if not isinstance(data["constraints"], list):
            errors.append("'constraints' must be a list")
        elif len(data["constraints"]) == 0:
            errors.append("'constraints' must have at least one item")

    if "success_criteria" in data:
        if not isinstance(data["success_criteria"], list):
            errors.append("'success_criteria' must be a list")
        elif len(data["success_criteria"]) == 0:
            errors.append("'success_criteria' must have at least one item")

    if "cuda_analogy_explanation" in data:
        if not isinstance(data["cuda_analogy_explanation"], str):
            errors.append("'cuda_analogy_explanation' must be a string")
        elif len(data["cuda_analogy_explanation"]) < 20:
            errors.append("'cuda_analogy_explanation' too short (min 20 chars)")

    return len(errors) == 0, errors


def validate_tasks_json(data: Dict[str, Any]) -> Tuple[bool, List[str]]:
    """
    Validate tasks.json structure.

    Required:
    - tasks (array of objects)
    - Each task has: id, description, weight
    - Weights sum to 1.0 (±0.01 tolerance)

    Args:
        data: Parsed tasks JSON

    Returns:
        Tuple of (is_valid, list_of_errors)
    """
    errors = []

    # Check tasks array exists
    if "tasks" not in data:
        errors.append("Missing 'tasks' key")
        return False, errors

    tasks = data["tasks"]

    if not isinstance(tasks, list):
        errors.append("'tasks' must be an array")
        return False, errors

    if len(tasks) < 3:
        errors.append("Must have at least 3 tasks")

    # Validate each task
    total_weight = 0.0

    for i, task in enumerate(tasks):
        if not isinstance(task, dict):
            errors.append(f"Task {i} is not an object")
            continue

        # Check required task keys
        if "id" not in task:
            errors.append(f"Task {i} missing 'id'")
        if "description" not in task:
            errors.append(f"Task {i} missing 'description'")
        if "weight" not in task:
            errors.append(f"Task {i} missing 'weight'")
        else:
            if not isinstance(task["weight"], (int, float)):
                errors.append(f"Task {i} 'weight' must be a number")
            else:
                total_weight += task["weight"]

    # Check weight sum
    if abs(total_weight - 1.0) > 0.01:
        errors.append(f"Task weights must sum to 1.0 (got {total_weight:.3f})")

    return len(errors) == 0, errors


def validate_expected_artifacts(data: Dict[str, Any]) -> Tuple[bool, List[str]]:
    """
    Validate expected_artifacts.json structure.

    Required:
    - artifacts (array of objects)
    - Each artifact has: artifact (string), required (bool), weight (float)
    - Required artifact weights sum to 1.0

    Args:
        data: Parsed expected_artifacts JSON

    Returns:
        Tuple of (is_valid, list_of_errors)
    """
    errors = []

    if "artifacts" not in data:
        errors.append("Missing 'artifacts' key")
        return False, errors

    artifacts = data["artifacts"]

    if not isinstance(artifacts, list):
        errors.append("'artifacts' must be an array")
        return False, errors

    # Standard required artifacts
    required_artifacts = {
        "spec.md",
        "tasks.json",
        "expected_artifacts.json",
        "grader.md",
        "receipts.jsonl",
        "summary.json"
    }

    found_artifacts = set()
    total_required_weight = 0.0

    for i, artifact in enumerate(artifacts):
        if not isinstance(artifact, dict):
            errors.append(f"Artifact {i} is not an object")
            continue

        # Check required keys
        if "artifact" not in artifact:
            errors.append(f"Artifact {i} missing 'artifact' key")
            continue

        artifact_name = artifact["artifact"]
        found_artifacts.add(artifact_name)

        if "required" not in artifact:
            errors.append(f"Artifact '{artifact_name}' missing 'required' key")

        if "weight" not in artifact:
            errors.append(f"Artifact '{artifact_name}' missing 'weight' key")
        else:
            if not isinstance(artifact["weight"], (int, float)):
                errors.append(f"Artifact '{artifact_name}' weight must be a number")
            elif artifact.get("required", False):
                total_required_weight += artifact["weight"]

    # Check for missing standard artifacts
    missing = required_artifacts - found_artifacts
    if missing:
        errors.append(f"Missing standard artifacts: {', '.join(missing)}")

    # Check required weights sum
    if abs(total_required_weight - 1.0) > 0.01:
        errors.append(
            f"Required artifact weights must sum to 1.0 (got {total_required_weight:.3f})"
        )

    return len(errors) == 0, errors


def validate_cuda_analogy(text: str) -> Tuple[bool, List[str]]:
    """
    Validate that text contains proper CUDA analogy.

    Checks for mentions of:
    - Kernel, kernel launch
    - Thread, warp, block, grid
    - Memory (SRAM, HBM, cache)
    - Profiling (nvprof, profiler)
    - Streams, concurrency

    Args:
        text: Lesson content (spec.md or lesson_plan)

    Returns:
        Tuple of (is_valid, list_of_warnings)
    """
    warnings = []

    cuda_keywords = {
        "kernel": r'\bkernel\b',
        "thread": r'\b(thread|warp|block)\b',
        "memory": r'\b(SRAM|HBM|cache|memory hierarchy)\b',
        "profiling": r'\b(nvprof|profil)',
        "stream": r'\b(stream|concurrent)\b',
    }

    found_concepts = []

    for concept, pattern in cuda_keywords.items():
        if re.search(pattern, text, re.IGNORECASE):
            found_concepts.append(concept)

    if len(found_concepts) == 0:
        warnings.append("No CUDA concepts found in text")
        return False, warnings

    if len(found_concepts) < 2:
        warnings.append(
            f"Only found {len(found_concepts)} CUDA concept(s): {', '.join(found_concepts)}. "
            "Recommend at least 2 for clear analogy."
        )

    return True, warnings


def validate_no_hype_language(text: str) -> Tuple[bool, List[str]]:
    """
    Check that text avoids hype and speculative language.

    Forbidden terms:
    - AGI, artificial general intelligence
    - Revolutionary, groundbreaking
    - Game-changing, paradigm shift
    - Unlimited potential

    Args:
        text: Content to validate

    Returns:
        Tuple of (is_valid, list_of_violations)
    """
    violations = []

    forbidden_patterns = {
        "AGI": r'\bAGI\b',
        "artificial general intelligence": r'\bartificial general intelligence\b',
        "revolutionary": r'\brevolutionary\b',
        "groundbreaking": r'\bgroundbreaking\b',
        "game-changing": r'\bgame[- ]changing\b',
        "paradigm shift": r'\bparadigm shift\b',
        "unlimited potential": r'\bunlimited potential\b',
    }

    for term, pattern in forbidden_patterns.items():
        if re.search(pattern, text, re.IGNORECASE):
            violations.append(f"Found forbidden hype term: '{term}'")

    return len(violations) == 0, violations


def validate_all(
    lesson_plan: Optional[Dict[str, Any]] = None,
    tasks: Optional[Dict[str, Any]] = None,
    artifacts: Optional[Dict[str, Any]] = None,
    spec_content: Optional[str] = None
) -> Tuple[bool, Dict[str, List[str]]]:
    """
    Run all validations on generated artifacts.

    Args:
        lesson_plan: Parsed lesson plan JSON
        tasks: Parsed tasks.json
        artifacts: Parsed expected_artifacts.json
        spec_content: spec.md file content

    Returns:
        Tuple of (all_valid, errors_by_type)
    """
    all_errors = {}

    if lesson_plan:
        valid, errors = validate_lesson_plan(lesson_plan)
        if not valid:
            all_errors["lesson_plan"] = errors

    if tasks:
        valid, errors = validate_tasks_json(tasks)
        if not valid:
            all_errors["tasks"] = errors

    if artifacts:
        valid, errors = validate_expected_artifacts(artifacts)
        if not valid:
            all_errors["artifacts"] = errors

    if spec_content:
        valid, errors = validate_cuda_analogy(spec_content)
        if not valid:
            all_errors["cuda_analogy"] = errors

        valid, errors = validate_no_hype_language(spec_content)
        if not valid:
            all_errors["hype_language"] = errors

    return len(all_errors) == 0, all_errors


# Example usage
if __name__ == "__main__":
    # Test lesson plan validation
    valid_plan = {
        "objective": "Understand LMU execution model",
        "constraints": ["No AGI claims", "Explicit success criteria"],
        "success_criteria": ["Map 6 concepts", "Generate valid spec.md"],
        "cuda_analogy_explanation": "LMU operations map to CUDA kernels, with inference calls as kernel launches"
    }

    is_valid, errors = validate_lesson_plan(valid_plan)
    print(f"Lesson plan valid: {is_valid}, Errors: {errors}")

    # Test tasks validation
    valid_tasks = {
        "tasks": [
            {"id": "task1", "description": "Do thing 1", "weight": 0.3},
            {"id": "task2", "description": "Do thing 2", "weight": 0.4},
            {"id": "task3", "description": "Do thing 3", "weight": 0.3}
        ]
    }

    is_valid, errors = validate_tasks_json(valid_tasks)
    print(f"Tasks valid: {is_valid}, Errors: {errors}")

    # Test CUDA analogy
    text_with_cuda = "This lesson maps LMU operations to CUDA kernel execution."
    is_valid, warnings = validate_cuda_analogy(text_with_cuda)
    print(f"CUDA analogy valid: {is_valid}, Warnings: {warnings}")

    # Test hype detection
    text_with_hype = "This revolutionary AGI system will change everything."
    is_valid, violations = validate_no_hype_language(text_with_hype)
    print(f"No hype: {is_valid}, Violations: {violations}")
