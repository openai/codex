package ai.solace.coder.exec.sandbox

import ai.solace.coder.core.error.CodexError
import ai.solace.coder.core.error.CodexResult
import ai.solace.coder.exec.process.CommandSpec
import ai.solace.coder.exec.process.ExecEnv
import ai.solace.coder.exec.process.ExecExpiration
import ai.solace.coder.exec.process.SandboxType
import ai.solace.coder.exec.process.platformGetSandbox
import ai.solace.coder.exec.process.platformGetMacosDirParams
import ai.solace.coder.protocol.models.SandboxPolicy

/**
 * Sandbox permissions levels
 */
enum class SandboxPermissions {
    UseDefault,
    RequireEscalated
}

/**
 * Sandbox preference for execution
 */
enum class SandboxPreference {
    Auto,
    Require,
    Forbid
}

/**
 * Sandbox manager for applying platform-specific sandbox policies
 *
 * TODO: Port from Rust codex-rs/core/src/sandboxing/mod.rs:
 * - [ ] select_initial() - choose sandbox type based on policy and preference
 * - [ ] transform() - convert CommandSpec to sandboxed ExecEnv
 * - [ ] denied() - detect likely sandbox denials from output
 * - [ ] Platform-specific sandbox wrapping:
 *   - macOS: sandbox-exec with Seatbelt profiles
 *   - Linux: codex-linux-sandbox with Landlock/seccomp
 *   - Windows: restricted token sandbox (codex-windows-sandbox crate)
 * - [ ] SandboxablePreference enum (Auto, Require, Forbid)
 * - [ ] Proper writable roots handling with read-only subpaths
 * - [ ] Assessment module for sandbox command safety analysis
 */
class SandboxManager {
    companion object {
        private const val CODEX_SANDBOX_ENV_VAR = "CODEX_SANDBOX"
        private const val CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR = "CODEX_SANDBOX_NETWORK_DISABLED"
    }

    /**
     * Transform a command specification for sandboxed execution
     */
    fun transform(
        spec: CommandSpec,
        policy: SandboxPolicy,
        sandboxPolicyCwd: String
    ): CodexResult<ExecEnv> {
        val mutEnv = spec.env.toMutableMap()
        
        // Apply network restrictions
        if (!policy.hasFullNetworkAccess()) {
            mutEnv[CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR] = "1"
        }

        val mutCommand = (listOf(spec.program) + spec.args).toMutableList()
        
        val (command, sandboxEnv, arg0Override) = when (selectInitialSandbox(policy, SandboxPreference.Auto)) {
            SandboxType.None -> {
                Triple(mutCommand, emptyMap<String, String>(), null)
            }
            SandboxType.MacosSeatbelt -> {
                createSeatbeltCommand(mutCommand, policy, sandboxPolicyCwd)
            }
            SandboxType.LinuxSeccomp -> {
                createLinuxSandboxCommand(mutCommand, policy, sandboxPolicyCwd)
            }
            SandboxType.WindowsRestrictedToken -> {
                Triple(mutCommand, emptyMap<String, String>(), null)
            }
        }

        mutEnv.putAll(sandboxEnv)

        return CodexResult.success(
            ExecEnv(
                command = command,
                cwd = spec.cwd,
                env = mutEnv,
                expiration = spec.expiration,
                sandbox = selectInitialSandbox(policy, SandboxPreference.Auto),
                withEscalatedPermissions = spec.withEscalatedPermissions,
                justification = spec.justification,
                arg0 = arg0Override
            )
        )
    }

    /**
     * Select initial sandbox type based on policy and preference
     */
    fun selectInitialSandbox(policy: SandboxPolicy, preference: SandboxPreference): SandboxType {
        return when (preference) {
            SandboxPreference.Forbid -> SandboxType.None
            SandboxPreference.Require -> {
                getPlatformSandbox() ?: SandboxType.None
            }
            SandboxPreference.Auto -> when (policy) {
                is SandboxPolicy.DangerFullAccess -> SandboxType.None
                else -> getPlatformSandbox() ?: SandboxType.None
            }
        }
    }

    /**
     * Check if execution was likely denied by sandbox
     */
    fun denied(sandbox: SandboxType, output: ai.solace.coder.exec.process.ExecToolCallOutput): Boolean {
        return isLikelySandboxDenied(sandbox, output)
    }

