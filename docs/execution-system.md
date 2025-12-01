# Execution System with Platform-Specific Sandboxing

This document describes the Kotlin Native implementation of the execution system with platform-specific sandboxing, converted from the original Rust implementation.

## Overview

The execution system provides secure command execution with platform-specific sandboxing capabilities. It supports:

- Process execution with timeout and streaming output
- Platform-specific sandboxing (Landlock on Linux, Seatbelt on macOS, Job Objects on Windows)
- Shell detection and command parsing
- Comprehensive error handling and sandbox violation detection

## Architecture

### Core Components

#### 1. ProcessExecutor (`ai.solace.coder.exec.process.ProcessExecutor`)

Main class responsible for executing commands with sandboxing and streaming support.

**Key Features:**
- Command execution with configurable timeouts
- Streaming stdout/stderr output
- Sandbox policy enforcement
- Process group management for cleanup

**Usage:**
```kotlin
val executor = ProcessExecutor()
val params = ExecParams(
    command = listOf("echo", "hello"),
    cwd = "/tmp",
    expiration = ExecExpiration.DefaultTimeout,
    env = mapOf("PATH" to "/usr/bin")
)
val result = executor.execute(params, sandboxPolicy, "/tmp")
```

#### 2. ShellDetector (`ai.solace.coder.exec.shell.ShellDetector`)

Detects and manages shell configurations across platforms.

**Supported Shells:**
- Unix: bash, zsh, sh
- Windows: PowerShell, cmd
- Cross-platform: PowerShell (pwsh)

**Key Methods:**
- `defaultUserShell()` - Gets the user's default shell
- `getShell(shellType, path)` - Gets a specific shell with optional path
- `detectShellType(shellPath)` - Detects shell type from path

#### 3. CommandParser (`ai.solace.coder.exec.shell.CommandParser`)

Parses shell command strings with proper quoting and escaping support.

**Features:**
- Handles single and double quotes
- Backslash escaping
- Shell-specific argument joining
- Built-in command detection

#### 4. SandboxManager (`ai.solace.coder.exec.sandbox.SandboxManager`)

Manages sandbox policy application and transformation.

**Responsibilities:**
- Policy transformation for platform-specific execution
- Sandbox type selection
- Command argument preparation

## Platform-Specific Implementations

### Linux - Landlock Sandbox

**Location:** `src/linuxX64Main/kotlin/ai/solace/coder/exec/sandbox/LinuxSandbox.kt`

**Features:**
- Landlock ABI v4 support
- Filesystem access control (read/write/execute)
- Network access restrictions
- Path-based sandboxing

**C Interop:** `src/linuxX64Main/resources/landlock.def`

**Key Constants:**
```kotlin
LANDLOCK_ACCESS_FS_READ = 0x01 | 0x08  // READ_FILE | READ_DIR
LANDLOCK_ACCESS_FS_WRITE = 0x02 | 0x20 | 0x10 | 0x80  // WRITE_FILE | REMOVE_FILE | REMOVE_DIR | TRUNCATE
LANDLOCK_ACCESS_NET_BIND_TCP = 0x01
LANDLOCK_ACCESS_NET_CONNECT_TCP = 0x02
```

**Requirements:**
- Linux kernel 5.13+ for full Landlock support
- Landlock ABI compatibility

### macOS - Seatbelt Sandbox

**Location:** `src/macosArm64Main/kotlin/ai/solace/coder/exec/sandbox/MacosSandbox.kt`

**Features:**
- Dynamic Seatbelt profile generation
- Path-based file access control
- Network access management
- macOS-specific directory parameters

**C Interop:** `src/macosArm64Main/resources/sandbox.def`

**Profile Structure:**
```
(version 1)
(deny default)
(allow process-fork)
(allow process-exec)
(allow file-read-metadata)
... (additional rules based on policy)
```

**Requirements:**
- macOS 10.6+ (Seatbelt introduced)
- `/usr/bin/sandbox-exec` availability

### Windows - Job Objects Sandbox

**Location:** `src/mingwX64Main/kotlin/ai/solace/coder/exec/sandbox/WindowsSandbox.kt`

**Features:**
- Job Object-based process containment
- UI restrictions
- Security token limitations
- Network access control via Windows Firewall integration

**Key Constants:**
```kotlin
JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE = 0x2000
JOB_OBJECT_UILIMIT_ALL = 0x000003FF
JOB_OBJECT_SECURITY_NO_ADMIN = 0x00000001
JOB_OBJECT_SECURITY_RESTRICTED_TOKEN = 0x00000002
```

**Requirements:**
- Windows Vista+ (Job Objects enhanced)
- Administrative privileges for some restrictions

## Sandbox Policies

### Policy Types

#### 1. DangerFullAccess
No sandboxing restrictions - for trusted operations only.

#### 2. WorkspaceWrite
- File access limited to specified writable roots
- Configurable network access
- Current working directory always included

#### 3. ReadOnly
- Read-only file system access
- No network access
- Suitable for inspection operations

