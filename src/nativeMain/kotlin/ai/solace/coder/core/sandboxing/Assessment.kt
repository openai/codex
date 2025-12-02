// port-lint: source core/src/sandboxing/assessment.rs
package ai.solace.coder.core.sandboxing

import ai.solace.coder.protocol.SandboxCommandAssessment
import ai.solace.coder.protocol.SandboxPolicy

// TODO: Port logic from core/src/sandboxing/assessment.rs
// This file should contain logic to assess the risk of a command.

fun assessCommand(
    command: List<String>,
    policy: SandboxPolicy
): SandboxCommandAssessment {
    if (command.isEmpty()) return SandboxCommandAssessment.Low

    val program = command.first().lowercase().substringAfterLast("/")
    
    // High risk commands
    val highRisk = setOf(
        "rm", "dd", "mkfs", "fdisk", "mount", "umount", "chown", "chmod", "sudo", "su"
    )
    
    // Medium risk commands
    val mediumRisk = setOf(
        "mv", "cp", "ln", "kill", "pkill", "killall", "shutdown", "reboot"
    )

    if (highRisk.contains(program)) {
        return SandboxCommandAssessment.High
    }
    
    if (mediumRisk.contains(program)) {
        return SandboxCommandAssessment.Medium
    }
    
    // Check for suspicious arguments in common commands
    if (program == "git" && command.contains("clean")) {
        return SandboxCommandAssessment.Medium
    }

    return SandboxCommandAssessment.Low
}
