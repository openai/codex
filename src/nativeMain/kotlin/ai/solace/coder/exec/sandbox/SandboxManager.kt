// port-lint: source codex-rs/core/src/sandboxing/mod.rs
package ai.solace.coder.exec.sandbox

import ai.solace.coder.exec.process.SandboxType
import ai.solace.coder.protocol.SandboxPolicy

/**
 * Preference for sandbox usage by a tool.
 * Mirrors Rust's SandboxablePreference from tools/sandboxing.rs
 */
enum class SandboxPreference {
    /** Automatically decide based on policy */
    Auto,

    /** Require sandbox execution */
    Require,

    /** Forbid sandbox - must run without sandbox */
    Forbid,
}

/**
 * Manager for sandbox selection and command transformation.
 * Mirrors Rust's SandboxManager from sandboxing/mod.rs
 */
class SandboxManager {

    /**
     * Select the initial sandbox type for a given policy and preference.
     *
     * @param policy The sandbox policy (ReadOnly, WorkspaceWrite, DangerFullAccess)
     * @param preference Tool's sandbox preference (Auto, Require, Forbid)
     * @return The selected sandbox type for this execution
     */
    fun selectInitialSandbox(policy: SandboxPolicy, preference: SandboxPreference): SandboxType {
        return when (preference) {
            SandboxPreference.Forbid -> SandboxType.None

            SandboxPreference.Require -> {
                // Require a platform sandbox when available
                getPlatformSandbox() ?: SandboxType.None
            }

            SandboxPreference.Auto -> when (policy) {
                is SandboxPolicy.DangerFullAccess -> SandboxType.None
                else -> getPlatformSandbox() ?: SandboxType.None
            }
        }
    }

    /**
     * Get the platform-specific sandbox type if available.
     * TODO: Implement platform detection (macOS Seatbelt, Linux Seccomp, Windows RestrictedToken)
     */
    private fun getPlatformSandbox(): SandboxType? {
        // TODO: Detect platform and return appropriate sandbox
        // For now, return None as placeholder
        return SandboxType.None
    }
}

