package ai.solace.coder.core.tools

import ai.solace.coder.core.error.CodexError
import ai.solace.coder.core.error.CodexResult
import ai.solace.coder.exec.process.ExecExpiration
import ai.solace.coder.exec.process.ExecParams
import ai.solace.coder.exec.process.ExecToolCallOutput
import ai.solace.coder.exec.process.ProcessExecutor
import kotlin.time.Duration

/**
 * Parameters for container exec tool calls.
 */
data class ContainerExecToolCallParams(
    val containerName: String,
    val command: List<String>,
    val workdir: String? = null,
    val user: String? = null,
    val env: List<String> = emptyList(),
    val timeoutMs: Long? = null,
    val withEscalatedPermissions: Boolean? = null,
    val justification: String? = null
)

/**
 * Handler for container.exec tool execution.
 * 
 * This handler processes Function payload types with container.exec,
 * executing commands within containers with proper isolation.
 */
class ContainerExecToolHandler(
    private val processExecutor: ProcessExecutor
) : ToolHandler {
    
    override val kind: ToolKind = ToolKind.Function
    
    override fun matchesKind(payload: ToolPayload): Boolean {
        return payload is ToolPayload.Function
    }
    
    override suspend fun handle(invocation: ToolInvocation): CodexResult<ToolOutput> {
        val params = when (val payload = invocation.payload) {
            is ToolPayload.Function -> {
                try {
                    parseContainerExecParams(payload.arguments)
                } catch (e: Exception) {
                    return CodexResult.failure(
                        CodexError.Fatal("Failed to parse container.exec arguments: ${e.message}")
                    )
                }
            }
            else -> {
                return CodexResult.failure(
                    CodexError.Fatal("Unsupported payload for container.exec handler")
                )
            }
        }
        
        // Execute the command in the container
        return try {
            val result = executeContainerCommand(invocation, params)
            val output = formatCommandOutput(result)
            
            CodexResult.success(
                ToolOutput.Function(
                    content = output,
                    success = if (result.exitCode == 0) true else false
                )
            )
        } catch (e: Exception) {
            CodexResult.failure(
                CodexError.Fatal("Container command execution failed: ${e.message}")
            )
        }
    }
    
    override fun getTimeoutMs(): Long {
        // Default to 10 minutes for container commands
        return 600000L
    }
    
    /**
     * Execute a container command with the given parameters.
     */
    private suspend fun executeContainerCommand(
        invocation: ToolInvocation,
        params: ContainerExecToolCallParams
    ): ExecToolCallOutput {
        val workingDir = invocation.turn.resolvePath(params.workdir)
        val timeoutMs = params.timeoutMs ?: getTimeoutMs()
        
        // Build the container execution command
        val containerCommand = buildContainerCommand(params)
        
        val execParams = ExecParams(
            command = containerCommand,
            cwd = workingDir,
expiration = ExecExpiration.Timeout(
                Duration.parse("${timeoutMs ?: 30000}ms")
            ),
            env = emptyMap(), // Would be populated from turn context
            withEscalatedPermissions = params.withEscalatedPermissions,
            justification = params.justification
        )
        
        val result = processExecutor.execute(
            params = execParams,
            sandboxPolicy = ai.solace.coder.protocol.models.SandboxPolicy.DangerFullAccess, // TODO: Convert from protocol SandboxPolicy
            sandboxCwd = invocation.turn.cwd
        )
        
        return when (result) {
            is CodexResult.Success -> result.getOrThrow()
            is CodexResult.Failure -> {
                throw result.error.toException()
            }
        }
    }
    
    /**
     * Build the container execution command.
     */
    private fun buildContainerCommand(params: ContainerExecToolCallParams): List<String> {
        val command = mutableListOf<String>()
        
        // Add docker command
        command.add("docker")
        command.add("exec")
        
        // Add container name
        command.add(params.containerName)
        
        // Add user if specified
        params.user?.let { user ->
            command.add("--user")
            command.add(user)
        }
        
        // Add workdir if specified
        params.workdir?.let { workdir ->
            command.add("--workdir")
            command.add(workdir)
        }
        
        // Add environment variables
        for (envVar in params.env) {
            command.add("--env")
            command.add(envVar)
        }
        
        // Add the command to execute
        command.addAll(params.command)
        
        return command
    }
    
    /**
     * Format command output for model consumption.
     */
    private fun formatCommandOutput(result: ExecToolCallOutput): String {
        val sections = mutableListOf<String>()
        
        sections.add("Exit code: ${result.exitCode}")
        sections.add("Duration: ${result.duration}")
        
        if (result.timedOut) {
            sections.add("Command timed out after ${result.duration}")
        }
        
        if (result.aggregatedOutput.text.isNotEmpty()) {
            sections.add("Output:")
            sections.add(result.aggregatedOutput.text)
        }
        
        if (result.stderr.text.isNotEmpty()) {
            sections.add("Error output:")
            sections.add(result.stderr.text)
        }
        
        return sections.joinToString("\n")
    }
    
    /**
     * Parse container exec tool call parameters from JSON arguments.
     */
    private fun parseContainerExecParams(arguments: String): ContainerExecToolCallParams {
        return try {
            if (arguments.contains("\"container_name\"")) {
                // Parse JSON format
                parseJsonContainerExecParams(arguments)
            } else {
                throw IllegalArgumentException("Invalid container.exec parameters: missing container_name")
            }
        } catch (e: Exception) {
            throw IllegalArgumentException("Invalid container.exec parameters: ${e.message}")
        }
    }
    
    /**
     * Simple JSON parser for container exec parameters.
     * In production, this would use kotlinx.serialization.
     */
    private fun parseJsonContainerExecParams(json: String): ContainerExecToolCallParams {
        val containerName = extractJsonString(json, "container_name") ?: 
            throw IllegalArgumentException("Missing container_name field")
        
        val command = extractJsonArray(json, "command") ?: 
            throw IllegalArgumentException("Missing command field")
        
        val workdir = extractJsonString(json, "workdir")
        val user = extractJsonString(json, "user")
        val env = extractJsonArray(json, "env") ?: emptyList()
        val timeoutMs = extractJsonLong(json, "timeout_ms")
        val withEscalated = extractJsonBoolean(json, "with_escalated_permissions")
        val justification = extractJsonString(json, "justification")
        
        return ContainerExecToolCallParams(
            containerName = containerName,
            command = command,
            workdir = workdir,
            user = user,
            env = env,
            timeoutMs = timeoutMs,
            withEscalatedPermissions = withEscalated,
            justification = justification
        )
    }
    
    /**
     * Extract a string array from JSON.
     */
    private fun extractJsonArray(json: String, key: String): List<String>? {
        val pattern = "\"$key\"\\s*:\\s*\\[(.*?)\\]".toRegex()
        val match = pattern.find(json)
        return if (match != null) {
            val content = match.groupValues[1]
            content.split(",").map { it.trim().removeSurrounding("\"") }
        } else {
            null
        }
    }
    
    /**
     * Extract a string value from JSON.
     */
    private fun extractJsonString(json: String, key: String): String? {
        val pattern = "\"$key\"\\s*:\\s*\"([^\"]*)\"".toRegex()
        val match = pattern.find(json)
        return match?.groupValues?.get(1)
    }
    
    /**
     * Extract a long value from JSON.
     */
    private fun extractJsonLong(json: String, key: String): Long? {
        val pattern = "\"$key\"\\s*:\\s*(\\d+)".toRegex()
        val match = pattern.find(json)
        return match?.groupValues?.get(1)?.toLongOrNull()
    }
    
    /**
     * Extract a boolean value from JSON.
     */
    private fun extractJsonBoolean(json: String, key: String): Boolean? {
        val pattern = "\"$key\"\\s*:\\s*(true|false)".toRegex()
        val match = pattern.find(json)
        return when (match?.groupValues?.get(1)) {
            "true" -> true
            "false" -> false
            else -> null
        }
    }
    
    companion object {
        /**
         * Create a ContainerExecToolHandler with the given process executor.
         */
        fun create(processExecutor: ProcessExecutor): ContainerExecToolHandler {
            return ContainerExecToolHandler(processExecutor)
        }
    }
}