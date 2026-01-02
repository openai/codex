#!/usr/bin/env python3
"""Test LMU components without Ollama."""

import sys
from pathlib import Path

# Test extract
print("Testing extract.py...")
from celaya.lmu.generator import extract

test_json = '{"key": "value", "number": 42}'
result, err = extract.extract_json_from_text(test_json)
assert result == {"key": "value", "number": 42}, f"Failed: {result}"
print("✓ extract works")

# Test with markdown fences
test_md = '```json\n{"test": true}\n```'
result, err = extract.extract_json_from_text(test_md)
assert result == {"test": True}, f"Failed: {result}"
print("✓ markdown extraction works")

# Test validators
print("\nTesting validators.py...")
from celaya.lmu.generator import validators

valid_plan = {
    "objective": "Test objective",
    "constraints": ["No hype"],
    "success_criteria": ["Test passes"],
    "cuda_analogy_explanation": "Maps to CUDA kernel execution patterns"
}
is_valid, errors = validators.validate_lesson_plan(valid_plan)
assert is_valid, f"Failed: {errors}"
print("✓ validators work")

# Test receipts
print("\nTesting receipts.py...")
from celaya.lmu.runtime.receipts import ReceiptWriter
import tempfile
import os

temp_file = tempfile.mktemp(suffix=".jsonl")
writer = ReceiptWriter(temp_file)
writer.lesson_start("test_lesson")
writer.op_start("test_op", lesson_id="test_lesson")
writer.op_done("test_op", duration_ms=100, lesson_id="test_lesson")

assert Path(temp_file).exists(), "Receipt file not created"
lines = Path(temp_file).read_text().strip().split('\n')
assert len(lines) == 3, f"Expected 3 receipts, got {len(lines)}"
os.unlink(temp_file)
print("✓ receipts work")

# Test grader
print("\nTesting grader.py...")
from celaya.lmu.grading.grader import LessonGrader

grader = LessonGrader()
weights = grader.get_lesson_weights("0.1")
assert "spec.md" in weights, "Missing spec.md weight"
print("✓ grader works")

print("\n" + "=" * 40)
print("All unit tests passed ✓")
print("=" * 40)
