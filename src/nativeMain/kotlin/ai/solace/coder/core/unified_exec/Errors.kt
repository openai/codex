// port-lint: source core/src/unified_exec/errors.rs
package ai.solace.coder.core.unified_exec

import ai.solace.coder.core.ExecToolCallOutput

sealed class UnifiedExecError : Exception() {
    data class CreateSession(override val message: String) : UnifiedExecError()
    
    // Called "session" in the model's training.
    data class UnknownSessionId(val processId: String) : UnifiedExecError() {
        override val message: String = "Unknown session id $processId"
    }
    
    object WriteToStdin : UnifiedExecError() {
        override val message: String = "failed to write to stdin"
    }
    
    object MissingCommandLine : UnifiedExecError() {
        override val message: String = "missing command line for unified exec request"
    }
    
    data class SandboxDenied(
        override val message: String,
        val output: ExecToolCallOutput
    ) : UnifiedExecError()

    companion object {
        fun createSession(message: String): UnifiedExecError = CreateSession(message)
        fun sandboxDenied(message: String, output: ExecToolCallOutput): UnifiedExecError = SandboxDenied(message, output)
    }
}