    /**
     * Create Seatbelt command for macOS
     */
    private fun createSeatbeltCommand(
        command: List<String>,
        policy: SandboxPolicy,
        sandboxPolicyCwd: String
    ): Triple<List<String>, Map<String, String>, String?> {
        val seatbeltEnv = mapOf(CODEX_SANDBOX_ENV_VAR to "seatbelt")
        val args = createSeatbeltCommandArgs(command, policy, sandboxPolicyCwd)
        val fullCommand = mutableListOf("/usr/bin/sandbox-exec")
        fullCommand.addAll(args)
        return Triple(fullCommand, seatbeltEnv, null)
    }

    /**
     * Create Linux sandbox command
     */
    private fun createLinuxSandboxCommand(
        command: List<String>,
        policy: SandboxPolicy,
        sandboxPolicyCwd: String
    ): Triple<List<String>, Map<String, String>, String?> {
        // This would need the codex-linux-sandbox executable path
        // For now, we'll create the args structure
        val args = createLinuxSandboxCommandArgs(command, policy, sandboxPolicyCwd)
        val fullCommand = mutableListOf("codex-linux-sandbox")
        fullCommand.addAll(args)
        return Triple(fullCommand, emptyMap(), "codex-linux-sandbox")
    }

    /**
     * Create Seatbelt command arguments
     */
    private fun createSeatbeltCommandArgs(
        command: List<String>,
        policy: SandboxPolicy,
        sandboxPolicyCwd: String
    ): List<String> {
        val (fileWritePolicy, fileWriteDirParams) = if (policy.hasFullDiskWriteAccess()) {
            Pair("(allow file-write* (regex #\"^/\"))", emptyList<Pair<String, String>>())
        } else {
            val writableRoots = policy.getWritableRootsWithCwd(sandboxPolicyCwd)
            val writableFolderPolicies = mutableListOf<String>()
            val fileWriteParams = mutableListOf<Pair<String, String>>()

            for ((index, wr) in writableRoots.withIndex()) {
                val rootParam = "WRITABLE_ROOT_$index"
                fileWriteParams.add(rootParam to wr.root)

                if (wr.readOnlySubpaths.isEmpty()) {
                    writableFolderPolicies.add("(subpath (param \"$rootParam\"))")
                } else {
                    val requireParts = mutableListOf<String>()
                    requireParts.add("(subpath (param \"$rootParam\"))")
                    for ((subpathIndex, ro) in wr.readOnlySubpaths.withIndex()) {
                        val roParam = "WRITABLE_ROOT_${index}_RO_$subpathIndex"
                        requireParts.add("(require-not (subpath (param \"$roParam\")))")
                        fileWriteParams.add(roParam to ro)
                    }
                    val policyComponent = "(require-all ${requireParts.joinToString(" ")} )"
                    writableFolderPolicies.add(policyComponent)
                }
            }

            if (writableFolderPolicies.isEmpty()) {
                Pair("", emptyList())
            } else {
                val fileWritePolicy = "(allow file-write*\n${writableFolderPolicies.joinToString(" ")}\n)"
                Pair(fileWritePolicy, fileWriteParams)
            }
        }

        val fileReadPolicy = if (policy.hasFullDiskReadAccess()) {
            "; allow read-only file operations\n(allow file-read*)"
        } else {
            ""
        }

        val networkPolicy = if (policy.hasFullNetworkAccess()) {
            getSeatbeltNetworkPolicy()
        } else {
            ""
        }

        val basePolicy = getSeatbeltBasePolicy()
        val fullPolicy = "$basePolicy\n$fileReadPolicy\n$fileWritePolicy\n$networkPolicy"

        val dirParams = fileWriteDirParams + getMacosDirParams()

        val seatbeltArgs = mutableListOf("-p", fullPolicy)
        val definitionArgs = dirParams.map { (key, value) -> "-D$key=$value" }
        seatbeltArgs.addAll(definitionArgs)
        seatbeltArgs.add("--")
        seatbeltArgs.addAll(command)

        return seatbeltArgs
    }

    /**
     * Create Linux sandbox command arguments
     */
    private fun createLinuxSandboxCommandArgs(
        command: List<String>,
        policy: SandboxPolicy,
        sandboxPolicyCwd: String
    ): List<String> {
        val sandboxPolicyJson = serializeSandboxPolicy(policy)
        
        return listOf(
            "--sandbox-policy-cwd", sandboxPolicyCwd,
            "--sandbox-policy", sandboxPolicyJson,
            "--"
        ) + command
    }

    /**
     * Get platform-specific sandbox
     */
    private fun getPlatformSandbox(): SandboxType? {
        return platformGetSandbox()
    }

