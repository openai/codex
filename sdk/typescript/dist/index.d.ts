import { ContentBlock } from '@modelcontextprotocol/sdk/types.js';

/** The status of a command execution. */
type CommandExecutionStatus = "in_progress" | "completed" | "failed";
/** A command executed by the agent. */
type CommandExecutionItem = {
    id: string;
    type: "command_execution";
    /** The command line executed by the agent. */
    command: string;
    /** Aggregated stdout and stderr captured while the command was running. */
    aggregated_output: string;
    /** Set when the command exits; omitted while still running. */
    exit_code?: number;
    /** Current status of the command execution. */
    status: CommandExecutionStatus;
};
/** Indicates the type of the file change. */
type PatchChangeKind = "add" | "delete" | "update";
/** A set of file changes by the agent. */
type FileUpdateChange = {
    path: string;
    kind: PatchChangeKind;
};
/** The status of a file change. */
type PatchApplyStatus = "completed" | "failed";
/** A set of file changes by the agent. Emitted once the patch succeeds or fails. */
type FileChangeItem = {
    id: string;
    type: "file_change";
    /** Individual file changes that comprise the patch. */
    changes: FileUpdateChange[];
    /** Whether the patch ultimately succeeded or failed. */
    status: PatchApplyStatus;
};
/** The status of an MCP tool call. */
type McpToolCallStatus = "in_progress" | "completed" | "failed";
/**
 * Represents a call to an MCP tool. The item starts when the invocation is dispatched
 * and completes when the MCP server reports success or failure.
 */
type McpToolCallItem = {
    id: string;
    type: "mcp_tool_call";
    /** Name of the MCP server handling the request. */
    server: string;
    /** The tool invoked on the MCP server. */
    tool: string;
    /** Arguments forwarded to the tool invocation. */
    arguments: unknown;
    /** Result payload returned by the MCP server for successful calls. */
    result?: {
        content: ContentBlock[];
        structured_content: unknown;
    };
    /** Error message reported for failed calls. */
    error?: {
        message: string;
    };
    /** Current status of the tool invocation. */
    status: McpToolCallStatus;
};
/** Response from the agent. Either natural-language text or JSON when structured output is requested. */
type AgentMessageItem = {
    id: string;
    type: "agent_message";
    /** Either natural-language text or JSON when structured output is requested. */
    text: string;
};
/** Agent's reasoning summary. */
type ReasoningItem = {
    id: string;
    type: "reasoning";
    text: string;
};
/** Captures a web search request. Completes when results are returned to the agent. */
type WebSearchItem = {
    id: string;
    type: "web_search";
    query: string;
};
/** Describes a non-fatal error surfaced as an item. */
type ErrorItem = {
    id: string;
    type: "error";
    message: string;
};
/** An item in the agent's to-do list. */
type TodoItem = {
    text: string;
    completed: boolean;
};
/**
 * Tracks the agent's running to-do list. Starts when the plan is issued, updates as steps change,
 * and completes when the turn ends.
 */
type TodoListItem = {
    id: string;
    type: "todo_list";
    items: TodoItem[];
};
/** Canonical union of thread items and their type-specific payloads. */
type ThreadItem = AgentMessageItem | ReasoningItem | CommandExecutionItem | FileChangeItem | McpToolCallItem | WebSearchItem | TodoListItem | ErrorItem;

/** Emitted when a new thread is started as the first event. */
type ThreadStartedEvent = {
    type: "thread.started";
    /** The identifier of the new thread. Can be used to resume the thread later. */
    thread_id: string;
};
/**
 * Emitted when a turn is started by sending a new prompt to the model.
 * A turn encompasses all events that happen while the agent is processing the prompt.
 */
type TurnStartedEvent = {
    type: "turn.started";
};
/** Describes the usage of tokens during a turn. */
type Usage = {
    /** The number of input tokens used during the turn. */
    input_tokens: number;
    /** The number of cached input tokens used during the turn. */
    cached_input_tokens: number;
    /** The number of output tokens used during the turn. */
    output_tokens: number;
};
/** Emitted when a turn is completed. Typically right after the assistant's response. */
type TurnCompletedEvent = {
    type: "turn.completed";
    usage: Usage;
};
/** Indicates that a turn failed with an error. */
type TurnFailedEvent = {
    type: "turn.failed";
    error: ThreadError;
};
/** Emitted when a new item is added to the thread. Typically the item is initially "in progress". */
type ItemStartedEvent = {
    type: "item.started";
    item: ThreadItem;
};
/** Emitted when an item is updated. */
type ItemUpdatedEvent = {
    type: "item.updated";
    item: ThreadItem;
};
/** Signals that an item has reached a terminal state—either success or failure. */
type ItemCompletedEvent = {
    type: "item.completed";
    item: ThreadItem;
};
/** Fatal error emitted by the stream. */
type ThreadError = {
    message: string;
};
/** Represents an unrecoverable error emitted directly by the event stream. */
type ThreadErrorEvent = {
    type: "error";
    message: string;
};
/** Top-level JSONL events emitted by codex exec. */
type ThreadEvent = ThreadStartedEvent | TurnStartedEvent | TurnCompletedEvent | TurnFailedEvent | ItemStartedEvent | ItemUpdatedEvent | ItemCompletedEvent | ThreadErrorEvent;

