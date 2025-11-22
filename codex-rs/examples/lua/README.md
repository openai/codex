# Lua Scripting Examples

This directory contains example Lua scripts demonstrating various use cases for the Lua scripting API in codex-rs.

## Examples

### 1. transform_data.lua

Demonstrates data transformation by converting an array of person objects into a new format.

**Usage:**
```lua
args = {
    data = {
        {name = "Alice", age = 30},
        {name = "Bob", age = 25},
        {name = "Charlie", age = 35}
    }
}
```

**Output:**
```json
{
  "count": 3,
  "transformed": [
    {"id": 1, "name": "ALICE", "age_group": "adult", "info": "Alice is 30 years old"},
    {"id": 2, "name": "BOB", "age_group": "young", "info": "Bob is 25 years old"},
    {"id": 3, "name": "CHARLIE", "age_group": "adult", "info": "Charlie is 35 years old"}
  ]
}
```

### 2. filter_and_sort.lua

Filters and sorts an array of numbers, calculating statistics.

**Usage:**
```lua
args = {
    numbers = {5, 2, 8, 1, 9, 3, 6, 4, 7},
    min_value = 5
}
```

**Output:**
```json
{
  "original_count": 9,
  "filtered_count": 5,
  "sorted": [9, 8, 7, 6, 5],
  "sum": 35,
  "average": 7
}
```

### 3. text_processing.lua

Analyzes text and performs various string operations.

**Usage:**
```lua
args = {
    text = "Hello World! This is a test. Hello again!"
}
```

**Output:**
```json
{
  "character_count": 42,
  "word_count": 8,
  "sentence_count": 2,
  "most_common_word": "hello",
  "most_common_count": 2,
  "word_frequencies": {
    "hello": 2,
    "world": 1,
    "this": 1,
    "is": 1,
    "a": 1,
    "test": 1,
    "again": 1
  },
  "reversed": "!niaga olleH .tset a si sihT !dlroW olleH",
  "uppercase": "HELLO WORLD! THIS IS A TEST. HELLO AGAIN!"
}
```

## Configuration

To enable Lua scripting in codex-rs, add the following to your `~/.codex/config.toml`:

```toml
[lua]
enabled = true
scripts_dir = "~/.codex/lua-scripts"  # Optional: directory for storing scripts
allow_file_io = false                 # Security: disable file I/O by default
allow_network = false                 # Security: disable network access
max_execution_time_ms = 5000          # Timeout in milliseconds
max_memory_bytes = 0                  # 0 = unlimited (not yet enforced)
```

Then add `lua_execute` to your experimental tools list:

```toml
[tools]
experimental_supported_tools = ["lua_execute", "read_file", "grep_files"]
```

## Security Notes

The Lua runtime runs in a sandboxed environment:
- No access to `io`, `os`, or `debug` libraries
- File I/O and network access disabled by default
- Execution timeout enforced
- Scripts run in isolated environment

## Writing Your Own Scripts

Lua scripts have access to a global `args` variable containing input parameters:

```lua
-- args is passed from the tool invocation
if not args or not args.input then
    return {error = "Missing input parameter"}
end

local result = process(args.input)

-- Return value is automatically converted to JSON
return result
```

### Data Types

The following conversions apply between Lua and JSON:

| Lua Type | JSON Type |
|----------|-----------|
| nil | null |
| boolean | boolean |
| number | number |
| string | string |
| table (array) | array |
| table (map) | object |

Functions, threads, and userdata cannot be returned and will cause errors.

## Testing Scripts

You can test scripts using the `lua_execute` tool:

```bash
# Via API or codex CLI when Lua support is enabled
lua_execute(script="return {result = 1 + 1}", args=null)
```