    /**
     * Check if execution was likely denied by sandbox
     */
    private fun isLikelySandboxDenied(
        sandbox: SandboxType,
        output: ai.solace.coder.exec.process.ExecToolCallOutput
    ): Boolean {
        if (sandbox == SandboxType.None || output.exitCode == 0) {
            return false
        }

        val quickRejectExitCodes = setOf(2, 126, 127)
        if (quickRejectExitCodes.contains(output.exitCode)) {
            return false
        }

        val sandboxDeniedKeywords = listOf(
            "operation not permitted",
            "permission denied",
            "read-only file system",
            "seccomp",
            "sandbox",
            "landlock",
            "failed to write file"
        )

        val hasSandboxKeyword = listOf(
            output.stderr.text,
            output.stdout.text,
            output.aggregatedOutput.text
        ).any { section ->
            section.lowercase().let { lower ->
                sandboxDeniedKeywords.any { keyword -> lower.contains(keyword) }
            }
        }

        return hasSandboxKeyword
    }

    /**
     * Serialize sandbox policy to JSON
     */
    private fun serializeSandboxPolicy(policy: SandboxPolicy): String {
        // This would use a JSON serialization library
        // For now, return a placeholder
        return policy.toString()
    }

    /**
     * Get Seatbelt base policy
     * Transliterated from codex-rs/core/src/seatbelt_base_policy.sbpl
     */
    private fun getSeatbeltBasePolicy(): String {
        return """
(version 1)

; inspired by Chrome's sandbox policy:
; https://source.chromium.org/chromium/chromium/src/+/main:sandbox/policy/mac/common.sb;l=273-319;drc=7b3962fe2e5fc9e2ee58000dc8fbf3429d84d3bd
; https://source.chromium.org/chromium/chromium/src/+/main:sandbox/policy/mac/renderer.sb;l=64;drc=7b3962fe2e5fc9e2ee58000dc8fbf3429d84d3bd

; start with closed-by-default
(deny default)

; child processes inherit the policy of their parent
(allow process-exec)
(allow process-fork)
(allow signal (target same-sandbox))

; Allow cf prefs to work.
(allow user-preference-read)

; process-info
(allow process-info* (target same-sandbox))

(allow file-write-data
  (require-all
    (path "/dev/null")
    (vnode-type CHARACTER-DEVICE)))

; sysctls permitted.
(allow sysctl-read
  (sysctl-name "hw.activecpu")
  (sysctl-name "hw.busfrequency_compat")
  (sysctl-name "hw.byteorder")
  (sysctl-name "hw.cacheconfig")
  (sysctl-name "hw.cachelinesize_compat")
  (sysctl-name "hw.cpufamily")
  (sysctl-name "hw.cpufrequency_compat")
  (sysctl-name "hw.cputype")
  (sysctl-name "hw.l1dcachesize_compat")
  (sysctl-name "hw.l1icachesize_compat")
  (sysctl-name "hw.l2cachesize_compat")
  (sysctl-name "hw.l3cachesize_compat")
  (sysctl-name "hw.logicalcpu_max")
  (sysctl-name "hw.machine")
  (sysctl-name "hw.memsize")
  (sysctl-name "hw.ncpu")
  (sysctl-name "hw.nperflevels")
  ; Chrome locks these CPU feature detection down a bit more tightly,
  ; but mostly for fingerprinting concerns which isn't an issue for codex.
  (sysctl-name-prefix "hw.optional.arm.")
  (sysctl-name-prefix "hw.optional.armv8_")
  (sysctl-name "hw.packages")
  (sysctl-name "hw.pagesize_compat")
  (sysctl-name "hw.pagesize")
  (sysctl-name "hw.physicalcpu")
  (sysctl-name "hw.physicalcpu_max")
  (sysctl-name "hw.tbfrequency_compat")
  (sysctl-name "hw.vectorunit")
  (sysctl-name "kern.hostname")
  (sysctl-name "kern.maxfilesperproc")
  (sysctl-name "kern.maxproc")
  (sysctl-name "kern.osproductversion")
  (sysctl-name "kern.osrelease")
  (sysctl-name "kern.ostype")
  (sysctl-name "kern.osvariant_status")
  (sysctl-name "kern.osversion")
  (sysctl-name "kern.secure_kernel")
  (sysctl-name "kern.usrstack64")
  (sysctl-name "kern.version")
  (sysctl-name "sysctl.proc_cputype")
  (sysctl-name "vm.loadavg")
  (sysctl-name-prefix "hw.perflevel")
  (sysctl-name-prefix "kern.proc.pgrp.")
  (sysctl-name-prefix "kern.proc.pid.")
  (sysctl-name-prefix "net.routetable.")
)

; Allow Java to set CPU type grade when required
(allow sysctl-write
  (sysctl-name "kern.grade_cputype"))

; IOKit
(allow iokit-open
  (iokit-registry-entry-class "RootDomainUserClient")
)

; needed to look up user info, see https://crbug.com/792228
(allow mach-lookup
  (global-name "com.apple.system.opendirectoryd.libinfo")
)

; Added on top of Chrome profile
; Needed for python multiprocessing on MacOS for the SemLock
(allow ipc-posix-sem)

(allow mach-lookup
  (global-name "com.apple.PowerManagement.control")
)
"""
    }

