// port-lint: source protocol/src/protocol.rs
package ai.solace.coder.protocol

import kotlinx.serialization.Serializable
import kotlinx.serialization.SerialName
import kotlinx.serialization.json.JsonElement
import kotlinx.cinterop.*
import platform.posix.getenv

// ========== Constants ==========

/**
 * Helper function to get environment variable in a platform-agnostic way.
 */
@OptIn(ExperimentalForeignApi::class)
private fun getEnvironmentVariable(name: String): String? {
    return getenv(name)?.toKString()
}

/**
 * Open/close tags for special user-input blocks.
 */
const val USER_INSTRUCTIONS_OPEN_TAG = "<user_instructions>"
const val USER_INSTRUCTIONS_CLOSE_TAG = "</user_instructions>"
const val ENVIRONMENT_CONTEXT_OPEN_TAG = "<environment_context>"
const val ENVIRONMENT_CONTEXT_CLOSE_TAG = "</environment_context>"
const val USER_MESSAGE_BEGIN = "## My request for Codex:"

// ========== Core Protocol Types ==========

/**
 * Submission Queue Entry - requests from user
 */
@Serializable
data class Submission(
    val id: String,
    val op: Op
)

/**
 * Submission operation - tagged union for all user operations.
 */
@Serializable
sealed class Op {
    @Serializable
    @SerialName("interrupt")
    object Interrupt : Op()

    @Serializable
    @SerialName("user_input")
    data class UserInput(
        val items: List<ai.solace.coder.protocol.UserInput>
    ) : Op()

    @Serializable
    @SerialName("user_turn")
    data class UserTurn(
        val items: List<ai.solace.coder.protocol.UserInput>,
        val cwd: String,
        @SerialName("approval_policy")
        val approvalPolicy: AskForApproval,
        @SerialName("sandbox_policy")
        val sandboxPolicy: SandboxPolicy,
        val model: String,
        val effort: ReasoningEffort? = null,
        val summary: ReasoningSummary,
        @SerialName("final_output_json_schema")
        val finalOutputJsonSchema: JsonElement? = null
    ) : Op()

    @Serializable
    @SerialName("override_turn_context")
    data class OverrideTurnContext(
        val cwd: String? = null,
        @SerialName("approval_policy")
        val approvalPolicy: AskForApproval? = null,
        @SerialName("sandbox_policy")
        val sandboxPolicy: SandboxPolicy? = null,
        val model: String? = null,
        val effort: ReasoningEffort? = null,
        val summary: ReasoningSummary? = null
    ) : Op()

    @Serializable
    @SerialName("exec_approval")
    data class ExecApproval(
        val id: String,
        val decision: ReviewDecision
    ) : Op()

    @Serializable
    @SerialName("patch_approval")
    data class PatchApproval(
        val id: String,
        val decision: ReviewDecision
    ) : Op()

    @Serializable
    @SerialName("resolve_elicitation")
    data class ResolveElicitation(
        @SerialName("server_name")
        val serverName: String,
        @SerialName("request_id")
        val requestId: String,
        val decision: ElicitationAction
    ) : Op()

    @Serializable
    @SerialName("add_to_history")
    data class AddToHistory(
        val text: String
    ) : Op()

    @Serializable
    @SerialName("get_history_entry_request")
    data class GetHistoryEntryRequest(
        val offset: Long,
        @SerialName("log_id")
        val logId: Long
    ) : Op()

    @Serializable
    @SerialName("list_mcp_tools")
    object ListMcpTools : Op()

    @Serializable
    @SerialName("list_custom_prompts")
    object ListCustomPrompts : Op()

    @Serializable
    @SerialName("compact")
    object Compact : Op()

    @Serializable
    @SerialName("undo")
    object Undo : Op()

    @Serializable
    @SerialName("review")
    data class Review(
        @SerialName("review_request")
        val reviewRequest: ReviewRequest
    ) : Op()

    @Serializable
    @SerialName("shutdown")
    object Shutdown : Op()

    @Serializable
    @SerialName("run_user_shell_command")
    data class RunUserShellCommand(
        val command: String
    ) : Op()
}

/**
 * Determines when user approval is required for commands.
 */
@Serializable
enum class AskForApproval {
    @SerialName("untrusted")
    UnlessTrusted,

    @SerialName("on-failure")
    OnFailure,

    @SerialName("on-request")
    OnRequest,

    @SerialName("never")
    Never
}

/**
 * Determines execution restrictions for model shell commands.
 */
@Serializable
sealed class SandboxPolicy {
    @Serializable
    @SerialName("danger-full-access")
    object DangerFullAccess : SandboxPolicy()

    @Serializable
    @SerialName("read-only")
    object ReadOnly : SandboxPolicy()

