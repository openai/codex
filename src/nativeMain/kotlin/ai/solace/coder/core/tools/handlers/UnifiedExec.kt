// port-lint: source core/src/tools/handlers/unified_exec.rs
package ai.solace.coder.core.tools

import ai.solace.coder.core.error.CodexError
import ai.solace.coder.core.error.CodexResult
import ai.solace.coder.exec.process.ExecExpiration
import ai.solace.coder.exec.process.ExecParams
import ai.solace.coder.exec.process.ExecToolCallOutput
import ai.solace.coder.exec.process.ProcessExecutor
import ai.solace.coder.protocol.ShellCommandToolCallParams
import kotlin.time.Duration

/**
 * Handler for shell_command tool execution.
 * 
 * This handler processes Function payload types with shell_command,
 * executing single shell commands through the user's shell.
 */
class ShellCommandToolHandler(
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
                    parseShellCommandParams(payload.arguments)
                } catch (e: Exception) {
                    return CodexResult.failure(
                        CodexError.Fatal("Failed to parse shell_command arguments: ${e.message}")
                    )
                }
            }
            else -> {
                return CodexResult.failure(
                    CodexError.Fatal("Unsupported payload for shell_command handler")
                )
            }
        }
        
        // Execute the command through the user's shell
        return try {
            val result = executeShellCommand(invocation, params)
            val output = formatCommandOutput(result)
            
            CodexResult.success(
                ToolOutput.Function(
                    content = output,
                    success = if (result.exitCode == 0) true else false
                )
            )
        } catch (e: Exception) {
            CodexResult.failure(
                CodexError.Fatal("Shell command execution failed: ${e.message}")
            )
        }
    }
    
    override fun getTimeoutMs(): Long {
        // Default to 5 minutes for shell commands
        return 300000L
    }
    
    /**
     * Execute a shell command with the given parameters.
     */
    private suspend fun executeShellCommand(
        invocation: ToolInvocation,
        params: ShellCommandToolCallParams
    ): ExecToolCallOutput {
        val workingDir = invocation.turn.resolvePath(params.workdir)
        val timeoutMs = params.timeoutMs ?: getTimeoutMs()
        
        // Get the user's shell and derive the full command
        val shellCommand = deriveShellCommand(params.command)
        
        val execParams = ExecParams(
            command = shellCommand,
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
            sandboxPolicy = ai.solace.coder.protocol.SandboxPolicy.DangerFullAccess, // TODO: Convert from protocol SandboxPolicy
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
     * Derive the full shell command for executing the user's command.
     * This simulates what the Rust ShellCommandHandler does.
     */
    private fun deriveShellCommand(userCommand: String): List<String> {
        // In a real implementation, this would detect the user's shell
        // and construct the appropriate command. For now, we'll use bash.
        val shellPath = "/bin/bash"
        val useLoginShell = true
        
        return if (useLoginShell) {
            listOf(shellPath, "-l", "-c", userCommand)
        } else {
            listOf(shellPath, "-c", userCommand)
        }
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
     * Parse shell command tool call parameters from JSON arguments.
     */
    private fun parseShellCommandParams(arguments: String): ShellCommandToolCallParams {
        return try {
            if (arguments.contains("\"command\"")) {
                // Parse JSON format
                parseJsonShellCommandParams(arguments)
            } else {
                // Assume it's a simple command string
                ShellCommandToolCallParams(
                    command = arguments,
                    workdir = null,
                    timeoutMs = null,
                    withEscalatedPermissions = null,
                    justification = null
                )
            }
        } catch (e: Exception) {
            throw IllegalArgumentException("Invalid shell_command parameters: ${e.message}")
        }
    }
    
    /**
     * Simple JSON parser for shell command parameters.
     * In production, this would use kotlinx.serialization.
     */
    private fun parseJsonShellCommandParams(json: String): ShellCommandToolCallParams {
        val command = extractJsonString(json, "command") ?: 
            throw IllegalArgumentException("Missing command field")
        
        val workdir = extractJsonString(json, "workdir")
        val timeoutMs = extractJsonLong(json, "timeout_ms")
        val withEscalated = extractJsonBoolean(json, "with_escalated_permissions")
        val justification = extractJsonString(json, "justification")
        
        return ShellCommandToolCallParams(
            command = command,
            workdir = workdir,
            timeoutMs = timeoutMs,
            withEscalatedPermissions = withEscalated,
            justification = justification
        )
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
         * Create a ShellCommandToolHandler with the given process executor.
         */
        fun create(processExecutor: ProcessExecutor): ShellCommandToolHandler {
            return ShellCommandToolHandler(processExecutor)
        }
    }
}