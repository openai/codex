# Aider Architecture Analysis

A comprehensive analysis of Aider's design, modules, and core capabilities.

**Project:** [aider-chat/aider](https://github.com/aider-chat/aider)
**Language:** Python 3.10+
**Type:** AI Pair Programming CLI Tool

---

## Table of Contents

1. [Project Overview](#project-overview)
2. [Architecture Summary](#architecture-summary)
3. [Repository Map (Core Feature)](#repository-map-core-feature)
4. [LLM Integration](#llm-integration)
5. [Coder System](#coder-system)
6. [Prompt Engineering](#prompt-engineering)
7. [Context Management](#context-management)
8. [Error Handling](#error-handling)
9. [Key Design Patterns](#key-design-patterns)
10. [File Structure Reference](#file-structure-reference)

---

## Project Overview

Aider is an AI pair programming tool that enables developers to use LLMs (GPT-4, Claude, Gemini, Deepseek, etc.) directly from the terminal to edit code files. It combines:

- **Semantic code understanding** via tree-sitter parsing
- **Intelligent context management** via PageRank-based file ranking
- **Multiple edit formats** for different use cases
- **Git integration** for version control and commit generation

### Key Statistics

| Metric | Value |
|--------|-------|
| Python Files | 80 |
| Main Package Files | 45+ |
| Coder Implementations | 14+ |
| Supported Edit Formats | 8+ |
| Supported Languages | 40+ |
| LLM Providers | 20+ (via LiteLLM) |

---

## Architecture Summary

### Module Structure

```
aider/
├── main.py              → CLI entry point (1,300+ lines)
├── args.py              → Argument parser (~900 lines)
├── models.py            → Model management (~1,300 lines)
├── llm.py               → Lazy LiteLLM wrapper
├── sendchat.py          → Message validation
├── repo.py              → Git integration (~600 lines)
├── repomap.py           → Repository mapping (~1,000 lines) ★ CORE
├── io.py                → Terminal I/O (~1,300 lines)
├── commands.py          → Command handlers (~1,500 lines)
├── history.py           → Chat summarization (~350 lines)
├── linter.py            → Code linting (~300 lines)
├── watch.py             → File watching (~300 lines)
├── coders/              → Edit strategy implementations
│   ├── base_coder.py       → Core orchestration (2,485 lines)
│   ├── base_prompts.py     → Base prompt class
│   ├── editblock_coder.py  → SEARCH/REPLACE format
│   ├── wholefile_coder.py  → Whole file replacement
│   ├── udiff_coder.py      → Unified diff format
│   ├── patch_coder.py      → Patch format
│   ├── architect_coder.py  → Two-stage planning
│   ├── ask_coder.py        → Q&A mode
│   ├── context_coder.py    → File selector
│   └── search_replace.py   → Edit matching logic
└── queries/             → Tree-sitter query files
    └── tree-sitter-language-pack/
        ├── python-tags.scm
        ├── javascript-tags.scm
        └── ... (40+ languages)
```

### Component Interaction Flow

```
main.py
  ├─> args.py (parse config)
  ├─> models.py (initialize LLM)
  ├─> repo.py (initialize git)
  ├─> repomap.py (analyze codebase) ★
  ├─> coders/ (select coder strategy)
  ├─> commands.py (handle user input)
  ├─> io.py (display output)
  └─> watch.py (monitor changes)

Coder (selected by edit_format)
  ├─> llm.py/sendchat.py (communicate with LLM)
  ├─> search_replace.py (apply edits)
  ├─> linter.py (validate code)
  ├─> repo.py (commit changes)
  └─> history.py (manage context)
```

---

## Repository Map (Core Feature)

The Repository Map is Aider's most sophisticated feature, enabling the LLM to understand the codebase structure without loading entire files.

### Architecture Overview

```
User Query
    ↓
get_repo_map() [base_coder.py:709]
    ↓
Extract Mentions:
  - File mentions (via get_file_mentions)
  - Identifier mentions (via get_ident_mentions)
    ↓
RepoMap.get_repo_map()
    ↓
Get Ranked Tags (PageRank)
    ↓
Format as Tree
    ↓
Insert into LLM Context as Read-Only Summary
```

### Phase 1: Tree-Sitter Code Parsing

**Implementation:** `repomap.py:get_tags_raw()` [lines 265-344]

For each file in the repository:

1. Detect language via file extension: `filename_to_lang(fname)`
2. Load tree-sitter parser for that language: `get_parser(lang)`
3. Load language-specific `.scm` query file (Scheme syntax)
4. Parse file into AST: `parser.parse(bytes(code, "utf-8"))`
5. Execute Scheme queries against AST: `query.captures(tree.root_node)`

**Example Tree-Sitter Query (Python):**

```scheme
(class_definition
  name: (identifier) @name.definition.class) @definition.class

(function_definition
  name: (identifier) @name.definition.function) @definition.function

(call
  function: [
      (identifier) @name.reference.call
      (attribute
        attribute: (identifier) @name.reference.call)
  ]) @reference.call
```

**Output:** Tags with `kind` = "def" (definition) or "ref" (reference)

**Fallback Strategy:** Pygments lexer for languages without rich tree-sitter references

### Phase 2: PageRank Ranking Algorithm

**Implementation:** `repomap.py:get_ranked_tags()` [lines 346-555]

This is the sophisticated core that determines which code is most relevant.

**Step 1: Build Definition & Reference Maps**

```python
defines = defaultdict(set)        # ident -> {files that define it}
references = defaultdict(list)    # ident -> [files that reference it]
```

**Step 2: Personalization Scoring**

```python
personalization[file] = 0.0
personalization[file] += personalize       if file in chat_fnames
personalization[file] = max(...)           if file in mentioned_fnames
personalization[file] += personalize       if any path component matches mentioned_idents
```

**Step 3: Build Directed Graph**

```python
G = nx.MultiDiGraph()
```

For each identifier, calculate multiplier:

```python
mul = 1.0
if ident in mentioned_idents:           mul *= 10    # 10x for mentioned
if is_snake/kebab/camel and len >= 8:  mul *= 10    # 10x for specific names
if ident.startswith("_"):               mul *= 0.1   # 0.1x for private
if len(defines[ident]) > 5:             mul *= 0.1   # 0.1x for common names
```

Edge weights with chat file boost:

```python
use_mul = mul
if referencer in chat_rel_fnames:      use_mul *= 50  # 50x boost!
num_refs = sqrt(num_refs)               # Scale down frequency
edge_weight = use_mul * num_refs
G.add_edge(referencer, definer, weight=edge_weight)
```

**Step 4: Apply PageRank**

```python
ranked = nx.pagerank(G, weight="weight", personalization=personalization)
```

**Step 5: Distribute Rank Across Definitions**

```python
for src, dst, data in G.out_edges(src, data=True):
    data["rank"] = src_rank * data["weight"] / total_weight
    ranked_definitions[(dst, ident)] += data["rank"]
```

### Phase 3: Token Budgeting

**Implementation:** `repomap.py:get_ranked_tags_map_uncached()` [lines 610-687]

Uses **binary search** to fit repo map within token budget:

```python
middle = min(max_map_tokens // 25, num_tags)
# Binary search loop
while True:
    tree = build_tree(first_middle_tags)
    tokens = token_count(tree)
    if tokens < max:
        lower_bound = middle
    else:
        upper_bound = middle
    if pct_err < 0.15:  # Within 15%
        break
```

### Phase 4: Formatting

**Implementation:** `repomap.py:render_tree()` and `to_tree()` [lines 691-765]

Output format:

```
file1.py:
    1  | def function1():
    2  |     return result

file2.py:
    10 | class MyClass:
    12 |     def method(self):
```

### Caching Strategy (3 Levels)

| Level | Location | Key | Purpose |
|-------|----------|-----|---------|
| 1 | `.aider.tags.cache.v4/` (SQLite) | file path | Per-file tag cache |
| 2 | In-memory | (fname, lines, mtime) | Tree rendering cache |
| 3 | Result cache | (files, tokens, mentions) | Full map result cache |

### Refresh Modes

- `"always"`: Recompute every time
- `"files"`: Cache based on file set
- `"auto"`: Cache if computation > 1 second
- `"manual"`: Only on explicit force_refresh

---

## LLM Integration

### Model System

**Key Files:**
- `models.py` - Central model management (~1,300 lines)
- `llm.py` - Lazy LiteLLM wrapper
- `sendchat.py` - Message validation

**ModelSettings Dataclass:**

```python
@dataclass
class ModelSettings:
    name: str                    # Model identifier
    edit_format: str             # Editing strategy
    weak_model_name: str         # Fallback for commits
    use_repo_map: bool           # Include repo summaries
    use_system_prompt: bool      # Support system prompts
    use_temperature: float|bool  # Temperature config
    streaming: bool              # Streaming support
    reasoning_tag: str           # For reasoning models
    caches_by_default: bool      # Prompt caching
    extra_params: dict           # Provider-specific
```

### Provider Adaptations

| Provider | Adaptations |
|----------|-------------|
| OpenAI | Temperature, tools, streaming, assistant prefill |
| Anthropic | Thinking tokens, prompt caching, beta headers |
| Deepseek | Alternating roles, reasoning content |
| O1 Family | No system prompts, no temperature |
| OpenRouter | Reasoning budget/effort settings |

### Token Budget Allocation

```
Max Input Tokens (e.g., 200,000)
├─ Repo map: ~12-25% (25,000 tokens)
├─ System prompt: ~5-10% (10,000 tokens)
├─ Examples: ~2-5% (5,000 tokens)
├─ Done messages (history): 20-30% (60,000 tokens)
├─ Chat files content: 20-40% (80,000 tokens)
└─ Current messages + safety margin: Remaining
```

---

## Coder System

### Edit Formats

| Format | Class | Description |
|--------|-------|-------------|
| `diff` | EditBlockCoder | SEARCH/REPLACE blocks with fuzzy matching |
| `whole` | WholeFileCoder | Entire file replacement |
| `udiff` | UnifiedDiffCoder | Standard unified diff format |
| `patch` | PatchCoder | Enhanced patch with explicit actions |
| `architect` | ArchitectCoder | Two-stage: planning → implementation |
| `ask` | AskCoder | Q&A only, no edits |
| `context` | ContextCoder | File recommendation only |

### Edit Application Flow

```python
get_edits()              # Parse response for edits
    ↓
apply_edits_dry_run()   # Test without modifying
    ↓
prepare_to_edit()       # Check permissions
    ├─ is_file_in_chat? → Can edit
    ├─ Is git-ignored? → Reject
    ├─ File doesn't exist? → Ask to create
    └─ File not in chat? → Ask permission
    ↓
apply_edits()           # Write to disk
    ↓
auto_commit()           # Git commit
```

### EditBlock Format Example

```
<<<<<<< SEARCH
def old_function():
    return "old"
=======
def old_function():
    return "new"
>>>>>>> REPLACE
```

---

## Prompt Engineering

### Prompt Hierarchy

```
base_prompts.py (CoderPrompts base class)
    ↓
{edit_format}_prompts.py (Specific prompt classes)
    ├─ EditBlockPrompts
    ├─ WholeFilePrompts
    ├─ UnifiedDiffPrompts
    └─ ...
```

### Prompt Components

| Component | Purpose |
|-----------|---------|
| `main_system` | Core edit format instructions |
| `system_reminder` | Additional rules at end |
| `example_messages` | Few-shot examples |
| `files_content_prefix` | File addition instruction |
| `repo_content_prefix` | Read-only repo context |
| `shell_cmd_prompt` | Shell command guidance |
| `lazy_prompt` | "NEVER leave comments..." |
| `overeager_prompt` | "Do what they ask, but no more" |

### Message Chunking (ChatChunks)

```python
chunks = ChatChunks()
chunks.system = [system_message]           # System instructions
chunks.examples = example_messages          # Few-shot examples
chunks.done = self.done_messages           # Previous conversation
chunks.repo = repo_summary                  # Repository summaries
chunks.readonly_files = readonly_context    # Reference files
chunks.chat_files = chat_file_contents      # Editable files
chunks.reminder = system_reminder           # Context reminder
chunks.cur = self.cur_messages             # Current turn
```

---

## Context Management

### Chat History

```python
self.cur_messages = []      # Current conversation turn
self.done_messages = []     # Previous conversation (moved when turn completes)
```

### Summarization

- Triggered when `summarizer.too_big(done_messages)` is true
- Runs in background thread
- Prompt: "Briefly summarize this partial conversation..."
- Preserves: function names, libraries, filenames

### Token Limits

```python
max_chat_history_tokens = min(max(max_input_tokens / 16, 1024), 8192)
repo_map_tokens = min(max(max_input_tokens / 8, 1024), 4096)
```

---

## Error Handling

### Retry Logic

```python
retry_delay = 0.125  # Start at 125ms
while True:
    try:
        yield from self.send(messages, functions)
        break
    except litellm_ex.exceptions_tuple() as err:
        if ex_info.name == "ContextWindowExceededError":
            show_exhausted_error()
            break
        if ex_info.retry and retry_delay < 60:
            retry_delay *= 2
            time.sleep(retry_delay)
            continue
```

### Reflection System

When edits fail, errors are sent back to LLM as `reflected_message`:

- Lint errors
- Test failures
- Edit format errors
- Max reflections: 3 (configurable)

---

## Key Design Patterns

### 1. Factory Pattern
`Coder.create()` - Factory for instantiating appropriate coder based on edit format

### 2. Strategy Pattern
Multiple coder classes implement different editing approaches, swappable at runtime

### 3. Observer Pattern
`FileWatcher` monitors file changes, triggers re-analysis

### 4. Adapter Pattern
- `Commands` adapts user input to coder operations
- `InputOutput` adapts terminal I/O
- Model providers adapt to unified interface

### 5. Template Method Pattern
`Coder.create()` manages workflow steps, subclasses implement specific logic

### 6. Chain of Responsibility
Argument parsing: CLI → config file → env vars → defaults

---

## File Structure Reference

### Core Files

| File | Lines | Purpose |
|------|-------|---------|
| `coders/base_coder.py` | 2,485 | Core orchestration |
| `models.py` | 1,300 | Model management |
| `commands.py` | 1,500 | Command handlers |
| `io.py` | 1,300 | Terminal I/O |
| `main.py` | 1,300 | CLI entry point |
| `repomap.py` | 1,000 | Repository mapping |
| `args.py` | 900 | Argument parser |
| `repo.py` | 600 | Git integration |

### Configuration Files

| File | Purpose |
|------|---------|
| `.aider.yml` | Main configuration |
| `.aider.model.settings.yml` | Custom model definitions |
| `.aider.model.metadata.json` | Model metadata |
| `.env` / `.aider/.env` | Environment variables |
| `.aiderignore` | Files Aider cannot edit |

### Technology Stack

| Component | Technology |
|-----------|------------|
| CLI | prompt_toolkit, configargparse |
| Terminal UI | rich |
| LLM Integration | litellm |
| Git Integration | GitPython |
| Code Analysis | grep-ast, tree-sitter |
| Web GUI | Streamlit |
| Web Scraping | Playwright |

---

## Summary

Aider's architecture centers on the **Repository Map** - a semantic code understanding system that:

1. **Parses** code using tree-sitter to extract definitions and references
2. **Ranks** code importance using PageRank on a dependency graph
3. **Filters** based on user mentions (files and identifiers)
4. **Budgets** tokens via binary search for optimal context usage
5. **Caches** aggressively at multiple levels
6. **Formats** as human-readable snippets with context

This enables Aider to provide targeted, context-aware code assistance without overwhelming the LLM with irrelevant code.
