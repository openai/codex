# Quick Start: patch_txt for LLMs

## For LLM Developers

Your LLM can use this tool to edit markdown and text files with 7 different modes:

### Minimal Example

```python
import subprocess
import json

def llm_edit_file(file, mode, **kwargs):
    """Simple function for LLMs to edit files."""
    params = {"file": file, "mode": mode, **kwargs}

    result = subprocess.run(
        ["python3", "patch_txt.py", file,
         "--json", json.dumps(params),
         "--output-json"],
        capture_output=True,
        text=True
    )

    return json.loads(result.stdout)

# Usage examples:
result = llm_edit_file(
    "book.md",
    mode="search_replace",
    search="old text",
    replace="new text"
)

result = llm_edit_file(
    "chapter1.md",
    mode="append",
    content="\n## New Chapter\n\nContent here..."
)
```

## For MCP Server Integration

Add to your MCP server's tool definitions:

```python
import json

# Load the tool schema
with open('patch_txt_tool.json') as f:
    PATCH_TXT_SCHEMA = json.load(f)

# In your tool handler:
async def handle_patch_txt(arguments: dict) -> str:
    """Handle patch_txt tool calls."""
    import subprocess

    result = subprocess.run(
        ["python3", "patch_txt.py",
         arguments["file"],
         "--json", json.dumps(arguments),
         "--output-json"],
        capture_output=True,
        text=True,
        timeout=30
    )

    output = json.loads(result.stdout)

    if output["success"]:
        return f"✓ {output['message']}\n\n{output.get('preview', '')}"
    else:
        return f"✗ Error: {output['message']}"
```

## Common LLM Use Cases

### 1. Fix Typos Throughout Book

```json
{
  "file": "manuscript.md",
  "mode": "search_replace",
  "search": "recieve",
  "replace": "receive"
}
```

### 2. Rewrite Specific Paragraphs

```json
{
  "file": "chapter3.md",
  "mode": "line_range",
  "start": 25,
  "end": 30,
  "content": "Improved paragraph with better flow and details..."
}
```

### 3. Add New Sections

```json
{
  "file": "book.md",
  "mode": "insert_after",
  "marker": "## Chapter 5",
  "content": "\n### Section 5.1: New Topic\n\nContent here...\n"
}
```

### 4. Build Book from Scratch

```python
# Start with title
llm_edit_file("book.md", mode="prepend",
              content="# My Novel\n\nBy Author Name\n\n")

# Add chapters
llm_edit_file("book.md", mode="append",
              content="## Chapter 1: The Beginning\n\nOnce upon a time...\n\n")

llm_edit_file("book.md", mode="append",
              content="## Chapter 2: The Journey\n\nThe adventure continues...\n\n")
```

## All 7 Modes Cheat Sheet

| Mode | Use Case | Required Params |
|------|----------|----------------|
| `search_replace` | Find and replace text | `search`, `replace` |
| `line_range` | Replace specific lines | `start`, `end`, `content` |
| `append` | Add to end of file | `content` |
| `prepend` | Add to beginning of file | `content` |
| `insert_after` | Insert after marker | `marker`, `content` |
| `insert_before` | Insert before marker | `marker`, `content` |
| `unified_diff` | Apply standard diff | `diff` |

## Safety Tips

1. **Always use `dry_run: true` first** to preview changes
2. **Backups are automatic** unless you set `no_backup: true`
3. **Check line numbers** before using `line_range` mode
4. **Use unique markers** for insert operations
5. **Test on small files** before batch operations

## Error Handling

```python
result = llm_edit_file("book.md", mode="search_replace",
                       search="text", replace="new")

if not result["success"]:
    print(f"Edit failed: {result['message']}")
    # Common errors:
    # - "Search string not found in file"
    # - "Marker not found: ..."
    # - "Start line N exceeds file length"
    # - "File must be .md or .txt"
```

## Performance Notes

- **Fast**: Edits complete in milliseconds
- **Large files**: No problem, uses efficient line-based processing
- **Concurrent edits**: Safe to run multiple instances on different files
- **Memory**: Low footprint, suitable for long-running LLM processes

## Integration Checklist

- [ ] Copy `patch_txt.py` to your project
- [ ] Test with `python3 test_patch_txt.py`
- [ ] Review `patch_txt_tool.json` for tool schema
- [ ] Add error handling for your use case
- [ ] Consider using `dry_run=true` for user confirmation
- [ ] Set up backup strategy (default .bak files or version control)

## Example: Interactive Book Editor

```python
def interactive_edit(file, user_instruction):
    """LLM interprets user instruction and edits file."""

    # LLM analyzes instruction and chooses appropriate mode
    if "replace" in user_instruction.lower():
        # Extract search/replace terms
        return llm_edit_file(file, mode="search_replace", ...)

    elif "add chapter" in user_instruction.lower():
        # Generate chapter content
        return llm_edit_file(file, mode="append", ...)

    elif "rewrite" in user_instruction.lower():
        # Identify line range and generate new content
        return llm_edit_file(file, mode="line_range", ...)

# Usage:
interactive_edit("book.md", "Replace 'hero' with 'protagonist'")
interactive_edit("book.md", "Add a new chapter about time travel")
interactive_edit("book.md", "Rewrite lines 50-60 with more drama")
```

## Ready to Use!

The tool is production-ready and tested. See `PATCH_TXT_README.md` for complete documentation.
