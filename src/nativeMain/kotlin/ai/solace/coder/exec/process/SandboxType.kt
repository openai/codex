// port-lint: source codex-rs/core/src/exec/mod.rs
package ai.solace.coder.exec.process

/**
 * Identifies which platform sandbox (if any) is in use for a particular execution.
 *
 * Mirrors Rust's SandboxType enum from codex-rs/core/src/exec/mod.rs
 */
enum class SandboxType {
    /** No sandbox - direct execution */
    None,

    /** macOS Seatbelt sandbox (macOS only) */
    MacosSeatbelt,

    /** Linux seccomp sandbox via codex-linux-sandbox */
    LinuxSeccomp,

    /** Windows restricted token sandbox (Windows only) */
    WindowsRestrictedToken,
}

