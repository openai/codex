# TODO Resolution Report

## Date: December 1, 2025

## Summary
Resolved key TODO items in ApiError.kt by porting TransportError types from Rust.

## Resolved TODOs

### 1. ✅ ApiError.kt - TransportError Integration

**Original TODOs:**
```kotlin
// TODO: Map TransportError and StatusCode once transport/types are ported.
data class Transport(val message: String) : ApiError() // TODO: replace with TransportError type
```

**Resolution:**
- Created `/src/nativeMain/kotlin/ai/solace/coder/client/error/TransportError.kt`
- Ported complete `TransportError` enum from `codex-rs/codex-client/src/error.rs`
- Ported `StreamError` enum as well
- Updated `ApiError.Transport` to use proper `TransportError` type
- Replaced `Int` status with `HttpStatusCode` from Ktor

**Changes Made:**

#### New File: TransportError.kt
```kotlin
sealed class TransportError : Exception() {
    data class Http(
        val status: HttpStatusCode,
        val headers: Headers? = null,
        val body: String? = null
    ) : TransportError()
    
    object RetryLimit : TransportError()
    object Timeout : TransportError()
    data class Network(override val message: String) : TransportError()
    data class Build(override val message: String) : TransportError()
}

sealed class StreamError : Exception() {
    data class Stream(override val message: String) : StreamError()
    object Timeout : StreamError()
}
```

#### Updated: ApiError.kt
```kotlin
// Before:
data class Transport(val message: String) : ApiError()
data class Api(val status: Int, val message: String) : ApiError()

// After:
data class Transport(val error: TransportError) : ApiError()
data class Api(val status: HttpStatusCode, override val message: String) : ApiError()
```

**Benefits:**
1. ✅ Proper type safety - can pattern match on specific transport errors
2. ✅ Maintains full error context (headers, body, status codes)
3. ✅ API compatibility with Rust implementation
4. ✅ Uses Ktor's HttpStatusCode type for better integration

## Compilation Status

✅ **Zero Errors**
- Only expected "never used" warnings for API types
- All types compile successfully
- Ready for use in API client code

## API Usage Examples

### Creating Transport Errors

```kotlin
// HTTP error
val httpError = ApiError.Transport(
    TransportError.Http(
        status = HttpStatusCode.BadRequest,
        headers = null,
        body = "Invalid request"
    )
)

// Network error
val networkError = ApiError.Transport(
    TransportError.Network("Connection refused")
)

// Timeout
val timeoutError = ApiError.Transport(TransportError.Timeout)

// Retry limit
val retryError = ApiError.Transport(TransportError.RetryLimit)
```

### Pattern Matching

```kotlin
when (error) {
    is ApiError.Transport -> when (error.error) {
        is TransportError.Http -> {
            println("HTTP ${error.error.status}: ${error.error.body}")
        }
        is TransportError.Network -> {
            println("Network error: ${error.error.message}")
        }
        TransportError.Timeout -> {
            println("Request timed out")
        }
        TransportError.RetryLimit -> {
            println("Max retries exceeded")
        }
        is TransportError.Build -> {
            println("Request build error: ${error.error.message}")
        }
    }
    is ApiError.Api -> {
        println("API error ${error.status.value}: ${error.message}")
    }
    is ApiError.RateLimit -> {
        println("Rate limited: ${error.message}")
    }
    // ... other cases
}
```

### Converting from TransportError

```kotlin
fun handleTransportError(transportError: TransportError): ApiError {
    return ApiError.Transport(transportError)
}
```

## Remaining TODOs in Related Files

### High Priority (Blocking API functionality)

1. **Auth.kt - JWT Parsing** (Line 733)
   ```kotlin
   // TODO: Implement JWT parsing
   // Needed for: Token validation, extracting claims
   ```

2. **Chat.kt / Responses.kt - SSE Streaming** (Lines 62-64)
   ```kotlin
   // TODO: Implement spawnChatStream once SSE parsing is ported
   // TODO: Implement spawnResponsesStream once SSE parsing is ported
   ```

3. **Auth.kt - Environment Variables** (Line 923)
   ```kotlin
   // TODO: Implement platform-specific environment variable reading
   ```

### Medium Priority (Platform features)

4. **Storage.kt - Unix File Permissions** (Line 108)
   ```kotlin
   // TODO: Set Unix file permissions to 0600 (owner read/write only)
   ```

5. **Storage.kt - Path Canonicalization** (Line 270)
   ```kotlin
   // TODO: Implement proper path canonicalization
   ```

6. **Storage.kt - Keychain Access** (Line 344)
   ```kotlin
   // TODO: Implement platform-specific keychain access:
   // - macOS: Security framework
   // - Linux: libsecret
   // - Windows: Credential Manager
   ```

### Low Priority (Testing & Optimization)

7. **Storage.kt - Mock Keychain** (Lines 467-496)
   ```kotlin
   // TODO: Implement mock keychain store for unit tests
   ```

8. **Various telemetry TODOs** in ModelClient.kt
   - Lines 457-492: Telemetry implementation stubs

## Files Modified

1. ✅ Created: `src/nativeMain/kotlin/ai/solace/coder/client/error/TransportError.kt` (67 lines)
2. ✅ Updated: `src/nativeMain/kotlin/ai/solace/coder/api/error/ApiError.kt` (72 lines)

## Testing Recommendations

### Unit Tests Needed

1. **TransportError Creation**
   ```kotlin
   @Test
   fun testHttpTransportError() {
       val error = TransportError.Http(
           status = HttpStatusCode.NotFound,
           body = "Resource not found"
       )
       assertEquals("http 404: Resource not found", error.message)
   }
   ```

2. **ApiError Wrapping**
   ```kotlin
   @Test
   fun testApiErrorWrapsTransportError() {
       val transportError = TransportError.Network("Connection failed")
       val apiError = ApiError.Transport(transportError)
       assertEquals("Connection failed", apiError.message)
   }
   ```

3. **Pattern Matching**
   ```kotlin
   @Test
   fun testPatternMatchingOnTransportError() {
       val error: ApiError = ApiError.Transport(TransportError.Timeout)
       val result = when (error) {
           is ApiError.Transport -> when (error.error) {
               TransportError.Timeout -> "timeout detected"
               else -> "other error"
           }
           else -> "not transport"
       }
       assertEquals("timeout detected", result)
   }
   ```

## Next Steps

1. ✅ **Complete** - TransportError types ported
2. ⏭️ **Next** - Port SSE (Server-Sent Events) parsing for streaming support
3. ⏭️ **Next** - Implement JWT parsing in Auth.kt
4. ⏭️ **Next** - Add environment variable reading (platform-specific)

## References

- **Rust Source**: `codex-rs/codex-client/src/error.rs`
- **Rust Source**: `codex-rs/codex-api/src/error.rs`
- **Kotlin Docs**: https://ktor.io/docs/status-codes.html (HttpStatusCode)
- **Kotlin Docs**: https://ktor.io/docs/headers.html (Headers)

