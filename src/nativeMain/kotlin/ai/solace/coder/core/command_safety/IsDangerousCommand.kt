// port-lint: source core/src/command_safety/is_dangerous_command.rs
package ai.solace.coder.core.command_safety

import ai.solace.coder.protocol.AskForApproval
import ai.solace.coder.protocol.SandboxPolicy
import ai.solace.coder.core.sandboxing.SandboxPermissions
import ai.solace.coder.core.bash.parseShellLcPlainCommands

/**
 * Determines if initial approval is required for a command.
 */
fun requiresInitialApproval(
    policy: AskForApproval,
    sandboxPolicy: SandboxPolicy,
    command: List<String>,
    sandboxPermissions: SandboxPermissions
): Boolean {
    if (isKnownSafeCommand(command)) {
        return false
    }
    return when (policy) {
        AskForApproval.Never, AskForApproval.OnFailure -> false
        AskForApproval.OnRequest -> {
            // In DangerFullAccess, only prompt if the command looks dangerous.
            if (sandboxPolicy is SandboxPolicy.DangerFullAccess) {
                return commandMightBeDangerous(command)
            }

            // In restricted sandboxes (ReadOnly/WorkspaceWrite), do not prompt for
            // non-escalated, non-dangerous commands — let the sandbox enforce
            // restrictions (e.g., block network/write) without a user prompt.
            if (sandboxPermissions.requiresEscalatedPermissions()) {
                return true
            }
            commandMightBeDangerous(command)
        }
        AskForApproval.UnlessTrusted -> !isKnownSafeCommand(command)
    }
}

/**
 * Checks if a command might be dangerous.
 */
fun commandMightBeDangerous(command: List<String>): Boolean {
    // Windows check
    if (isDangerousCommandWindows(command)) {
        return true
    }

    if (isDangerousToCallWithExec(command)) {
        return true
    }

    // Support `bash -lc "<script>"` where any part of the script might contain a dangerous command.
    val allCommands = parseShellLcPlainCommands(command)
    if (allCommands != null && allCommands.any { cmd -> isDangerousToCallWithExec(cmd) }) {
        return true
    }

    return false
}

private fun isDangerousToCallWithExec(command: List<String>): Boolean {
    val cmd0 = command.firstOrNull()

    return when {
        cmd0 != null && (cmd0.endsWith("git") || cmd0.endsWith("/git")) -> {
            val subCommand = command.getOrNull(1)
            subCommand == "reset" || subCommand == "rm"
        }

        cmd0 == "rm" -> {
            val arg1 = command.getOrNull(1)
            arg1 == "-f" || arg1 == "-rf"
        }

        // for sudo <cmd> simply do the check for <cmd>
        cmd0 == "sudo" -> isDangerousToCallWithExec(command.drop(1))

        // ── anything else ─────────────────────────────────────────────────
        else -> false
    }
}