#### 4. Custom
- Fine-grained rule-based control
- Individual file read/write rules
- Network access rules
- Process execution restrictions

### Policy Transformation

The `SandboxManager` transforms high-level policies into platform-specific configurations:

```kotlin
val execEnv = sandboxManager.transform(
    spec = CommandSpec(...),
    policy = SandboxPolicy.WorkspaceWrite(...),
    sandboxPolicyCwd = "/workspace"
)
```

## Error Handling

### Error Types

#### CodexError.SandboxError
- `Unsupported` - Sandbox not available on platform
- `CreationFailed` - Failed to create sandbox resources
- `ApplicationFailed` - Failed to apply sandbox policy
- `Timeout` - Command execution timed out
- `Denied` - Sandbox violation detected

#### Sandbox Violation Detection

The system detects sandbox violations through:
- Exit code analysis
- Output keyword scanning
- Platform-specific error patterns

**Detection Keywords:**
- "operation not permitted"
- "permission denied"
- "read-only file system"
- "seccomp", "sandbox", "landlock"

## Platform Process Handling

### Expect/Actual Pattern

Platform-specific implementations use Kotlin's expect/actual pattern:

```kotlin
// Common interface
expect class ProcessHandle {
    val stdout: ByteReadChannel?
    val stderr: ByteReadChannel?
    suspend fun onAwait(): Int
}

// Platform-specific implementation
actual class ProcessHandle { ... }
```

### Process Creation

Each platform implements process creation with:
- Pipe creation for stdout/stderr
- Fork/exec (Unix) or CreateProcess (Windows)
- Environment variable setup
- Working directory configuration

## Streaming and Output

### Output Streaming

The system provides real-time output streaming:
- Chunked reading from process stdout/stderr
- Event-based output deltas
- Aggregated output collection
- Truncation support for large outputs

### Flow-based Architecture

While the current implementation uses coroutines, the architecture supports Flow-based streaming for future enhancements:

```kotlin
// Planned Flow-based streaming
fun executeWithFlow(params: ExecParams): Flow<ExecOutputChunk>
```

## Testing

### Unit Tests

**Location:** `src/nativeTest/kotlin/ai/solace/coder/exec/process/ProcessExecutorTest.kt`

**Coverage:**
- Data structure validation
- Policy transformation
- Error handling
- Platform detection

### Integration Testing

Integration tests require:
- Native binary execution
- Platform-specific sandbox availability
- Temporary directory management

## Limitations and Considerations

### Current Limitations

1. **Coroutine IO**: The current implementation uses simplified coroutine IO due to Kotlin Native limitations
2. **C Interop**: Native function calls are placeholders requiring actual cinterop implementation
3. **PTY Support**: Pseudo-terminal support not yet implemented
4. **Flow Streaming**: Full Flow-based streaming pending coroutine IO enhancements

### Platform-Specific Limitations

#### Linux
- Requires Landlock-compatible kernel (5.13+)
- Limited to filesystem and network sandboxing
- No GUI application restrictions

#### macOS
- Dependent on Seatbelt policy complexity limits
- Some system calls may not be fully restrictable
- Dynamic profile generation overhead

#### Windows
- Job Object limitations on certain Windows versions
- Network restrictions require Windows Firewall integration
- Some legacy applications may not respect Job Object limits

### Security Considerations

1. **Sandbox Escape**: While sandboxing provides significant protection, determined attackers may find escape vectors
2. **Resource Limits**: Current implementation doesn't enforce resource limits (CPU, memory)
3. **Time-of-Check-Time-of-Use**: File path validation occurs before execution

## Future Enhancements

### Planned Features

1. **PTY Support**: Interactive terminal sessions with pseudo-terminals
2. **Resource Limits**: CPU, memory, and disk usage restrictions
3. **Flow Streaming**: Full reactive streaming with backpressure
4. **Advanced Policies**: More granular control over system calls
5. **Audit Logging**: Comprehensive execution logging for security analysis

### Performance Optimizations

1. **Process Pooling**: Reuse of sandboxed processes for repeated operations
2. **Policy Caching**: Cache compiled sandbox policies
3. **Async IO**: True asynchronous I/O for better performance

## Migration from Rust

### Key Differences

1. **Memory Management**: Kotlin Native uses automatic memory management vs Rust's manual approach
2. **Error Handling**: Kotlin uses exceptions vs Rust's Result types
3. **Concurrency**: Kotlin coroutines vs Rust's async/await
4. **Platform Abstraction**: Kotlin's expect/actual vs Rust's cfg attributes

### Compatibility

The Kotlin implementation maintains API compatibility with the Rust version:
- Same sandbox policy structure
- Equivalent error handling
- Identical execution semantics
- Compatible output formats

## Conclusion

The Kotlin Native execution system provides a robust, secure foundation for command execution with platform-specific sandboxing. While the current implementation includes placeholders for native interop, the architecture and design patterns support full feature parity with the original Rust implementation.

The modular design allows for incremental enhancement, with clear separation between platform-specific and cross-platform concerns. This approach ensures maintainability while providing the security isolation necessary for safe code execution.