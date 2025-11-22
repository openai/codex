# Lua Scripting API Documentation

## Overview

The Lua scripting API in codex-rs allows users to extend the assistant's capabilities with custom data transformations, business logic, and event handling through sandboxed Lua scripts.

## Features

- **Sandboxed Execution**: Scripts run in a restricted environment without access to system resources
- **JSON Integration**: Seamless conversion between Lua tables and JSON objects
- **Timeout Protection**: Configurable execution time limits
- **Data Transformation**: Process and transform data using Lua's flexible scripting
- **Event Handling**: React to tool completions and other events (future feature)

## Architecture

### Components

1. **lua-runtime** - Core Lua runtime crate with sandboxing
2. **LuaHandler** - Tool handler implementing the Lua execution interface
3. **LuaConfig** - Configuration system for Lua settings

### Integration Points

The Lua API integrates into codex-rs through:
- Tool system: `lua_execute` tool registered alongside other tools
- Configuration: TOML-based configuration in `~/.codex/config.toml`
- Event system: Hook points for future event-driven scripting

## Configuration

### Basic Configuration

Add to `~/.codex/config.toml`:

```toml
[lua]
enabled = true
scripts_dir = "~/.codex/lua-scripts"
allow_file_io = false
allow_network = false
max_execution_time_ms = 5000
max_memory_bytes = 0  # 0 = unlimited (not yet enforced)

[tools]
experimental_supported_tools = ["lua_execute", "read_file", "grep_files"]
```

### Configuration Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `enabled` | boolean | `false` | Enable Lua scripting support |
| `scripts_dir` | string | `null` | Optional directory for storing Lua scripts |
| `allow_file_io` | boolean | `false` | Allow file I/O operations (currently disabled in sandbox) |
| `allow_network` | boolean | `false` | Allow network operations (currently disabled in sandbox) |
| `max_execution_time_ms` | number | `5000` | Maximum script execution time in milliseconds |
| `max_memory_bytes` | number | `0` | Maximum memory usage (0 = unlimited, not yet enforced) |

## Usage

### Tool Invocation

The `lua_execute` tool accepts two parameters:

```json
{
  "script": "return args.x + args.y",
  "args": {
    "x": 10,
    "y": 20
  }
}
```

### Lua Script Structure

Scripts have access to a global `args` variable:

```lua
-- Check if required arguments are present
if not args or not args.input then
    return {error = "Missing required 'input' parameter"}
end

-- Process the input
local result = process(args.input)

-- Return the result (automatically converted to JSON)
return result
```

### Data Type Conversion

#### Lua to JSON

| Lua Type | JSON Type | Notes |
|----------|-----------|-------|
| `nil` | `null` | |
| `boolean` | `boolean` | |
| `number` | `number` | Integers and floats supported |
| `string` | `string` | Must be valid UTF-8 |
| `table` (array) | `array` | Consecutive integer keys starting from 1 |
| `table` (map) | `object` | String or number keys |
| `function` | Error | Functions cannot be returned |
| `thread` | Error | Threads cannot be returned |
| `userdata` | Error | Userdata cannot be returned |

#### JSON to Lua

| JSON Type | Lua Type | Notes |
|-----------|----------|-------|
| `null` | `nil` | |
| `boolean` | `boolean` | |
| `number` | `number` or `integer` | Integers preserved when possible |
| `string` | `string` | |
| `array` | `table` | Keys are consecutive integers starting from 1 |
| `object` | `table` | String keys |

## Security

### Sandbox Restrictions

The Lua runtime enforces several security restrictions:

1. **Disabled Libraries**:
   - `io` - File I/O operations
   - `os` - Operating system operations
   - `debug` - Debugging facilities
   - `dofile` - Loading external files
   - `loadfile` - Loading external files

2. **Timeout Protection**:
   - Scripts are limited by `max_execution_time_ms`
   - Default timeout: 5000ms (5 seconds)
   - Non-terminating loops will time out

3. **Isolated Environment**:
   - Each script runs in a fresh Lua environment
   - No shared state between executions
   - Global `_SANDBOXED` flag set to `true`

### Safe Lua Features

The following Lua features remain available:

- String manipulation (`string.*`)
- Table operations (`table.*`)
- Mathematical functions (`math.*`)
- Type checking and conversion
- Control structures (if, for, while)
- Function definitions
- Local variables

## Examples

### Example 1: Simple Calculation

```lua
-- Calculate compound interest
local principal = args.principal or 1000
local rate = args.rate or 0.05
local years = args.years or 10

local amount = principal * math.pow(1 + rate, years)
local interest = amount - principal

return {
    principal = principal,
    rate = rate,
    years = years,
    final_amount = amount,
    interest_earned = interest
}
```

### Example 2: Data Transformation

```lua
-- Transform array of objects
if not args or not args.users then
    return {error = "No users provided"}
end

local active_users = {}
for _, user in ipairs(args.users) do
    if user.active then
        table.insert(active_users, {
            id = user.id,
            name = string.upper(user.name),
            email = string.lower(user.email)
        })
    end
end

return {
    total = #args.users,
    active = #active_users,
    users = active_users
}
```

### Example 3: Text Analysis

