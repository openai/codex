# linux-sandbox Package Summary

## Purpose
Linux-specific sandboxing implementation providing secure execution environments using modern Linux security features. Ensures safe command execution with fine-grained access control.

## Key Components

### Landlock Integration
- **Filesystem Restrictions**: Path-based access control
- **Rule Management**: Dynamic permission rules
- **Sandbox Creation**: Landlock sandbox setup
- **Permission Enforcement**: Runtime access checks

### Seccomp Filtering
- **System Call Filtering**: Restrict syscall access
- **BPF Programs**: Berkeley Packet Filter rules
- **Allowlist/Denylist**: Syscall permission models
- **Architecture Support**: Multi-arch compatibility

### Process Isolation
- **Namespace Isolation**: Process namespacing
- **Resource Limits**: CPU/memory restrictions
- **Capability Dropping**: Reduce process privileges
- **UID/GID Management**: User isolation

## Main Functionality
1. **Sandbox Creation**: Set up secure execution environment
2. **Permission Management**: Configure access rights
3. **System Call Filtering**: Restrict dangerous operations
4. **Process Containment**: Isolate process execution
5. **Security Enforcement**: Apply security policies

## Dependencies
- `landlock`: Landlock LSM bindings
- `seccompiler`: Seccomp BPF compiler
- Linux kernel 5.13+ (for Landlock)
- Platform-specific syscall definitions

## Integration Points
- Used by `core` for secure execution
- Works with `execpolicy` for policy enforcement
- Platform-specific implementation
- Transparent to higher layers

## Security Features

### Filesystem Security
- **Path Restrictions**: Limit file access
- **Read-only Paths**: Prevent modifications
- **Execution Control**: Restrict binary execution
- **Mount Restrictions**: Control mount operations

### System Call Security
- **Network Isolation**: Block network syscalls
- **Process Control**: Restrict process operations
- **IPC Restrictions**: Limit inter-process communication
- **Device Access**: Control device operations

### Resource Protection
- **Memory Limits**: Prevent memory exhaustion
- **CPU Quotas**: Limit CPU usage
- **File Descriptor Limits**: Control FD usage
- **Process Limits**: Restrict subprocess creation

## Implementation Details

### Landlock Rules
- File read permissions
- File write permissions
- Directory traversal
- File execution
- File creation/deletion

### Seccomp Filters
- Syscall allowlists
- Argument filtering
- Return value control
- Error injection
- Tracing support

## Platform Requirements
- Linux kernel 5.13+
- Landlock LSM enabled
- Seccomp support
- BPF support
- Appropriate capabilities

## Fallback Strategies
- Graceful degradation
- Feature detection
- Alternative security measures
- Warning mechanisms
- Compatibility modes

## Performance Impact
- Minimal runtime overhead
- One-time setup cost
- Efficient syscall filtering
- Low memory footprint
- Negligible latency

## Testing & Validation
- Security policy testing
- Escape attempt detection
- Permission verification
- Compatibility testing
- Performance benchmarks