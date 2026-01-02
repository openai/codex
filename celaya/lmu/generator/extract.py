"""
Organization: Celaya Solutions
Project: LMU Curriculum Runtime
Version: 0.1.0
Generated: 2026-01-02T18:54:00Z
Purpose: Defensive JSON extraction from model outputs
Status: Experimental
"""

import json
import re
from typing import Dict, Any, Optional, Tuple


def strip_markdown_fences(text: str) -> str:
    """
    Remove markdown code fences from text.

    Handles:
    - ```json ... ```
    - ``` ... ```
    - Text before/after fences

    Args:
        text: Raw model output

    Returns:
        Text with fences removed
    """
    # Remove ```json or ``` fences
    text = re.sub(r'^```(?:json)?\s*\n', '', text, flags=re.MULTILINE)
    text = re.sub(r'\n```\s*$', '', text, flags=re.MULTILINE)

    # Also handle inline fences
    text = re.sub(r'```(?:json)?\s*', '', text)

    return text.strip()


def sanitize_json_string(text: str) -> str:
    """
    Sanitize common JSON errors from model output.

    Fixes:
    - Trailing commas in objects/arrays
    - Single quotes to double quotes
    - Unescaped newlines in strings

    Args:
        text: Potentially malformed JSON

    Returns:
        Sanitized JSON string
    """
    # Remove trailing commas before } or ]
    text = re.sub(r',(\s*[}\]])', r'\1', text)

    # Replace single quotes with double quotes (naive, but often works)
    # Note: This is imperfect, but handles common LLM errors
    text = text.replace("'", '"')

    # Remove unescaped newlines in string values (very naive)
    # Better: proper JSON repair library, but this handles 80% of cases
    text = text.replace('\n', '\\n')

    return text


def extract_json_from_text(text: str) -> Tuple[Optional[Dict[str, Any]], Optional[str]]:
    """
    Extract JSON from text that may contain markdown, prose, etc.

    Strategy:
    1. Strip markdown fences
    2. Look for {...} or [...] patterns
    3. Attempt to parse
    4. Sanitize and retry on failure
    5. Return partial data if truncated

    Args:
        text: Raw model output

    Returns:
        Tuple of (parsed_json, error_message)
        - (dict, None) on success
        - (None, error_msg) on failure
        - (partial_dict, error_msg) on partial success
    """
    if not text or not text.strip():
        return None, "Empty input"

    # Step 1: Strip markdown fences
    cleaned = strip_markdown_fences(text)

    # Step 2: Try to find JSON object or array
    json_match = re.search(r'(\{.*\}|\[.*\])', cleaned, re.DOTALL)

    if not json_match:
        return None, "No JSON object or array found in text"

    json_str = json_match.group(1)

    # Step 3: Attempt direct parse
    try:
        parsed = json.loads(json_str)
        return parsed, None
    except json.JSONDecodeError as e:
        # Parse failed, try sanitization
        pass

    # Step 4: Sanitize and retry
    try:
        sanitized = sanitize_json_string(json_str)
        parsed = json.loads(sanitized)
        return parsed, "Parsed after sanitization (model output had errors)"
    except json.JSONDecodeError as e:
        # Still failing, check for truncation
        pass

    # Step 5: Handle truncation - try to extract partial data
    try:
        # Add closing braces if missing
        truncated_fix = json_str
        open_braces = truncated_fix.count('{')
        close_braces = truncated_fix.count('}')

        if open_braces > close_braces:
            truncated_fix += '}' * (open_braces - close_braces)

        open_brackets = truncated_fix.count('[')
        close_brackets = truncated_fix.count(']')

        if open_brackets > close_brackets:
            truncated_fix += ']' * (open_brackets - close_brackets)

        sanitized = sanitize_json_string(truncated_fix)
        parsed = json.loads(sanitized)

        return parsed, f"Partial parse (truncated output, added {open_braces - close_braces} braces)"

    except json.JSONDecodeError as e:
        return None, f"Failed to parse JSON: {str(e)[:100]}"


def extract_json_safely(
    text: str,
    default: Optional[Dict[str, Any]] = None
) -> Dict[str, Any]:
    """
    Extract JSON with fallback to default.

    Never raises exceptions - returns default on any failure.

    Args:
        text: Raw model output
        default: Fallback value (default: empty dict)

    Returns:
        Parsed JSON or default value
    """
    if default is None:
        default = {}

    try:
        parsed, error = extract_json_from_text(text)
        if parsed is not None:
            return parsed
        else:
            return default
    except Exception:
        # Catch-all: never crash
        return default


def validate_json_schema(
    data: Dict[str, Any],
    required_keys: list[str]
) -> Tuple[bool, Optional[str]]:
    """
    Validate that JSON contains required keys.

    Args:
        data: Parsed JSON object
        required_keys: List of required key names

    Returns:
        Tuple of (is_valid, error_message)
    """
    if not isinstance(data, dict):
        return False, "Data is not a JSON object"

    missing_keys = [key for key in required_keys if key not in data]

    if missing_keys:
        return False, f"Missing required keys: {', '.join(missing_keys)}"

    return True, None


# Example usage and testing
if __name__ == "__main__":
    # Test cases for defensive extraction

    # Test 1: Clean JSON
    test1 = '{"key": "value", "number": 42}'
    result1, err1 = extract_json_from_text(test1)
    print(f"Test 1: {result1}, Error: {err1}")

    # Test 2: JSON with markdown fences
    test2 = '''```json
    {
      "key": "value",
      "array": [1, 2, 3]
    }
    ```'''
    result2, err2 = extract_json_from_text(test2)
    print(f"Test 2: {result2}, Error: {err2}")

    # Test 3: JSON with trailing comma
    test3 = '{"key": "value", "number": 42,}'
    result3, err3 = extract_json_from_text(test3)
    print(f"Test 3: {result3}, Error: {err3}")

    # Test 4: Truncated JSON
    test4 = '{"key": "value", "nested": {"incomplete":'
    result4, err4 = extract_json_from_text(test4)
    print(f"Test 4: {result4}, Error: {err4}")

    # Test 5: JSON in prose
    test5 = '''Here is the result:
    {"status": "success", "data": [1, 2, 3]}
    That's the output.'''
    result5, err5 = extract_json_from_text(test5)
    print(f"Test 5: {result5}, Error: {err5}")

    # Test 6: Completely invalid
    test6 = "This is not JSON at all"
    result6, err6 = extract_json_from_text(test6)
    print(f"Test 6: {result6}, Error: {err6}")

    # Test 7: Safe extraction with default
    test7 = "Invalid JSON"
    result7 = extract_json_safely(test7, default={"fallback": True})
    print(f"Test 7 (safe): {result7}")