    @Serializable
    @SerialName("workspace-write")
    data class WorkspaceWrite(
        @SerialName("writable_roots")
        val writableRoots: List<String> = emptyList(),
        @SerialName("network_access")
        val networkAccess: Boolean = false,
        @SerialName("exclude_tmpdir_env_var")
        val excludeTmpdirEnvVar: Boolean = false,
        @SerialName("exclude_slash_tmp")
        val excludeSlashTmp: Boolean = false
    ) : SandboxPolicy()

    fun hasFullDiskReadAccess(): Boolean = true

    fun hasFullDiskWriteAccess(): Boolean = when (this) {
        is DangerFullAccess -> true
        is ReadOnly -> false
        is WorkspaceWrite -> false
    }

    fun hasFullNetworkAccess(): Boolean = when (this) {
        is DangerFullAccess -> true
        is ReadOnly -> false
        is WorkspaceWrite -> networkAccess
    }

    /**
     * Returns the list of writable roots with read-only subpaths.
     */
    fun getWritableRootsWithCwd(cwd: String): List<WritableRoot> {
        return when (this) {
            is DangerFullAccess -> emptyList()
            is ReadOnly -> emptyList()
            is WorkspaceWrite -> {
                val roots = mutableListOf<String>()
                roots.addAll(writableRoots)
                roots.add(cwd)

                if (!excludeSlashTmp) {
                    roots.add("/tmp")
                }

                if (!excludeTmpdirEnvVar) {
                    val tmpdir = getEnvironmentVariable("TMPDIR")
                    if (tmpdir != null && tmpdir.isNotEmpty()) {
                        roots.add(tmpdir)
                    }
                }

                roots.map { root ->
                    val gitPath = if (root.endsWith("/")) "${root}.git" else "$root/.git"
                    WritableRoot(
                        root = root,
                        readOnlySubpaths = listOf(gitPath)
                    )
                }
            }
        }
    }

    companion object {
        fun newReadOnlyPolicy(): SandboxPolicy = ReadOnly
        fun newWorkspaceWritePolicy(): SandboxPolicy = WorkspaceWrite()
    }
}

/**
 * A writable root path with read-only subpaths.
 */
@Serializable
data class WritableRoot(
    val root: String,
    @SerialName("read_only_subpaths")
    val readOnlySubpaths: List<String>
) {
    fun isPathWritable(path: String): Boolean {
        if (!path.startsWith(root)) {
            return false
        }
        for (subpath in readOnlySubpaths) {
            if (path.startsWith(subpath)) {
                return false
            }
        }
        return true
    }
}

/**
 * Event Queue Entry - events from agent.
 */
@Serializable
data class Event(
    val id: String,
    val msg: EventMsg
)

/**
 * Response event from the agent - tagged union with 40+ event types.
 */
@Serializable
sealed class EventMsg {
    @Serializable
    @SerialName("error")
    data class Error(val payload: ErrorEvent) : EventMsg()

    @Serializable
    @SerialName("warning")
    data class Warning(val payload: WarningEvent) : EventMsg()

    @Serializable
    @SerialName("context_compacted")
    data class ContextCompacted(val payload: ContextCompactedEvent) : EventMsg()

    @Serializable
    @SerialName("task_started")
    data class TaskStarted(val payload: TaskStartedEvent) : EventMsg()

    @Serializable
    @SerialName("task_complete")
    data class TaskComplete(val payload: TaskCompleteEvent) : EventMsg()

    @Serializable
    @SerialName("token_count")
    data class TokenCount(val payload: TokenCountEvent) : EventMsg()

    @Serializable
    @SerialName("agent_message")
    data class AgentMessage(val payload: AgentMessageEvent) : EventMsg()

    @Serializable
    @SerialName("user_message")
    data class UserMessage(val payload: UserMessageEvent) : EventMsg()

    @Serializable
    @SerialName("agent_message_delta")
    data class AgentMessageDelta(val payload: AgentMessageDeltaEvent) : EventMsg()

    @Serializable
    @SerialName("agent_reasoning")
    data class AgentReasoning(val payload: AgentReasoningEvent) : EventMsg()

    @Serializable
    @SerialName("agent_reasoning_delta")
    data class AgentReasoningDelta(val payload: AgentReasoningDeltaEvent) : EventMsg()

    @Serializable
    @SerialName("agent_reasoning_raw_content")
    data class AgentReasoningRawContent(val payload: AgentReasoningRawContentEvent) : EventMsg()

    @Serializable
    @SerialName("agent_reasoning_raw_content_delta")
    data class AgentReasoningRawContentDelta(val payload: AgentReasoningRawContentDeltaEvent) : EventMsg()

    @Serializable
    @SerialName("agent_reasoning_section_break")
    data class AgentReasoningSectionBreak(val payload: AgentReasoningSectionBreakEvent) : EventMsg()

    @Serializable
    @SerialName("session_configured")
    data class SessionConfigured(val payload: SessionConfiguredEvent) : EventMsg()

