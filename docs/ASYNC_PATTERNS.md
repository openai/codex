# Async Patterns and Error Handling in Codex Kotlin Native

This document describes the async/await patterns and error handling approach used in the Kotlin Native port of Codex.

## Overview

The Kotlin Native implementation uses `kotlinx.coroutines` to provide async/await functionality that maps to Rust's `tokio` async runtime.

## Mapping from Rust to Kotlin

### Async/Await

| Rust | Kotlin | Notes |
|------|--------|-------|
| `async fn foo()` | `suspend fun foo()` | Suspend functions are the Kotlin equivalent of async functions |
| `tokio::spawn(async { ... })` | `launch { ... }` | Background coroutine launch |
| `async_channel::Sender<T>` | `Channel<T>` | Channel-based communication |
| `async_channel::Receiver<T>` | `Channel<T>` | Receiving from channels |
| `tokio::sync::Mutex` | `kotlinx.coroutines.sync.Mutex` | Async-aware mutex |
| `tokio::sync::RwLock` | Not used | Using Mutex for simplicity |
| `Future` | `Deferred<T>` | Representing future values |
| `Stream` | `Flow<T>` | Reactive streams |

### Error Handling

The codebase uses a Result-based pattern similar to Rust:

```kotlin
sealed class CodexResult<out T> {
    data class Success<T>(val value: T) : CodexResult<T>()
    data class Failure(val error: CodexError) : CodexResult<Nothing>()
}

sealed class CodexError {
    data class Fatal(val message: String) : CodexError()
    data class Io(val message: String) : CodexError()
    data class Http(val statusCode: Int, val message: String?) : CodexError()
    // ... more error types
}
```

## Core Components

### 1. Codex Class

The main entry point using Channel-based queues:

```kotlin
class Codex(
    private val submissionChannel: Channel<Submission>,
    private val eventChannel: Channel<Event>
) {
    suspend fun submit(op: Op): CodexResult<String>
    suspend fun nextEvent(): CodexResult<Event>
    fun eventFlow(): Flow<Event>
}
```

**Key Points:**
- Submissions use bounded channel (capacity 64)
- Events use unbounded channel
- Flow API for reactive event consumption

### 2. Session Management

Session handles turn execution and state:

```kotlin
class Session {
    suspend fun newTurn(updates: SessionSettingsUpdate): TurnContext
    suspend fun sendEvent(turnContext: TurnContext, msg: EventMsg)
    suspend fun recordConversationItems(turnContext: TurnContext, items: List<ResponseItem>)
    suspend fun abortAllTasks(reason: String)
}
```

**Key Points:**
- All mutable state protected by `Mutex`
- Single active turn at a time
- Cancellation via `CancellationToken`

### 3. HTTP Client

Ktor-based HTTP client with retry logic:

```kotlin
class CodexHttpClient {
    fun streamPrompt(
        model: String,
        prompt: ResponsesPrompt,
        options: ResponsesOptions
    ): Flow<CodexResult<ResponseEvent>>
}
```

**Key Points:**
- Returns Flow of results for streaming
- Exponential backoff retry logic
- Authentication via AuthManager
- SSE parsing for event stream

### 4. SSE Parser

Parses server-sent events from API responses:

```kotlin
class SseParser {
    fun parse(sseData: String): List<ResponseEvent>
}
```

**Key Points:**
- Handles standard SSE format
- Robust error handling (skips invalid events)
- Supports all Codex event types

## Error Handling Patterns

### 1. Result-Based Error Handling

Prefer Result types over exceptions:

```kotlin
suspend fun doWork(): CodexResult<String> {
    return try {
        val result = performWork()
        CodexResult.success(result)
    } catch (e: Exception) {
        CodexResult.failure(CodexError.Io(e.message ?: "Unknown error"))
    }
}

// Usage
when (val result = doWork()) {
    is CodexResult.Success -> println(result.value)
    is CodexResult.Failure -> println("Error: ${result.error}")
}
```

### 2. Error Propagation

Use map/flatMap for error propagation:

```kotlin
suspend fun processData(): CodexResult<ProcessedData> {
    return fetchData()
        .map { data -> transform(data) }
        .flatMap { transformed -> validate(transformed) }
}
```

### 3. Retry Logic

HTTP client implements exponential backoff:

