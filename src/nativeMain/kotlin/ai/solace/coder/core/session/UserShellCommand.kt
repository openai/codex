// port-lint: source core/src/user_shell_command.rs
package ai.solace.coder.core.session

import ai.solace.coder.exec.process.ExecToolCallOutput
import ai.solace.coder.protocol.ContentItem
import ai.solace.coder.protocol.ResponseItem

/**
 * User shell command formatting utilities.
 *
 * Ported from Rust codex-rs/core/src/user_shell_command.rs
 */

const val USER_SHELL_COMMAND_OPEN = "<user_shell_command>"
const val USER_SHELL_COMMAND_CLOSE = "</user_shell_command>"

/**
 * Check if text is a user shell command.
 */
fun isUserShellCommandText(text: String): Boolean {
    val trimmed = text.trimStart()
    val lowered = trimmed.lowercase()
    return lowered.startsWith(USER_SHELL_COMMAND_OPEN)
}

/**
 * Format duration line for output.
 */
private fun formatDurationLine(duration: kotlin.time.Duration): String {
    val durationSeconds = duration.inWholeMilliseconds / 1000.0
    // Manual formatting since Kotlin Native doesn't have String.format
    val intPart = durationSeconds.toLong()
    val fracPart = ((durationSeconds - intPart) * 10000).toLong()
    val fracStr = fracPart.toString().padStart(4, '0')
    return "Duration: $intPart.$fracStr seconds"
}

/**
 * Format the body of a user shell command record.
 */
private fun formatUserShellCommandBody(
    command: String,
    execOutput: ExecToolCallOutput,
    truncationPolicy: ai.solace.coder.core.context.TruncationPolicy
): String {
    val sections = mutableListOf<String>()
    sections.add("<command>")
    sections.add(command)
    sections.add("</command>")
    sections.add("<result>")
    sections.add("Exit code: ${execOutput.exitCode}")
    sections.add(formatDurationLine(execOutput.duration))
    sections.add("Output:")
    sections.add(formatExecOutputStr(execOutput, truncationPolicy))
    sections.add("</result>")
    return sections.joinToString("\n")
}

/**
 * Format a user shell command record.
 */
fun formatUserShellCommandRecord(
    command: String,
    execOutput: ExecToolCallOutput,
    truncationPolicy: ai.solace.coder.core.context.TruncationPolicy
): String {
    val body = formatUserShellCommandBody(command, execOutput, truncationPolicy)
    return "$USER_SHELL_COMMAND_OPEN\n$body\n$USER_SHELL_COMMAND_CLOSE"
}

/**
 * Create a ResponseItem for a user shell command.
 */
fun userShellCommandRecordItem(
    command: String,
    execOutput: ExecToolCallOutput,
    truncationPolicy: ai.solace.coder.core.context.TruncationPolicy
): ResponseItem {
    return ResponseItem.Message(
        id = null,
        role = "user",
        content = listOf(
            ContentItem.InputText(
                formatUserShellCommandRecord(command, execOutput, truncationPolicy)
            )
        )
    )
}

/**
 * Format execution output string with truncation.
 */
private fun formatExecOutputStr(
    output: ExecToolCallOutput,
    truncationPolicy: ai.solace.coder.core.context.TruncationPolicy
): String {
    // Use aggregated output if available, otherwise combine stdout/stderr
    val content = if (output.aggregatedOutput.text.isNotEmpty()) {
        output.aggregatedOutput.text
    } else {
        buildString {
            if (output.stdout.text.isNotEmpty()) {
                append(output.stdout.text)
            }
            if (output.stderr.text.isNotEmpty()) {
                if (isNotEmpty()) append("\n")
                append(output.stderr.text)
            }
        }
    }
    return ai.solace.coder.core.context.truncateText(content, truncationPolicy)
}

// ExecToolCallOutput is now imported from ai.solace.coder.exec.process