    @Serializable
    @SerialName("mcp_startup_update")
    data class McpStartupUpdate(val payload: McpStartupUpdateEvent) : EventMsg()

    @Serializable
    @SerialName("mcp_startup_complete")
    data class McpStartupComplete(val payload: McpStartupCompleteEvent) : EventMsg()

    @Serializable
    @SerialName("mcp_tool_call_begin")
    data class McpToolCallBegin(val payload: McpToolCallBeginEvent) : EventMsg()

    @Serializable
    @SerialName("mcp_tool_call_end")
    data class McpToolCallEnd(val payload: McpToolCallEndEvent) : EventMsg()

    @Serializable
    @SerialName("web_search_begin")
    data class WebSearchBegin(val payload: WebSearchBeginEvent) : EventMsg()

    @Serializable
    @SerialName("web_search_end")
    data class WebSearchEnd(val payload: WebSearchEndEvent) : EventMsg()

    @Serializable
    @SerialName("exec_command_begin")
    data class ExecCommandBegin(val payload: ExecCommandBeginEvent) : EventMsg()

    @Serializable
    @SerialName("exec_command_output_delta")
    data class ExecCommandOutputDelta(val payload: ExecCommandOutputDeltaEvent) : EventMsg()

    @Serializable
    @SerialName("exec_command_end")
    data class ExecCommandEnd(val payload: ExecCommandEndEvent) : EventMsg()

    @Serializable
    @SerialName("view_image_tool_call")
    data class ViewImageToolCall(val payload: ViewImageToolCallEvent) : EventMsg()

    @Serializable
    @SerialName("exec_approval_request")
    data class ExecApprovalRequest(val payload: ExecApprovalRequestEvent) : EventMsg()

    @Serializable
    @SerialName("elicitation_request")
    data class ElicitationRequest(val payload: ElicitationRequestEvent) : EventMsg()

    @Serializable
    @SerialName("apply_patch_approval_request")
    data class ApplyPatchApprovalRequest(val payload: ApplyPatchApprovalRequestEvent) : EventMsg()

    @Serializable
    @SerialName("deprecation_notice")
    data class DeprecationNotice(val payload: DeprecationNoticeEvent) : EventMsg()

    @Serializable
    @SerialName("background_event")
    data class BackgroundEvent(val payload: BackgroundEventEvent) : EventMsg()

    @Serializable
    @SerialName("undo_started")
    data class UndoStarted(val payload: UndoStartedEvent) : EventMsg()

    @Serializable
    @SerialName("undo_completed")
    data class UndoCompleted(val payload: UndoCompletedEvent) : EventMsg()

    @Serializable
    @SerialName("stream_error")
    data class StreamError(val payload: StreamErrorEvent) : EventMsg()

    @Serializable
    @SerialName("patch_apply_begin")
    data class PatchApplyBegin(val payload: PatchApplyBeginEvent) : EventMsg()

    @Serializable
    @SerialName("patch_apply_end")
    data class PatchApplyEnd(val payload: PatchApplyEndEvent) : EventMsg()

    @Serializable
    @SerialName("turn_diff")
    data class TurnDiff(val payload: TurnDiffEvent) : EventMsg()

    @Serializable
    @SerialName("get_history_entry_response")
    data class GetHistoryEntryResponse(val payload: GetHistoryEntryResponseEvent) : EventMsg()

    @Serializable
    @SerialName("mcp_list_tools_response")
    data class McpListToolsResponse(val payload: McpListToolsResponseEvent) : EventMsg()

    @Serializable
    @SerialName("list_custom_prompts_response")
    data class ListCustomPromptsResponse(val payload: ListCustomPromptsResponseEvent) : EventMsg()

    @Serializable
    @SerialName("plan_update")
    data class PlanUpdate(val payload: UpdatePlanArgs) : EventMsg()

    @Serializable
    @SerialName("turn_aborted")
    data class TurnAborted(val payload: TurnAbortedEvent) : EventMsg()

    @Serializable
    @SerialName("shutdown_complete")
    object ShutdownComplete : EventMsg()

    @Serializable
    @SerialName("entered_review_mode")
    data class EnteredReviewMode(val payload: ReviewRequest) : EventMsg()

    @Serializable
    @SerialName("exited_review_mode")
    data class ExitedReviewMode(val payload: ExitedReviewModeEvent) : EventMsg()

    @Serializable
    @SerialName("raw_response_item")
    data class RawResponseItem(val payload: RawResponseItemEvent) : EventMsg()

    @Serializable
    @SerialName("item_started")
    data class ItemStarted(val payload: ItemStartedEvent) : EventMsg()

    @Serializable
    @SerialName("item_completed")
    data class ItemCompleted(val payload: ItemCompletedEvent) : EventMsg()