    /**
     * Get Seatbelt network policy
     * Transliterated from codex-rs/core/src/seatbelt_network_policy.sbpl
     */
    private fun getSeatbeltNetworkPolicy(): String {
        return """
; when network access is enabled, these policies are added after those in seatbelt_base_policy.sbpl
; Ref https://source.chromium.org/chromium/chromium/src/+/main:sandbox/policy/mac/network.sb;drc=f8f264d5e4e7509c913f4c60c2639d15905a07e4

(allow network-outbound)
(allow network-inbound)
(allow system-socket)

(allow mach-lookup
    ; Used to look up the _CS_DARWIN_USER_CACHE_DIR in the sandbox.
    (global-name "com.apple.bsd.dirhelper")
    (global-name "com.apple.system.opendirectoryd.membership")

    ; Communicate with the security server for TLS certificate information.
    (global-name "com.apple.SecurityServer")
    (global-name "com.apple.networkd")
    (global-name "com.apple.ocspd")
    (global-name "com.apple.trustd.agent")

    ; Read network configuration.
    (global-name "com.apple.SystemConfiguration.DNSConfiguration")
    (global-name "com.apple.SystemConfiguration.configd")
)

(allow sysctl-read
  (sysctl-name-regex #"^net.routetable")
)

(allow file-write*
  (subpath (param "DARWIN_USER_CACHE_DIR"))
)
"""
    }

    /**
     * Get macOS directory parameters
     */
    private fun getMacosDirParams(): List<Pair<String, String>> {
        return platformGetMacosDirParams()
    }
}

/**
 * Extension function to get writable roots with cwd from policy
 */
private fun SandboxPolicy.getWritableRootsWithCwd(cwd: String): List<WritableRoot> {
    return when (this) {
        is SandboxPolicy.WorkspaceWrite -> {
            val roots = this.writable_roots.map { path ->
                WritableRoot(path, emptyList())
            }
            // Add cwd as writable root
            roots + WritableRoot(cwd, emptyList())
        }
        else -> emptyList()
    }
}

/**
 * Writable root configuration
 */
private data class WritableRoot(
    val root: String,
    val readOnlySubpaths: List<String>
)

/**
 * Extension functions for SandboxPolicy
 */
private fun SandboxPolicy.hasFullNetworkAccess(): Boolean {
    return when (this) {
        is SandboxPolicy.DangerFullAccess -> true
        is SandboxPolicy.WorkspaceWrite -> this.networkAccess
        else -> false
    }
}

private fun SandboxPolicy.hasFullDiskWriteAccess(): Boolean {
    return when (this) {
        is SandboxPolicy.DangerFullAccess -> true
        else -> false
    }
}

private fun SandboxPolicy.hasFullDiskReadAccess(): Boolean {
    return when (this) {
        is SandboxPolicy.DangerFullAccess -> true
        else -> false
    }
}

// =============================================================================
// Approval Store and Context
// Ported from Rust codex-rs/core/src/tools/sandboxing.rs
// =============================================================================

/**
 * Store for caching approval decisions across tool calls.
 *
 * Ported from Rust codex-rs/core/src/tools/sandboxing.rs ApprovalStore
 */
class ApprovalStore {
    private val map = mutableMapOf<String, ReviewDecision>()

    /**
     * Get a cached approval decision for a key.
     */
    fun <K> get(key: K): ReviewDecision? where K : Any {
        val s = serializeKey(key) ?: return null
        return map[s]
    }

    /**
     * Put an approval decision into the cache.
     */
    fun <K> put(key: K, value: ReviewDecision) where K : Any {
        val s = serializeKey(key) ?: return
        map[s] = value
    }

