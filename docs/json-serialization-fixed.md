# JSON Serialization Types Fixed

## Summary

Replaced all `Any` placeholder types with proper `JsonElement` from kotlinx.serialization in the api package.

## Changes Made

### File: `api/common/Common.kt`

✅ **Updated Prompt data class**:
```kotlin
// Before:
val tools: List<Any>, // TODO: proper type for tool definitions
val outputSchema: Any? // TODO: replace with a JSON Value type

// After:
val tools: List<JsonElement>,
val outputSchema: JsonElement?
```

✅ **Updated TextFormat data class**:
```kotlin
// Before:
val schema: Any, // TODO: JSON Value

// After:
val schema: JsonElement,
```

✅ **Updated ResponsesApiRequest data class**:
```kotlin
// Before:
val tools: List<Any>, // TODO: JSON Value

// After:
val tools: List<JsonElement>,
```

✅ **Updated createTextParamForRequest function**:
```kotlin
// Before:
outputSchema: Any?, // TODO: JSON Value

// After:
outputSchema: JsonElement?,
```

## Rationale

- `JsonElement` is the proper kotlinx.serialization type for representing any JSON value
- It's already imported and used throughout the api package
- Provides type safety while allowing flexibility for tool definitions and schemas
- Matches the JSON handling patterns used elsewhere in the codebase

## Compilation Status

✅ All files compile successfully
✅ No type errors
✅ Only "never used" warnings remain (expected)

## Benefits

1. **Type Safety**: `JsonElement` is type-safe compared to `Any`
2. **Consistency**: Uses the same JSON type throughout the api package
3. **Serialization Support**: Works seamlessly with kotlinx.serialization
4. **API Clarity**: Clear that these fields contain JSON data
5. **IDE Support**: Better autocomplete and type checking

## Related TODOs Resolved

- ✅ "TODO: proper type for tool definitions (JSON schema)"
- ✅ "TODO: replace with a JSON Value type once available"
- ✅ "TODO: JSON Value" (multiple occurrences)

All serialization-related TODOs in Common.kt have been resolved.

