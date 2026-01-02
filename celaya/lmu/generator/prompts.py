"""
Organization: Celaya Solutions
Project: LMU Curriculum Runtime
Version: 0.1.0
Generated: 2026-01-02T18:54:00Z
Purpose: All prompt templates for curriculum generation
Status: Experimental
"""

from typing import Dict, List, Any
from datetime import datetime, timezone


def get_header() -> str:
    """Generate Celaya Solutions header for all generated files."""
    return f"""# Organization: Celaya Solutions
# Project: LMU Curriculum Runtime
# Version: 0.1.0
# Generated: {datetime.now(timezone.utc).isoformat()}
# Purpose: {{purpose}}
# Status: Experimental
"""


# Prompt templates - no inline prompts elsewhere
PLAN_LESSON_PROMPT = """You are generating a lesson specification for an LMU (Language Model Unit) curriculum.

LMU treats inference as a programmable execution model, analogous to CUDA kernels on GPUs.

## Lesson Context
- Lesson ID: {lesson_id}
- Lesson Name: {lesson_name}
- Description: {lesson_description}
- Phase: {phase}
- Dependencies: {dependencies}
- CUDA Analogy: {cuda_analogy}

## Your Task
Plan the lesson structure. Generate a JSON object with:
- objective (string): What the learner will accomplish
- constraints (list): Rules that must be followed
- success_criteria (list): Testable conditions for passing
- cuda_analogy_explanation (string): How this lesson maps to CUDA concepts

## Requirements
- No speculative language (no AGI claims, no hype)
- Constraints must be testable
- Success criteria must be measurable
- CUDA analogy must be explicit and accurate

Output JSON only, no markdown fences, no prose.
"""

GENERATE_SPEC_PROMPT = """Generate a complete spec.md file for this LMU lesson.

## Lesson Plan
{lesson_plan}

## Your Task
Write a spec.md file containing:
- Objective (what learner will accomplish)
- Constraints (explicit rules)
- Success Criteria (testable conditions)
- CUDA Analogy (detailed mapping to GPU concepts)

## Format Requirements
- Use markdown format
- Include Celaya Solutions header (provided below)
- Be precise and engineering-focused
- No filler, no motivational language

## Header Template
{header}

Output the complete spec.md content. No JSON, just markdown.
"""

GENERATE_TASKS_PROMPT = """Generate a tasks.json file for this LMU lesson.

## Lesson Plan
{lesson_plan}

## Your Task
Create a JSON object with:
- tasks (array): List of explicit tasks
- Each task has: id, description, weight (float, sum to 1.0)

## Requirements
- Tasks must be specific and actionable
- Weights must sum to exactly 1.0
- At least 3 tasks per lesson
- No vague or unmeasurable tasks

Output JSON only, no markdown fences.
"""

GENERATE_EXPECTED_ARTIFACTS_PROMPT = """Generate expected_artifacts.json for this lesson.

## Lesson Plan
{lesson_plan}

## Standard Artifacts (always required)
- spec.md
- tasks.json
- expected_artifacts.json
- run.sh or run.py
- grader.md
- receipts.jsonl (generated at runtime)
- summary.json (generated at runtime)

## Your Task
Create a JSON object listing all expected artifacts with:
- artifact (string): filename
- required (bool): whether missing fails the lesson
- weight (float): grading weight (sum to 1.0 for required artifacts)

Output JSON only, no markdown fences.
"""

GENERATE_RUNNER_PROMPT = """Generate a run.sh or run.py script for this lesson.

## Lesson Context
- Lesson ID: {lesson_id}
- Tasks: {tasks}

## Your Task
Create a runnable script that:
- Executes the lesson tasks
- Emits receipts to receipts.jsonl
- Generates summary.json on completion
- Uses Ollama for any LLM calls
- Handles errors gracefully (no crashes)

## Requirements
- Single command execution: ./run.sh or python run.py
- Must emit receipts for every operation
- Must generate summary.json
- Must exit with code 0 on success, 1 on failure

Choose bash (run.sh) for simple lessons, Python (run.py) for complex ones.

Output the complete script content. No JSON, no markdown fences.
"""

GENERATE_GRADER_PROMPT = """Generate a grader.md file explaining how this lesson is scored.

## Lesson Plan
{lesson_plan}

## Expected Artifacts
{expected_artifacts}

## Your Task
Write a grader.md file that explains:
- How each artifact is weighted
- What constitutes passing (≥70% score)
- Partial credit rules
- How receipts and summary.json factor in

## Format
Use markdown. Include Celaya Solutions header.

Output the complete grader.md content.
"""

RETRY_PROMPT_TIGHTENED = """RETRY ATTEMPT {attempt}/{max_retries}

Previous attempt failed: {failure_reason}

## Tightened Constraints
- Reduce output length by 30%
- Simplify structure
- Focus on core requirements only
- Avoid optional fields

{original_prompt}

This is attempt {attempt}. Output must be simpler and more focused.
"""

VALIDATE_CUDA_ANALOGY_PROMPT = """Validate that this lesson includes a proper CUDA analogy.

## Lesson Content
{lesson_content}

## Required CUDA Concepts (at least one must be mentioned)
- Kernel, kernel launch
- Warp, thread, grid
- SRAM, HBM, memory hierarchy
- nvprof, profiling
- Stream, concurrent execution

## Your Task
Check if the lesson explicitly maps LMU concepts to CUDA equivalents.

Output JSON:
{{
  "has_cuda_analogy": bool,
  "cuda_concepts_found": [list of concepts mentioned],
  "is_valid": bool
}}

No markdown fences.
"""


def format_prompt(template: str, **kwargs) -> str:
    """
    Format a prompt template with kwargs.

    Args:
        template: Prompt template string
        **kwargs: Values to substitute

    Returns:
        Formatted prompt string
    """
    return template.format(**kwargs)


def get_retry_prompt(
    original_prompt: str,
    attempt: int,
    max_retries: int,
    failure_reason: str
) -> str:
    """
    Generate a retry prompt with tightened constraints.

    Args:
        original_prompt: The original prompt that failed
        attempt: Current attempt number (1-indexed)
        max_retries: Maximum retry attempts
        failure_reason: Why the previous attempt failed

    Returns:
        Modified prompt with tighter constraints
    """
    return format_prompt(
        RETRY_PROMPT_TIGHTENED,
        attempt=attempt,
        max_retries=max_retries,
        failure_reason=failure_reason,
        original_prompt=original_prompt
    )


# Prompt registry for validation
PROMPT_REGISTRY = {
    "plan_lesson": PLAN_LESSON_PROMPT,
    "generate_spec": GENERATE_SPEC_PROMPT,
    "generate_tasks": GENERATE_TASKS_PROMPT,
    "generate_expected_artifacts": GENERATE_EXPECTED_ARTIFACTS_PROMPT,
    "generate_runner": GENERATE_RUNNER_PROMPT,
    "generate_grader": GENERATE_GRADER_PROMPT,
    "validate_cuda_analogy": VALIDATE_CUDA_ANALOGY_PROMPT,
}


def list_available_prompts() -> List[str]:
    """Return list of available prompt template names."""
    return list(PROMPT_REGISTRY.keys())
