# Getting Started with Lua Scripting in codex-rs

## Quick Start

### 1. Enable Lua Scripting

Add to your `~/.codex/config.toml`:

```toml
[lua]
enabled = true

[tools]
experimental_supported_tools = ["lua_execute"]
```

### 2. Try a Simple Script

The `lua_execute` tool allows you to run Lua code directly:

```lua
-- Simple calculation
return 1 + 1
```

Result: `2`

### 3. Pass Arguments

Use the `args` parameter to pass data:

```lua
-- With arguments
return args.x + args.y
```

Call with: `{script: "return args.x + args.y", args: {x: 10, y: 20}}`

Result: `30`

### 4. Try an Example

Use one of the provided examples:

```bash
# From the examples/lua directory:
cat transform_data.lua
```

Then invoke with appropriate arguments.

## What You Can Do

- **Data Transformation**: Process and reshape JSON data
- **Calculations**: Perform custom mathematical operations
- **Text Processing**: Analyze and manipulate strings
- **Filtering & Sorting**: Process arrays with custom logic
- **Business Rules**: Implement domain-specific logic

## What's Not Allowed

Due to sandboxing, you cannot:

- Access the filesystem (`io` library disabled)
- Make network requests (`os` library disabled)
- Execute system commands
- Use debugging features

## Next Steps

1. Read the examples in this directory
2. Check out the full documentation at `docs/lua-scripting.md`
3. Write your own scripts
4. Report issues at https://github.com/anthropics/claude-code/issues

## Example Use Cases

### Calculate Fibonacci Numbers

```lua
local function fib(n)
    if n <= 1 then return n end
    return fib(n-1) + fib(n-2)
end

local n = args.n or 10
local result = {}
for i = 0, n do
    table.insert(result, fib(i))
end

return {sequence = result}
```

### Aggregate Data

```lua
local data = args.sales or {}
local total = 0
local by_region = {}

for _, sale in ipairs(data) do
    total = total + sale.amount
    local region = sale.region
    by_region[region] = (by_region[region] or 0) + sale.amount
end

return {
    total_sales = total,
    by_region = by_region,
    count = #data
}
```

### Validate Email Format

```lua
local function is_valid_email(email)
    return string.match(email, "^[%w%.%-_]+@[%w%.%-_]+%.[a-zA-Z]+$") ~= nil
end

local emails = args.emails or {}
local valid = {}
local invalid = {}

for _, email in ipairs(emails) do
    if is_valid_email(email) then
        table.insert(valid, email)
    else
        table.insert(invalid, email)
    end
end

return {
    valid_count = #valid,
    invalid_count = #invalid,
    valid = valid,
    invalid = invalid
}
```

## Tips

1. **Start Simple**: Begin with basic scripts and gradually add complexity
2. **Test Incrementally**: Test each function as you build it
3. **Handle Errors**: Always validate input and handle edge cases
4. **Return Structured Data**: Use tables with meaningful keys
5. **Read the Docs**: The full documentation has many more examples

## Help

For more information:
- Full documentation: `docs/lua-scripting.md`
- Examples: This directory
- Lua reference: https://www.lua.org/manual/5.4/
