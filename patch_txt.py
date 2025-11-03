#!/usr/bin/env python3
"""
patch_txt - A tool for LLMs to edit and patch text and markdown files.

This tool supports multiple patch formats optimized for prose editing:
1. Search/Replace: Simple text replacement
2. Unified Diff: Standard diff format
3. Line Range: Replace specific line ranges
4. Append/Prepend: Add content to beginning or end
"""

import argparse
import difflib
import json
import os
import sys
from dataclasses import dataclass
from enum import Enum
from pathlib import Path
from typing import List, Optional, Tuple


class PatchMode(Enum):
    """Supported patch modes."""
    SEARCH_REPLACE = "search_replace"
    UNIFIED_DIFF = "unified_diff"
    LINE_RANGE = "line_range"
    APPEND = "append"
    PREPEND = "prepend"
    INSERT_AFTER = "insert_after"
    INSERT_BEFORE = "insert_before"


@dataclass
class PatchResult:
    """Result of applying a patch."""
    success: bool
    message: str
    lines_changed: int = 0
    preview: Optional[str] = None


class PatchTxtTool:
    """Main tool for applying patches to text files."""

    def __init__(self, file_path: str, create_backup: bool = True, dry_run: bool = False):
        """
        Initialize the patch tool.

        Args:
            file_path: Path to the file to patch
            create_backup: Whether to create a .bak backup before modifying
            dry_run: If True, show what would change without applying
        """
        self.file_path = Path(file_path)
        self.create_backup = create_backup
        self.dry_run = dry_run

        # Validate file
        if not self.file_path.exists():
            # Allow creating new files
            self.content = ""
            self.lines = []
        else:
            if self.file_path.suffix not in ['.md', '.txt']:
                raise ValueError(f"File must be .md or .txt, got: {self.file_path.suffix}")

            with open(self.file_path, 'r', encoding='utf-8') as f:
                self.content = f.read()
            self.lines = self.content.splitlines(keepends=True)

    def apply_search_replace(self, search: str, replace: str,
                            count: int = -1, case_sensitive: bool = True) -> PatchResult:
        """
        Apply a simple search and replace operation.

        Args:
            search: Text to search for
            replace: Text to replace with
            count: Maximum number of replacements (-1 for all)
            case_sensitive: Whether search is case-sensitive

        Returns:
            PatchResult with operation details
        """
        if not search:
            return PatchResult(False, "Search string cannot be empty")

        original_content = self.content

        # Perform replacement
        if case_sensitive:
            new_content = original_content.replace(search, replace, count)
        else:
            # Case-insensitive replacement
            import re
            pattern = re.compile(re.escape(search), re.IGNORECASE)
            new_content = pattern.sub(replace, original_content, count=count)

        if original_content == new_content:
            return PatchResult(False, "Search string not found in file")

        # Count occurrences
        occurrences = original_content.count(search) if case_sensitive else \
                     len([m for m in re.finditer(re.escape(search), original_content, re.IGNORECASE)])
        replaced = min(occurrences, count) if count > 0 else occurrences

        # Generate preview
        preview = self._generate_diff_preview(original_content, new_content)

        if not self.dry_run:
            self._write_file(new_content)

        status = "Would replace" if self.dry_run else "Replaced"
        return PatchResult(
            True,
            f"{status} {replaced} occurrence(s) of search string",
            lines_changed=replaced,
            preview=preview
        )

    def apply_unified_diff(self, diff_text: str) -> PatchResult:
        """
        Apply a unified diff patch.

        Args:
            diff_text: Unified diff format text

        Returns:
            PatchResult with operation details
        """
        try:
            # Parse and apply the diff
            import patch_ng
            pset = patch_ng.fromstring(diff_text.encode())

            if not pset:
                return PatchResult(False, "Invalid or empty diff")

            # Apply the patch
            if not self.dry_run:
                result = pset.apply(strip=0, root=str(self.file_path.parent))
                if result:
                    # Re-read the file
                    with open(self.file_path, 'r', encoding='utf-8') as f:
                        self.content = f.read()
                    return PatchResult(True, "Unified diff applied successfully")
                else:
                    return PatchResult(False, "Failed to apply unified diff")
            else:
                return PatchResult(True, "Dry run: unified diff would be applied",
                                 preview=diff_text)
        except ImportError:
            # Fallback: manual diff parsing
            return self._apply_unified_diff_manual(diff_text)
        except Exception as e:
            return PatchResult(False, f"Error applying unified diff: {str(e)}")

    def _apply_unified_diff_manual(self, diff_text: str) -> PatchResult:
        """Manual unified diff parsing (fallback)."""
        lines = diff_text.splitlines()
        hunks = []
        current_hunk = []

        for line in lines:
            if line.startswith('@@'):
                if current_hunk:
                    hunks.append(current_hunk)
                current_hunk = [line]
            elif line.startswith((' ', '+', '-')) and current_hunk:
                current_hunk.append(line)

        if current_hunk:
            hunks.append(current_hunk)

        if not hunks:
            return PatchResult(False, "No valid hunks found in diff")

        # Apply hunks
        new_lines = self.lines.copy()
        offset = 0

        for hunk in hunks:
            # Parse hunk header
            header = hunk[0]
            import re
            match = re.search(r'@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@', header)
            if not match:
                continue

            old_start = int(match.group(1)) - 1
            hunk_lines = hunk[1:]

            # Apply hunk
            i = old_start + offset
            for hunk_line in hunk_lines:
                if hunk_line.startswith('-'):
                    # Remove line
                    if i < len(new_lines):
                        new_lines.pop(i)
                        offset -= 1
                elif hunk_line.startswith('+'):
                    # Add line
                    content = hunk_line[1:] + '\n'
                    new_lines.insert(i, content)
                    i += 1
                    offset += 1
                else:
                    # Context line
                    i += 1

        new_content = ''.join(new_lines)
        preview = self._generate_diff_preview(self.content, new_content)

        if not self.dry_run:
            self._write_file(new_content)

        status = "Would apply" if self.dry_run else "Applied"
        return PatchResult(True, f"{status} unified diff", preview=preview)

    def apply_line_range(self, start_line: int, end_line: int,
                        new_content: str) -> PatchResult:
        """
        Replace a range of lines with new content.

        Args:
            start_line: Starting line number (1-indexed)
            end_line: Ending line number (1-indexed, inclusive)
            new_content: New content to insert

        Returns:
            PatchResult with operation details
        """
        if start_line < 1:
            return PatchResult(False, "Line numbers must start at 1")

        if end_line < start_line:
            return PatchResult(False, "End line must be >= start line")

        # Convert to 0-indexed
        start_idx = start_line - 1
        end_idx = end_line

        if start_idx >= len(self.lines):
            return PatchResult(False, f"Start line {start_line} exceeds file length")

        original_content = self.content
        new_lines = self.lines.copy()

        # Ensure new_content ends with newline if it's not empty
        replacement_lines = new_content.splitlines(keepends=True)
        if replacement_lines and not replacement_lines[-1].endswith('\n'):
            replacement_lines[-1] += '\n'

        # Replace the range
        new_lines[start_idx:end_idx] = replacement_lines
        new_content_str = ''.join(new_lines)

        preview = self._generate_diff_preview(original_content, new_content_str)
        lines_changed = end_line - start_line + 1

        if not self.dry_run:
            self._write_file(new_content_str)

        status = "Would replace" if self.dry_run else "Replaced"
        return PatchResult(
            True,
            f"{status} lines {start_line}-{end_line}",
            lines_changed=lines_changed,
            preview=preview
        )

    def apply_append(self, content: str) -> PatchResult:
        """Append content to the end of the file."""
        original_content = self.content

        # Ensure content starts on new line if file isn't empty
        if original_content and not original_content.endswith('\n'):
            new_content = original_content + '\n' + content
        else:
            new_content = original_content + content

        preview = self._generate_diff_preview(original_content, new_content)

        if not self.dry_run:
            self._write_file(new_content)

        status = "Would append" if self.dry_run else "Appended"
        return PatchResult(True, f"{status} content to end of file", preview=preview)

    def apply_prepend(self, content: str) -> PatchResult:
        """Prepend content to the beginning of the file."""
        original_content = self.content

        # Ensure content ends with newline if file isn't empty
        if original_content and not content.endswith('\n'):
            new_content = content + '\n' + original_content
        else:
            new_content = content + original_content

        preview = self._generate_diff_preview(original_content, new_content)

        if not self.dry_run:
            self._write_file(new_content)

        status = "Would prepend" if self.dry_run else "Prepended"
        return PatchResult(True, f"{status} content to beginning of file", preview=preview)

    def apply_insert_after(self, marker: str, content: str,
                          first_occurrence: bool = True) -> PatchResult:
        """
        Insert content after a marker string.

        Args:
            marker: Text to search for
            content: Content to insert after marker
            first_occurrence: If True, insert after first match only

        Returns:
            PatchResult with operation details
        """
        original_content = self.content

        if marker not in original_content:
            return PatchResult(False, f"Marker not found: {marker}")

        if first_occurrence:
            # Insert after first occurrence
            parts = original_content.split(marker, 1)
            new_content = parts[0] + marker + '\n' + content + parts[1]
        else:
            # Insert after all occurrences
            new_content = original_content.replace(marker, marker + '\n' + content)

        preview = self._generate_diff_preview(original_content, new_content)

        if not self.dry_run:
            self._write_file(new_content)

        status = "Would insert" if self.dry_run else "Inserted"
        return PatchResult(True, f"{status} content after marker", preview=preview)

    def apply_insert_before(self, marker: str, content: str,
                           first_occurrence: bool = True) -> PatchResult:
        """
        Insert content before a marker string.

        Args:
            marker: Text to search for
            content: Content to insert before marker
            first_occurrence: If True, insert before first match only

        Returns:
            PatchResult with operation details
        """
        original_content = self.content

        if marker not in original_content:
            return PatchResult(False, f"Marker not found: {marker}")

        if first_occurrence:
            # Insert before first occurrence
            parts = original_content.split(marker, 1)
            new_content = parts[0] + content + '\n' + marker + parts[1]
        else:
            # Insert before all occurrences
            new_content = original_content.replace(marker, content + '\n' + marker)

        preview = self._generate_diff_preview(original_content, new_content)

        if not self.dry_run:
            self._write_file(new_content)

        status = "Would insert" if self.dry_run else "Inserted"
        return PatchResult(True, f"{status} content before marker", preview=preview)

    def _generate_diff_preview(self, old_content: str, new_content: str,
                              context_lines: int = 3) -> str:
        """Generate a unified diff preview."""
        old_lines = old_content.splitlines(keepends=True)
        new_lines = new_content.splitlines(keepends=True)

        diff = difflib.unified_diff(
            old_lines,
            new_lines,
            fromfile=str(self.file_path),
            tofile=str(self.file_path),
            lineterm='',
            n=context_lines
        )

        return '\n'.join(diff)

    def _write_file(self, content: str):
        """Write content to file, optionally creating backup."""
        if self.create_backup and self.file_path.exists():
            backup_path = self.file_path.with_suffix(self.file_path.suffix + '.bak')
            import shutil
            shutil.copy2(self.file_path, backup_path)

        # Create parent directories if needed
        self.file_path.parent.mkdir(parents=True, exist_ok=True)

        with open(self.file_path, 'w', encoding='utf-8') as f:
            f.write(content)

        self.content = content
        self.lines = content.splitlines(keepends=True)


