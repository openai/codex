"""
Organization: Celaya Solutions
Project: LMU Curriculum Runtime
Version: 0.1.0
Generated: 2026-01-02T19:20:00Z
Purpose: Mock Ollama for testing without server
Status: Experimental
"""

import json


class MockOllamaRunner:
    """Mock Ollama for testing."""

    def __init__(self, model: str = "mock", base_url: str = "mock"):
        self.model = model
        self.base_url = base_url

    def generate(self, prompt: str, max_tokens: int = 1000, temperature: float = 0.7) -> str:
        """Return mock JSON responses based on prompt."""

        prompt_lower = prompt.lower()

        # Plan lesson
        if "plan" in prompt_lower and "lesson" in prompt_lower:
            return json.dumps({
                "objective": "Map LMU concepts to CUDA equivalents",
                "constraints": ["No speculative claims", "Testable criteria only"],
                "success_criteria": ["Map 6 concepts correctly", "Generate valid spec.md"],
                "cuda_analogy_explanation": "LMU operations map to CUDA kernel launches with deterministic execution"
            })

        # Generate tasks.json (check before spec)
        elif "tasks.json" in prompt_lower or "tasks" in prompt_lower:
            return json.dumps({
                "tasks": [
                    {"id": "task1", "description": "Map LMU to CUDA concepts", "weight": 0.4},
                    {"id": "task2", "description": "Generate spec.md with analogy", "weight": 0.35},
                    {"id": "task3", "description": "Validate output against schema", "weight": 0.25}
                ]
            })

        # Generate spec.md
        elif "spec.md" in prompt_lower or "spec" in prompt_lower:
            return """# Organization: Celaya Solutions
# Lesson 0.1: Introduction to LMU

## Objective
Understand LMU execution model through CUDA kernel analogy.

## Constraints
- No speculative language
- Testable success criteria only
- Explicit CUDA mapping required

## Success Criteria
- Map 6 LMU/CUDA concept pairs
- Generate valid spec.md with kernel analogy
- Pass validation

## CUDA Analogy
LMU operations are analogous to CUDA kernel execution:
- LMU operation = CUDA kernel
- LMU runner = kernel launch
- LMU lane = warp
- KV cache = SRAM/HBM memory hierarchy
"""

        # Default
        else:
            return json.dumps({
                "response": "Mock Ollama response",
                "model": self.model
            })

    def verify_connectivity(self) -> bool:
        """Always return True for mock."""
        return True
