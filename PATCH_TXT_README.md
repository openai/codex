# patch_txt - LLM-Friendly Text File Editing Tool

A Python tool designed for LLMs to edit and patch markdown (.md) and text (.txt) files, optimized for book writing and prose editing.

## Features

- **Multiple Edit Modes**: 7 different ways to edit files
- **LLM-Friendly**: Simple JSON interface, clear error messages
- **Safe by Default**: Creates backups before modifying files
- **Dry Run Mode**: Preview changes before applying
- **Detailed Feedback**: Shows diffs and change summaries

## Installation

```bash
# Make executable
chmod +x patch_txt.py

# No dependencies required for basic functionality
# Optional: Install patch_ng for advanced diff support
pip install patch-ng
```

## Edit Modes

### 1. Search and Replace

Replace text throughout the file.

```bash
python patch_txt.py book.md \
  --mode search_replace \
  --search "old text" \
  --replace "new text"
```

**Options:**
- `--count N`: Limit to N replacements (-1 for all)
- `--case-sensitive`: Enable case-sensitive matching

**JSON Example:**
```json
{
  "file": "chapter1.md",
  "mode": "search_replace",
  "search": "protagonist walks",
  "replace": "protagonist runs",
  "count": 1
}
```

### 2. Line Range Replacement

Replace specific lines with new content.

```bash
python patch_txt.py book.md \
  --mode line_range \
  --start 10 \
  --end 15 \
  --content "New paragraph content here."
```

**JSON Example:**
```json
{
  "file": "chapter2.md",
  "mode": "line_range",
  "start": 10,
  "end": 15,
  "content": "The hero stood at the crossroads.\n\nEach path led to uncertainty."
}
```

### 3. Append

Add content to the end of the file.

```bash
python patch_txt.py book.md \
  --mode append \
  --content "\n## Epilogue\n\nYears later..."
```

**JSON Example:**
```json
{
  "file": "book.md",
  "mode": "append",
  "content": "\n## Acknowledgments\n\nThanks to my readers..."
}
```

### 4. Prepend

Add content to the beginning of the file.

```bash
python patch_txt.py book.md \
  --mode prepend \
  --content "# My Novel\n\nBy Author Name\n\n---\n\n"
```

**JSON Example:**
```json
{
  "file": "book.md",
  "mode": "prepend",
  "content": "---\ntitle: My Book\nauthor: Jane Doe\ndate: 2025-01-15\n---\n\n"
}
```

### 5. Insert After Marker

Insert content after a specific marker string.

```bash
python patch_txt.py book.md \
  --mode insert_after \
  --marker "## Chapter 3" \
  --content "\n*Previously: Our hero escaped the dungeon.*\n"
```

**Options:**
- `--first-only`: Only insert after first occurrence (default: true)

**JSON Example:**
```json
{
  "file": "novel.md",
  "mode": "insert_after",
  "marker": "## Part Two",
  "content": "\n> \"The journey of a thousand miles begins with a single step.\" - Lao Tzu\n",
  "first_only": true
}
```

### 6. Insert Before Marker

Insert content before a specific marker string.

```bash
python patch_txt.py book.md \
  --mode insert_before \
  --marker "## Conclusion" \
  --content "\n---\n\n"
```

**JSON Example:**
```json
{
  "file": "article.md",
  "mode": "insert_before",
  "marker": "## References",
  "content": "\n## Further Reading\n\nFor more information, see...\n"
}
```

### 7. Unified Diff

Apply a standard unified diff patch.

```bash
python patch_txt.py book.md \
  --mode unified_diff \
  --diff-file changes.patch
```

**JSON Example:**
```json
{
  "file": "chapter1.md",
  "mode": "unified_diff",
  "diff": "--- chapter1.md\n+++ chapter1.md\n@@ -1,3 +1,3 @@\n # Chapter 1\n \n-The old text.\n+The new text.\n"
}
```

## JSON Interface

Perfect for programmatic/LLM use:

```bash
# From command line
python patch_txt.py book.md --json '{"mode":"append","content":"New chapter"}'

# From file
python patch_txt.py book.md --json-file edit_commands.json

# With JSON output
python patch_txt.py book.md --json '{"mode":"search_replace","search":"old","replace":"new"}' --output-json
```

## Common Options

- `--dry-run`: Preview changes without applying them
- `--no-backup`: Don't create .bak backup files
- `--output-json`: Return results as JSON
- `--content-file FILE`: Read content from a file instead of command line