    @Serializable
    @SerialName("agent_message_content_delta")
    data class AgentMessageContentDelta(val payload: AgentMessageContentDeltaEvent) : EventMsg()

    @Serializable
    @SerialName("reasoning_content_delta")
    data class ReasoningContentDelta(val payload: ReasoningContentDeltaEvent) : EventMsg()

    @Serializable
    @SerialName("reasoning_raw_content_delta")
    data class ReasoningRawContentDelta(val payload: ReasoningRawContentDeltaEvent) : EventMsg()
}

/**
 * Codex error information.
 */
@Serializable
sealed class CodexErrorInfo {
    @Serializable
    @SerialName("context_window_exceeded")
    object ContextWindowExceeded : CodexErrorInfo()

    @Serializable
    @SerialName("usage_limit_exceeded")
    object UsageLimitExceeded : CodexErrorInfo()

    @Serializable
    @SerialName("http_connection_failed")
    data class HttpConnectionFailed(
        @SerialName("http_status_code")
        val httpStatusCode: Int? = null
    ) : CodexErrorInfo()

    @Serializable
    @SerialName("response_stream_connection_failed")
    data class ResponseStreamConnectionFailed(
        @SerialName("http_status_code")
        val httpStatusCode: Int? = null
    ) : CodexErrorInfo()

    @Serializable
    @SerialName("internal_server_error")
    object InternalServerError : CodexErrorInfo()

    @Serializable
    @SerialName("unauthorized")
    object Unauthorized : CodexErrorInfo()

    @Serializable
    @SerialName("bad_request")
    object BadRequest : CodexErrorInfo()

    @Serializable
    @SerialName("sandbox_error")
    object SandboxError : CodexErrorInfo()

    @Serializable
    @SerialName("response_stream_disconnected")
    data class ResponseStreamDisconnected(
        @SerialName("http_status_code")
        val httpStatusCode: Int? = null
    ) : CodexErrorInfo()

    @Serializable
    @SerialName("response_too_many_failed_attempts")
    data class ResponseTooManyFailedAttempts(
        @SerialName("http_status_code")
        val httpStatusCode: Int? = null
    ) : CodexErrorInfo()

    @Serializable
    @SerialName("other")
    object Other : CodexErrorInfo()
}

// ========== Event Payload Types ==========

@Serializable
data class ErrorEvent(
    val message: String,
    @SerialName("codex_error_info")
    val codexErrorInfo: CodexErrorInfo? = null
)

@Serializable
data class WarningEvent(
    val message: String
)

@Serializable
class ContextCompactedEvent

@Serializable
data class TaskStartedEvent(
    @SerialName("model_context_window")
    val modelContextWindow: Long? = null
)

@Serializable
data class TaskCompleteEvent(
    @SerialName("last_agent_message")
    val lastAgentMessage: String? = null
)

private const val BASELINE_TOKENS: Long = 12000

@Serializable
data class TokenUsage(
    @SerialName("input_tokens")
    val inputTokens: Long = 0,
    @SerialName("cached_input_tokens")
    val cachedInputTokens: Long = 0,
    @SerialName("output_tokens")
    val outputTokens: Long = 0,
    @SerialName("reasoning_output_tokens")
    val reasoningOutputTokens: Long = 0,
    @SerialName("total_tokens")
    val totalTokens: Long = 0
) {
    fun isZero(): Boolean = totalTokens == 0L
    fun cachedInput(): Long = cachedInputTokens.coerceAtLeast(0)
    fun nonCachedInput(): Long = (inputTokens - cachedInput()).coerceAtLeast(0)
    fun blendedTotal(): Long = (nonCachedInput() + outputTokens.coerceAtLeast(0)).coerceAtLeast(0)
    fun tokensInContextWindow(): Long = totalTokens

    fun percentOfContextWindowRemaining(contextWindow: Long): Long {
        if (contextWindow <= BASELINE_TOKENS) return 0
        val effectiveWindow = contextWindow - BASELINE_TOKENS
        val used = (tokensInContextWindow() - BASELINE_TOKENS).coerceAtLeast(0)
        val remaining = (effectiveWindow - used).coerceAtLeast(0)
        return ((remaining.toDouble() / effectiveWindow.toDouble()) * 100.0)
            .coerceIn(0.0, 100.0).toLong()
    }

    fun addAssign(other: TokenUsage): TokenUsage {
        return TokenUsage(
            inputTokens = inputTokens + other.inputTokens,
            cachedInputTokens = cachedInputTokens + other.cachedInputTokens,
            outputTokens = outputTokens + other.outputTokens,
            reasoningOutputTokens = reasoningOutputTokens + other.reasoningOutputTokens,
            totalTokens = totalTokens + other.totalTokens
        )
    }
}

