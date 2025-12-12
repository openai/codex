// port-lint: source codex-rs/exec/src/exec_events.rs
package ai.solace.coder.exec

import ai.solace.coder.protocol.ContentBlock
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement

/**
 * Top-level JSONL events emitted by codex exec.
 */
@Serializable
sealed class ThreadEvent {
    /**
     * Emitted when a new thread is started as the first event.
     */
    @Serializable
    @SerialName("thread.started")
    data class ThreadStarted(val event: ThreadStartedEvent) : ThreadEvent()

    /**
     * Emitted when a turn is started by sending a new prompt to the model.
     * A turn encompasses all events that happen while agent is processing the prompt.
     */
    @Serializable
    @SerialName("turn.started")
    data class TurnStarted(val event: TurnStartedEvent) : ThreadEvent()

    /**
     * Emitted when a turn is completed. Typically right after the assistant's response.
     */
    @Serializable
    @SerialName("turn.completed")
    data class TurnCompleted(val event: TurnCompletedEvent) : ThreadEvent()

    /**
     * Indicates that a turn failed with an error.
     */
    @Serializable
    @SerialName("turn.failed")
    data class TurnFailed(val event: TurnFailedEvent) : ThreadEvent()

    /**
     * Emitted when a new item is added to the thread. Typically the item will be in an "in progress" state.
     */
    @Serializable
    @SerialName("item.started")
    data class ItemStarted(val event: ItemStartedEvent) : ThreadEvent()

    /**
     * Emitted when an item is updated.
     */
    @Serializable
    @SerialName("item.updated")
    data class ItemUpdated(val event: ItemUpdatedEvent) : ThreadEvent()

    /**
     * Signals that an item has reached a terminal state—either success or failure.
     */
    @Serializable
    @SerialName("item.completed")
    data class ItemCompleted(val event: ItemCompletedEvent) : ThreadEvent()

    /**
     * Represents an unrecoverable error emitted directly by the event stream.
     */
    @Serializable
    @SerialName("error")
    data class Error(val event: ThreadErrorEvent) : ThreadEvent()
}

@Serializable
data class ThreadStartedEvent(
    /**
     * The identifier of the new thread. Can be used to resume the thread later.
     */
    @SerialName("thread_id")
    val threadId: String
)

@Serializable
data class TurnStartedEvent(
    // Empty struct in Rust, but we keep it for API consistency
    val placeholder: Unit = Unit
)

@Serializable
data class TurnCompletedEvent(
    val usage: Usage
)

@Serializable
data class TurnFailedEvent(
    val error: ThreadErrorEvent
)

/**
 * Describes the usage of tokens during a turn.
 */
@Serializable
data class Usage(
    /**
     * The number of input tokens used during the turn.
     */
    @SerialName("input_tokens")
    val inputTokens: Long = 0,

    /**
     * The number of cached input tokens used during the turn.
     */
    @SerialName("cached_input_tokens")
    val cachedInputTokens: Long = 0,

    /**
     * The number of output tokens used during the turn.
     */
    @SerialName("output_tokens")
    val outputTokens: Long = 0
)

@Serializable
data class ItemStartedEvent(
    val item: ThreadItem
)

@Serializable
data class ItemCompletedEvent(
    val item: ThreadItem
)

@Serializable
data class ItemUpdatedEvent(
    val item: ThreadItem
)

/**
 * Fatal error emitted by the stream.
 */
@Serializable
data class ThreadErrorEvent(
    val message: String
)

/**
 * Canonical representation of a thread item and its domain-specific payload.
 */
@Serializable
data class ThreadItem(
    val id: String,
    /**
     * Flattened details (using JsonContentPolymorphicSerializer would require more setup)
     * In Rust this uses #[serde(flatten)], we'll handle via custom serializer or flattening manually.
     */
    val details: ThreadItemDetails
)

/**
 * Typed payloads for each supported thread item type.
 */
@Serializable
sealed class ThreadItemDetails {
    /**
     * Response from the agent.
     * Either a natural-language response or a JSON string when structured output is requested.
     */
    @Serializable
    @SerialName("agent_message")
    data class AgentMessage(val item: AgentMessageItem) : ThreadItemDetails()

    /**
     * Agent's reasoning summary.
     */
    @Serializable
    @SerialName("reasoning")
    data class Reasoning(val item: ReasoningItem) : ThreadItemDetails()

    /**
     * Tracks a command executed by the agent. The item starts when the command is
     * spawned, and completes when the process exits with an exit code.
     */
    @Serializable
    @SerialName("command_execution")
    data class CommandExecution(val item: CommandExecutionItem) : ThreadItemDetails()