```kotlin
private fun calculateBackoff(retryCount: Int): Long {
    val baseDelay = 1000L // 1 second
    val maxDelay = 16000L // 16 seconds
    val delay = baseDelay * (1L shl (retryCount - 1))
    return minOf(delay, maxDelay)
}
```

## Concurrency Patterns

### 1. Channel-Based Communication

Submission/Event queues:

```kotlin
val submissionChannel = Channel<Submission>(64) // Bounded
val eventChannel = Channel<Event>(Channel.UNLIMITED) // Unbounded

// Send
submissionChannel.send(submission)

// Receive
val event = eventChannel.receive()

// As Flow
eventChannel.receiveAsFlow().collect { event ->
    handleEvent(event)
}
```

### 2. Mutex for State Protection

All mutable state protected:

```kotlin
private val mutex = Mutex()
private var state: SessionState = initialState

suspend fun updateState(update: StateUpdate) {
    mutex.withLock {
        state = state.apply(update)
    }
}
```

### 3. Cancellation

Using CancellationToken for task cancellation:

```kotlin
class CancellationToken {
    private var cancelled = false
    private val mutex = Mutex()
    
    suspend fun cancel() {
        mutex.withLock { cancelled = true }
    }
    
    suspend fun isCancelled(): Boolean {
        return mutex.withLock { cancelled }
    }
}
```

## Testing Patterns

### 1. Coroutine Testing

Use `runTest` for testing suspend functions:

```kotlin
@Test
fun testAsync() = runTest {
    val result = someAsyncOperation()
    assertEquals(expected, result)
}
```

### 2. Channel Testing

Test channel-based communication:

```kotlin
@Test
fun testChannelCommunication() = runTest {
    val channel = Channel<String>(10)
    
    launch {
        channel.send("test")
    }
    
    val received = channel.receive()
    assertEquals("test", received)
}
```

### 3. Flow Testing

Test Flow emissions:

```kotlin
@Test
fun testFlow() = runTest {
    val flow = flowOf(1, 2, 3)
    val results = flow.toList()
    assertEquals(listOf(1, 2, 3), results)
}
```

## Best Practices

1. **Always use suspend functions for async operations**
   - Don't block threads in Kotlin Native

2. **Prefer Result types over exceptions**
   - Makes error handling explicit
   - Easier to compose operations

3. **Protect mutable state with Mutex**
   - Prevents data races
   - Ensures thread safety

4. **Use Flow for streaming data**
   - Natural fit for SSE streams
   - Composable operators

5. **Implement proper cancellation**
   - Clean up resources
   - Graceful shutdown

6. **Test async code with runTest**
   - Deterministic testing
   - Automatic time control

## Platform Considerations

### Kotlin/Native Specifics

1. **No reflection at runtime**
   - All serialization must be compile-time

2. **Memory model**
   - New memory model (since Kotlin 1.7.20)
   - Shared state requires synchronization

3. **Threading**
   - Coroutines handle thread management
   - Don't create threads manually

4. **Interop with C**
   - Use for platform-specific code
   - Wrap in suspend functions where needed

## Migration from Rust

When converting Rust code to Kotlin:

1. Replace `async fn` with `suspend fun`
2. Replace `tokio::spawn` with `launch`
3. Replace `async_channel` with `Channel`
4. Replace `tokio::sync::Mutex` with `kotlinx.coroutines.sync.Mutex`
5. Replace `Result<T, E>` with `CodexResult<T>`
6. Replace `Stream` with `Flow`
7. Replace `.await` with just calling suspend function

Example conversion:

```rust
// Rust
async fn process_data() -> Result<String, CodexErr> {
    let data = fetch_data().await?;
    Ok(transform(data))
}
```

```kotlin
// Kotlin
suspend fun processData(): CodexResult<String> {
    return when (val result = fetchData()) {
        is CodexResult.Success -> {
            CodexResult.success(transform(result.value))
        }
        is CodexResult.Failure -> result
    }
}
```

## Future Enhancements

1. **Structured concurrency**
   - Use coroutineScope for automatic cancellation
   - Better error handling across coroutines

2. **Flow operators**
   - More sophisticated stream processing
   - Backpressure handling

3. **Timeout handling**
   - withTimeout for operations
   - Configurable timeouts per operation

4. **Performance monitoring**
   - Coroutine debugging
   - Performance metrics collection