@Serializable
data class TokenUsageInfo(
    @SerialName("total_token_usage")
    val totalTokenUsage: TokenUsage,
    @SerialName("last_token_usage")
    val lastTokenUsage: TokenUsage,
    @SerialName("model_context_window")
    val modelContextWindow: Long? = null
) {
    fun appendLastUsage(last: TokenUsage): TokenUsageInfo {
        return copy(totalTokenUsage = totalTokenUsage.addAssign(last), lastTokenUsage = last)
    }

    fun fillToContextWindow(contextWindow: Long): TokenUsageInfo {
        val delta = (contextWindow - totalTokenUsage.totalTokens).coerceAtLeast(0)
        return copy(
            modelContextWindow = contextWindow,
            totalTokenUsage = TokenUsage(totalTokens = contextWindow),
            lastTokenUsage = TokenUsage(totalTokens = delta)
        )
    }

    companion object {
        fun newOrAppend(info: TokenUsageInfo?, last: TokenUsage?, modelContextWindow: Long?): TokenUsageInfo? {
            if (info == null && last == null) return null
            var result = info ?: TokenUsageInfo(TokenUsage(), TokenUsage(), modelContextWindow)
            if (last != null) result = result.appendLastUsage(last)
            return result
        }

        fun fullContextWindow(contextWindow: Long): TokenUsageInfo {
            return TokenUsageInfo(TokenUsage(), TokenUsage(), contextWindow).fillToContextWindow(contextWindow)
        }
    }
}

@Serializable
data class FinalOutput(
    @SerialName("token_usage")
    val tokenUsage: TokenUsage
) {
    override fun toString(): String {
        return buildString {
            append("Token usage: total=")
            append(formatWithSeparators(tokenUsage.blendedTotal()))
            append(" input=")
            append(formatWithSeparators(tokenUsage.nonCachedInput()))
            if (tokenUsage.cachedInput() > 0) {
                append(" (+ ")
                append(formatWithSeparators(tokenUsage.cachedInput()))
                append(" cached)")
            }
            append(" output=")
            append(formatWithSeparators(tokenUsage.outputTokens))
            if (tokenUsage.reasoningOutputTokens > 0) {
                append(" (reasoning ")
                append(formatWithSeparators(tokenUsage.reasoningOutputTokens))
                append(")")
            }
        }
    }
}

@Serializable
data class TokenCountEvent(
    val info: TokenUsageInfo? = null,
    @SerialName("rate_limits")
    val rateLimits: RateLimitSnapshot? = null
)

@Serializable
data class RateLimitSnapshot(
    val primary: RateLimitWindow? = null,
    val secondary: RateLimitWindow? = null,
    val credits: CreditsSnapshot? = null
)

@Serializable
data class RateLimitWindow(
    @SerialName("used_percent")
    val usedPercent: Double,
    @SerialName("window_minutes")
    val windowMinutes: Long? = null,
    @SerialName("resets_at")
    val resetsAt: Long? = null
)

@Serializable
data class CreditsSnapshot(
    @SerialName("has_credits")
    val hasCredits: Boolean,
    val unlimited: Boolean,
    val balance: String? = null
)

@Serializable
data class AgentMessageEvent(val message: String)

@Serializable
data class UserMessageEvent(val message: String, val images: List<String>? = null)

@Serializable
data class AgentMessageDeltaEvent(val delta: String)

@Serializable
data class AgentReasoningEvent(val text: String)

@Serializable
data class AgentReasoningRawContentEvent(val text: String)

@Serializable
data class AgentReasoningRawContentDeltaEvent(val delta: String)

@Serializable
data class AgentReasoningSectionBreakEvent(
    @SerialName("item_id") val itemId: String = "",
    @SerialName("summary_index") val summaryIndex: Long = 0
)

@Serializable
data class AgentReasoningDeltaEvent(val delta: String)

@Serializable
data class McpInvocation(
    val server: String,
    val tool: String,
    val arguments: JsonElement? = null
)

@Serializable
data class McpToolCallBeginEvent(
    @SerialName("call_id") val callId: String,
    val invocation: McpInvocation
)

@Serializable
data class McpToolCallEndEvent(
    @SerialName("call_id") val callId: String,
    val invocation: McpInvocation,
    val duration: String,
    val result: McpResult<CallToolResult, String>
) {
    fun isSuccess(): Boolean = result.error == null && result.value?.isError != true
}

@Serializable
data class WebSearchBeginEvent(@SerialName("call_id") val callId: String)

@Serializable
data class WebSearchEndEvent(
    @SerialName("call_id") val callId: String,
    val query: String
)

@Serializable
enum class ExecCommandSource {
    @SerialName("agent") Agent,
    @SerialName("user_shell") UserShell,
    @SerialName("unified_exec_startup") UnifiedExecStartup,
    @SerialName("unified_exec_interaction") UnifiedExecInteraction
}