    private fun serializeKey(key: Any): String? {
        // Simple serialization - in production would use JSON
        return key.toString()
    }
}

// ReviewDecision is defined in ai.solace.coder.protocol.Protocol

/**
 * Context for approval requests.
 *
 * Ported from Rust codex-rs/core/src/tools/sandboxing.rs ApprovalCtx
 */
data class ApprovalCtx(
    val callId: String,
    val retryReason: String? = null,
    val risk: SandboxCommandAssessment? = null
)

/**
 * Assessment result for sandbox command safety analysis.
 *
 * Ported from Rust codex-rs/protocol/src/protocol.rs SandboxCommandAssessment
 */
data class SandboxCommandAssessment(
    val safe: Boolean,
    val reason: String? = null
)

/**
 * Specifies what tool orchestrator should do with a given tool call.
 *
 * Ported from Rust codex-rs/core/src/tools/sandboxing.rs ApprovalRequirement
 */
sealed class ApprovalRequirement {
    /**
     * No approval required for this tool call.
     */
    data class Skip(
        /** The first attempt should skip sandboxing (e.g., when explicitly greenlit by policy). */
        val bypassSandbox: Boolean = false
    ) : ApprovalRequirement()

    /**
     * Approval required for this tool call.
     */
    data class NeedsApproval(
        val reason: String? = null
    ) : ApprovalRequirement()

    /**
     * Execution forbidden for this tool call.
     */
    data class Forbidden(
        val reason: String
    ) : ApprovalRequirement()
}

/**
 * Sandbox override mode for tool execution.
 *
 * Ported from Rust codex-rs/core/src/tools/sandboxing.rs SandboxOverride
 */
enum class SandboxOverride {
    NoOverride,
    BypassSandboxFirstAttempt
}

/**
 * Sandbox execution preference.
 *
 * Ported from Rust codex-rs/core/src/tools/sandboxing.rs SandboxablePreference
 */
enum class SandboxablePreference {
    Auto,
    Require,
    Forbid
}

/**
 * Captures the command metadata needed to re-run a tool request without sandboxing.
 *
 * Ported from Rust codex-rs/core/src/tools/sandboxing.rs SandboxRetryData
 */
data class SandboxRetryData(
    val command: List<String>,
    val cwd: String
)

/**
 * Tool execution context.
 *
 * Ported from Rust codex-rs/core/src/tools/sandboxing.rs ToolCtx
 */
data class ToolCtx(
    val callId: String,
    val toolName: String
)

/**
 * Error type for tool execution.
 *
 * Ported from Rust codex-rs/core/src/tools/sandboxing.rs ToolError
 */
sealed class ToolError {
    data class Rejected(val message: String) : ToolError()
    data class Codex(val error: ai.solace.coder.core.error.CodexError) : ToolError()
}

/**
 * Sandbox attempt context for tool execution.
 *
 * Ported from Rust codex-rs/core/src/tools/sandboxing.rs SandboxAttempt
 */
data class SandboxAttempt(
    val sandbox: ai.solace.coder.exec.process.SandboxType,
    val policy: SandboxPolicy,
    val manager: SandboxManager,
    val sandboxCwd: String,
    val codexLinuxSandboxExe: String? = null
) {
    /**
     * Transform a command spec into an execution environment with sandboxing.
     */
    fun envFor(spec: CommandSpec): CodexResult<ExecEnv> {
        return manager.transform(spec, policy, sandboxCwd)
    }
}

/**
 * Determine the default approval requirement based on policy.
 *
 * - Never, OnFailure: do not ask
 * - OnRequest: ask unless sandbox policy is DangerFullAccess
 * - UnlessTrusted: always ask
 *
 * Ported from Rust codex-rs/core/src/tools/sandboxing.rs default_approval_requirement
 */
fun defaultApprovalRequirement(
    policy: ai.solace.coder.protocol.AskForApproval,
    sandboxPolicy: SandboxPolicy
): ApprovalRequirement {
    val needsApproval = when (policy) {
        ai.solace.coder.protocol.AskForApproval.Never,
        ai.solace.coder.protocol.AskForApproval.OnFailure -> false
        ai.solace.coder.protocol.AskForApproval.OnRequest -> sandboxPolicy !is SandboxPolicy.DangerFullAccess
        ai.solace.coder.protocol.AskForApproval.UnlessTrusted -> true
    }

    return if (needsApproval) {
        ApprovalRequirement.NeedsApproval(reason = null)
    } else {
        ApprovalRequirement.Skip(bypassSandbox = false)
    }
}