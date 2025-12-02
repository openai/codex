// port-lint: source core/src/tools/handlers/plan.rs
package ai.solace.coder.core.tools.handlers

import ai.solace.coder.core.error.CodexError
import ai.solace.coder.core.error.CodexResult
import ai.solace.coder.core.tools.ToolHandler
import ai.solace.coder.core.tools.ToolInvocation
import ai.solace.coder.core.tools.ToolKind
import ai.solace.coder.core.tools.ToolOutput
import ai.solace.coder.core.tools.ToolPayload
import ai.solace.coder.protocol.StepStatus
import ai.solace.coder.protocol.UpdatePlanArgs
import kotlinx.serialization.json.Json

/**
 * Handler for the update_plan tool.
 * Allows the model to record and update its task plan.
 *
 * This tool doesn't do anything useful computationally. However, it gives the model
 * a structured way to record its plan that clients can read and render.
 * The _inputs_ to this function are useful to clients, not the outputs.
 *
 * Ported from Rust codex-rs/core/src/tools/handlers/plan.rs
 */
class PlanHandler : ToolHandler {

    override val kind: ToolKind = ToolKind.Function

    override suspend fun handle(invocation: ToolInvocation): CodexResult<ToolOutput> {
        val payload = invocation.payload
        if (payload !is ToolPayload.Function) {
            return CodexResult.failure(
                CodexError.Fatal("update_plan handler received unsupported payload")
            )
        }

        val args = try {
            json.decodeFromString<UpdatePlanArgs>(payload.arguments)
        } catch (e: Exception) {
            return CodexResult.failure(
                CodexError.Fatal("failed to parse function arguments: ${e.message}")
            )
        }

        // Validate that at most one step is in_progress
        val inProgressCount = args.plan.count {
            it.status == StepStatus.InProgress
        }
        if (inProgressCount > 1) {
            return CodexResult.failure(
                CodexError.Fatal("at most one step can be in_progress at a time")
            )
        }

        // The actual plan update event would be sent through the session
        // For now, we return success - the session layer handles event emission
        return CodexResult.success(
            ToolOutput.Function(
                content = "Plan updated",
                contentItems = null,
                success = true
            )
        )
    }

    companion object {
        private val json = Json {
            ignoreUnknownKeys = true
            isLenient = true
        }
    }
}
