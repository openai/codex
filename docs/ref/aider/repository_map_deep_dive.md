# Aider Repository Map: Deep Dive Technical Analysis

A comprehensive technical analysis of Aider's Repository Map feature - the core capability that enables semantic code understanding.

**Source File:** `aider/repomap.py` (849 lines)
**Version:** Cache v4 (with tree-sitter-language-pack)

---

## Table of Contents

1. [Overview](#1-overview)
2. [Data Structures](#2-data-structures)
3. [Tree-Sitter Parsing Pipeline](#3-tree-sitter-parsing-pipeline)
4. [Tree-Sitter Query Files](#4-tree-sitter-query-files)
5. [PageRank Ranking Algorithm](#5-pagerank-ranking-algorithm)
6. [Token Budgeting](#6-token-budgeting)
7. [Caching Strategy](#7-caching-strategy)
8. [Refresh Modes](#8-refresh-modes)
9. [Integration with Coder](#9-integration-with-coder)
10. [Important Files Filtering](#10-important-files-filtering)
11. [Tree Context Rendering](#11-tree-context-rendering)
12. [Complete Data Flow](#12-complete-data-flow)

---

## 1. Overview

The Repository Map is Aider's sophisticated system for providing the LLM with a condensed view of the codebase. Instead of loading entire files, it:

1. **Parses** code using tree-sitter to extract definitions and references
2. **Builds** a dependency graph between files
3. **Ranks** files using PageRank to find the most relevant code
4. **Budgets** tokens via binary search to fit within context limits
5. **Caches** results at multiple levels for performance

### Why Repository Map Matters

| Without Repo Map | With Repo Map |
|-----------------|---------------|
| Load entire codebase (millions of tokens) | Load only relevant snippets (thousands of tokens) |
| LLM can't understand large projects | LLM sees project structure semantically |
| No prioritization of relevant code | Chat files boost related code by 50x |
| Static context | Dynamic context based on user mentions |

---

## 2. Data Structures

### 2.1 Tag Namedtuple

```python
# repomap.py:28
Tag = namedtuple("Tag", "rel_fname fname line name kind".split())
```

| Field | Type | Description |
|-------|------|-------------|
| `rel_fname` | str | Relative file path from repo root |
| `fname` | str | Absolute file path |
| `line` | int | Line number (0-indexed), -1 for Pygments refs |
| `name` | str | Identifier name (function, class, variable) |
| `kind` | str | Either "def" (definition) or "ref" (reference) |

**Example:**
```python
Tag(
    rel_fname="src/utils.py",
    fname="/home/user/project/src/utils.py",
    line=42,
    name="calculate_hash",
    kind="def"
)
```

### 2.2 RepoMap Class Attributes

```python
# repomap.py:41-82
class RepoMap:
    TAGS_CACHE_DIR = ".aider.tags.cache.v4"  # SQLite cache directory
    warned_files = set()                      # Files that triggered warnings

    def __init__(
        self,
        map_tokens=1024,           # Base token budget
        root=None,                 # Repository root directory
        main_model=None,           # Model for token counting
        io=None,                   # I/O abstraction
        repo_content_prefix=None,  # Prefix for repo content in prompt
        verbose=False,             # Debug output
        max_context_window=None,   # Model's max context
        map_mul_no_files=8,        # Multiplier when no files in chat
        refresh="auto",            # Refresh strategy
    ):
        # Instance variables
        self.max_map_tokens = map_tokens
        self.map_mul_no_files = map_mul_no_files
        self.cache_threshold = 0.95

        # Cache layers
        self.tree_cache = {}           # Level 2: Tree rendering
        self.tree_context_cache = {}   # TreeContext objects
        self.map_cache = {}            # Level 3: Full map results
        self.map_processing_time = 0   # For auto refresh mode
        self.last_map = None           # For manual refresh mode
```

### 2.3 Cache Structures

**Level 1: SQLite Disk Cache (TAGS_CACHE)**
```python
# Key: absolute file path (str)
# Value: {"mtime": float, "data": [Tag, ...]}

self.TAGS_CACHE["/path/to/file.py"] = {
    "mtime": 1703123456.789,
    "data": [
        Tag("file.py", "/path/to/file.py", 0, "MyClass", "def"),
        Tag("file.py", "/path/to/file.py", 5, "my_method", "def"),
        Tag("file.py", "/path/to/file.py", 10, "helper_func", "ref"),
    ]
}
```

**Level 2: In-Memory Tree Cache**
```python
# Key: (rel_fname, tuple(sorted(lines_of_interest)), mtime)
# Value: rendered string

self.tree_cache[("utils.py", (5, 10, 25), 1703123456.789)] = """
  5│ def calculate_hash(data):
  6│     \"\"\"Calculate MD5 hash.\"\"\"
...
 10│     return result
...
 25│ class HashManager:
"""
```

**Level 3: Map Result Cache**
```python
# Key: (chat_fnames, other_fnames, max_tokens, mentioned_fnames, mentioned_idents)
# Value: complete repo map string

self.map_cache[(
    ("main.py", "utils.py"),        # chat files
    ("helper.py", "config.py"),      # other files
    4096,                            # max tokens
    ("models.py",),                  # mentioned files
    ("calculate", "hash"),           # mentioned identifiers
)] = "helper.py:\n  1│ def helper_func():..."
```

---

## 3. Tree-Sitter Parsing Pipeline

### 3.1 Entry Point: get_tags()

```python
# repomap.py:232-263
def get_tags(self, fname, rel_fname):
    # Step 1: Check cache freshness
    file_mtime = self.get_mtime(fname)
    if file_mtime is None:
        return []

    cache_key = fname
    val = self.TAGS_CACHE.get(cache_key)

    # Step 2: Return cached if mtime matches
    if val is not None and val.get("mtime") == file_mtime:
        return self.TAGS_CACHE[cache_key]["data"]

    # Step 3: Cache miss - parse file
    data = list(self.get_tags_raw(fname, rel_fname))

    # Step 4: Update cache
    self.TAGS_CACHE[cache_key] = {"mtime": file_mtime, "data": data}
    return data
```

### 3.2 Core Parsing: get_tags_raw()

```python
# repomap.py:265-344
def get_tags_raw(self, fname, rel_fname):
    # Step 1: Language detection
    lang = filename_to_lang(fname)  # e.g., "python", "rust", "javascript"
    if not lang:
        return

    # Step 2: Get tree-sitter parser and language
    try:
        language = get_language(lang)  # Tree-sitter language object
        parser = get_parser(lang)       # Tree-sitter parser
    except Exception as err:
        print(f"Skipping file {fname}: {err}")
        return

    # Step 3: Load query file
    query_scm = get_scm_fname(lang)  # e.g., "python-tags.scm"
    if not query_scm.exists():
        return
    query_scm = query_scm.read_text()  # Read Scheme query

    # Step 4: Parse source code
    code = self.io.read_text(fname)
    if not code:
        return
    tree = parser.parse(bytes(code, "utf-8"))  # AST

    # Step 5: Execute query
    query = language.query(query_scm)
    captures = query.captures(tree.root_node)

    # Step 6: Extract tags from captures
    saw = set()  # Track what kinds we've seen

    # Handle tree-sitter-language-pack format
    if USING_TSL_PACK:
        all_nodes = []
        for tag, nodes in captures.items():
            all_nodes += [(node, tag) for node in nodes]
    else:
        all_nodes = list(captures)

    for node, tag in all_nodes:
        # Determine tag kind from capture name
        if tag.startswith("name.definition."):
            kind = "def"
        elif tag.startswith("name.reference."):
            kind = "ref"
        else:
            continue

        saw.add(kind)

        yield Tag(
            rel_fname=rel_fname,
            fname=fname,
            name=node.text.decode("utf-8"),
            kind=kind,
            line=node.start_point[0],
        )

    # Step 7: Fallback to Pygments if no refs found
    if "ref" in saw:
        return
    if "def" not in saw:
        return

    # Some languages (e.g., C++) only have defs in tree-sitter
    # Use Pygments to backfill references
    try:
        lexer = guess_lexer_for_filename(fname, code)
    except Exception:
        return

    tokens = list(lexer.get_tokens(code))
    tokens = [token[1] for token in tokens if token[0] in Token.Name]

    for token in tokens:
        yield Tag(
            rel_fname=rel_fname,
            fname=fname,
            name=token,
            kind="ref",
            line=-1,  # No line number from Pygments
        )
```

### 3.3 Query File Location

```python
# repomap.py:786-810
def get_scm_fname(lang):
    # Primary: tree-sitter-language-pack (newer, more languages)
    if USING_TSL_PACK:
        subdir = "tree-sitter-language-pack"
        path = resources.files(__package__).joinpath(
            "queries", subdir, f"{lang}-tags.scm"
        )
        if path.exists():
            return path

    # Fallback: tree-sitter-languages (older)
    subdir = "tree-sitter-languages"
    return resources.files(__package__).joinpath(
        "queries", subdir, f"{lang}-tags.scm"
    )
```

**Directory Structure:**
```
aider/queries/
├── tree-sitter-language-pack/  # 30+ languages (primary)
│   ├── python-tags.scm
│   ├── rust-tags.scm
│   ├── javascript-tags.scm
│   └── ...
└── tree-sitter-languages/      # 26+ languages (fallback)
    ├── python-tags.scm
    ├── rust-tags.scm
    └── ...
```

---

## 4. Tree-Sitter Query Files

### 4.1 Scheme Query Syntax

Tree-sitter queries use S-expression syntax to match AST patterns:

```scheme
; Match a pattern
(node_type
  field: (child_type) @capture_name) @parent_capture

; Capture naming convention for Aider:
; @name.definition.{class|function|method|...}  → kind="def"
; @name.reference.{call|import|...}             → kind="ref"
```

### 4.2 Python Query (python-tags.scm)

```scheme
; Module-level constant assignments
(module
  (expression_statement
    (assignment
      left: (identifier) @name.definition.constant) @definition.constant))

; Class definitions
(class_definition
  name: (identifier) @name.definition.class) @definition.class

; Function definitions
(function_definition
  name: (identifier) @name.definition.function) @definition.function

; Function/method calls (references)
(call
  function: [
      (identifier) @name.reference.call
      (attribute
        attribute: (identifier) @name.reference.call)
  ]) @reference.call
```

**Example Parse:**
```python
# Source code
class MyClass:
    def my_method(self):
        helper_func()
```

**Extracted Tags:**
```
Tag(name="MyClass",     kind="def", line=0)
Tag(name="my_method",   kind="def", line=1)
Tag(name="helper_func", kind="ref", line=2)
```

### 4.3 Rust Query (rust-tags.scm)

```scheme
; ADT definitions
(struct_item
    name: (type_identifier) @name.definition.class) @definition.class

(enum_item
    name: (type_identifier) @name.definition.class) @definition.class

(union_item
    name: (type_identifier) @name.definition.class) @definition.class

; Type aliases
(type_item
    name: (type_identifier) @name.definition.class) @definition.class

; Method definitions (inside impl blocks)
(declaration_list
    (function_item
        name: (identifier) @name.definition.method) @definition.method)

; Function definitions
(function_item
    name: (identifier) @name.definition.function) @definition.function

; Trait definitions
(trait_item
    name: (type_identifier) @name.definition.interface) @definition.interface

; Module definitions
(mod_item
    name: (identifier) @name.definition.module) @definition.module

; Macro definitions
(macro_definition
    name: (identifier) @name.definition.macro) @definition.macro

; Function calls (references)
(call_expression
    function: (identifier) @name.reference.call) @reference.call

(call_expression
    function: (field_expression
        field: (field_identifier) @name.reference.call)) @reference.call

; Macro invocations
(macro_invocation
    macro: (identifier) @name.reference.call) @reference.call

; Trait implementations
(impl_item
    trait: (type_identifier) @name.reference.implementation) @reference.implementation

(impl_item
    type: (type_identifier) @name.reference.implementation
    !trait) @reference.implementation
```

### 4.4 Supported Languages (56+)

**tree-sitter-language-pack (30):**
arduino, c, chatito, clojure, commonlisp, cpp, csharp, d, dart, elisp, elixir, elm, gleam, go, java, javascript, lua, matlab, ocaml, ocaml_interface, pony, properties, python, r, racket, ruby, rust, solidity, swift, udev

**tree-sitter-languages (26):**
c, c_sharp, cpp, dart, elisp, elixir, elm, fortran, go, haskell, hcl, java, javascript, julia, kotlin, matlab, ocaml, ocaml_interface, php, python, ql, ruby, rust, scala, typescript, zig

---

## 5. PageRank Ranking Algorithm

### 5.1 Algorithm Overview

The ranking algorithm uses **personalized PageRank** on a directed graph where:
- **Nodes** = Files
- **Edges** = File A references a definition in File B
- **Personalization** = Boost files in chat or mentioned by user

### 5.2 Complete Implementation

```python
# repomap.py:346-555
def get_ranked_tags(
    self, chat_fnames, other_fnames, mentioned_fnames, mentioned_idents, progress=None
):
    import networkx as nx

    # ═══════════════════════════════════════════════════════════════
    # PHASE 1: Data Collection
    # ═══════════════════════════════════════════════════════════════

    defines = defaultdict(set)        # ident → {files that define it}
    references = defaultdict(list)    # ident → [files that reference it]
    definitions = defaultdict(set)    # (file, ident) → {Tag objects}
    personalization = dict()          # file → personalization score

    fnames = set(chat_fnames).union(set(other_fnames))
    chat_rel_fnames = set()
    fnames = sorted(fnames)

    # Default personalization = 100 / num_files
    personalize = 100 / len(fnames)

    # Show progress bar for large repos
    if len(fnames) - len(self.TAGS_CACHE) > 100:
        self.io.tool_output(
            "Initial repo scan can be slow in larger repos, but only happens once."
        )
        fnames = tqdm(fnames, desc="Scanning repo")

    # ═══════════════════════════════════════════════════════════════
    # PHASE 2: Build Personalization Vector
    # ═══════════════════════════════════════════════════════════════

    for fname in fnames:
        rel_fname = self.get_rel_fname(fname)
        current_pers = 0.0

        # Boost files in chat
        if fname in chat_fnames:
            current_pers += personalize
            chat_rel_fnames.add(rel_fname)

        # Boost mentioned files
        if rel_fname in mentioned_fnames:
            current_pers = max(current_pers, personalize)

        # Boost files matching mentioned identifiers
        path_obj = Path(rel_fname)
        path_components = set(path_obj.parts)
        basename_with_ext = path_obj.name
        basename_without_ext, _ = os.path.splitext(basename_with_ext)
        components_to_check = path_components.union({
            basename_with_ext,
            basename_without_ext
        })

        matched_idents = components_to_check.intersection(mentioned_idents)
        if matched_idents:
            current_pers += personalize

        if current_pers > 0:
            personalization[rel_fname] = current_pers

        # Collect tags
        tags = list(self.get_tags(fname, rel_fname))
        for tag in tags:
            if tag.kind == "def":
                defines[tag.name].add(rel_fname)
                key = (rel_fname, tag.name)
                definitions[key].add(tag)
            elif tag.kind == "ref":
                references[tag.name].append(rel_fname)

    # Fallback if no references found
    if not references:
        references = dict((k, list(v)) for k, v in defines.items())

    # ═══════════════════════════════════════════════════════════════
    # PHASE 3: Build Dependency Graph
    # ═══════════════════════════════════════════════════════════════

    G = nx.MultiDiGraph()

    # Self-edges for isolated definitions
    for ident in defines.keys():
        if ident in references:
            continue
        for definer in defines[ident]:
            G.add_edge(definer, definer, weight=0.1, ident=ident)

    # Intersection of defined and referenced identifiers
    idents = set(defines.keys()).intersection(set(references.keys()))

    for ident in idents:
        definers = defines[ident]

        # ─────────────────────────────────────────────────────────
        # Calculate identifier multiplier
        # ─────────────────────────────────────────────────────────
        mul = 1.0

        # Check naming convention (specific names are more valuable)
        is_snake = ("_" in ident) and any(c.isalpha() for c in ident)
        is_kebab = ("-" in ident) and any(c.isalpha() for c in ident)
        is_camel = any(c.isupper() for c in ident) and any(c.islower() for c in ident)

        # +10x if user mentioned this identifier
        if ident in mentioned_idents:
            mul *= 10

        # +10x if specific naming style AND length >= 8
        if (is_snake or is_kebab or is_camel) and len(ident) >= 8:
            mul *= 10

        # -10x (0.1x) if private (starts with _)
        if ident.startswith("_"):
            mul *= 0.1

        # -10x (0.1x) if too common (defined in >5 files)
        if len(defines[ident]) > 5:
            mul *= 0.1

        # ─────────────────────────────────────────────────────────
        # Add edges to graph
        # ─────────────────────────────────────────────────────────
        for referencer, num_refs in Counter(references[ident]).items():
            for definer in definers:
                use_mul = mul

                # +50x if referencer is in chat!
                if referencer in chat_rel_fnames:
                    use_mul *= 50

                # Scale down by sqrt to reduce dominance of frequent refs
                num_refs = math.sqrt(num_refs)

                G.add_edge(
                    referencer,
                    definer,
                    weight=use_mul * num_refs,
                    ident=ident
                )

    # ═══════════════════════════════════════════════════════════════
    # PHASE 4: Run PageRank
    # ═══════════════════════════════════════════════════════════════

    if personalization:
        pers_args = dict(
            personalization=personalization,
            dangling=personalization
        )
    else:
        pers_args = dict()

    try:
        ranked = nx.pagerank(G, weight="weight", **pers_args)
    except ZeroDivisionError:
        try:
            ranked = nx.pagerank(G, weight="weight")
        except ZeroDivisionError:
            return []

    # ═══════════════════════════════════════════════════════════════
    # PHASE 5: Distribute Rank to Definitions
    # ═══════════════════════════════════════════════════════════════

    ranked_definitions = defaultdict(float)

    for src in G.nodes:
        src_rank = ranked[src]
        total_weight = sum(
            data["weight"]
            for _src, _dst, data in G.out_edges(src, data=True)
        )

        for _src, dst, data in G.out_edges(src, data=True):
            # Distribute source rank proportionally to edge weights
            data["rank"] = src_rank * data["weight"] / total_weight
            ident = data["ident"]
            ranked_definitions[(dst, ident)] += data["rank"]

    # ═══════════════════════════════════════════════════════════════
    # PHASE 6: Sort and Filter Results
    # ═══════════════════════════════════════════════════════════════

    ranked_tags = []
    ranked_definitions = sorted(
        ranked_definitions.items(),
        reverse=True,
        key=lambda x: (x[1], x[0])  # Sort by rank, then by (file, ident)
    )

    for (fname, ident), rank in ranked_definitions:
        # Skip files already in chat
        if fname in chat_rel_fnames:
            continue
        ranked_tags += list(definitions.get((fname, ident), []))

    # Add files without tags (just filenames)
    rel_other_fnames_without_tags = set(
        self.get_rel_fname(fname) for fname in other_fnames
    )
    fnames_already_included = set(rt[0] for rt in ranked_tags)

    top_rank = sorted(
        [(rank, node) for (node, rank) in ranked.items()],
        reverse=True
    )
    for rank, fname in top_rank:
        if fname in rel_other_fnames_without_tags:
            rel_other_fnames_without_tags.remove(fname)
        if fname not in fnames_already_included:
            ranked_tags.append((fname,))  # Tuple with just filename

    for fname in rel_other_fnames_without_tags:
        ranked_tags.append((fname,))

    return ranked_tags
```

### 5.3 Weight Multiplier Summary

| Condition | Multiplier | Purpose |
|-----------|------------|---------|
| User mentioned identifier | 10x | Prioritize explicit mentions |
| snake_case/camelCase + len≥8 | 10x | Specific names are valuable |
| Private (starts with `_`) | 0.1x | Deprioritize internal APIs |
| Common (defined in >5 files) | 0.1x | Deprioritize utility functions |
| Referenced from chat file | 50x | **Critical:** Focus on chat context |
| High frequency refs | sqrt(n) | Prevent dominance |

**Combined Example:**
```
User says: "fix the calculate_user_score function"

calculate_user_score:
  - mentioned by user: 10x
  - snake_case + len=20: 10x
  - not private: 1x
  - defined in 1 file: 1x
  Final multiplier: 100x

If referenced from chat file: 100x * 50x = 5000x boost!
```

---

## 6. Token Budgeting

### 6.1 Token Estimation

```python
# repomap.py:88-100
def token_count(self, text):
    len_text = len(text)

    # Small text: exact count
    if len_text < 200:
        return self.main_model.token_count(text)

    # Large text: sample-based estimation
    lines = text.splitlines(keepends=True)
    num_lines = len(lines)
    step = num_lines // 100 or 1
    lines = lines[::step]  # Sample every Nth line
    sample_text = "".join(lines)
    sample_tokens = self.main_model.token_count(sample_text)

    # Extrapolate
    est_tokens = sample_tokens / len(sample_text) * len_text
    return est_tokens
```

### 6.2 Binary Search Algorithm

```python
# repomap.py:610-687
def get_ranked_tags_map_uncached(
    self, chat_fnames, other_fnames, max_map_tokens,
    mentioned_fnames, mentioned_idents
):
    # Get ranked tags
    ranked_tags = self.get_ranked_tags(...)

    # Prepend important files
    special_fnames = filter_important_files(other_rel_fnames)
    ranked_tags = special_fnames + ranked_tags

    # Binary search setup
    num_tags = len(ranked_tags)
    lower_bound = 0
    upper_bound = num_tags
    best_tree = None
    best_tree_tokens = 0

    # Initial estimate: assume ~25 tokens per tag
    middle = min(int(max_map_tokens // 25), num_tags)

    while lower_bound <= upper_bound:
        # Build tree with first `middle` tags
        tree = self.to_tree(ranked_tags[:middle], chat_rel_fnames)
        num_tokens = self.token_count(tree)

        # Calculate percentage error
        pct_err = abs(num_tokens - max_map_tokens) / max_map_tokens
        ok_err = 0.15  # 15% tolerance

        # Update best result
        if (num_tokens <= max_map_tokens and num_tokens > best_tree_tokens) \
           or pct_err < ok_err:
            best_tree = tree
            best_tree_tokens = num_tokens

            # Close enough - stop searching
            if pct_err < ok_err:
                break

        # Binary search step
        if num_tokens < max_map_tokens:
            lower_bound = middle + 1  # Need more tags
        else:
            upper_bound = middle - 1  # Need fewer tags

        middle = int((lower_bound + upper_bound) // 2)

    return best_tree
```

### 6.3 Dynamic Budget Expansion

```python
# repomap.py:119-131
# When no files in chat, expand budget
padding = 4096
if max_map_tokens and self.max_context_window:
    target = min(
        int(max_map_tokens * self.map_mul_no_files),  # 8x default
        self.max_context_window - padding,
    )
else:
    target = 0

if not chat_files and self.max_context_window and target > 0:
    max_map_tokens = target  # Expanded budget
```

**Example:**
- Default: `map_tokens = 4096`
- No files in chat: `map_tokens = min(4096 * 8, context_window - 4096) = 32768`

---

## 7. Caching Strategy

### 7.1 Level 1: SQLite Disk Cache

```python
# repomap.py:216-221
def load_tags_cache(self):
    path = Path(self.root) / self.TAGS_CACHE_DIR
    try:
        self.TAGS_CACHE = Cache(path)  # diskcache.Cache
    except SQLITE_ERRORS as e:
        self.tags_cache_error(e)
```

**Cache Directory:** `.aider.tags.cache.v4/`

**Error Handling:**
```python
# repomap.py:176-214
def tags_cache_error(self, original_error=None):
    # Try to recreate cache
    if path.exists():
        shutil.rmtree(path)

    try:
        new_cache = Cache(path)
        # Test cache works
        new_cache["test"] = "test"
        _ = new_cache["test"]
        del new_cache["test"]
        self.TAGS_CACHE = new_cache
    except SQLITE_ERRORS:
        # Fallback to in-memory dict
        self.TAGS_CACHE = dict()
```

### 7.2 Level 2: Tree Rendering Cache

```python
# repomap.py:691-727
def render_tree(self, abs_fname, rel_fname, lois):
    mtime = self.get_mtime(abs_fname)
    key = (rel_fname, tuple(sorted(lois)), mtime)

    # Check cache
    if key in self.tree_cache:
        return self.tree_cache[key]

    # Check TreeContext cache
    if (rel_fname not in self.tree_context_cache
        or self.tree_context_cache[rel_fname]["mtime"] != mtime):

        code = self.io.read_text(abs_fname) or ""
        if not code.endswith("\n"):
            code += "\n"

        context = TreeContext(
            rel_fname, code,
            color=False,
            line_number=False,
            child_context=False,
            last_line=False,
            margin=0,
            mark_lois=False,
            loi_pad=0,
            show_top_of_file_parent_scope=False,
        )
        self.tree_context_cache[rel_fname] = {
            "context": context,
            "mtime": mtime
        }

    context = self.tree_context_cache[rel_fname]["context"]
    context.lines_of_interest = set()
    context.add_lines_of_interest(lois)
    context.add_context()
    res = context.format()

    # Cache result
    self.tree_cache[key] = res
    return res
```

### 7.3 Level 3: Map Result Cache

```python
# repomap.py:557-608
def get_ranked_tags_map(self, chat_fnames, other_fnames, max_map_tokens,
                        mentioned_fnames, mentioned_idents, force_refresh):
    # Build cache key
    cache_key = [
        tuple(sorted(chat_fnames)) if chat_fnames else None,
        tuple(sorted(other_fnames)) if other_fnames else None,
        max_map_tokens,
    ]

    # Include mentions in key for "auto" mode
    if self.refresh == "auto":
        cache_key += [
            tuple(sorted(mentioned_fnames)) if mentioned_fnames else None,
            tuple(sorted(mentioned_idents)) if mentioned_idents else None,
        ]
    cache_key = tuple(cache_key)

    # Determine if we should use cache
    use_cache = False
    if not force_refresh:
        if self.refresh == "manual" and self.last_map:
            return self.last_map

        if self.refresh == "always":
            use_cache = False
        elif self.refresh == "files":
            use_cache = True
        elif self.refresh == "auto":
            use_cache = self.map_processing_time > 1.0

        if use_cache and cache_key in self.map_cache:
            return self.map_cache[cache_key]

    # Generate new map
    start_time = time.time()
    result = self.get_ranked_tags_map_uncached(...)
    end_time = time.time()
    self.map_processing_time = end_time - start_time

    # Update cache
    self.map_cache[cache_key] = result
    self.last_map = result

    return result
```

---

## 8. Refresh Modes

| Mode | Behavior | Use Case |
|------|----------|----------|
| `"auto"` (default) | Cache if computation > 1s | Balance freshness & performance |
| `"files"` | Cache based on file set only | Ignore mention changes |
| `"always"` | Never cache | Development/debugging |
| `"manual"` | Only update on force_refresh | Maximum performance |

### 8.1 Auto Mode Logic

```python
if self.refresh == "auto":
    # Include mentions in cache key
    cache_key += [mentioned_fnames, mentioned_idents]

    # Only use cache if last computation was slow
    use_cache = self.map_processing_time > 1.0
```

### 8.2 Manual Mode Logic

```python
if self.refresh == "manual" and self.last_map:
    return self.last_map  # Always return cached result
```

---

## 9. Integration with Coder

### 9.1 Mention Extraction

```python
# base_coder.py:678-682
def get_ident_mentions(self, text):
    # Split on non-word characters
    # \W+ = [^a-zA-Z0-9_]+
    words = set(re.split(r"\W+", text))
    return words
```

**Example:**
```
Input: "Fix the calculate_hash function in utils.py"
Output: {"Fix", "the", "calculate_hash", "function", "in", "utils", "py"}
```

### 9.2 Filename-Identifier Matching

```python
# base_coder.py:684-707
def get_ident_filename_matches(self, idents):
    all_fnames = defaultdict(set)
    for fname in self.get_all_relative_files():
        path = Path(fname)
        base = path.stem.lower()  # Filename without extension
        if len(base) >= 5:
            all_fnames[base].add(fname)

    matches = set()
    for ident in idents:
        if len(ident) < 5:
            continue
        matches.update(all_fnames[ident.lower()])

    return matches
```

**Example:**
```
User mentions: "components"
Files: ["src/components.py", "lib/utils.py"]
Matches: {"src/components.py"}
```

### 9.3 Repo Map Generation in Coder

```python
# base_coder.py:709-748
def get_repo_map(self, force_refresh=False):
    if not self.repo_map:
        return

    # Extract mentions from current message
    cur_msg_text = self.get_cur_message_text()
    mentioned_fnames = self.get_file_mentions(cur_msg_text)
    mentioned_idents = self.get_ident_mentions(cur_msg_text)

    # Match identifiers to filenames
    mentioned_fnames.update(self.get_ident_filename_matches(mentioned_idents))

    # Prepare file sets
    all_abs_files = set(self.get_all_abs_files())
    repo_abs_read_only_fnames = set(self.abs_read_only_fnames) & all_abs_files
    chat_files = set(self.abs_fnames) | repo_abs_read_only_fnames
    other_files = all_abs_files - chat_files

    # Primary attempt: with chat context
    repo_content = self.repo_map.get_repo_map(
        chat_files, other_files,
        mentioned_fnames=mentioned_fnames,
        mentioned_idents=mentioned_idents,
        force_refresh=force_refresh,
    )

    # Fallback 1: global map if disjoint
    if not repo_content:
        repo_content = self.repo_map.get_repo_map(
            set(), all_abs_files,
            mentioned_fnames=mentioned_fnames,
            mentioned_idents=mentioned_idents,
        )

    # Fallback 2: completely unhinted
    if not repo_content:
        repo_content = self.repo_map.get_repo_map(set(), all_abs_files)

    return repo_content
```

### 9.4 Message Formatting

```python
# base_coder.py:750-761
def get_repo_messages(self):
    repo_messages = []
    repo_content = self.get_repo_map()
    if repo_content:
        repo_messages += [
            dict(role="user", content=repo_content),
            dict(
                role="assistant",
                content="Ok, I won't try and edit those files without asking first.",
            ),
        ]
    return repo_messages
```

---

## 10. Important Files Filtering

### 10.1 File Patterns (177+)

```python
# special.py:3-177
ROOT_IMPORTANT_FILES = [
    # Version Control
    ".gitignore", ".gitattributes",

    # Documentation
    "README", "README.md", "README.txt", "README.rst",
    "CONTRIBUTING", "CONTRIBUTING.md",
    "LICENSE", "LICENSE.md", "LICENSE.txt",
    "CHANGELOG", "CHANGELOG.md",
    "SECURITY", "SECURITY.md",
    "CODEOWNERS",

    # Package Management
    "requirements.txt", "Pipfile", "Pipfile.lock",
    "pyproject.toml", "setup.py", "setup.cfg",
    "package.json", "package-lock.json", "yarn.lock",
    "Gemfile", "Gemfile.lock",
    "composer.json", "composer.lock",
    "pom.xml", "build.gradle", "build.gradle.kts",
    "go.mod", "go.sum",
    "Cargo.toml", "Cargo.lock",
    "mix.exs", "rebar.config",
    "project.clj", "Podfile",

    # Configuration
    ".env", ".env.example", ".editorconfig",
    "tsconfig.json", "jsconfig.json",
    ".babelrc", "babel.config.js",
    ".eslintrc", ".prettierrc",
    ".pylintrc", ".flake8",
    "mypy.ini", "tox.ini",
    "pyrightconfig.json",

    # Build
    "webpack.config.js", "rollup.config.js",
    "gulpfile.js", "Gruntfile.js",
    "MANIFEST.in",

    # Testing
    "pytest.ini", "phpunit.xml",
    "jest.config.js", "karma.conf.js",
    ".nycrc", ".nycrc.json",

    # CI/CD
    ".travis.yml", ".gitlab-ci.yml",
    "Jenkinsfile", "azure-pipelines.yml",
    ".circleci/config.yml",
    ".github/dependabot.yml",

    # Docker
    "Dockerfile", "docker-compose.yml",

    # Cloud
    "serverless.yml", "firebase.json",
    "netlify.toml", "vercel.json",
    "terraform.tf", "main.tf",
    "kubernetes.yaml", "k8s.yaml",

    # API
    "swagger.yaml", "openapi.yaml",

    # ... (177+ total patterns)
]
```

### 10.2 Filter Function

```python
# special.py:184-203
def is_important(file_path):
    file_name = os.path.basename(file_path)
    dir_name = os.path.normpath(os.path.dirname(file_path))
    normalized_path = os.path.normpath(file_path)

    # Special case: GitHub Actions workflows
    if dir_name == os.path.normpath(".github/workflows") \
       and file_name.endswith(".yml"):
        return True

    return normalized_path in NORMALIZED_ROOT_IMPORTANT_FILES

def filter_important_files(file_paths):
    return list(filter(is_important, file_paths))
```

### 10.3 Usage in Repo Map

```python
# repomap.py:637-643
other_rel_fnames = sorted(set(
    self.get_rel_fname(fname) for fname in other_fnames
))
special_fnames = filter_important_files(other_rel_fnames)

# Remove duplicates
ranked_tags_fnames = set(tag[0] for tag in ranked_tags)
special_fnames = [fn for fn in special_fnames if fn not in ranked_tags_fnames]
special_fnames = [(fn,) for fn in special_fnames]

# Prepend important files to ranked tags
ranked_tags = special_fnames + ranked_tags
```

---

## 11. Tree Context Rendering

### 11.1 to_tree() Method

```python
# repomap.py:729-765
def to_tree(self, tags, chat_rel_fnames):
    if not tags:
        return ""

    cur_fname = None
    cur_abs_fname = None
    lois = None  # Lines of interest
    output = ""

    # Dummy tag to flush final file
    dummy_tag = (None,)

    for tag in sorted(tags) + [dummy_tag]:
        this_rel_fname = tag[0]

        # Skip files already in chat
        if this_rel_fname in chat_rel_fnames:
            continue

        # When filename changes, output previous file
        if this_rel_fname != cur_fname:
            if lois is not None:
                output += "\n"
                output += cur_fname + ":\n"
                output += self.render_tree(cur_abs_fname, cur_fname, lois)
                lois = None
            elif cur_fname:
                output += "\n" + cur_fname + "\n"

            if type(tag) is Tag:
                lois = []
                cur_abs_fname = tag.fname
            cur_fname = this_rel_fname

        if lois is not None:
            lois.append(tag.line)

    # Truncate long lines (minified JS, etc.)
    output = "\n".join([line[:100] for line in output.splitlines()]) + "\n"

    return output
```

### 11.2 Output Format

**Files with Tags:**
```
src/utils.py:
  5│ def calculate_hash(data):
  6│     """Calculate MD5 hash."""
...
 10│     return result
...
 25│ class HashManager:
 26│     def __init__(self):
```

**Files without Tags (just listed):**
```
config/settings.json
data/sample.csv
```

---

## 12. Complete Data Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                        USER MESSAGE                              │
│ "Fix the calculate_hash function in utils.py"                    │
└────────────────────────────┬────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                    MENTION EXTRACTION                            │
│ base_coder.py:678-717                                           │
├─────────────────────────────────────────────────────────────────┤
│ get_ident_mentions() → {"Fix", "calculate_hash", "utils", ...}  │
│ get_file_mentions() → {"utils.py"}                              │
│ get_ident_filename_matches() → {"src/utils.py"}                 │
└────────────────────────────┬────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                    REPOMAP.get_repo_map()                        │
│ repomap.py:102-166                                              │
├─────────────────────────────────────────────────────────────────┤
│ 1. Check max_map_tokens > 0                                     │
│ 2. Expand budget if no chat files (8x)                          │
│ 3. Call get_ranked_tags_map()                                   │
└────────────────────────────┬────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                 get_ranked_tags_map() [CACHE]                    │
│ repomap.py:557-608                                              │
├─────────────────────────────────────────────────────────────────┤
│ 1. Build cache key from files + tokens + mentions               │
│ 2. Check refresh mode (auto/files/always/manual)                │
│ 3. Return cached if valid                                       │
│ 4. Otherwise call get_ranked_tags_map_uncached()                │
└────────────────────────────┬────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│             get_ranked_tags_map_uncached()                       │
│ repomap.py:610-687                                              │
├─────────────────────────────────────────────────────────────────┤
│ 1. Call get_ranked_tags() → PageRank                            │
│ 2. Prepend important files                                       │
│ 3. Binary search for token budget                                │
│ 4. Call to_tree() for formatting                                 │
└────────────────────────────┬────────────────────────────────────┘
                             │
          ┌──────────────────┴──────────────────┐
          │                                     │
          ▼                                     ▼
┌──────────────────────────────┐  ┌──────────────────────────────┐
│      get_ranked_tags()       │  │      to_tree()                │
│   repomap.py:346-555         │  │   repomap.py:729-765          │
├──────────────────────────────┤  ├──────────────────────────────┤
│ FOR EACH FILE:               │  │ FOR EACH TAG:                 │
│ 1. get_tags() [CACHE L1]     │  │ 1. Skip if in chat            │
│    ├─ Check mtime            │  │ 2. Group by filename          │
│    ├─ Return cached if fresh │  │ 3. render_tree() [CACHE L2]   │
│    └─ get_tags_raw() if miss │  │ 4. Truncate to 100 chars      │
│                              │  └──────────────────────────────┘
│ 2. Build personalization     │
│    ├─ chat files: +base      │
│    ├─ mentioned files: +base │
│    └─ path matches: +base    │
│                              │
│ 3. Build defines/references  │
│                              │
│ FOR EACH IDENTIFIER:         │
│ 4. Calculate multiplier      │
│    ├─ mentioned: 10x         │
│    ├─ specific name: 10x     │
│    ├─ private (_): 0.1x      │
│    └─ common (>5): 0.1x      │
│                              │
│ 5. Build graph edges         │
│    └─ chat referencer: 50x   │
│                              │
│ 6. nx.pagerank()             │
│                              │
│ 7. Distribute rank to defs   │
│                              │
│ 8. Sort by rank, filter chat │
└──────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────┐
│                      get_tags_raw()                              │
│ repomap.py:265-344                                              │
├─────────────────────────────────────────────────────────────────┤
│ 1. filename_to_lang("utils.py") → "python"                      │
│ 2. get_language("python") → tree-sitter Language                │
│ 3. get_parser("python") → tree-sitter Parser                    │
│ 4. get_scm_fname("python") → "python-tags.scm"                  │
│ 5. parser.parse(code) → AST                                     │
│ 6. language.query(scm).captures(ast) → nodes                    │
│ 7. Extract Tags from @name.definition.* and @name.reference.*   │
│ 8. Fallback to Pygments if no refs                              │
└────────────────────────────┬────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                       FINAL OUTPUT                               │
├─────────────────────────────────────────────────────────────────┤
│ Here are summaries of some files present in my git repository.  │
│ Do not propose changes to these files, treat them as read-only. │
│                                                                  │
│ src/utils.py:                                                    │
│  42│ def calculate_hash(data):                                   │
│  43│     """Calculate MD5 hash of input data."""                │
│ ...                                                              │
│  50│     return hashlib.md5(data).hexdigest()                   │
│                                                                  │
│ src/hash_manager.py:                                             │
│  10│ class HashManager:                                          │
│  15│     def verify_hash(self, data, expected):                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## Summary

The Repository Map is Aider's most sophisticated feature, combining:

1. **Tree-sitter** for language-aware AST parsing (56+ languages)
2. **PageRank** for intelligent relevance ranking
3. **Binary search** for optimal token budgeting
4. **Multi-level caching** for performance
5. **Personalization** for context-aware results

Key multipliers that drive relevance:
- **50x** for references from files in chat
- **10x** for user-mentioned identifiers
- **10x** for specific naming patterns
- **0.1x** for private/common names

This enables Aider to provide targeted, context-aware code assistance for projects of any size.
