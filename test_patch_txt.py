#!/usr/bin/env python3
"""
Test script demonstrating patch_txt usage for book editing.
"""

import json
import subprocess
import sys
from pathlib import Path


def run_patch(params, dry_run=True):
    """Run patch_txt with given parameters."""
    params['dry_run'] = dry_run

    cmd = [
        sys.executable,
        'patch_txt.py',
        params['file'],
        '--json', json.dumps(params),
        '--output-json'
    ]

    result = subprocess.run(cmd, capture_output=True, text=True)

    try:
        output = json.loads(result.stdout)
        return output
    except json.JSONDecodeError:
        print(f"Error: {result.stdout}")
        print(f"Stderr: {result.stderr}")
        return None


def demo_search_replace():
    """Demonstrate search and replace."""
    print("=" * 60)
    print("DEMO 1: Search and Replace")
    print("=" * 60)
    print("Task: Change 'protagonist walks' to 'protagonist runs'\n")

    params = {
        'file': 'example_book.md',
        'mode': 'search_replace',
        'search': 'protagonist walks',
        'replace': 'protagonist runs'
    }

    result = run_patch(params, dry_run=True)
    if result and result['success']:
        print(f"✓ {result['message']}")
        print("\nPreview:")
        print(result['preview'])
    print()


def demo_line_range():
    """Demonstrate line range replacement."""
    print("=" * 60)
    print("DEMO 2: Line Range Replacement")
    print("=" * 60)
    print("Task: Replace lines 5-6 with new content\n")

    params = {
        'file': 'example_book.md',
        'mode': 'line_range',
        'start': 5,
        'end': 6,
        'content': 'She had spent years preparing for this moment. Every decision had led her here.\n\nNow it was time to act.'
    }

    result = run_patch(params, dry_run=True)
    if result and result['success']:
        print(f"✓ {result['message']}")
        print("\nPreview:")
        print(result['preview'])
    print()


def demo_insert_after():
    """Demonstrate insert after marker."""
    print("=" * 60)
    print("DEMO 3: Insert After Marker")
    print("=" * 60)
    print("Task: Add an epigraph after Chapter 1 heading\n")

    params = {
        'file': 'example_book.md',
        'mode': 'insert_after',
        'marker': '## Chapter 1: A New Dawn',
        'content': '\n> *"Every journey begins with a single step."* - Ancient Proverb\n',
        'first_only': True
    }

    result = run_patch(params, dry_run=True)
    if result and result['success']:
        print(f"✓ {result['message']}")
        print("\nPreview:")
        print(result['preview'])
    print()


def demo_append():
    """Demonstrate append to file."""
    print("=" * 60)
    print("DEMO 4: Append New Chapter")
    print("=" * 60)
    print("Task: Add a new chapter at the end\n")

    params = {
        'file': 'example_book.md',
        'mode': 'append',
        'content': '\n## Chapter 4: Resolution\n\nThe story comes to a close. Our hero has learned valuable lessons.\n\nThe end... or is it just the beginning?\n'
    }

    result = run_patch(params, dry_run=True)
    if result and result['success']:
        print(f"✓ {result['message']}")
        print("\nPreview (last 15 lines):")
        preview_lines = result['preview'].split('\n')
        print('\n'.join(preview_lines[-15:]))
    print()


def demo_prepend():
    """Demonstrate prepend to file."""
    print("=" * 60)
    print("DEMO 5: Prepend Frontmatter")
    print("=" * 60)
    print("Task: Add YAML frontmatter at the beginning\n")

    params = {
        'file': 'example_book.md',
        'mode': 'prepend',
        'content': '---\ntitle: The Adventure Begins\nauthor: Jane Doe\ndate: 2025-01-15\ngenre: Fantasy\n---\n\n'
    }

    result = run_patch(params, dry_run=True)
    if result and result['success']:
        print(f"✓ {result['message']}")
        print("\nPreview (first 20 lines):")
        preview_lines = result['preview'].split('\n')
        print('\n'.join(preview_lines[:20]))
    print()


