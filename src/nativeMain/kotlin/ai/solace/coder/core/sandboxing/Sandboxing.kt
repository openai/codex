// port-lint: source core/src/sandboxing/mod.rs
package ai.solace.coder.core.sandboxing

import ai.solace.coder.core.error.CodexError
import ai.solace.coder.core.error.CodexResult
import ai.solace.coder.core.CommandSpec
import ai.solace.coder.core.ExecEnv
import ai.solace.coder.core.ExecToolCallOutput
import ai.solace.coder.core.SandboxType
import ai.solace.coder.core.StdoutStream
import ai.solace.coder.core.isLikelySandboxDenied
import ai.solace.coder.core.platformGetMacosDirParams
import ai.solace.coder.protocol.SandboxPolicy

/**
 * Sandbox permissions levels
 */
enum class SandboxPermissions {
    UseDefault,
    RequireEscalated;

    fun requiresEscalatedPermissions(): Boolean {
        return this == RequireEscalated
    }

    companion object {
        fun from(withEscalatedPermissions: Boolean): SandboxPermissions {
            return if (withEscalatedPermissions) {
                RequireEscalated
            } else {
                UseDefault
            }
        }
    }
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
 * Sandbox manager for applying platform-specific sandbox policies
 */
class SandboxManager {
    companion object {
        private const val CODEX_SANDBOX_ENV_VAR = "CODEX_SANDBOX"
        private const val CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR = "CODEX_SANDBOX_NETWORK_DISABLED"
        private const val MACOS_PATH_TO_SEATBELT_EXECUTABLE = "/usr/bin/sandbox-exec"
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
        val seatbeltEnv = mutableMapOf<String, String>()
        seatbeltEnv[CODEX_SANDBOX_ENV_VAR] = "seatbelt"
        val args = createSeatbeltCommandArgs(command, policy, sandboxPolicyCwd)
        val fullCommand = mutableListOf<String>()
        fullCommand.add(MACOS_PATH_TO_SEATBELT_EXECUTABLE)
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
            val roots = this.writableRoots.map { path ->
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