@Serializable
data class ExecCommandBeginEvent(
    @SerialName("call_id") val callId: String,
    @SerialName("process_id") val processId: String? = null,
    @SerialName("turn_id") val turnId: String,
    val command: List<String>,
    val cwd: String,
    @SerialName("parsed_cmd") val parsedCmd: List<ParsedCommand>,
    val source: ExecCommandSource = ExecCommandSource.Agent,
    @SerialName("interaction_input") val interactionInput: String? = null
)

@Serializable
data class ExecCommandEndEvent(
    @SerialName("call_id") val callId: String,
    @SerialName("process_id") val processId: String? = null,
    @SerialName("turn_id") val turnId: String,
    val command: List<String>,
    val cwd: String,
    @SerialName("parsed_cmd") val parsedCmd: List<ParsedCommand>,
    val source: ExecCommandSource = ExecCommandSource.Agent,
    @SerialName("interaction_input") val interactionInput: String? = null,
    val stdout: String,
    val stderr: String,
    @SerialName("aggregated_output") val aggregatedOutput: String = "",
    @SerialName("exit_code") val exitCode: Int,
    val duration: String,
    @SerialName("formatted_output") val formattedOutput: String
)

@Serializable
data class ViewImageToolCallEvent(
    @SerialName("call_id") val callId: String,
    val path: String
)

@Serializable
enum class ExecOutputStream {
    @SerialName("stdout") Stdout,
    @SerialName("stderr") Stderr
}

@Serializable
data class ExecCommandOutputDeltaEvent(
    @SerialName("call_id") val callId: String,
    val stream: ExecOutputStream,
    val chunk: ByteArray
) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other == null || this::class != other::class) return false
        other as ExecCommandOutputDeltaEvent
        return callId == other.callId && stream == other.stream && chunk.contentEquals(other.chunk)
    }
    override fun hashCode(): Int = 31 * (31 * callId.hashCode() + stream.hashCode()) + chunk.contentHashCode()
}

@Serializable
data class BackgroundEventEvent(val message: String)

@Serializable
data class DeprecationNoticeEvent(val summary: String, val details: String? = null)

@Serializable
data class UndoStartedEvent(val message: String? = null)

@Serializable
data class UndoCompletedEvent(val success: Boolean, val message: String? = null)

@Serializable
data class StreamErrorEvent(
    val message: String,
    @SerialName("codex_error_info") val codexErrorInfo: CodexErrorInfo? = null
)

@Serializable
data class PatchApplyBeginEvent(
    @SerialName("call_id") val callId: String,
    @SerialName("turn_id") val turnId: String = "",
    @SerialName("auto_approved") val autoApproved: Boolean,
    val changes: Map<String, FileChange>
)

@Serializable
data class PatchApplyEndEvent(
    @SerialName("call_id") val callId: String,
    @SerialName("turn_id") val turnId: String = "",
    val stdout: String,
    val stderr: String,
    val success: Boolean,
    val changes: Map<String, FileChange> = emptyMap()
)

@Serializable
data class TurnDiffEvent(@SerialName("unified_diff") val unifiedDiff: String)

@Serializable
data class GetHistoryEntryResponseEvent(
    val offset: Long,
    @SerialName("log_id") val logId: Long,
    val entry: HistoryEntry? = null
)

@Serializable
data class McpListToolsResponseEvent(
    val tools: Map<String, McpTool>,
    val resources: Map<String, List<McpResource>>,
    @SerialName("resource_templates") val resourceTemplates: Map<String, List<McpResourceTemplate>>,
    @SerialName("auth_statuses") val authStatuses: Map<String, McpAuthStatus>
)

@Serializable
data class McpStartupUpdateEvent(val server: String, val status: McpStartupStatus)

@Serializable
sealed class McpStartupStatus {
    @Serializable @SerialName("starting") object Starting : McpStartupStatus()
    @Serializable @SerialName("ready") object Ready : McpStartupStatus()
    @Serializable @SerialName("failed") data class Failed(val error: String) : McpStartupStatus()
    @Serializable @SerialName("cancelled") object Cancelled : McpStartupStatus()
}

@Serializable
data class McpStartupCompleteEvent(
    val ready: List<String> = emptyList(),
    val failed: List<McpStartupFailure> = emptyList(),
    val cancelled: List<String> = emptyList()
)

@Serializable
data class McpStartupFailure(val server: String, val error: String)

@Serializable
enum class McpAuthStatus {
    @SerialName("unsupported") Unsupported,
    @SerialName("not_logged_in") NotLoggedIn,
    @SerialName("bearer_token") BearerToken,
    @SerialName("oauth") OAuth
}

@Serializable
data class ListCustomPromptsResponseEvent(
    @SerialName("custom_prompts") val customPrompts: List<CustomPrompt>
)