```lua
-- Analyze text and return statistics
local text = args.text or ""

-- Count words
local words = 0
for _ in string.gmatch(text, "%S+") do
    words = words + 1
end

-- Count lines
local lines = 1
for _ in string.gmatch(text, "\n") do
    lines = lines + 1
end

return {
    characters = #text,
    words = words,
    lines = lines,
    uppercase = string.upper(text),
    first_100_chars = string.sub(text, 1, 100)
}
```

## Error Handling

### Script Errors

Errors in Lua scripts are caught and returned to the caller:

```json
{
  "error": "Lua execution error: attempt to perform arithmetic on a nil value"
}
```

### Timeout Errors

Scripts exceeding the timeout limit return:

```json
{
  "error": "Lua timeout after 5000ms"
}
```

### Conversion Errors

Invalid data type conversions return:

```json
{
  "error": "Failed to convert value: Functions cannot be converted to JSON"
}
```

## Best Practices

### 1. Validate Input

Always check that required arguments are present:

```lua
if not args or not args.required_param then
    return {error = "Missing required parameter: required_param"}
end
```

### 2. Handle Edge Cases

Consider empty inputs, null values, and boundary conditions:

```lua
local items = args.items or {}
if #items == 0 then
    return {count = 0, result = {}}
end
```

### 3. Return Structured Data

Use consistent return formats:

```lua
-- Good: Structured response
return {
    success = true,
    data = result,
    count = #result
}

-- Avoid: Bare values or inconsistent formats
return result  -- Less informative
```

### 4. Avoid Infinite Loops

The timeout will catch infinite loops, but it's better to prevent them:

```lua
-- Bad: Potential infinite loop
-- while condition do
--     -- might never terminate
-- end

-- Good: Bounded iteration
local max_iterations = 1000
local count = 0
while condition and count < max_iterations do
    count = count + 1
    -- process
end
```

### 5. Use Descriptive Names

Make code readable:

```lua
-- Good
local total_price = calculate_total(items)

-- Avoid
local t = calc(i)
```

## Limitations

Current limitations of the Lua API:

1. **No File I/O**: Cannot read or write files (even if `allow_file_io = true`)
2. **No Network Access**: Cannot make HTTP requests or network connections
3. **No State Persistence**: Each execution is independent
4. **No Coroutines**: Lua coroutines not supported
5. **Memory Limits**: Memory limit configuration not yet enforced
6. **No Module System**: Cannot use `require()` to load modules

## Future Enhancements

Planned features for future releases:

1. **Event Hooks**: Register Lua functions to handle events
   ```lua
   function on_tool_complete(event)
       -- Handle tool completion
   end
   ```

2. **Script Loading**: Load scripts from configured directory
   ```lua
   -- ~/.codex/lua-scripts/my_script.lua
   lua_execute(script_name="my_script", args={...})
   ```

3. **Shared Libraries**: Common utility functions
   ```lua
   local utils = require("codex.utils")
   utils.validate_email(args.email)
   ```

4. **Incremental Execution**: Support for generator-style scripts
   ```lua
   for i = 1, 10 do
       yield({step = i, progress = i * 10})
   end
   ```

## Troubleshooting

### Tool Not Available

If `lua_execute` is not available:

1. Check that Lua is enabled in config:
   ```toml
   [lua]
   enabled = true
   ```

2. Add `lua_execute` to experimental tools:
   ```toml
   [tools]
   experimental_supported_tools = ["lua_execute"]
   ```

3. Restart codex-rs

### Script Timeouts

If scripts timeout frequently:

1. Increase timeout limit:
   ```toml
   [lua]
   max_execution_time_ms = 10000  # 10 seconds
   ```

2. Optimize script logic
3. Break large operations into smaller chunks

### Conversion Errors

If you get conversion errors:

1. Check return value types
2. Avoid returning functions or userdata
3. Ensure strings are valid UTF-8
4. Use structured tables for complex data

## API Reference

### Global Variables

- `args` - Input parameters passed to the script (table or nil)
- `_SANDBOXED` - Boolean flag indicating sandboxed execution (always true)

### Available Standard Libraries

- `string.*` - String manipulation functions
- `table.*` - Table manipulation functions
- `math.*` - Mathematical functions
- `tonumber()` - Convert value to number
- `tostring()` - Convert value to string
- `type()` - Get value type
- `ipairs()` - Iterate array-like tables
- `pairs()` - Iterate all table entries
- `next()` - Get next table entry
- `select()` - Return selected arguments
- `assert()` - Assertion checking
- `error()` - Raise an error
- `pcall()` - Protected call
- `xpcall()` - Extended protected call

### Disabled Features

- `io.*` - File I/O
- `os.*` - Operating system
- `debug.*` - Debugging
- `dofile()` - Load file
- `loadfile()` - Load file
- `require()` - Module system
- `package.*` - Package management

## Contributing

To extend the Lua API:

1. **Add Runtime Features**: Modify `lua-runtime/src/lib.rs`
2. **Add Tool Features**: Modify `core/src/tools/handlers/lua.rs`
3. **Add Configuration**: Update `core/src/config/types.rs`
4. **Add Tests**: Add tests to verify new functionality
5. **Update Documentation**: Keep this document current

## References

- [Lua 5.4 Reference Manual](https://www.lua.org/manual/5.4/)
- [mlua Documentation](https://docs.rs/mlua/)
- [codex-rs Architecture](./architecture.md)