def main():
    """Command-line interface."""
    parser = argparse.ArgumentParser(
        description='Patch text and markdown files for LLM book editing',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Search and replace
  python patch_txt.py book.md --mode search_replace --search "old text" --replace "new text"

  # Replace line range
  python patch_txt.py book.md --mode line_range --start 10 --end 20 --content "New content"

  # Append to file
  python patch_txt.py book.md --mode append --content "## New Chapter"

  # Insert after marker
  python patch_txt.py book.md --mode insert_after --marker "# Chapter 1" --content "Introduction text"

  # Dry run to preview changes
  python patch_txt.py book.md --mode search_replace --search "old" --replace "new" --dry-run

  # JSON input for programmatic use
  python patch_txt.py book.md --json '{"mode":"search_replace","search":"old","replace":"new"}'
        """
    )

    parser.add_argument('file', help='Path to .md or .txt file to patch')
    parser.add_argument('--mode', choices=[m.value for m in PatchMode],
                       help='Patch mode to use')
    parser.add_argument('--json', help='JSON string with all parameters')
    parser.add_argument('--json-file', help='Path to JSON file with parameters')

    # Search/replace options
    parser.add_argument('--search', help='Text to search for (search_replace mode)')
    parser.add_argument('--replace', help='Replacement text (search_replace mode)')
    parser.add_argument('--count', type=int, default=-1,
                       help='Max replacements, -1 for all (search_replace mode)')
    parser.add_argument('--case-sensitive', action='store_true', default=True,
                       help='Case-sensitive search (search_replace mode)')

    # Line range options
    parser.add_argument('--start', type=int, help='Start line number (line_range mode)')
    parser.add_argument('--end', type=int, help='End line number (line_range mode)')

    # Content options
    parser.add_argument('--content', help='Content to insert/append/prepend')
    parser.add_argument('--content-file', help='Read content from file')

    # Marker options
    parser.add_argument('--marker', help='Marker text (insert_after/insert_before modes)')
    parser.add_argument('--first-only', action='store_true',
                       help='Only affect first occurrence (insert modes)')

    # Diff options
    parser.add_argument('--diff', help='Unified diff text (unified_diff mode)')
    parser.add_argument('--diff-file', help='Path to unified diff file')

    # General options
    parser.add_argument('--dry-run', action='store_true',
                       help='Preview changes without applying')
    parser.add_argument('--no-backup', action='store_true',
                       help='Do not create .bak backup file')
    parser.add_argument('--output-json', action='store_true',
                       help='Output result as JSON')

    args = parser.parse_args()

    # Parse JSON input if provided
    if args.json or args.json_file:
        if args.json:
            params = json.loads(args.json)
        else:
            with open(args.json_file, 'r') as f:
                params = json.load(f)

        # Override args with JSON params
        for key, value in params.items():
            setattr(args, key.replace('-', '_'), value)

    # Read content from file if specified
    if args.content_file:
        with open(args.content_file, 'r', encoding='utf-8') as f:
            args.content = f.read()

    # Read diff from file if specified
    if args.diff_file:
        with open(args.diff_file, 'r', encoding='utf-8') as f:
            args.diff = f.read()

    try:
        # Initialize tool
        tool = PatchTxtTool(
            args.file,
            create_backup=not args.no_backup,
            dry_run=args.dry_run
        )

        # Apply appropriate patch method
        if args.mode == PatchMode.SEARCH_REPLACE.value:
            if not args.search or args.replace is None:
                parser.error("--search and --replace required for search_replace mode")
            result = tool.apply_search_replace(
                args.search, args.replace, args.count, args.case_sensitive
            )

        elif args.mode == PatchMode.LINE_RANGE.value:
            if args.start is None or args.end is None or not args.content:
                parser.error("--start, --end, and --content required for line_range mode")
            result = tool.apply_line_range(args.start, args.end, args.content)

        elif args.mode == PatchMode.APPEND.value:
            if not args.content:
                parser.error("--content required for append mode")
            result = tool.apply_append(args.content)

        elif args.mode == PatchMode.PREPEND.value:
            if not args.content:
                parser.error("--content required for prepend mode")
            result = tool.apply_prepend(args.content)

        elif args.mode == PatchMode.INSERT_AFTER.value:
            if not args.marker or not args.content:
                parser.error("--marker and --content required for insert_after mode")
            result = tool.apply_insert_after(
                args.marker, args.content, first_occurrence=args.first_only
            )

        elif args.mode == PatchMode.INSERT_BEFORE.value:
            if not args.marker or not args.content:
                parser.error("--marker and --content required for insert_before mode")
            result = tool.apply_insert_before(
                args.marker, args.content, first_occurrence=args.first_only
            )

        elif args.mode == PatchMode.UNIFIED_DIFF.value:
            if not args.diff:
                parser.error("--diff or --diff-file required for unified_diff mode")
            result = tool.apply_unified_diff(args.diff)

        else:
            parser.error(f"Mode required. Choose from: {', '.join(m.value for m in PatchMode)}")

        # Output result
        if args.output_json:
            output = {
                'success': result.success,
                'message': result.message,
                'lines_changed': result.lines_changed,
                'preview': result.preview,
                'file': str(args.file)
            }
            print(json.dumps(output, indent=2))
        else:
            status = "✓" if result.success else "✗"
            print(f"{status} {result.message}")
            if result.preview and args.dry_run:
                print("\nPreview of changes:")
                print(result.preview)

        return 0 if result.success else 1

    except Exception as e:
        if args.output_json:
            print(json.dumps({
                'success': False,
                'message': str(e),
                'error': type(e).__name__
            }))
        else:
            print(f"✗ Error: {e}", file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