type TurnOptions = {
    /** JSON schema describing the expected agent output. */
    outputSchema?: unknown;
    /** AbortSignal to cancel the turn. */
    signal?: AbortSignal;
};

/** Completed turn. */
type Turn = {
    items: ThreadItem[];
    finalResponse: string;
    usage: Usage | null;
};
/** Alias for `Turn` to describe the result of `run()`. */
type RunResult = Turn;
/** The result of the `runStreamed` method. */
type StreamedTurn = {
    events: AsyncGenerator<ThreadEvent>;
};
/** Alias for `StreamedTurn` to describe the result of `runStreamed()`. */
type RunStreamedResult = StreamedTurn;
/** An input to send to the agent. */
type UserInput = {
    type: "text";
    text: string;
} | {
    type: "local_image";
    path: string;
};
type Input = string | UserInput[];
/** Respesent a thread of conversation with the agent. One thread can have multiple consecutive turns. */
declare class Thread {
    private _exec;
    private _options;
    private _id;
    private _threadOptions;
    /** Returns the ID of the thread. Populated after the first turn starts. */
    get id(): string | null;
    /** Provides the input to the agent and streams events as they are produced during the turn. */
    runStreamed(input: Input, turnOptions?: TurnOptions): Promise<StreamedTurn>;
    private runStreamedInternal;
    /** Provides the input to the agent and returns the completed turn. */
    run(input: Input, turnOptions?: TurnOptions): Promise<Turn>;
}

type CodexOptions = {
    codexPathOverride?: string;
    baseUrl?: string;
    apiKey?: string;
    /**
     * Environment variables passed to the Codex CLI process. When provided, the SDK
     * will not inherit variables from `process.env`.
     */
    env?: Record<string, string>;
};

type ApprovalMode = "never" | "on-request" | "on-failure" | "untrusted";
type SandboxMode = "read-only" | "workspace-write" | "danger-full-access";
type ModelReasoningEffort = "minimal" | "low" | "medium" | "high" | "xhigh";
type ThreadOptions = {
    model?: string;
    sandboxMode?: SandboxMode;
    workingDirectory?: string;
    skipGitRepoCheck?: boolean;
    modelReasoningEffort?: ModelReasoningEffort;
    networkAccessEnabled?: boolean;
    webSearchEnabled?: boolean;
    approvalPolicy?: ApprovalMode;
    additionalDirectories?: string[];
};

/**
 * Codex is the main class for interacting with the Codex agent.
 *
 * Use the `startThread()` method to start a new thread or `resumeThread()` to resume a previously started thread.
 */
declare class Codex {
    private exec;
    private options;
    constructor(options?: CodexOptions);
    /**
     * Starts a new conversation with an agent.
     * @returns A new thread instance.
     */
    startThread(options?: ThreadOptions): Thread;
    /**
     * Resumes a conversation with an agent based on the thread id.
     * Threads are persisted in ~/.codex/sessions.
     *
     * @param id The id of the thread to resume.
     * @returns A new thread instance.
     */
    resumeThread(id: string, options?: ThreadOptions): Thread;
}

export { type AgentMessageItem, type ApprovalMode, Codex, type CodexOptions, type CommandExecutionItem, type ErrorItem, type FileChangeItem, type Input, type ItemCompletedEvent, type ItemStartedEvent, type ItemUpdatedEvent, type McpToolCallItem, type ModelReasoningEffort, type ReasoningItem, type RunResult, type RunStreamedResult, type SandboxMode, Thread, type ThreadError, type ThreadErrorEvent, type ThreadEvent, type ThreadItem, type ThreadOptions, type ThreadStartedEvent, type TodoListItem, type TurnCompletedEvent, type TurnFailedEvent, type TurnOptions, type TurnStartedEvent, type Usage, type UserInput, type WebSearchItem };