    /**
     * Represents a set of file changes by the agent. The item is emitted only as a
     * completed event once the patch succeeds or fails.
     */
    @Serializable
    @SerialName("file_change")
    data class FileChange(val item: FileChangeItem) : ThreadItemDetails()

    /**
     * Represents a call to an MCP tool. The item starts when the invocation is
     * dispatched and completes when the MCP server reports success or failure.
     */
    @Serializable
    @SerialName("mcp_tool_call")
    data class McpToolCall(val item: McpToolCallItem) : ThreadItemDetails()

    /**
     * Captures a web search request. It starts when the search is kicked off
     * and completes when results are returned to the agent.
     */
    @Serializable
    @SerialName("web_search")
    data class WebSearch(val item: WebSearchItem) : ThreadItemDetails()

    /**
     * Tracks the agent's running to-do list. It starts when the plan is first
     * issued, updates as steps change state, and completes when the turn ends.
     */
    @Serializable
    @SerialName("todo_list")
    data class TodoList(val item: TodoListItem) : ThreadItemDetails()

    /**
     * Describes a non-fatal error surfaced as an item.
     */
    @Serializable
    @SerialName("error")
    data class Error(val item: ErrorItem) : ThreadItemDetails()
}

/**
 * Response from the agent.
 * Either a natural-language response or a JSON string when structured output is requested.
 */
@Serializable
data class AgentMessageItem(
    val text: String
)

/**
 * Agent's reasoning summary.
 */
@Serializable
data class ReasoningItem(
    val text: String
)

/**
 * The status of a command execution.
 */
@Serializable
enum class CommandExecutionStatus {
    @SerialName("in_progress")
    IN_PROGRESS,

    @SerialName("completed")
    COMPLETED,

    @SerialName("failed")
    FAILED,

    @SerialName("declined")
    DECLINED;

    companion object {
        val DEFAULT = IN_PROGRESS
    }
}

/**
 * A command executed by the agent.
 */
@Serializable
data class CommandExecutionItem(
    val command: String,

    @SerialName("aggregated_output")
    val aggregatedOutput: String,

    @SerialName("exit_code")
    val exitCode: Int? = null,

    val status: CommandExecutionStatus = CommandExecutionStatus.DEFAULT
)

/**
 * A set of file changes by the agent.
 */
@Serializable
data class FileUpdateChange(
    val path: String,
    val kind: PatchChangeKind
)

/**
 * The status of a file change.
 */
@Serializable
enum class PatchApplyStatus {
    @SerialName("in_progress")
    IN_PROGRESS,

    @SerialName("completed")
    COMPLETED,

    @SerialName("failed")
    FAILED
}

/**
 * A set of file changes by the agent.
 */
@Serializable
data class FileChangeItem(
    val changes: List<FileUpdateChange>,
    val status: PatchApplyStatus
)

/**
 * Indicates the type of the file change.
 */
@Serializable
enum class PatchChangeKind {
    @SerialName("add")
    ADD,

    @SerialName("delete")
    DELETE,

    @SerialName("update")
    UPDATE
}

/**
 * The status of an MCP tool call.
 */
@Serializable
enum class McpToolCallStatus {
    @SerialName("in_progress")
    IN_PROGRESS,

    @SerialName("completed")
    COMPLETED,

    @SerialName("failed")
    FAILED;

    companion object {
        val DEFAULT = IN_PROGRESS
    }
}

/**
 * Result payload produced by an MCP tool invocation.
 */
@Serializable
data class McpToolCallItemResult(
    val content: List<ContentBlock>,

    @SerialName("structured_content")
    val structuredContent: JsonElement? = null
)

/**
 * Error details reported by a failed MCP tool invocation.
 */
@Serializable
data class McpToolCallItemError(
    val message: String
)

/**
 * A call to an MCP tool.
 */
@Serializable
data class McpToolCallItem(
    val server: String,
    val tool: String,
    val arguments: JsonElement = kotlinx.serialization.json.JsonNull,
    val result: McpToolCallItemResult? = null,
    val error: McpToolCallItemError? = null,
    val status: McpToolCallStatus = McpToolCallStatus.DEFAULT
)

/**
 * A web search request.
 */
@Serializable
data class WebSearchItem(
    val query: String
)

/**
 * An error notification.
 */
@Serializable
data class ErrorItem(
    val message: String
)

/**
 * An item in agent's to-do list.
 */
@Serializable
data class TodoItem(
    val text: String,
    val completed: Boolean
)

@Serializable
data class TodoListItem(
    val items: List<TodoItem>
)