@Serializable
data class SessionConfiguredEvent(
    @SerialName("session_id") val sessionId: ConversationId,
    val model: String,
    @SerialName("model_provider_id") val modelProviderId: String,
    @SerialName("approval_policy") val approvalPolicy: AskForApproval,
    @SerialName("sandbox_policy") val sandboxPolicy: SandboxPolicy,
    val cwd: String,
    @SerialName("reasoning_effort") val reasoningEffort: ReasoningEffort? = null,
    @SerialName("history_log_id") val historyLogId: Long,
    @SerialName("history_entry_count") val historyEntryCount: Long,
    @SerialName("initial_messages") val initialMessages: List<EventMsg>? = null,
    @SerialName("rollout_path") val rolloutPath: String
)

@Serializable
enum class ReviewDecision {
    @SerialName("approved") Approved,
    @SerialName("approved_for_session") ApprovedForSession,
    @SerialName("denied") Denied,
    @SerialName("abort") Abort
}

@Serializable
sealed class FileChange {
    @Serializable @SerialName("add") data class Add(val content: String) : FileChange()
    @Serializable @SerialName("delete") data class Delete(val content: String) : FileChange()
    @Serializable @SerialName("update") data class Update(
        @SerialName("unified_diff") val unifiedDiff: String,
        @SerialName("move_path") val movePath: String? = null
    ) : FileChange()
}

@Serializable
data class Chunk(
    @SerialName("orig_index") val origIndex: Int,
    @SerialName("deleted_lines") val deletedLines: List<String>,
    @SerialName("inserted_lines") val insertedLines: List<String>
)

@Serializable
data class TurnAbortedEvent(val reason: TurnAbortReason)

@Serializable
enum class TurnAbortReason {
    @SerialName("interrupted") Interrupted,
    @SerialName("replaced") Replaced,
    @SerialName("review_ended") ReviewEnded
}

// ========== Session Types ==========

@Serializable
sealed class InitialHistory {
    @Serializable @SerialName("new") object New : InitialHistory()
    @Serializable @SerialName("resumed") data class Resumed(val payload: ResumedHistory) : InitialHistory()
    @Serializable @SerialName("forked") data class Forked(val items: List<RolloutItem>) : InitialHistory()

    fun getRolloutItems(): List<RolloutItem> = when (this) {
        is New -> emptyList()
        is Resumed -> payload.history
        is Forked -> items
    }

    fun getEventMsgs(): List<EventMsg>? = when (this) {
        is New -> null
        is Resumed -> payload.history.mapNotNull { if (it is RolloutItem.EventMsgItem) it.payload else null }
        is Forked -> items.mapNotNull { if (it is RolloutItem.EventMsgItem) it.payload else null }
    }
}

@Serializable
data class ResumedHistory(
    @SerialName("conversation_id") val conversationId: ConversationId,
    val history: List<RolloutItem>,
    @SerialName("rollout_path") val rolloutPath: String
)

@Serializable
enum class SessionSource {
    @SerialName("cli") Cli,
    @SerialName("vscode") VSCode,
    @SerialName("exec") Exec,
    @SerialName("mcp") Mcp,
    @SerialName("subagent") SubAgent,
    @SerialName("unknown") Unknown
}

@Serializable
sealed class SubAgentSource {
    @Serializable @SerialName("review") object Review : SubAgentSource()
    @Serializable @SerialName("compact") object Compact : SubAgentSource()
    @Serializable @SerialName("other") data class Other(val name: String) : SubAgentSource()
}

@Serializable
data class SessionMeta(
    val id: ConversationId,
    val timestamp: String,
    val cwd: String,
    val originator: String,
    @SerialName("cli_version") val cliVersion: String,
    val instructions: String? = null,
    val source: SessionSource = SessionSource.VSCode,
    @SerialName("model_provider") val modelProvider: String? = null
)

@Serializable
data class SessionMetaLine(val meta: SessionMeta, val git: GitInfo? = null)

@Serializable
sealed class RolloutItem {
    @Serializable @SerialName("session_meta") data class SessionMetaItem(val payload: SessionMetaLine) : RolloutItem()
    @Serializable @SerialName("response_item") data class ResponseItemHolder(val payload: ResponseItem) : RolloutItem()
    @Serializable @SerialName("compacted") data class Compacted(val payload: CompactedItem) : RolloutItem()
    @Serializable @SerialName("turn_context") data class TurnContextHolder(val payload: TurnContextItem) : RolloutItem()
    @Serializable @SerialName("event_msg") data class EventMsgItem(val payload: EventMsg) : RolloutItem()
}

@Serializable
data class CompactedItem(
    val message: String,
    @SerialName("replacement_history") val replacementHistory: List<ResponseItem>? = null
)

@Serializable
data class TurnContextItem(
    val cwd: String,
    @SerialName("approval_policy") val approvalPolicy: AskForApproval,
    @SerialName("sandbox_policy") val sandboxPolicy: SandboxPolicy,
    val model: String,
    val effort: ReasoningEffort? = null,
    val summary: ReasoningSummary
)

