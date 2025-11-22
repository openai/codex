-- Example: Filtering and Sorting Data
-- This script filters and sorts an array of numbers

-- Input: args.numbers (array of numbers), args.min_value (optional filter)
-- Example: {"numbers": [5, 2, 8, 1, 9, 3], "min_value": 3}

if not args or not args.numbers then
    return {error = "No numbers provided. Pass numbers as args.numbers"}
end

local numbers = args.numbers
local min_value = args.min_value or 0

-- Filter numbers greater than or equal to min_value
local filtered = {}
for _, num in ipairs(numbers) do
    if num >= min_value then
        table.insert(filtered, num)
    end
end

-- Sort in descending order
table.sort(filtered, function(a, b) return a > b end)

-- Calculate statistics
local sum = 0
for _, num in ipairs(filtered) do
    sum = sum + num
end

local avg = #filtered > 0 and (sum / #filtered) or 0

return {
    original_count = #numbers,
    filtered_count = #filtered,
    sorted = filtered,
    sum = sum,
    average = avg
}
