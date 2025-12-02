# Quick Reference: Rust to Kotlin Mapping

## Package Structure

```
Rust                                  Kotlin
----                                  ------
codex-rs/codex-api/                → ai.solace.coder.api/
  ├── src/auth.rs                  → auth/AuthProvider.kt, AuthHeaders.kt
  ├── src/error.rs                 → error/ApiError.kt
  ├── src/provider.rs              → provider/Provider.kt
  ├── src/common.rs                → common/Common.kt
  ├── src/requests/                → requests/
  ├── src/endpoint/                → endpoint/
  ├── src/sse/                     → sse/
  ├── src/telemetry.rs             → telemetry/Telemetry.kt
  └── src/rate_limits.rs           → ratelimits/RateLimits.kt

codex-rs/protocol/                 → ai.solace.coder.protocol/
  ├── src/account.rs               → Account.kt
  ├── src/models.rs                → Models.kt
  ├── src/protocol.rs              → Protocol.kt
  └── (11 other files)             → (11 other .kt files)

codex-rs/core/src/auth.rs          → ai.solace.coder.client.auth/AuthManager.kt
```

## Port-Lint Header Format

Always use full workspace path:
```kotlin
// port-lint: source codex-rs/<crate-name>/src/<file>.rs
package ai.solace.coder.<package>
```

Examples:
```kotlin
// port-lint: source codex-rs/codex-api/src/auth.rs
// port-lint: source codex-rs/protocol/src/models.rs
// port-lint: source codex-rs/core/src/auth.rs
```

## Type Mappings

### Basic Types
| Rust | Kotlin |
|------|--------|
| `String` | `String` |
| `&str` | `String` |
| `Vec<T>` | `List<T>` or `MutableList<T>` |
| `Option<T>` | `T?` |
| `Result<T, E>` | `Result<T>` or throw |
| `PathBuf` | `String` |
| `bool` | `Boolean` |
| `u64` | `ULong` |
| `i64` | `Long` |

### Complex Types
| Rust | Kotlin |
|------|--------|
| `pub enum Foo { A, B }` | `enum class Foo { A, B }` |
| `pub enum Foo { A { x: i64 } }` | `sealed class Foo { data class A(val x: Long) }` |
| `pub struct Foo { x: i64 }` | `data class Foo(val x: Long)` |
| `impl Foo { fn bar() }` | `class Foo { fun bar() }` |
| `impl Foo { fn new() -> Self }` | `companion object { fun new(): Foo }` |

### Serialization
| Rust | Kotlin |
|------|--------|
| `#[serde(tag = "type")]` | `@Serializable` on sealed class |
| `#[serde(rename = "foo")]` | `@SerialName("foo")` |
| `#[serde(rename_all = "snake_case")]` | `@SerialName` per variant |
| `#[serde(default)]` | Default parameter values |
| `#[serde(skip_serializing)]` | Omit or conditional |

## Naming Conventions

| Rust | Kotlin |
|------|--------|
| `snake_case` functions | `camelCase` functions |
| `SnakeCase` types | `PascalCase` types |
| `SCREAMING_SNAKE_CASE` constants | `SCREAMING_SNAKE_CASE` constants |

Examples:
```rust
// Rust
pub fn get_bearer_token() -> Option<String>
pub struct BearerToken { ... }
pub const MAX_RETRIES: u64 = 3;
```

```kotlin
// Kotlin
fun getBearerToken(): String?
data class BearerToken(...)
const val MAX_RETRIES: Long = 3
```

## Method Patterns

### Instance Methods
```rust
// Rust
impl Provider {
    pub fn url_for_path(&self, path: &str) -> String { ... }
}
```

```kotlin
// Kotlin
class Provider {
    fun urlForPath(path: String): String { ... }
}
```

### Static/Associated Functions
```rust
// Rust
impl SandboxPolicy {
    pub fn new_read_only_policy() -> Self { ... }
}
```

```kotlin
// Kotlin
class SandboxPolicy {
    companion object {
        fun newReadOnlyPolicy(): SandboxPolicy { ... }
    }
}
```

### Extension Traits
```rust
// Rust
pub trait AuthProviderExt {
    fn add_headers(&self, req: &mut Request);
}
```

```kotlin
// Kotlin
fun <T : AuthProvider> T.addHeaders(req: HttpRequestBuilder) { ... }
// Or as a standalone function:
fun addAuthHeaders(auth: AuthProvider, req: HttpRequestBuilder) { ... }
```

## Common Idioms

### Builder Pattern
```rust
// Rust
pub struct ChatRequestBuilder<'a> {
    model: &'a str,
    // ...
}

impl<'a> ChatRequestBuilder<'a> {
    pub fn new(model: &'a str) -> Self { ... }
    pub fn conversation_id(mut self, id: Option<String>) -> Self {
        self.conversation_id = id;
        self
    }
    pub fn build(self) -> Result<ChatRequest, Error> { ... }
}
```

```kotlin
// Kotlin
class ChatRequestBuilder(
    private val model: String,
    // ...
) {
    private var conversationId: String? = null
    
    fun conversationId(id: String?): ChatRequestBuilder {
        conversationId = id
        return this
    }
    
    fun build(provider: Provider): Result<ChatRequest> { ... }
}
```

### Error Handling
```rust
// Rust
pub enum ApiError {
    Transport(TransportError),
    Api { status: StatusCode, message: String },
}

impl From<TransportError> for ApiError {
    fn from(err: TransportError) -> Self {
        ApiError::Transport(err)
    }
}
```

```kotlin
// Kotlin
sealed class ApiError {
    data class Transport(val error: TransportError) : ApiError()
    data class Api(val status: Int, val message: String) : ApiError()
}

// No From trait needed - use constructors directly
```

## HTTP Integration

### Rust (codex-client)
```rust
use codex_client::Request;

let mut req = provider.build_request(Method::POST, "path");
req.headers.insert("key", "value");
req.body = Some(json_value);
```

### Kotlin (Ktor)
```kotlin
import io.ktor.client.request.*
import io.ktor.http.*

val req = provider.buildRequest(HttpMethod.Post, "path") {
    headers.append("key", "value")
    setBody(jsonValue.toString())
}
```

## Async/Concurrency

### Rust (tokio)
```rust
pub async fn stream_request(&self, req: Request) -> Result<ResponseStream, Error> {
    let response = self.transport.send(req).await?;
    Ok(response)
}
```

### Kotlin (coroutines)
```kotlin
suspend fun streamRequest(req: Request): Result<ResponseStream> {
    return try {
        val response = transport.send(req)
        Result.success(response)
    } catch (e: Exception) {
        Result.failure(e)
    }
}
```

## Quick Checklist for New Ports

- [ ] Create file with correct port-lint header
- [ ] Convert type names to Kotlin conventions
- [ ] Map all enum/struct variants
- [ ] Convert method names to camelCase
- [ ] Add @Serializable and @SerialName where needed
- [ ] Convert Option<T> to T?
- [ ] Convert Result<T, E> to Result<T> or throw
- [ ] Convert &self methods to instance methods
- [ ] Convert associated functions to companion object
- [ ] Add TODOs for external dependencies
- [ ] Verify compilation
- [ ] Check for "dishonest code" (oversimplified logic)
- [ ] Document any semantic differences

## Resources

- **Rust source**: `codex-rs/` directory
- **Kotlin target**: `src/nativeMain/kotlin/ai/solace/coder/` directory
- **Guidelines**: `ratatui-kotlin/CLAUDE.md` and `AGENTS.md`
- **Status docs**: `docs/codex-api-port-status.md`, `docs/protocol-port-verification.md`

