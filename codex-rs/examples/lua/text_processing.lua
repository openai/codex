-- Example: Text Processing
-- This script demonstrates string manipulation and text analysis

-- Input: args.text (string to analyze)
-- Example: {"text": "Hello World! This is a test."}

if not args or not args.text then
    return {error = "No text provided. Pass text as args.text"}
end

local text = args.text

-- Count words
local word_count = 0
for word in string.gmatch(text, "%S+") do
    word_count = word_count + 1
end

-- Count sentences (simple heuristic)
local sentence_count = 0
for _ in string.gmatch(text, "[.!?]+") do
    sentence_count = sentence_count + 1
end

-- Extract words and count frequency
local words = {}
local word_freq = {}
for word in string.gmatch(text, "%w+") do
    local lower = string.lower(word)
    table.insert(words, lower)
    word_freq[lower] = (word_freq[lower] or 0) + 1
end

-- Find most common word
local max_count = 0
local most_common = nil
for word, count in pairs(word_freq) do
    if count > max_count then
        max_count = count
        most_common = word
    end
end

return {
    character_count = #text,
    word_count = word_count,
    sentence_count = sentence_count > 0 and sentence_count or 1,
    most_common_word = most_common,
    most_common_count = max_count,
    word_frequencies = word_freq,
    reversed = string.reverse(text),
    uppercase = string.upper(text)
}