@Serializable
data class RolloutLine(val timestamp: String, val item: RolloutItem)

@Serializable
data class GitInfo(
    @SerialName("commit_hash") val commitHash: String? = null,
    val branch: String? = null,
    @SerialName("repository_url") val repositoryUrl: String? = null
)

// ========== Review Types ==========

@Serializable
data class ReviewRequest(
    val prompt: String,
    @SerialName("user_facing_hint") val userFacingHint: String,
    @SerialName("append_to_original_thread") val appendToOriginalThread: Boolean = false
)

@Serializable
data class ReviewOutputEvent(
    val findings: List<ReviewFinding>,
    @SerialName("overall_correctness") val overallCorrectness: String,
    @SerialName("overall_explanation") val overallExplanation: String,
    @SerialName("overall_confidence_score") val overallConfidenceScore: Float
)

@Serializable
data class ReviewFinding(
    val title: String,
    val body: String,
    @SerialName("confidence_score") val confidenceScore: Float,
    val priority: Int,
    @SerialName("code_location") val codeLocation: ReviewCodeLocation
)

@Serializable
data class ReviewCodeLocation(
    @SerialName("absolute_file_path") val absoluteFilePath: String,
    @SerialName("line_range") val lineRange: ReviewLineRange
)

@Serializable
data class ReviewLineRange(val start: Int, val end: Int)

@Serializable
data class ExitedReviewModeEvent(@SerialName("review_output") val reviewOutput: ReviewOutputEvent? = null)

/**
 * Stream info event containing a message.
 *
 * Ported from Rust protocol/src/protocol.rs StreamInfoEvent.
 */
@Serializable
data class StreamInfoEvent(val message: String)

// ========== Item Events ==========

@Serializable
data class RawResponseItemEvent(val item: ResponseItem)

@Serializable
data class ItemStartedEvent(
    @SerialName("thread_id") val threadId: ConversationId,
    @SerialName("turn_id") val turnId: String,
    val item: TurnItem
) {
    fun asLegacyEvents(showRawAgentReasoning: Boolean): List<EventMsg> = when (val i = item) {
        is TurnItem.WebSearch -> listOf(EventMsg.WebSearchBegin(WebSearchBeginEvent(callId = i.item.id)))
        else -> emptyList()
    }
}

@Serializable
data class ItemCompletedEvent(
    @SerialName("thread_id") val threadId: ConversationId,
    @SerialName("turn_id") val turnId: String,
    val item: TurnItem
) {
    fun asLegacyEvents(showRawAgentReasoning: Boolean): List<EventMsg> = item.asLegacyEvents(showRawAgentReasoning)
}

@Serializable
data class AgentMessageContentDeltaEvent(
    @SerialName("thread_id") val threadId: String,
    @SerialName("turn_id") val turnId: String,
    @SerialName("item_id") val itemId: String,
    val delta: String
) {
    fun asLegacyEvents(): List<EventMsg> = listOf(EventMsg.AgentMessageDelta(AgentMessageDeltaEvent(delta = delta)))
}

@Serializable
data class ReasoningContentDeltaEvent(
    @SerialName("thread_id") val threadId: String,
    @SerialName("turn_id") val turnId: String,
    @SerialName("item_id") val itemId: String,
    val delta: String,
    @SerialName("summary_index") val summaryIndex: Long = 0
) {
    fun asLegacyEvents(): List<EventMsg> = listOf(EventMsg.AgentReasoningDelta(AgentReasoningDeltaEvent(delta = delta)))
}

@Serializable
data class ReasoningRawContentDeltaEvent(
    @SerialName("thread_id") val threadId: String,
    @SerialName("turn_id") val turnId: String,
    @SerialName("item_id") val itemId: String,
    val delta: String,
    @SerialName("content_index") val contentIndex: Long = 0
) {
    fun asLegacyEvents(): List<EventMsg> = listOf(EventMsg.AgentReasoningRawContentDelta(AgentReasoningRawContentDeltaEvent(delta = delta)))
}

// ========== MCP Types ==========

@Serializable
data class McpTool(val name: String, val description: String? = null)

@Serializable
data class McpResource(val uri: String, val name: String? = null)

@Serializable
data class McpResourceTemplate(
    @SerialName("uri_template") val uriTemplate: String,
    val name: String? = null
)

@Serializable
data class McpResult<T, E>(val value: T? = null, val error: E? = null) {
    val isSuccess: Boolean get() = error == null
    val isFailure: Boolean get() = error != null
}

interface HasLegacyEvent {
    fun asLegacyEvents(showRawAgentReasoning: Boolean): List<EventMsg>
}

@Serializable
data class ConversationPathResponseEvent(
    @SerialName("conversation_id") val conversationId: ConversationId,
    val path: String
)
