-- Example: Data Transformation with Lua
-- This script demonstrates transforming JSON data

-- Input: args.data should be an array of objects
-- Example: {"data": [{"name": "Alice", "age": 30}, {"name": "Bob", "age": 25}]}

if not args or not args.data then
    return {error = "No data provided. Pass data as args.data"}
end

local result = {}

for i, person in ipairs(args.data) do
    table.insert(result, {
        id = i,
        name = string.upper(person.name),
        age_group = person.age < 30 and "young" or "adult",
        info = person.name .. " is " .. person.age .. " years old"
    })
end

return {
    count = #result,
    transformed = result
}