## Use Cases for Book Writing

### Fix Consistent Typos

```json
{
  "file": "manuscript.md",
  "mode": "search_replace",
  "search": "teh",
  "replace": "the"
}
```

### Rewrite a Scene

```json
{
  "file": "chapter5.md",
  "mode": "line_range",
  "start": 45,
  "end": 67,
  "content": "The confrontation scene rewritten...\n\nWith better pacing and dialogue."
}
```

### Add Chapter Epigraphs

```json
{
  "file": "chapter3.md",
  "mode": "insert_after",
  "marker": "# Chapter 3: The Journey",
  "content": "\n> *\"Not all those who wander are lost.\"* - J.R.R. Tolkien\n"
}
```

### Build Table of Contents

```json
{
  "file": "book.md",
  "mode": "prepend",
  "content": "# Table of Contents\n\n1. [Chapter 1](#chapter-1)\n2. [Chapter 2](#chapter-2)\n\n---\n\n"
}
```

### Add Revision Notes

```json
{
  "file": "draft.md",
  "mode": "insert_before",
  "marker": "## Conclusion",
  "content": "\n<!-- TODO: Expand this section with more examples -->\n\n"
}
```

## Error Handling

The tool provides clear error messages:

- **File not found**: Will create new file if parent directory exists
- **Invalid file extension**: Only .md and .txt allowed
- **Marker not found**: Reports when insert marker doesn't exist
- **Invalid line range**: Validates line numbers
- **Empty search string**: Prevents accidental global replacements

## Output Formats

### Standard Output

```
✓ Replaced 3 occurrence(s) of search string
```

### JSON Output

```json
{
  "success": true,
  "message": "Replaced 3 occurrence(s) of search string",
  "lines_changed": 3,
  "preview": "--- book.md\n+++ book.md\n@@ -10,7 +10,7 @@\n...",
  "file": "book.md"
}
```

## Integration with LLM Tools

### As an MCP Tool

See `patch_txt_tool.json` for the complete tool schema.

### As a Python Function

```python
from patch_txt import PatchTxtTool

tool = PatchTxtTool("book.md", create_backup=True, dry_run=False)
result = tool.apply_search_replace("old text", "new text")

if result.success:
    print(result.message)
    if result.preview:
        print(result.preview)
else:
    print(f"Error: {result.message}")
```

### Direct Execution

```python
import subprocess
import json

params = {
    "file": "chapter1.md",
    "mode": "search_replace",
    "search": "protagonist",
    "replace": "hero"
}

result = subprocess.run(
    ["python", "patch_txt.py", "chapter1.md", "--json", json.dumps(params), "--output-json"],
    capture_output=True,
    text=True
)

output = json.loads(result.stdout)
print(output["message"])
```

## Safety Features

1. **Automatic Backups**: Creates .bak files before modifications
2. **Dry Run Mode**: Preview all changes before applying
3. **File Validation**: Only accepts .md and .txt files
4. **Error Messages**: Clear feedback on what went wrong
5. **Diff Previews**: See exactly what will change

## Best Practices

1. **Use Dry Run First**: Test complex edits with `--dry-run`
2. **Specific Markers**: Use unique text for insert operations
3. **Line Numbers**: Check current line numbers before using line_range mode
4. **Version Control**: Commit before major edits
5. **Incremental Edits**: Make small, focused changes

## Examples for Common Book Editing Tasks

### Global Find and Replace

```bash
python patch_txt.py manuscript.md \
  --mode search_replace \
  --search "Jane Smith" \
  --replace "Sarah Johnson"
```

### Replace a Paragraph

```bash
python patch_txt.py chapter3.md \
  --mode line_range \
  --start 42 \
  --end 44 \
  --content-file new_paragraph.txt
```

### Add Copyright Notice

```bash
python patch_txt.py book.md \
  --mode prepend \
  --content "© 2025 Author Name. All rights reserved.\n\n---\n\n"
```

### Insert Scene Break

```bash
python patch_txt.py chapter7.md \
  --mode insert_after \
  --marker "She closed the door behind her." \
  --content "\n\n* * *\n\n"
```

### Add Footnote

```bash
python patch_txt.py essay.md \
  --mode insert_after \
  --marker "historical event[^1]" \
  --content "\n\n[^1]: This occurred in 1776." \
  --first-only
```

## License

MIT License - feel free to use in your projects!