def demo_insert_before():
    """Demonstrate insert before marker."""
    print("=" * 60)
    print("DEMO 6: Insert Scene Break")
    print("=" * 60)
    print("Task: Insert a scene break before Chapter 3\n")

    params = {
        'file': 'example_book.md',
        'mode': 'insert_before',
        'marker': '## Chapter 3: The Journey',
        'content': '\n* * *\n\n',
        'first_only': True
    }

    result = run_patch(params, dry_run=True)
    if result and result['success']:
        print(f"✓ {result['message']}")
        print("\nPreview:")
        print(result['preview'])
    print()


def demo_multiple_edits():
    """Demonstrate applying multiple edits in sequence."""
    print("=" * 60)
    print("DEMO 7: Multiple Edits (Simulated)")
    print("=" * 60)
    print("Task: Apply multiple changes to the book\n")

    edits = [
        {
            'description': '1. Fix typo: "protagonist walks" → "protagonist strides"',
            'params': {
                'file': 'example_book.md',
                'mode': 'search_replace',
                'search': 'protagonist walks',
                'replace': 'protagonist strides'
            }
        },
        {
            'description': '2. Add author note after Chapter 2',
            'params': {
                'file': 'example_book.md',
                'mode': 'insert_after',
                'marker': '## Chapter 2: Discovery',
                'content': '\n*Author\'s Note: This chapter was inspired by real events.*\n'
            }
        },
        {
            'description': '3. Add copyright footer',
            'params': {
                'file': 'example_book.md',
                'mode': 'append',
                'content': '\n\n---\n\n© 2025 Jane Doe. All rights reserved.\n'
            }
        }
    ]

    for edit in edits:
        print(f"\n{edit['description']}")
        result = run_patch(edit['params'], dry_run=True)
        if result and result['success']:
            print(f"  ✓ {result['message']}")
        else:
            print(f"  ✗ Failed: {result.get('message', 'Unknown error')}")

    print()


def demo_json_workflow():
    """Demonstrate JSON-based workflow."""
    print("=" * 60)
    print("DEMO 8: JSON Workflow for LLMs")
    print("=" * 60)
    print("Task: Show how an LLM would use the tool via JSON\n")

    # Example JSON that an LLM might generate
    llm_commands = [
        {
            "operation": "fix_typo",
            "file": "example_book.md",
            "mode": "search_replace",
            "search": "protagonist walks",
            "replace": "protagonist strides",
            "reason": "More dynamic verb choice"
        },
        {
            "operation": "enhance_description",
            "file": "example_book.md",
            "mode": "line_range",
            "start": 7,
            "end": 7,
            "content": "The old bookstore on the corner, with its weathered sign and creaking door, had always been her sanctuary.",
            "reason": "Add sensory details"
        }
    ]

    print("LLM-generated editing commands:")
    print(json.dumps(llm_commands, indent=2))
    print("\nExecuting commands...")

    for i, cmd in enumerate(llm_commands, 1):
        print(f"\n{i}. {cmd['operation']}: {cmd.get('reason', '')}")
        result = run_patch(cmd, dry_run=True)
        if result and result['success']:
            print(f"   ✓ {result['message']}")
        else:
            print(f"   ✗ Failed")

    print()


def main():
    """Run all demonstrations."""
    print("\n")
    print("╔" + "=" * 58 + "╗")
    print("║" + " " * 58 + "║")
    print("║" + "  PATCH_TXT DEMONSTRATION - LLM Book Editing Tool  ".center(58) + "║")
    print("║" + " " * 58 + "║")
    print("╚" + "=" * 58 + "╝")
    print("\n")
    print("This demo shows how an LLM can use patch_txt to edit books.")
    print("All operations run in DRY RUN mode (preview only).\n")

    # Check if example file exists
    if not Path('example_book.md').exists():
        print("Error: example_book.md not found!")
        print("Please ensure the example file exists in the current directory.")
        return 1

    # Run demonstrations
    demo_search_replace()
    demo_line_range()
    demo_insert_after()
    demo_append()
    demo_prepend()
    demo_insert_before()
    demo_multiple_edits()
    demo_json_workflow()

    print("=" * 60)
    print("DEMONSTRATION COMPLETE")
    print("=" * 60)
    print("\nTo apply changes for real, set dry_run=False or omit --dry-run")
    print("To see the tool schema for LLM integration, check patch_txt_tool.json")
    print("\nFor more examples, see PATCH_TXT_README.md")
    print()

    return 0


if __name__ == '__main__':
    sys.exit(main())
