package ai.solace.coder.protocol.models

import kotlinx.serialization.Serializable

/**
 * Sandbox policy for command execution
 */
@Serializable
sealed class SandboxPolicy {
    /**
     * Full access without sandboxing restrictions
     */
    @Serializable
    @kotlinx.serialization.SerialName("danger_full_access")
    data object DangerFullAccess : SandboxPolicy()

    /**
     * Workspace write policy with specific writable roots
     */
    @Serializable
    @kotlinx.serialization.SerialName("workspace_write")
    data class WorkspaceWrite(
        @kotlinx.serialization.SerialName("writable_roots")
        val writableRoots: List<String>,
        @kotlinx.serialization.SerialName("network_access")
        val networkAccess: Boolean,
        @kotlinx.serialization.SerialName("exclude_tmpdir_env_var")
        val excludeTmpdirEnvVar: Boolean = false,
        @kotlinx.serialization.SerialName("exclude_slash_tmp")
        val excludeSlashTmp: Boolean = false
    ) : SandboxPolicy()

    /**
     * Read-only policy
     */
    @Serializable
    @kotlinx.serialization.SerialName("read_only")
    data class ReadOnly(
        @kotlinx.serialization.SerialName("readable_paths")
        val readablePaths: List<String> = emptyList(),
        @kotlinx.serialization.SerialName("network_access")
        val networkAccess: Boolean = false
    ) : SandboxPolicy()

    /**
     * Custom policy with specific rules
     */
    @Serializable
    @kotlinx.serialization.SerialName("custom")
    data class Custom(
        val rules: List<SandboxRule>,
        @kotlinx.serialization.SerialName("network_access")
        val networkAccess: Boolean = false
    ) : SandboxPolicy()
}

/**
 * Individual sandbox rules
 */
@Serializable
sealed class SandboxRule {
    @Serializable
    @kotlinx.serialization.SerialName("file_read")
    data class FileRead(
        val paths: List<String>,
        val recursive: Boolean = true
    ) : SandboxRule()

    @Serializable
    @kotlinx.serialization.SerialName("file_write")
    data class FileWrite(
        val paths: List<String>,
        val recursive: Boolean = true,
        @kotlinx.serialization.SerialName("read_only_subpaths")
        val readOnlySubpaths: List<String> = emptyList()
    ) : SandboxRule()

    @Serializable
    @kotlinx.serialization.SerialName("network_access")
    data class NetworkAccess(
        @kotlinx.serialization.SerialName("allowed_hosts")
        val allowedHosts: List<String>? = null,
        @kotlinx.serialization.SerialName("allowed_ports")
        val allowedPorts: List<Int>? = null,
        @kotlinx.serialization.SerialName("blocked_hosts")
        val blockedHosts: List<String>? = null
    ) : SandboxRule()

    @Serializable
    @kotlinx.serialization.SerialName("process_exec")
    data class ProcessExec(
        @kotlinx.serialization.SerialName("allowed_executables")
        val allowedExecutables: List<String>,
        @kotlinx.serialization.SerialName("blocked_executables")
        val blockedExecutables: List<String> = emptyList()
    ) : SandboxRule()
}

// ExecOutputStream and ExecCommandOutputDeltaEvent are defined in Protocol.kt