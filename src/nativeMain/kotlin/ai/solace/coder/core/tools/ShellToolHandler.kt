package ai.solace.coder.core.tools

import ai.solace.coder.core.error.CodexError
import ai.solace.coder.core.error.CodexResult
import ai.solace.coder.exec.process.ExecExpiration
import ai.solace.coder.exec.process.ExecParams
import ai.solace.coder.exec.process.ExecToolCallOutput
import ai.solace.coder.exec.process.ProcessExecutor
import ai.solace.coder.protocol.models.ShellToolCallParams
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlin.time.Duration

/**
 * Handler for shell tool execution.
 *
 * This handler processes both Function and LocalShell payload types,
 * executing shell commands with proper sandboxing and approval handling.
 *
 * TODO: Port from Rust codex-rs/core/src/tools/handlers/shell.rs:
 * - [ ] Apply_patch interception - detect apply_patch commands and route to ApplyPatchHandler
 * - [ ] ExecPolicy integration for approval requirements per command
 * - [ ] ToolEmitter for event emission (begin/finish events)
 * - [ ] ToolOrchestrator for approval workflow
 * - [ ] ShellRuntime for actual execution
 * - [ ] Proper env creation from shell_environment_policy
 * - [ ] Login shell support via ShellCommandHandler
 * - [ ] is_safe_command() proper implementation with shell-aware parsing
 * - [ ] Support for freeform vs structured output formatting
 */
class ShellToolHandler(
    private val processExecutor: ProcessExecutor
) : ToolHandler {
    
    override val kind: ToolKind = ToolKind.Function
    
    override fun matchesKind(payload: ToolPayload): Boolean {
        return payload is ToolPayload.Function || payload is ToolPayload.LocalShell
    }
    
    override fun isMutating(invocation: ToolInvocation): Boolean {
        // Check if the command is potentially mutating
        return when (val payload = invocation.payload) {
            is ToolPayload.Function -> {
                try {
                    val params = parseShellParams(payload.arguments)
                    !isKnownSafeCommand(params.command)
                } catch (e: Exception) {
                    true // Assume mutating if we can't parse
                }
            }
            is ToolPayload.LocalShell -> {
                !isKnownSafeCommand(payload.params.command)
            }
            else -> true
        }
    }
    
    override suspend fun handle(invocation: ToolInvocation): CodexResult<ToolOutput> {
        val params = when (val payload = invocation.payload) {
            is ToolPayload.Function -> {
                try {
                    parseShellParams(payload.arguments)
                } catch (e: Exception) {
                    return CodexResult.failure(
                        CodexError.Fatal("Failed to parse shell arguments: ${e.message}")
                    )
                }
            }
            is ToolPayload.LocalShell -> {
                payload.params
            }
            else -> {
                return CodexResult.failure(
                    CodexError.Fatal("Unsupported payload for shell handler")
                )
            }
        }
        
        // Check approval policy for escalated permissions
        if (params.withEscalatedPermissions == true &&
            invocation.turn.approvalPolicy != ai.solace.coder.protocol.AskForApproval.OnRequest) {
            return CodexResult.failure(
                CodexError.Fatal(
                    "Approval policy is ${invocation.turn.approvalPolicy}; " +
                    "reject command with escalated permissions"
                )
            )
        }
        
        // Execute the command
        return try {
            val result = executeCommand(invocation, params)
            val output = formatCommandOutput(result)
            
            CodexResult.success(
                ToolOutput.Function(
                    content = output,
                    success = if (result.exitCode == 0) true else false
                )
            )
        } catch (e: Exception) {
            CodexResult.failure(
                CodexError.Fatal("Command execution failed: ${e.message}")
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
    private suspend fun executeCommand(
        invocation: ToolInvocation,
        params: ShellToolCallParams
    ): ExecToolCallOutput {
        val workingDir = invocation.turn.resolvePath(params.workdir)
        val timeoutMs = params.timeoutMs ?: getTimeoutMs()
        
        return withContext(Dispatchers.Default) {
            val execParams = ExecParams(
                command = params.command,
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
            
            when (result) {
                is CodexResult.Success -> result.getOrThrow()
                is CodexResult.Failure -> {
                    throw result.error.toException()
                }
            }
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
     * Parse shell tool call parameters from JSON arguments.
     */
    private fun parseShellParams(arguments: String): ShellToolCallParams {
        // In a real implementation, this would use a JSON parser
        // For now, we'll do a simple implementation
        return try {
            // This is a simplified parser - in production, use kotlinx.serialization
            if (arguments.contains("\"command\"")) {
                // Parse JSON format
                parseJsonShellParams(arguments)
            } else {
                // Assume it's a simple command string
                ShellToolCallParams(
                    command = arguments.split(" "),
                    workdir = null,
                    timeoutMs = null,
                    withEscalatedPermissions = null,
                    justification = null
                )
            }
        } catch (e: Exception) {
            throw IllegalArgumentException("Invalid shell parameters: ${e.message}")
        }
    }
    
    /**
     * Simple JSON parser for shell parameters.
     * In production, this would use kotlinx.serialization.
     */
    private fun parseJsonShellParams(json: String): ShellToolCallParams {
        // This is a very basic JSON parser - replace with proper serialization
        val command = extractJsonArray(json, "command") ?: 
            throw IllegalArgumentException("Missing command field")
        
        val workdir = extractJsonString(json, "workdir")
        val timeoutMs = extractJsonLong(json, "timeout_ms")
        val withEscalated = extractJsonBoolean(json, "with_escalated_permissions")
        val justification = extractJsonString(json, "justification")
        
        return ShellToolCallParams(
            command = command,
            workdir = workdir,
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
    
    /**
     * Check if a command is known to be safe (non-mutating).
     */
    private fun isKnownSafeCommand(command: List<String>): Boolean {
        if (command.isEmpty()) return false
        
        val firstCommand = command[0].lowercase()
        
        // List of commands that are generally safe (read-only operations)
        val safeCommands = setOf(
            "ls", "cat", "pwd", "whoami", "date", "echo", "grep", "find",
            "which", "whereis", "type", "help", "man", "info", "wc", "head",
            "tail", "sort", "uniq", "cut", "awk", "sed", "diff", "cmp",
            "file", "stat", "du", "df", "free", "uname", "uptime", "ps",
            "top", "htop", "history", "env", "printenv", "alias", "jobs",
            "fg", "bg", "dirs", "pushd", "popd", "cd", "tree", "less",
            "more", "strings", "hexdump", "od", "base64", "xz", "gzip",
            "gunzip", "tar", "zip", "unzip", "git", "hg", "svn"
        )
        
        return safeCommands.contains(firstCommand)
    }
    
    companion object {
        /**
         * Create a ShellToolHandler with the given process executor.
         */
        fun create(processExecutor: ProcessExecutor): ShellToolHandler {
            return ShellToolHandler(processExecutor)
        }
    }
}