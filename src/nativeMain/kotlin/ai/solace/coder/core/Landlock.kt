// port-lint: source core/src/landlock.rs
package ai.solace.coder.core

import ai.solace.coder.protocol.SandboxPolicy
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

/**
 * Converts the sandbox policy into the CLI invocation for `codex-linux-sandbox`.
 */
fun createLinuxSandboxCommandArgs(
    command: List<String>,
    sandboxPolicy: SandboxPolicy,
    sandboxPolicyCwd: String
): List<String> {
    val sandboxPolicyJson = try {
        Json.encodeToString(sandboxPolicy)
    } catch (e: Exception) {
        throw RuntimeException("Failed to serialize SandboxPolicy to JSON", e)
    }

    val linuxCmd = mutableListOf<String>()
    linuxCmd.add("--sandbox-policy-cwd")
    linuxCmd.add(sandboxPolicyCwd)
    linuxCmd.add("--sandbox-policy")
    linuxCmd.add(sandboxPolicyJson)
    // Separator so that command arguments starting with `-` are not parsed as
    // options of the helper itself.
    linuxCmd.add("--")

    // Append the original tool command.
    linuxCmd.addAll(command)

    return linuxCmd
}
