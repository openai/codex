// port-lint: source core/src/codex.rs
package ai.solace.coder.core.session

import ai.solace.coder.client.auth.AuthManager
import ai.solace.coder.client.auth.AuthMode
import ai.solace.coder.core.context.ContextManager
import ai.solace.coder.core.context.TruncationPolicy
import ai.solace.coder.core.context.truncateText
import ai.solace.coder.core.error.CodexError
import ai.solace.coder.core.error.CodexResult
import ai.solace.coder.core.features.Feature
import ai.solace.coder.core.features.Features
import ai.solace.coder.core.tools.ProcessedResponseItem
import ai.solace.coder.core.tools.ToolCall
import ai.solace.coder.core.tools.ToolCallProcessor
import ai.solace.coder.core.tools.ToolCallRuntime
import ai.solace.coder.core.tools.ToolRegistry
import ai.solace.coder.core.tools.ToolRouter
import ai.solace.coder.core.tools.ApplyPatchToolType
import ai.solace.coder.exec.sandbox.ApprovalStore
import ai.solace.coder.protocol.SandboxCommandAssessment
import ai.solace.coder.protocol.SandboxRiskLevel
import ai.solace.coder.exec.shell.Shell
import ai.solace.coder.exec.shell.ShellDetector
import ai.solace.coder.mcp.connection.McpConnectionManager
import ai.solace.coder.mcp.connection.McpServerConfig
import ai.solace.coder.protocol.ResponseEvent
import ai.solace.coder.utils.concurrent.CancellationToken
import ai.solace.coder.protocol.AskForApproval
import ai.solace.coder.protocol.ApplyPatchApprovalRequestEvent
import ai.solace.coder.protocol.BackgroundEventEvent
import ai.solace.coder.protocol.Event
import ai.solace.coder.protocol.EventMsg
import ai.solace.coder.protocol.ExecApprovalRequestEvent
import ai.solace.coder.protocol.ItemCompletedEvent
import ai.solace.coder.protocol.ItemStartedEvent
import ai.solace.coder.protocol.Op
import ai.solace.coder.protocol.RawResponseItemEvent
import ai.solace.coder.protocol.ReviewDecision
import ai.solace.coder.protocol.SandboxPolicy
import ai.solace.coder.protocol.StreamErrorEvent
import ai.solace.coder.protocol.Submission
import ai.solace.coder.protocol.TaskCompleteEvent
import ai.solace.coder.protocol.TaskStartedEvent
import ai.solace.coder.protocol.TokenCountEvent
import ai.solace.coder.protocol.TurnAbortReason
import ai.solace.coder.protocol.TurnAbortedEvent
import ai.solace.coder.protocol.TurnItem
import ai.solace.coder.protocol.UndoCompletedEvent
import ai.solace.coder.protocol.UndoStartedEvent
import ai.solace.coder.protocol.CodexErrorInfo
import ai.solace.coder.protocol.ErrorEvent
import ai.solace.coder.protocol.GetHistoryEntryResponseEvent
import ai.solace.coder.protocol.McpListToolsResponseEvent
import ai.solace.coder.protocol.ListCustomPromptsResponseEvent
import ai.solace.coder.protocol.SessionConfiguredEvent
import ai.solace.coder.protocol.McpTool
import ai.solace.coder.protocol.McpResource
import ai.solace.coder.protocol.McpResourceTemplate
import ai.solace.coder.protocol.McpAuthStatus
import ai.solace.coder.protocol.CustomPrompt
import ai.solace.coder.protocol.ReviewRequest
import ai.solace.coder.protocol.ElicitationAction
import ai.solace.coder.protocol.TokenUsage
import ai.solace.coder.protocol.TokenUsageInfo
import ai.solace.coder.protocol.RateLimitSnapshot
import ai.solace.coder.protocol.HistoryEntry
import ai.solace.coder.protocol.FileChange
import ai.solace.coder.protocol.ReasoningEffort
import ai.solace.coder.protocol.ReasoningEffortConfig
import ai.solace.coder.protocol.ReasoningSummary
import ai.solace.coder.protocol.ReasoningSummaryConfig
import ai.solace.coder.protocol.TurnContextItem
import ai.solace.coder.protocol.TurnDiffEvent
import ai.solace.coder.protocol.AgentMessageContentDeltaEvent
import ai.solace.coder.protocol.ReasoningContentDeltaEvent
import ai.solace.coder.protocol.ReasoningRawContentDeltaEvent
import ai.solace.coder.protocol.AgentReasoningSectionBreakEvent
import ai.solace.coder.protocol.RolloutItem
import ai.solace.coder.protocol.UserMessageItem
import ai.solace.coder.protocol.ReasoningItem
import ai.solace.coder.protocol.ReasoningItemReasoningSummary
import ai.solace.coder.protocol.ReasoningItemContent
import ai.solace.coder.protocol.WebSearchItem
import ai.solace.coder.protocol.FunctionCallOutputPayload
import ai.solace.coder.protocol.InitialHistory
import ai.solace.coder.protocol.SessionSource
import ai.solace.coder.protocol.UserInput as ProtocolUserInput
import ai.solace.coder.protocol.ContentItem
import ai.solace.coder.protocol.ResponseInputItem
import ai.solace.coder.protocol.ResponseItem
import ai.solace.coder.protocol.WarningEvent
import ai.solace.coder.protocol.ContextCompactedEvent
import ai.solace.coder.protocol.ExitedReviewModeEvent
import ai.solace.coder.protocol.ReviewOutputEvent
import ai.solace.coder.protocol.ReviewFinding
import ai.solace.coder.protocol.ReviewCodeLocation
import ai.solace.coder.protocol.ReviewLineRange
import ai.solace.coder.protocol.ExecCommandBeginEvent
import ai.solace.coder.protocol.ExecCommandEndEvent
import ai.solace.coder.protocol.ExecCommandSource
import ai.solace.coder.protocol.ParsedCommand
import ai.solace.coder.exec.process.ProcessExecutor
import ai.solace.coder.exec.process.ExecParams
import ai.solace.coder.exec.process.ExecExpiration
import ai.solace.coder.utils.git.CreateGhostCommitOptions
import kotlin.time.Duration
import kotlin.time.measureTime
import ai.solace.coder.utils.git.GhostSnapshotReport
import ai.solace.coder.utils.git.GitToolingError
import ai.solace.coder.utils.git.ShellGitOperations
import ai.solace.coder.utils.readiness.ReadinessFlag
import ai.solace.coder.utils.readiness.ReadinessToken
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.serialization.json.JsonElement
import kotlin.concurrent.atomics.AtomicLong
import kotlin.concurrent.atomics.ExperimentalAtomicApi
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.minutes

/**
 * Timeout for graceful task interruption before forcing abort.
 * Ported from Rust codex-rs/core/src/tasks/mod.rs GRACEFULL_INTERRUPTION_TIMEOUT_MS
 */
private val GRACEFUL_INTERRUPTION_TIMEOUT = 100.milliseconds

/**
 * Timeout for user shell commands (1 hour).
 * Ported from Rust codex-rs/core/src/tasks/user_shell.rs USER_SHELL_TIMEOUT_MS
 */
private val USER_SHELL_TIMEOUT = 60.minutes

/**
 * Maximum tokens for compaction user messages.
 * Ported from Rust codex-rs/core/src/compact.rs COMPACT_USER_MESSAGE_MAX_TOKENS
 */
private const val COMPACT_USER_MESSAGE_MAX_TOKENS = 20_000

/**
 * Prompt template for context compaction.
 * Ported from Rust codex-rs/core/templates/compact/prompt.md
 */
private const val SUMMARIZATION_PROMPT = """You are performing a CONTEXT CHECKPOINT COMPACTION. Create a handoff summary for another LLM that will resume the task.

Include:
- Current progress and key decisions made
- Important context, constraints, or user preferences
- What remains to be done (clear next steps)
- Any critical data, examples, or references needed to continue

Be concise, structured, and focused on helping the next LLM seamlessly continue the work.
"""

/**
 * Prefix for summary messages (used to identify compacted content).
 * Ported from Rust codex-rs/core/templates/compact/summary_prefix.md
 */
private const val SUMMARY_PREFIX = """Another language model started to solve this problem and produced a summary of its thinking process. You also have access to the state of the tools that were used by that language model. Use this to build on the work that has already been done and avoid duplicating work. Here is the summary produced by the other language model, use the information in this summary to assist with your own analysis:"""

/**
 * The high-level interface to the Codex system.
 * It operates as a queue pair where you send submissions and receive events.
 *
 * Ported from Rust codex-rs/core/src/codex.rs Codex struct
 */
@OptIn(ExperimentalAtomicApi::class)
class Codex internal constructor(
    private val nextId: AtomicLong,
    private val txSub: Channel<Submission>,
    private val rxEvent: Channel<Event>
) {
    /**
     * Submit the `op` wrapped in a `Submission` with a unique ID.
     */
    suspend fun submit(op: Op): CodexResult<String> {
        val id = nextId.fetchAndAdd(1L).toString()
        val sub = Submission(id = id, op = op)
        return submitWithId(sub).map { id }
    }

    /**
     * Use sparingly: prefer `submit()` so Codex is responsible for generating
     * unique IDs for each submission.
     */
    suspend fun submitWithId(sub: Submission): CodexResult<Unit> {
        return try {
            txSub.send(sub)
            CodexResult.success(Unit)
        } catch (e: Exception) {
            CodexResult.failure(CodexError.InternalAgentDied)
        }
    }

    /**
     * Receive the next event from the agent.
     */
    suspend fun nextEvent(): CodexResult<Event> {
        return try {
            val event = rxEvent.receive()
            CodexResult.success(event)
        } catch (e: Exception) {
            CodexResult.failure(CodexError.InternalAgentDied)
        }
    }

    /**
     * Get a Flow of events for reactive consumption.
     */
    fun eventFlow(): Flow<Event> = rxEvent.receiveAsFlow()

    companion object {
        const val INITIAL_SUBMIT_ID = ""
        const val SUBMISSION_CHANNEL_CAPACITY = 64
    }
}

/**
 * Wrapper returned by [Codex.spawn] containing the spawned [Codex],
 * the submission id for the initial `ConfigureSession` request and the
 * unique session id.
 *
 * Ported from Rust codex-rs/core/src/codex.rs CodexSpawnOk
 */
data class CodexSpawnOk(
    val codex: Codex,
    val conversationId: ConversationId
)

/**
 * Unique identifier for a conversation/session.
 *
 * Ported from Rust codex_protocol::ConversationId
 */
typealias ConversationId = String

/**
 * Spawn a new [Codex] and initialize the session.
 *
 * Ported from Rust codex-rs/core/src/codex.rs Codex::spawn
 */
@OptIn(ExperimentalAtomicApi::class)
suspend fun spawnCodex(
    config: Config,
    authManager: AuthManager,
    conversationHistory: InitialHistory = InitialHistory.New,
    sessionSource: SessionSource = SessionSource.Cli
): CodexResult<CodexSpawnOk> {
    val txSub = Channel<Submission>(Codex.SUBMISSION_CHANNEL_CAPACITY)
    val txEvent = Channel<Event>(Channel.UNLIMITED)
    val rxEvent = Channel<Event>(Channel.UNLIMITED)

    val userInstructions = getUserInstructions(config)

    val execPolicy = execPolicyFor(config.features, config.codexHome)

    val sessionConfiguration = SessionConfiguration(
        provider = config.modelProvider,
        model = config.model,
        modelReasoningEffort = config.modelReasoningEffort,
        modelReasoningSummary = config.modelReasoningSummary,
        developerInstructions = config.developerInstructions,
        userInstructions = userInstructions,
        baseInstructions = config.baseInstructions,
        compactPrompt = config.compactPrompt,
        approvalPolicy = config.approvalPolicy,
        sandboxPolicy = config.sandboxPolicy,
        cwd = config.cwd,
        features = config.features,
        execPolicy = execPolicy,
        sessionSource = sessionSource
    )

    val session = Session.new(
        sessionConfiguration = sessionConfiguration,
        config = config,
        authManager = authManager,
        txEvent = txEvent,
        initialHistory = conversationHistory,
        sessionSource = sessionSource
    )

    if (session == null) {
        return CodexResult.failure(CodexError.InternalAgentDied)
    }

    val conversationId = session.conversationId

    // Spawn the submission loop
    val scope = CoroutineScope(Dispatchers.Default + SupervisorJob())
    scope.launch {
        submissionLoop(session, config, txSub)
    }

    // Forward events from session to output channel
    scope.launch {
        for (event in txEvent) {
            rxEvent.send(event)
        }
    }

    val codex = Codex(
        nextId = AtomicLong(0L),
        txSub = txSub,
        rxEvent = rxEvent
    )

    return CodexResult.success(CodexSpawnOk(
        codex = codex,
        conversationId = conversationId
    ))
}

// =============================================================================
// Session
// =============================================================================

/**
 * Context for an initialized model agent.
 *
 * A session has at most 1 running task at a time, and can be interrupted by user input.
 *
 * Ported from Rust codex-rs/core/src/codex.rs Session
 */
@OptIn(ExperimentalAtomicApi::class)
class Session private constructor(
    val conversationId: ConversationId,
    private val txEvent: Channel<Event>,
    private val state: SessionState,
    private val stateMutex: Mutex,
    val activeTurn: ActiveTurnHolder,
    val services: SessionServices,
    private val nextInternalSubId: AtomicLong
) {
    private val scope = CoroutineScope(Dispatchers.Default + SupervisorJob())

    /**
     * Get the event sender channel.
     */
    fun getTxEvent(): Channel<Event> = txEvent

    /**
     * Ensure all rollout writes are durably flushed.
     */
    suspend fun flushRollout() {
        services.rollout?.flush()
    }

    /**
     * Generate the next internal submission ID.
     */
    @OptIn(ExperimentalAtomicApi::class)
    fun nextInternalSubId(): String {
        val id = nextInternalSubId.fetchAndAdd(1L)
        return "auto-compact-$id"
    }

    /**
     * Get total token usage.
     */
    suspend fun getTotalTokenUsage(): Long {
        return stateMutex.withLock {
            state.getTotalTokenUsage()
        }
    }

    /**
     * Record initial conversation history.
     */
    suspend fun recordInitialHistory(conversationHistory: InitialHistory) {
        val turnContext = newTurn(SessionSettingsUpdate())
        when (conversationHistory) {
            is InitialHistory.New -> {
                val items = buildInitialContext(turnContext)
                recordConversationItems(turnContext, items)
                flushRollout()
            }
            is InitialHistory.Resumed -> {
                val rolloutItems = conversationHistory.payload.history
                val reconstructed = reconstructHistoryFromRollout(turnContext, rolloutItems)
                if (reconstructed.isNotEmpty()) {
                    recordIntoHistory(reconstructed, turnContext)
                }
                flushRollout()
            }
            is InitialHistory.Forked -> {
                val rolloutItems = conversationHistory.items
                val reconstructed = reconstructHistoryFromRollout(turnContext, rolloutItems)
                if (reconstructed.isNotEmpty()) {
                    recordIntoHistory(reconstructed, turnContext)
                }
                persistRolloutItems(rolloutItems)
                flushRollout()
            }
        }
    }

    /**
     * Update session settings.
     */
    suspend fun updateSettings(updates: SessionSettingsUpdate) {
        stateMutex.withLock {
            state.sessionConfiguration = state.sessionConfiguration.apply(updates)
        }
    }

    /**
     * Create a new turn context.
     */
    suspend fun newTurn(updates: SessionSettingsUpdate): TurnContext {
        val subId = nextInternalSubId()
        return newTurnWithSubId(subId, updates)
    }

    /**
     * Create a new turn context with a specific submission ID.
     */
    suspend fun newTurnWithSubId(subId: String, updates: SessionSettingsUpdate): TurnContext {
        val sessionConfiguration = stateMutex.withLock {
            val config = state.sessionConfiguration.apply(updates)
            state.sessionConfiguration = config
            config
        }

        return makeTurnContext(
            authManager = services.authManager,
            sessionConfiguration = sessionConfiguration,
            conversationId = conversationId,
            subId = subId,
            finalOutputJsonSchema = updates.finalOutputJsonSchema
        )
    }

    /**
     * Build an environment update item if the context changed.
     */
    fun buildEnvironmentUpdateItem(
        previous: TurnContext?,
        next: TurnContext
    ): ResponseItem? {
        if (previous == null) return null

        val prevContext = EnvironmentContext.from(previous)
        val nextContext = EnvironmentContext.from(next)
        if (prevContext.equalsExceptShell(nextContext)) {
            return null
        }
        return EnvironmentContext.diff(previous, next).toResponseItem()
    }

    /**
     * Persist the event to rollout and send it to clients.
     */
    suspend fun sendEvent(turnContext: TurnContext, msg: EventMsg) {
        val event = Event(
            id = turnContext.subId,
            msg = msg
        )
        sendEventRaw(event)

        // Send legacy events if applicable
        // Note: EventMsg doesn't implement asLegacyEvents in this port yet
        // TODO: Implement legacy event conversion when needed
    }

    /**
     * Send a raw event without legacy conversion.
     */
    suspend fun sendEventRaw(event: Event) {
        val rolloutItems = listOf(RolloutItem.EventMsgItem(event.msg))
        persistRolloutItems(rolloutItems)
        try {
            txEvent.send(event)
        } catch (e: Exception) {
            println("ERROR: failed to send event: ${e.message}")
        }
    }

    /**
     * Emit a turn item started event.
     */
    suspend fun emitTurnItemStarted(turnContext: TurnContext, item: TurnItem) {
        sendEvent(
            turnContext,
            EventMsg.ItemStarted(ItemStartedEvent(
                threadId = ai.solace.coder.protocol.ConversationId.fromString(conversationId).getOrThrow(),
                turnId = turnContext.subId,
                item = item
            ))
        )
    }

    /**
     * Emit a turn item completed event.
     */
    suspend fun emitTurnItemCompleted(turnContext: TurnContext, item: TurnItem) {
        sendEvent(
            turnContext,
            EventMsg.ItemCompleted(ItemCompletedEvent(
                threadId = ai.solace.coder.protocol.ConversationId.fromString(conversationId).getOrThrow(),
                turnId = turnContext.subId,
                item = item
            ))
        )
    }

    /**
     * Assess a sandbox command for safety.
     */
    suspend fun assessSandboxCommand(
        turnContext: TurnContext,
        callId: String,
        command: List<String>,
        failureMessage: String?
    ): SandboxCommandAssessment? {
        // TODO: Implement command assessment
        return null
    }

    /**
     * Emit an exec approval request event and await the user's decision.
     */
    suspend fun requestCommandApproval(
        turnContext: TurnContext,
        callId: String,
        command: List<String>,
        cwd: String,
        reason: String?,
        risk: SandboxCommandAssessment?
    ): ReviewDecision {
        val subId = turnContext.subId
        val deferred = CompletableDeferred<ReviewDecision>()

        val prevEntry = activeTurn.withLock { turn ->
            turn?.turnState?.insertPendingApproval(subId, deferred)
        }
        if (prevEntry != null) {
            println("WARN: Overwriting existing pending approval for sub_id: $subId")
        }

        val event = EventMsg.ExecApprovalRequest(ExecApprovalRequestEvent(
            callId = callId,
            turnId = turnContext.subId,
            command = command,
            cwd = cwd,
            reason = reason,
            risk = risk,
            parsedCmd = emptyList()
        ))
        sendEvent(turnContext, event)

        return try {
            deferred.await()
        } catch (e: Exception) {
            ReviewDecision.Denied
        }
    }

    /**
     * Emit a patch approval request event and await the user's decision.
     */
    suspend fun requestPatchApproval(
        turnContext: TurnContext,
        callId: String,
        changes: Map<String, ai.solace.coder.protocol.FileChange>,
        reason: String?,
        grantRoot: String?
    ): CompletableDeferred<ReviewDecision> {
        val subId = turnContext.subId
        val deferred = CompletableDeferred<ReviewDecision>()

        val prevEntry = activeTurn.withLock { turn ->
            turn?.turnState?.insertPendingApproval(subId, deferred)
        }
        if (prevEntry != null) {
            println("WARN: Overwriting existing pending approval for sub_id: $subId")
        }

        val event = EventMsg.ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent(
            callId = callId,
            turnId = turnContext.subId,
            changes = changes,
            reason = reason,
            grantRoot = grantRoot
        ))
        sendEvent(turnContext, event)

        return deferred
    }

    /**
     * Notify approval for a pending request.
     */
    suspend fun notifyApproval(subId: String, decision: ReviewDecision) {
        val entry = activeTurn.withLock { turn ->
            turn?.turnState?.removePendingApproval(subId)
        }
        if (entry != null) {
            entry.complete(decision)
        } else {
            println("WARN: No pending approval found for sub_id: $subId")
        }
    }

    /**
     * Records input items: always append to conversation history and
     * persist these response items to rollout.
     */
    suspend fun recordConversationItems(turnContext: TurnContext, items: List<ResponseItem>) {
        recordIntoHistory(items, turnContext)
        persistRolloutResponseItems(items)
        sendRawResponseItems(turnContext, items)
    }

    /**
     * Reconstruct history from rollout items.
     */
    private fun reconstructHistoryFromRollout(
        turnContext: TurnContext,
        rolloutItems: List<ai.solace.coder.protocol.RolloutItem>
    ): List<ResponseItem> {
        val history = ContextManager()
        for (rolloutItem in rolloutItems) {
            when (rolloutItem) {
                is ai.solace.coder.protocol.RolloutItem.ResponseItemHolder -> {
                    history.recordItems(listOf(rolloutItem.payload), turnContext.truncationPolicy)
                }
                is ai.solace.coder.protocol.RolloutItem.Compacted -> {
                    val compacted = rolloutItem.payload
                    if (compacted.replacementHistory != null) {
                        history.replace(compacted.replacementHistory)
                    } else {
                        val snapshot = history.getHistory()
                        val userMessages = collectUserMessages(snapshot)
                        val rebuilt = buildCompactedHistory(
                            buildInitialContext(turnContext),
                            userMessages,
                            compacted.message
                        )
                        history.replace(rebuilt)
                    }
                }
                else -> {}
            }
        }
        return history.getHistory()
    }

    /**
     * Append ResponseItems to the in-memory conversation history only.
     */
    suspend fun recordIntoHistory(items: List<ResponseItem>, turnContext: TurnContext) {
        stateMutex.withLock {
            state.recordItems(items, turnContext.truncationPolicy)
        }
    }

    /**
     * Replace the entire conversation history.
     */
    suspend fun replaceHistory(items: List<ResponseItem>) {
        stateMutex.withLock {
            state.replaceHistory(items)
        }
    }

    /**
     * Persist response items to rollout.
     */
    private suspend fun persistRolloutResponseItems(items: List<ResponseItem>) {
        val rolloutItems = items.map { RolloutItem.ResponseItemHolder(payload = it) }
        persistRolloutItems(rolloutItems)
    }

    /**
     * Check if a feature is enabled.
     */
    suspend fun enabled(feature: Feature): Boolean {
        return stateMutex.withLock {
            state.sessionConfiguration.features.enabled(feature)
        }
    }

    /**
     * Send raw response items as events.
     */
    private suspend fun sendRawResponseItems(turnContext: TurnContext, items: List<ResponseItem>) {
        for (item in items) {
            sendEvent(
                turnContext,
                EventMsg.RawResponseItem(RawResponseItemEvent(item = item))
            )
        }
    }

    /**
     * Build initial context items.
     */
    fun buildInitialContext(turnContext: TurnContext): List<ResponseItem> {
        val items = mutableListOf<ResponseItem>()

        turnContext.developerInstructions?.let { instructions ->
            items.add(DeveloperInstructions(instructions).toResponseItem())
        }

        turnContext.userInstructions?.let { instructions ->
            items.add(UserInstructions(
                text = instructions,
                directory = turnContext.cwd
            ).toResponseItem())
        }

        items.add(EnvironmentContext(
            cwd = turnContext.cwd,
            approvalPolicy = turnContext.approvalPolicy,
            sandboxPolicy = turnContext.sandboxPolicy,
            shell = userShell()
        ).toResponseItem())

        return items
    }

    /**
     * Persist rollout items.
     */
    suspend fun persistRolloutItems(items: List<ai.solace.coder.protocol.RolloutItem>) {
        services.rollout?.recordItems(items)
    }

    /**
     * Clone the current conversation history.
     */
    suspend fun cloneHistory(): ContextManager {
        return stateMutex.withLock {
            state.cloneHistory()
        }
    }

    /**
     * Update token usage info.
     */
    suspend fun updateTokenUsageInfo(turnContext: TurnContext, tokenUsage: TokenUsage?) {
        stateMutex.withLock {
            if (tokenUsage != null) {
                state.updateTokenInfoFromUsage(tokenUsage, turnContext.modelContextWindow)
            }
        }
        sendTokenCountEvent(turnContext)
    }

    /**
     * Recompute token usage from history.
     */
    suspend fun recomputeTokenUsage(turnContext: TurnContext) {
        // TODO: Implement token estimation
        // val estimatedTotalTokens = cloneHistory().estimateTokenCount(turnContext) ?: return
        val estimatedTotalTokens = 0L

        stateMutex.withLock {
            val existingInfo = state.tokenInfo
            val newLastUsage = TokenUsage(
                inputTokens = 0,
                cachedInputTokens = 0,
                outputTokens = 0,
                reasoningOutputTokens = 0,
                totalTokens = maxOf(estimatedTotalTokens, 0)
            )

            state.tokenInfo = if (existingInfo != null) {
                existingInfo.copy(
                    lastTokenUsage = newLastUsage,
                    modelContextWindow = existingInfo.modelContextWindow ?: turnContext.modelContextWindow
                )
            } else {
                TokenUsageInfo(
                    totalTokenUsage = TokenUsage(),
                    lastTokenUsage = newLastUsage,
                    modelContextWindow = turnContext.modelContextWindow
                )
            }
        }
        sendTokenCountEvent(turnContext)
    }

    /**
     * Update rate limits.
     */
    suspend fun updateRateLimits(turnContext: TurnContext, newRateLimits: RateLimitSnapshot) {
        stateMutex.withLock {
            state.rateLimits = newRateLimits
        }
        sendTokenCountEvent(turnContext)
    }

    /**
     * Send token count event.
     */
    private suspend fun sendTokenCountEvent(turnContext: TurnContext) {
        val (info, rateLimits) = stateMutex.withLock {
            Pair(state.tokenInfo, state.rateLimits)
        }
        val event = EventMsg.TokenCount(TokenCountEvent(
            info = info,
            rateLimits = rateLimits
        ))
        sendEvent(turnContext, event)
    }

    /**
     * Set total tokens to full (context window exceeded).
     */
    suspend fun setTotalTokensFull(turnContext: TurnContext) {
        val contextWindow = turnContext.modelContextWindow ?: return
        stateMutex.withLock {
            state.tokenInfo = TokenUsageInfo.fullContextWindow(contextWindow)
        }
        sendTokenCountEvent(turnContext)
    }

    /**
     * Record a user input item to conversation history and also persist a
     * corresponding UserMessage EventMsg to rollout.
     */
    suspend fun recordInputAndRolloutUsermsg(
        turnContext: TurnContext,
        responseInput: ResponseInputItem
    ) {
        val responseItem = responseInput.toResponseItem()
        recordConversationItems(turnContext, listOf(responseItem))

        // Create a TurnItem.UserMessage for the user message
        val userMessageItem = UserMessageItem.new(emptyList())
        val turnItem = TurnItem.UserMessage(item = userMessageItem)
        emitTurnItemStarted(turnContext, turnItem)
        emitTurnItemCompleted(turnContext, turnItem)
    }

    /**
     * Notify about a background event.
     */
    suspend fun notifyBackgroundEvent(turnContext: TurnContext, message: String) {
        val event = EventMsg.BackgroundEvent(BackgroundEventEvent(message = message))
        sendEvent(turnContext, event)
    }

    /**
     * Notify about a stream error.
     */
    suspend fun notifyStreamError(
        turnContext: TurnContext,
        message: String,
        codexError: CodexError
    ) {
        val codexErrorInfo = CodexErrorInfo.ResponseStreamDisconnected(
            httpStatusCode = codexError.httpStatusCodeValue()
        )
        val event = EventMsg.StreamError(StreamErrorEvent(
            message = message,
            codexErrorInfo = codexErrorInfo
        ))
        sendEvent(turnContext, event)
    }

    /**
     * Maybe start a ghost snapshot task.
     */
    suspend fun maybeStartGhostSnapshot(
        turnContext: TurnContext,
        cancellationToken: CancellationToken
    ) {
        if (!enabled(Feature.GhostCommit)) {
            return
        }

        val readinessFlag = ReadinessFlag.new()
        val tokenResult = readinessFlag.subscribe()

        tokenResult.fold(
            onSuccess = { token ->
                println("INFO: spawning ghost snapshot task")
                val task = GhostSnapshotTask(token, readinessFlag)
                task.run(
                    SessionTaskContext(this),
                    turnContext,
                    emptyList(),
                    cancellationToken
                )
            },
            onFailure = { error ->
                println("WARN: failed to subscribe to ghost snapshot readiness: $error")
            }
        )
    }

    /**
     * Returns the input if there was no task running to inject into.
     */
    suspend fun injectInput(input: List<UserInput>): Result<Unit> {
        return activeTurn.withLock { turn ->
            if (turn != null) {
                for (item in input) {
                    turn.turnState.pushPendingInput(item.toResponseInputItem())
                }
                Result.success(Unit)
            } else {
                Result.failure(Exception("No active turn"))
            }
        }
    }

    /**
     * Get pending input from the active turn.
     */
    suspend fun getPendingInput(): List<ResponseInputItem> {
        return activeTurn.withLock { turn ->
            turn?.turnState?.takePendingInput() ?: emptyList()
        }
    }

    /**
     * Interrupt the current task.
     */
    suspend fun interruptTask() {
        println("INFO: interrupt received: abort current task, if any")
        val hasActiveTurn = activeTurn.withLock { it != null }
        if (hasActiveTurn) {
            abortAllTasks(TurnAbortReason.Interrupted)
        } else {
            cancelMcpStartup()
        }
    }

    /**
     * Get the user notification service.
     */
    fun notifier(): UserNotifier = services.notifier

    /**
     * Get the user's shell.
     */
    fun userShell(): Shell = services.userShell

    /**
     * Check if raw agent reasoning should be shown.
     */
    fun showRawAgentReasoning(): Boolean = services.showRawAgentReasoning

    /**
     * Cancel MCP startup.
     */
    private suspend fun cancelMcpStartup() {
        services.mcpStartupCancellationToken.cancel()
    }

    /**
     * Spawn a new task.
     *
     * Ported from Rust codex-rs/core/src/tasks/mod.rs Session::spawn_task
     */
    suspend fun spawnTask(
        turnContext: TurnContext,
        input: List<UserInput>,
        task: SessionTask
    ) {
        // Abort any existing tasks first (Rust: self.abort_all_tasks(Replaced).await)
        abortAllTasks(TurnAbortReason.Replaced)

        val cancellationToken = CancellationToken()
        val done = CompletableDeferred<Unit>()
        val runningTask = RunningTask(
            done = done,
            kind = task.kind(),
            task = task,
            cancellationToken = cancellationToken,
            turnContext = turnContext
        )

        // Register the task (Rust: self.register_new_active_task(running_task).await)
        registerNewActiveTask(runningTask)

        // Launch the task
        scope.launch {
            val sessionContext = SessionTaskContext(this@Session)
            try {
                val lastAgentMessage = task.run(
                    sessionContext,
                    turnContext,
                    input,
                    cancellationToken
                )

                // Signal task completion
                done.complete(Unit)

                // Flush rollout after task completes (Rust: session_ctx.clone_session().flush_rollout().await)
                flushRollout()

                // Only emit TaskComplete if not cancelled (Rust: if !task_cancellation_token.is_cancelled())
                if (!cancellationToken.isCancelled()) {
                    onTaskFinished(turnContext, lastAgentMessage)
                }
            } catch (e: Exception) {
                done.complete(Unit)
                // Log but don't propagate - task failures are handled via events
                println("WARN: Task ${task.kind()} failed: ${e.message}")
            }
        }
    }

    /**
     * Abort all running tasks.
     *
     * Ported from Rust codex-rs/core/src/tasks/mod.rs Session::abort_all_tasks
     */
    suspend fun abortAllTasks(reason: TurnAbortReason) {
        for (task in takeAllRunningTasks()) {
            handleTaskAbort(task, reason)
        }
    }

    /**
     * Register a new active task, creating a new ActiveTurn.
     *
     * Ported from Rust codex-rs/core/src/tasks/mod.rs Session::register_new_active_task
     */
    private suspend fun registerNewActiveTask(task: RunningTask) {
        activeTurn.withLock {
            val turn = ActiveTurn()
            turn.addTask(task)
            turn
        }
    }

    /**
     * Take all running tasks, clearing the active turn.
     *
     * Ported from Rust codex-rs/core/src/tasks/mod.rs Session::take_all_running_tasks
     */
    private suspend fun takeAllRunningTasks(): List<RunningTask> {
        val currentTurn = activeTurn.get()
        activeTurn.set(null)

        return currentTurn?.let { turn ->
            turn.clearPending()
            turn.drainTasks()
        } ?: emptyList()
    }

    /**
     * Handle graceful abort of a single task.
     *
     * Ported from Rust codex-rs/core/src/tasks/mod.rs handle_task_abort
     */
    private suspend fun handleTaskAbort(
        runningTask: RunningTask,
        reason: TurnAbortReason
    ) {
        val subId = runningTask.turnContext.subId

        // Early return if already cancelled (Rust: if task.cancellation_token.is_cancelled())
        if (runningTask.cancellationToken.isCancelled()) {
            return
        }

        // Signal cancellation
        runningTask.cancellationToken.cancel()

        // Wait for graceful completion with timeout (Rust: select! with sleep)
        val completedGracefully = withTimeoutOrNull(GRACEFUL_INTERRUPTION_TIMEOUT) {
            runningTask.done.await()
            true
        } ?: false

        if (!completedGracefully) {
            println("WARN: task $subId didn't complete gracefully after ${GRACEFUL_INTERRUPTION_TIMEOUT.inWholeMilliseconds}ms")
        }

        // Call the task's abort hook for cleanup (Rust: session_task.abort(session_ctx, ...).await)
        val sessionContext = SessionTaskContext(this@Session)
        try {
            runningTask.task.abort(sessionContext, runningTask.turnContext)
        } catch (e: Exception) {
            println("WARN: task $subId abort hook failed: ${e.message}")
        }

        // Emit turn aborted event per-task (Rust: self.send_event(task.turn_context.as_ref(), event).await)
        sendEvent(
            runningTask.turnContext,
            EventMsg.TurnAborted(TurnAbortedEvent(reason = reason))
        )
    }

    /**
     * Called when a task finishes.
     */
    private suspend fun onTaskFinished(turnContext: TurnContext, lastAgentMessage: String?) {
        activeTurn.withLock { turn ->
            turn?.removeTask(turnContext.subId)
        }

        // Emit task complete event
        sendEvent(
            turnContext,
            EventMsg.TaskComplete(TaskCompleteEvent(
                lastAgentMessage = lastAgentMessage
            ))
        )
    }

    /**
     * Parse an MCP tool name into server and tool parts.
     */
    suspend fun parseMcpToolName(toolName: String): Pair<String, String>? {
        return services.mcpConnectionManager.parseToolName(toolName)
    }

    /**
     * Call an MCP tool.
     */
    suspend fun callTool(
        server: String,
        tool: String,
        arguments: JsonElement?
    ): Result<ai.solace.coder.protocol.CallToolResult> {
        return services.mcpConnectionManager.callTool(server, tool, arguments)
    }

    companion object {
        /**
         * Create a new session.
         */
        @OptIn(ExperimentalAtomicApi::class)
        suspend fun new(
            sessionConfiguration: SessionConfiguration,
            config: Config,
            authManager: AuthManager,
            txEvent: Channel<Event>,
            initialHistory: InitialHistory,
            sessionSource: SessionSource
        ): Session? {
            println("DEBUG: Configuring session: model=${sessionConfiguration.model}; provider=${sessionConfiguration.provider}")

            if (!isAbsolutePath(sessionConfiguration.cwd)) {
                println("ERROR: cwd is not absolute: ${sessionConfiguration.cwd}")
                return null
            }

            val conversationId: ConversationId = when (initialHistory) {
                is InitialHistory.New, is InitialHistory.Forked -> generateConversationId()
                is InitialHistory.Resumed -> initialHistory.payload.conversationId.toString()
            }

            // Initialize services
            val rolloutRecorder = RolloutRecorder.new(config, conversationId)
            val defaultShell = ShellDetector().defaultUserShell()

            val state = SessionState(sessionConfiguration)

            val services = SessionServices(
                mcpConnectionManager = McpConnectionManager(),
                mcpStartupCancellationToken = CancellationToken(),
                unifiedExecManager = UnifiedExecSessionManager(),
                notifier = UserNotifier(config.notify),
                rollout = rolloutRecorder,
                userShell = defaultShell,
                showRawAgentReasoning = config.showRawAgentReasoning,
                authManager = authManager,
                toolApprovals = ApprovalStore()
            )

            val session = Session(
                conversationId = conversationId,
                txEvent = txEvent,
                state = state,
                stateMutex = Mutex(),
                activeTurn = ActiveTurnHolder(),
                services = services,
                nextInternalSubId = AtomicLong(0L)
            )

            // Send SessionConfigured event
            val event = Event(
                id = Codex.INITIAL_SUBMIT_ID,
                msg = EventMsg.SessionConfigured(SessionConfiguredEvent(
                    sessionId = ai.solace.coder.protocol.ConversationId.fromString(conversationId).getOrThrow(),
                    model = sessionConfiguration.model,
                    modelProviderId = config.modelProviderId ?: "",
                    approvalPolicy = sessionConfiguration.approvalPolicy,
                    sandboxPolicy = sessionConfiguration.sandboxPolicy,
                    cwd = sessionConfiguration.cwd,
                    reasoningEffort = sessionConfiguration.modelReasoningEffort,
                    historyLogId = 0L,
                    historyEntryCount = 0L,
                    initialMessages = emptyList(),
                    rolloutPath = rolloutRecorder?.rolloutPath ?: ""
                ))
            )
            session.sendEventRaw(event)

            // Initialize MCP connection manager
            val mcpServerConfigs = config.mcpServers.mapValues { (_, v) ->
                ai.solace.coder.mcp.connection.McpServerConfig(
                    command = v.command,
                    args = v.args,
                    env = v.env
                )
            }
            services.mcpConnectionManager.initialize(
                mcpServerConfigs,
                txEvent,
                CancellationToken()
            )

            // Record initial history
            session.recordInitialHistory(initialHistory)

            return session
        }

        /**
         * Create a turn context.
         *
         * Note: In Rust, this is done in make_turn_context() which also creates the ModelClient.
         * The client parameter should be passed in once ModelClient creation is integrated.
         * Model info (model, modelFamily, modelContextWindow, etc.) is accessed via client.
         */
        private fun makeTurnContext(
            authManager: AuthManager?,
            sessionConfiguration: SessionConfiguration,
            conversationId: ConversationId,
            subId: String,
            finalOutputJsonSchema: JsonElement? = null,
            client: ai.solace.coder.core.client.ModelClient? = null
        ): TurnContext {
            val modelFamily = findFamilyForModel(sessionConfiguration.model)
                ?: sessionConfiguration.modelFamily

            val toolsConfig = ToolsConfig(
                modelFamily = modelFamily,
                features = sessionConfiguration.features
            )

            return TurnContext(
                subId = subId,
                client = client,  // Model info accessed via client
                cwd = sessionConfiguration.cwd,
                developerInstructions = sessionConfiguration.developerInstructions,
                baseInstructions = sessionConfiguration.baseInstructions,
                compactPrompt = sessionConfiguration.compactPrompt,
                userInstructions = sessionConfiguration.userInstructions,
                approvalPolicy = sessionConfiguration.approvalPolicy,
                sandboxPolicy = sessionConfiguration.sandboxPolicy,
                shellEnvironmentPolicy = sessionConfiguration.shellEnvironmentPolicy,
                toolsConfig = toolsConfig,
                finalOutputJsonSchema = finalOutputJsonSchema,
                codexLinuxSandboxExe = sessionConfiguration.codexLinuxSandboxExe,
                toolCallGate = ReadinessFlag(),
                execPolicy = sessionConfiguration.execPolicy,
                truncationPolicy = TruncationPolicy.Tokens(8000)
            )
        }

        private fun generateConversationId(): ConversationId {
            val chars = "0123456789abcdef"
            return buildString {
                append("conv_")
                repeat(16) {
                    append(chars.random())
                }
            }
        }

        private fun isAbsolutePath(path: String): Boolean {
            return path.startsWith("/") || (path.length >= 3 && path[1] == ':' && path[2] == '\\')
        }
    }
}

// =============================================================================
// TurnContext - Context for a single turn of the conversation
// =============================================================================

/**
 * Context needed for a single turn of the conversation.
 *
 * Ported from Rust codex-rs/core/src/codex.rs TurnContext struct (lines 272-292)
 *
 * Note: Model configuration (model, modelFamily, modelContextWindow, reasoningEffort,
 * reasoningSummary) and stream configuration (streamMaxRetries, autoCompactTokenLimit)
 * are accessed via the [client] field, not stored directly here. This matches the
 * Rust architecture where TurnContext.client provides access to ModelClient which
 * holds the Config.
 */
data class TurnContext(
    val subId: String,
    /**
     * The model client for this turn - provides access to OTEL, config, and API calls.
     * Access model info via client.getModel(), client.getModelContextWindow(), etc.
     * TODO: Make this non-nullable once ModelClient creation is properly integrated
     * into the turn lifecycle (see Rust codex-rs/core/src/codex.rs make_turn_context).
     */
    val client: ai.solace.coder.core.client.ModelClient? = null,
    /**
     * The session's current working directory. All relative paths provided by
     * the model as well as sandbox policies are resolved against this path.
     */
    val cwd: String,
    val developerInstructions: String? = null,
    val baseInstructions: String? = null,
    val compactPrompt: String? = null,
    val userInstructions: String? = null,
    val approvalPolicy: AskForApproval,
    val sandboxPolicy: SandboxPolicy,
    val shellEnvironmentPolicy: ShellEnvironmentPolicy = ShellEnvironmentPolicy.Inherit(),
    val toolsConfig: ToolsConfig = ToolsConfig(),
    val finalOutputJsonSchema: JsonElement? = null,
    val codexLinuxSandboxExe: String? = null,
    val toolCallGate: ReadinessFlag? = null,
    val execPolicy: ExecPolicy = ExecPolicy(),
    val truncationPolicy: TruncationPolicy = TruncationPolicy.Tokens(8000)
) {
    // =========================================================================
    // Convenience accessors - delegate to client for model/config info
    // These match the Rust pattern of accessing via turn_context.client.*
    // =========================================================================

    /** Get the model name. Delegates to client.getModel(). */
    val model: String get() = client?.getModel() ?: "unknown"

    /** Get the model family. Delegates to client.getModelFamily(). */
    val modelFamily: ModelFamily get() = client?.getModelFamily() ?: ModelFamily.default()

    /** Get the model context window. Delegates to client.getModelContextWindow(). */
    val modelContextWindow: Long? get() = client?.getModelContextWindow()

    /** Get the reasoning effort config. Delegates to client.getReasoningEffort(). */
    val reasoningEffort: ReasoningEffortConfig? get() = client?.getReasoningEffort()

    /** Get the reasoning summary config. Delegates to client.getReasoningSummary(). */
    val reasoningSummary: ReasoningSummaryConfig get() = client?.getReasoningSummary() ?: ReasoningSummary.Auto

    /** Get the auto-compact token limit. Delegates to client.getAutoCompactTokenLimit(). */
    val autoCompactTokenLimit: Long? get() = client?.getAutoCompactTokenLimit()

    /** Stream max retries - from client config. */
    val streamMaxRetries: Int get() = client?.config()?.streamMaxRetries ?: 3

    /**
     * Whether the model family supports parallel tool calls.
     */
    val modelFamilySupportsParallelToolCalls: Boolean
        get() = toolsConfig.modelFamily.supportsParallelToolCalls

    /**
     * Get the compact prompt, falling back to SUMMARIZATION_PROMPT if not set.
     * Ported from Rust codex-rs/core/src/codex.rs TurnContext::compact_prompt()
     */
    fun compactPromptOrDefault(): String = compactPrompt ?: SUMMARIZATION_PROMPT

    /**
     * Resolves a relative path against the turn's CWD.
     * Ported from Rust codex-rs/core/src/codex.rs TurnContext::resolve_path()
     */
    fun resolvePath(path: String?): String {
        if (path == null) return cwd
        if (path.startsWith("/") || path.matches(Regex("^[A-Za-z]:.*"))) {
            return path
        }
        return if (cwd.endsWith("/") || cwd.endsWith("\\")) {
            "$cwd$path"
        } else {
            "$cwd/$path"
        }
    }

    /**
     * Returns the compact prompt to use for this turn.
     */
    fun getCompactPrompt(): String {
        return compactPrompt ?: DEFAULT_COMPACT_PROMPT
    }

    companion object {
        private val DEFAULT_COMPACT_PROMPT = """
            Summarize the conversation history concisely while preserving:
            - User's original requests and intent
            - Key decisions and technical approach
            - Important context for continuing the work

            Omit:
            - Verbose explanations and redundant details
            - Off-topic discussions and failed attempts
            - Internal debugging and troubleshooting

            Focus on:
            - Current state of the work
            - Next steps to continue
            - Critical constraints and requirements
        """.trimIndent()
    }
}

// =============================================================================
// SessionConfiguration - Configuration that applies across all turns
// =============================================================================

/**
 * Session configuration that applies across all turns.
 *
 * Ported from Rust codex-rs/core/src/codex.rs SessionConfiguration struct (lines 308-354)
 */
data class SessionConfiguration(
    val provider: ModelProviderInfo,
    val model: String,
    val modelReasoningEffort: ReasoningEffortConfig? = null,
    val modelReasoningSummary: ReasoningSummaryConfig = ReasoningSummary.Auto,
    val developerInstructions: String? = null,
    val userInstructions: String? = null,
    val baseInstructions: String? = null,
    val compactPrompt: String? = null,
    val approvalPolicy: AskForApproval,
    val sandboxPolicy: SandboxPolicy,
    val cwd: String,
    val features: Features,
    val execPolicy: ExecPolicy,
    val sessionSource: SessionSource,
    val shellEnvironmentPolicy: ShellEnvironmentPolicy = ShellEnvironmentPolicy.Inherit(),
    val codexLinuxSandboxExe: String? = null,
    val modelFamily: ModelFamily = ModelFamily.default()
) {
    /**
     * Applies updates to create a new configuration.
     */
    fun apply(updates: SessionSettingsUpdate): SessionConfiguration {
        return copy(
            model = updates.model ?: model,
            modelReasoningEffort = updates.reasoningEffort ?: modelReasoningEffort,
            modelReasoningSummary = updates.reasoningSummary ?: modelReasoningSummary,
            approvalPolicy = updates.approvalPolicy ?: approvalPolicy,
            sandboxPolicy = updates.sandboxPolicy ?: sandboxPolicy,
            cwd = updates.cwd ?: cwd
        )
    }
}

/**
 * Updates that can be applied to session settings.
 *
 * Ported from Rust codex-rs/core/src/codex.rs SessionSettingsUpdate struct (lines 381-390)
 */
data class SessionSettingsUpdate(
    val cwd: String? = null,
    val approvalPolicy: AskForApproval? = null,
    val sandboxPolicy: SandboxPolicy? = null,
    val model: String? = null,
    val reasoningEffort: ReasoningEffortConfig? = null,
    val reasoningSummary: ReasoningSummaryConfig? = null,
    val finalOutputJsonSchema: JsonElement? = null
)

// =============================================================================
// Supporting Types for TurnContext
// =============================================================================

/**
 * Shell environment inheritance policy.
 *
 * Ported from Rust codex-rs/core/src/config/types.rs ShellEnvironmentPolicy
 */
sealed class ShellEnvironmentPolicy {
    data class Inherit(
        val filter: ShellEnvironmentInheritFilter = ShellEnvironmentInheritFilter.All
    ) : ShellEnvironmentPolicy()

    data class Sanitize(
        val additionalVars: Map<String, String> = emptyMap()
    ) : ShellEnvironmentPolicy()
}

enum class ShellEnvironmentInheritFilter {
    Core, All, None
}

/**
 * Tool configuration for a turn.
 *
 * Ported from Rust codex-rs/core/src/tools/spec.rs ToolsConfig
 */
// port-lint: ignore-duplicate - Extended version with Features, differs from ToolSpec.ToolsConfig
data class ToolsConfig(
    val shellType: ShellToolType = ShellToolType.Default,
    val applyPatchToolType: ApplyPatchToolType? = null,
    val webSearchRequest: Boolean = false,
    val includeViewImageTool: Boolean = true,
    val experimentalSupportedTools: List<String> = emptyList(),
    val modelFamily: ModelFamily = ModelFamily.default(),
    val features: Features = Features.withDefaults()
)

enum class ShellToolType { Default, UnifiedExec, None }
// ApplyPatchToolType is imported from ai.solace.coder.core.tools.ToolSpec

/**
 * Execution policy for commands.
 *
 * Ported from Rust codex-execpolicy.
 */
data class ExecPolicy(
    val enabled: Boolean = true,
    val defaultAction: ExecPolicyAction = ExecPolicyAction.Ask
)

enum class ExecPolicyAction { Allow, Deny, Ask }

// =============================================================================
// Submission Loop
// =============================================================================

/**
 * Main submission loop that processes operations.
 *
 * Ported from Rust codex-rs/core/src/codex.rs submission_loop
 */
private suspend fun submissionLoop(
    sess: Session,
    config: Config,
    rxSub: Channel<Submission>
) {
    var previousContext: TurnContext? = sess.newTurn(SessionSettingsUpdate())

    for (sub in rxSub) {
        println("DEBUG: Submission: ${sub.op}")

        when (val op = sub.op) {
            is Op.Interrupt -> {
                Handlers.interrupt(sess)
            }
            is Op.OverrideTurnContext -> {
                Handlers.overrideTurnContext(sess, SessionSettingsUpdate(
                    cwd = op.cwd,
                    approvalPolicy = op.approvalPolicy,
                    sandboxPolicy = op.sandboxPolicy,
                    model = op.model,
                    reasoningEffort = op.effort,
                    reasoningSummary = op.summary
                ))
            }
            is Op.UserInput -> {
                previousContext = Handlers.userInputOrTurn(sess, sub.id, op, previousContext)
            }
            is Op.UserTurn -> {
                previousContext = Handlers.userInputOrTurn(sess, sub.id, op, previousContext)
            }
            is Op.ExecApproval -> {
                Handlers.execApproval(sess, op.id, op.decision)
            }
            is Op.PatchApproval -> {
                Handlers.patchApproval(sess, op.id, op.decision)
            }
            is Op.AddToHistory -> {
                Handlers.addToHistory(sess, config, op.text)
            }
            is Op.GetHistoryEntryRequest -> {
                Handlers.getHistoryEntryRequest(sess, config, sub.id, op.offset.toInt(), op.logId)
            }
            is Op.ListMcpTools -> {
                Handlers.listMcpTools(sess, config, sub.id)
            }
            is Op.ListCustomPrompts -> {
                Handlers.listCustomPrompts(sess, sub.id)
            }
            is Op.Undo -> {
                Handlers.undo(sess, sub.id)
            }
            is Op.Compact -> {
                Handlers.compact(sess, sub.id)
            }
            is Op.RunUserShellCommand -> {
                previousContext = Handlers.runUserShellCommand(sess, sub.id, op.command, previousContext)
            }
            is Op.ResolveElicitation -> {
                Handlers.resolveElicitation(sess, op.serverName, op.requestId, op.decision)
            }
            is Op.Shutdown -> {
                if (Handlers.shutdown(sess, sub.id)) {
                    break
                }
            }
            is Op.Review -> {
                Handlers.review(sess, config, sub.id, op.reviewRequest)
            }
            else -> {
                // Ignore unknown ops
            }
        }
    }
    println("DEBUG: Agent loop exited")
}

// =============================================================================
// Operation Handlers
// =============================================================================

/**
 * Operation handlers module.
 *
 * Ported from Rust codex-rs/core/src/codex.rs mod handlers
 */
private object Handlers {
    suspend fun interrupt(sess: Session) {
        sess.interruptTask()
    }

    suspend fun overrideTurnContext(sess: Session, updates: SessionSettingsUpdate) {
        sess.updateSettings(updates)
    }

    suspend fun userInputOrTurn(
        sess: Session,
        subId: String,
        op: Op,
        previousContext: TurnContext?
    ): TurnContext {
        val (protocolItems, updates) = when (op) {
            is Op.UserTurn -> Pair(
                op.items,
                SessionSettingsUpdate(
                    cwd = op.cwd,
                    approvalPolicy = op.approvalPolicy,
                    sandboxPolicy = op.sandboxPolicy,
                    model = op.model,
                    reasoningEffort = op.effort,
                    reasoningSummary = op.summary,
                    finalOutputJsonSchema = op.finalOutputJsonSchema
                )
            )
            is Op.UserInput -> Pair(op.items, SessionSettingsUpdate())
            else -> throw IllegalArgumentException("Unexpected op type")
        }

        // Convert protocol UserInput to session UserInput
        val items = protocolItems.map { input ->
            when (input) {
                is ProtocolUserInput.Text -> UserInput.Text(content = input.text)
                is ProtocolUserInput.Image -> UserInput.Text(content = "[Image: ${input.imageUrl}]")
                is ProtocolUserInput.LocalImage -> UserInput.FileRef(path = input.path)
            }
        }

        val currentContext = sess.newTurnWithSubId(subId, updates)

        // Attempt to inject input into current task
        val injectResult = sess.injectInput(items)
        if (injectResult.isFailure) {
            val envItem = sess.buildEnvironmentUpdateItem(previousContext, currentContext)
            if (envItem != null) {
                sess.recordConversationItems(currentContext, listOf(envItem))
            }

            sess.spawnTask(currentContext, items, RegularTask())
        }

        return currentContext
    }

    suspend fun runUserShellCommand(
        sess: Session,
        subId: String,
        command: String,
        previousContext: TurnContext?
    ): TurnContext {
        val turnContext = sess.newTurnWithSubId(subId, SessionSettingsUpdate())
        sess.spawnTask(
            turnContext,
            emptyList(),
            UserShellCommandTask(command)
        )
        return turnContext
    }

    suspend fun resolveElicitation(
        sess: Session,
        serverName: String,
        requestId: String,
        decision: ElicitationAction
    ) {
        val response = McpConnectionManager.ElicitationResponse(
            action = decision,
            content = null
        )
        try {
            sess.services.mcpConnectionManager.resolveElicitation(serverName, requestId, response)
        } catch (e: Exception) {
            println("WARN: failed to resolve elicitation request in session: ${e.message}")
        }
    }

    suspend fun execApproval(sess: Session, id: String, decision: ReviewDecision) {
        when (decision) {
            ReviewDecision.Abort -> sess.interruptTask()
            else -> sess.notifyApproval(id, decision)
        }
    }

    suspend fun patchApproval(sess: Session, id: String, decision: ReviewDecision) {
        when (decision) {
            ReviewDecision.Abort -> sess.interruptTask()
            else -> sess.notifyApproval(id, decision)
        }
    }

    suspend fun addToHistory(sess: Session, config: Config, text: String) {
        // TODO: Implement message history append
    }

    suspend fun getHistoryEntryRequest(
        sess: Session,
        config: Config,
        subId: String,
        offset: Int,
        logId: Long
    ) {
        // TODO: Implement history entry lookup
        val event = Event(
            id = subId,
            msg = EventMsg.GetHistoryEntryResponse(GetHistoryEntryResponseEvent(
                offset = offset.toLong(),
                logId = logId,
                entry = null
            ))
        )
        sess.sendEventRaw(event)
    }

    suspend fun listMcpTools(sess: Session, config: Config, subId: String) {
        val tools = sess.services.mcpConnectionManager.listAllTools()
        val event = Event(
            id = subId,
            msg = EventMsg.McpListToolsResponse(McpListToolsResponseEvent(
                tools = tools,
                resources = emptyMap(),
                resourceTemplates = emptyMap(),
                authStatuses = emptyMap()
            ))
        )
        sess.sendEventRaw(event)
    }

    suspend fun listCustomPrompts(sess: Session, subId: String) {
        val customPrompts = discoverCustomPrompts()
        val event = Event(
            id = subId,
            msg = EventMsg.ListCustomPromptsResponse(ListCustomPromptsResponseEvent(
                customPrompts = customPrompts
            ))
        )
        sess.sendEventRaw(event)
    }

    suspend fun undo(sess: Session, subId: String) {
        val turnContext = sess.newTurnWithSubId(subId, SessionSettingsUpdate())
        sess.spawnTask(turnContext, emptyList(), UndoTask())
    }

    suspend fun compact(sess: Session, subId: String) {
        val turnContext = sess.newTurnWithSubId(subId, SessionSettingsUpdate())
        sess.spawnTask(
            turnContext,
            listOf(UserInput.Text(turnContext.getCompactPrompt())),
            CompactTask()
        )
    }

    suspend fun shutdown(sess: Session, subId: String): Boolean {
        sess.abortAllTasks(TurnAbortReason.Interrupted)
        sess.services.unifiedExecManager.terminateAllSessions()
        println("INFO: Shutting down Codex instance")

        // Flush and shutdown rollout recorder
        sess.services.rollout?.shutdown()

        val event = Event(
            id = subId,
            msg = EventMsg.ShutdownComplete
        )
        sess.sendEventRaw(event)
        return true
    }

    suspend fun review(sess: Session, config: Config, subId: String, reviewRequest: ReviewRequest) {
        val turnContext = sess.newTurnWithSubId(subId, SessionSettingsUpdate())
        spawnReviewThread(sess, config, turnContext, subId, reviewRequest)
    }
}

// =============================================================================
// Run Task
// =============================================================================

/**
 * Takes a user message as input and runs a loop where, at each turn, the model
 * replies with either:
 *
 * - requested function calls
 * - an assistant message
 *
 * Ported from Rust codex-rs/core/src/codex.rs run_task
 */
suspend fun runTask(
    sess: Session,
    turnContext: TurnContext,
    input: List<UserInput>,
    cancellationToken: CancellationToken
): String? {
    if (input.isEmpty()) {
        return null
    }

    val event = EventMsg.TaskStarted(TaskStartedEvent(
        modelContextWindow = turnContext.modelContextWindow
    ))
    sess.sendEvent(turnContext, event)

    val initialInputForTurn = ResponseInputItem.fromUserInput(input)
    sess.recordInputAndRolloutUsermsg(turnContext, initialInputForTurn)

    sess.maybeStartGhostSnapshot(turnContext, cancellationToken.child())

    var lastAgentMessage: String? = null
    val turnDiffTracker = SharedTurnDiffTracker()

    while (!cancellationToken.isCancelled()) {
        val pendingInput = sess.getPendingInput()
            .map { it.toResponseItem() }

        val turnInput = run {
            sess.recordConversationItems(turnContext, pendingInput)
            sess.cloneHistory().getHistoryForPrompt()
        }

        val turnInputMessages = turnInput
            .filterIsInstance<ResponseItem.Message>()
            .filter { it.role == "user" }
            .flatMap { msg ->
                msg.content.filterIsInstance<ContentItem.InputText>()
                    .map { it.text }
            }

        val turnResult = runTurn(
            sess = sess,
            turnContext = turnContext,
            turnDiffTracker = turnDiffTracker,
            input = turnInput,
            cancellationToken = cancellationToken.child()
        )

        when {
            turnResult.isSuccess -> {
                val processedItems = turnResult.getOrThrow()
                val limit = turnContext.autoCompactTokenLimit ?: Long.MAX_VALUE
                val totalUsageTokens = sess.getTotalTokenUsage()
                val tokenLimitReached = totalUsageTokens >= limit

                val processor = ToolCallProcessor()
                val processResult = processor.processItems(processedItems, sess, turnContext)

                if (tokenLimitReached) {
                    if (shouldUseRemoteCompactTask(sess)) {
                        runInlineRemoteAutoCompactTask(sess, turnContext)
                    } else {
                        runInlineAutoCompactTask(sess, turnContext)
                    }
                    continue
                }

                if (processResult.responses.isEmpty()) {
                    lastAgentMessage = getLastAssistantMessageFromTurn(processResult.itemsToRecord)
                    sess.notifier().notify(UserNotification.AgentTurnComplete(
                        threadId = sess.conversationId,
                        turnId = turnContext.subId,
                        cwd = turnContext.cwd,
                        inputMessages = turnInputMessages,
                        lastAssistantMessage = lastAgentMessage
                    ))
                    break
                }
                continue
            }
            turnResult.exceptionOrNull() is TurnAbortedException -> {
                val aborted = turnResult.exceptionOrNull() as TurnAbortedException
                val processor = ToolCallProcessor()
                processor.processItems(aborted.danglingArtifacts, sess, turnContext)
                break
            }
            else -> {
                val error = turnResult.exceptionOrNull()
                println("INFO: Turn error: ${error?.message}")
                val errorEvent = EventMsg.Error(ErrorEvent(
                    message = error?.message ?: "Unknown error",
                    codexErrorInfo = null
                ))
                sess.sendEvent(turnContext, errorEvent)
                break
            }
        }
    }

    return lastAgentMessage
}

/**
 * Run a single turn of the conversation.
 *
 * Ported from Rust codex-rs/core/src/codex.rs run_turn
 */
private suspend fun runTurn(
    sess: Session,
    turnContext: TurnContext,
    turnDiffTracker: SharedTurnDiffTracker,
    input: List<ResponseItem>,
    cancellationToken: CancellationToken
): Result<List<ProcessedResponseItem>> {
    val mcpTools = sess.services.mcpConnectionManager.listAllTools()
    val router = ToolRouter(ToolRegistry())

    val modelSupportsParallel = turnContext.modelFamilySupportsParallelToolCalls
    val parallelToolCalls = modelSupportsParallel && sess.enabled(Feature.ParallelToolCalls)

    var baseInstructions = turnContext.baseInstructions
    if (parallelToolCalls) {
        val family = findFamilyForModel(turnContext.model)
        if (family != null) {
            val newInstructions = (baseInstructions ?: family.baseInstructions) +
                "\n\n" + PARALLEL_INSTRUCTIONS
            baseInstructions = newInstructions
        }
    }

    // TODO: Convert ToolRouter specs to Prompt ToolSpec format
    val prompt = Prompt(
        input = input,
        tools = emptyList(), // router.getToolSpecs() - needs type conversion
        parallelToolCalls = parallelToolCalls,
        baseInstructionsOverride = baseInstructions,
        outputSchema = turnContext.finalOutputJsonSchema
    )

    var retries = 0
    val maxRetries = turnContext.streamMaxRetries

    while (true) {
        val result = tryRunTurn(
            router = router,
            sess = sess,
            turnContext = turnContext,
            turnDiffTracker = turnDiffTracker,
            prompt = prompt,
            cancellationToken = cancellationToken.child()
        )

        when {
            result.isSuccess -> return result
            result.exceptionOrNull() is TurnAbortedException -> return result
            result.exceptionOrNull() is InterruptedException ->
                return Result.failure(result.exceptionOrNull()!!)
            result.exceptionOrNull() is ContextWindowExceededException -> {
                sess.setTotalTokensFull(turnContext)
                return result
            }
            else -> {
                if (retries < maxRetries) {
                    retries++
                    val delay = backoff(retries)
                    println("WARN: stream disconnected - retrying turn ($retries/$maxRetries in ${delay}ms)...")

                    val error = result.exceptionOrNull()
                    sess.notifyStreamError(
                        turnContext,
                        "Reconnecting... $retries/$maxRetries",
                        CodexError.Stream(error?.message ?: "Unknown error")
                    )

                    kotlinx.coroutines.delay(delay)
                } else {
                    return result
                }
            }
        }
    }
}

/**
 * Try to run a single turn, handling streaming and tool calls.
 *
 * Ported from Rust codex-rs/core/src/codex.rs try_run_turn (lines 2165-2402)
 */
private suspend fun tryRunTurn(
    router: ToolRouter,
    sess: Session,
    turnContext: TurnContext,
    turnDiffTracker: SharedTurnDiffTracker,
    prompt: Prompt,
    cancellationToken: CancellationToken
): Result<List<ProcessedResponseItem>> {
    // Persist turn context to rollout
    val rolloutItem = RolloutItem.TurnContextHolder(TurnContextItem(
        cwd = turnContext.cwd,
        approvalPolicy = turnContext.approvalPolicy,
        sandboxPolicy = turnContext.sandboxPolicy,
        model = turnContext.model,
        effort = turnContext.reasoningEffort,
        summary = turnContext.reasoningSummary
    ))
    sess.persistRolloutItems(listOf(rolloutItem))

    // Create tool runtime for handling tool calls
    val toolRuntime = ToolCallRuntime(
        router = router,
        session = sess,
        turnContext = turnContext,
        tracker = turnDiffTracker
    )

    // Output queue for processed items (parallel tool execution results)
    val outputQueue = mutableListOf<Deferred<Result<ProcessedResponseItem>>>()
    var activeItem: TurnItem? = null

    // Stream from model client
    // TODO: Implement actual streaming from model client
    // For now, simulate with a channel that would receive ResponseEvent items
    val streamChannel = Channel<ResponseEvent>(Channel.UNLIMITED)

    // In production, this would be populated by the model client stream
    // For now, we'll process any events that come through
    try {
        while (true) {
            // Check for cancellation
            if (cancellationToken.isCancelled()) {
                val processedItems = outputQueue.mapNotNull { deferred ->
                    try { deferred.await().getOrNull() } catch (_: Exception) { null }
                }
                return Result.failure(TurnAbortedException(processedItems))
            }

            // Try to receive next event (with timeout to check cancellation)
            val event = withTimeoutOrNull(100) {
                streamChannel.receiveCatching().getOrNull()
            }

            // If no events and stream is closed, we're done waiting
            // In real implementation, stream would send Completed event
            if (event == null) {
                // For now, return success with collected items
                // Real implementation would wait for Completed event
                break
            }

            when (event) {
                is ResponseEvent.Created -> {
                    // No-op, stream created
                }

                is ResponseEvent.OutputItemDone -> {
                    val previouslyActiveItem = activeItem
                    activeItem = null

                    // Try to build a tool call from the item
                    val toolCallResult = router.buildToolCall(sess, event.item)

                    when {
                        toolCallResult.isSuccess() && toolCallResult.getOrNull() != null -> {
                            val call = toolCallResult.getOrThrow()!!
                            println("INFO: ToolCall: ${call.toolName} ${call.payload}")

                            // Handle tool call asynchronously
                            val deferred = CoroutineScope(Dispatchers.Default).async {
                                val responseResult = toolRuntime.handleToolCall(call, null)
                                val response = responseResult.getOrNull()
                                Result.success(ProcessedResponseItem(
                                    item = event.item,
                                    response = response
                                ))
                            }
                            outputQueue.add(deferred)
                        }

                        toolCallResult.isSuccess() -> {
                            // Not a tool call, handle as non-tool response
                            val turnItem = handleNonToolResponseItem(event.item)
                            if (turnItem != null) {
                                if (previouslyActiveItem == null) {
                                    sess.emitTurnItemStarted(turnContext, turnItem)
                                }
                                sess.emitTurnItemCompleted(turnContext, turnItem)
                            }

                            val deferred = CoroutineScope(Dispatchers.Default).async {
                                Result.success(ProcessedResponseItem(
                                    item = event.item,
                                    response = null
                                ))
                            }
                            outputQueue.add(deferred)
                        }

                        else -> {
                            // Tool call error - respond with error message
                            val errorResult = toolCallResult as? CodexResult.Failure
                            val errorMessage = errorResult?.error?.toException()?.message ?: "Unknown tool call error"

                            val response = ResponseInputItem.FunctionCallOutput(
                                callId = "",
                                output = FunctionCallOutputPayload(
                                    content = errorMessage,
                                    success = false
                                )
                            )

                            val deferred = CoroutineScope(Dispatchers.Default).async {
                                Result.success(ProcessedResponseItem(
                                    item = event.item,
                                    response = response
                                ))
                            }
                            outputQueue.add(deferred)
                        }
                    }
                }

                is ResponseEvent.OutputItemAdded -> {
                    val turnItem = handleNonToolResponseItem(event.item)
                    if (turnItem != null) {
                        sess.emitTurnItemStarted(turnContext, turnItem)
                        activeItem = turnItem
                    }
                }

                is ResponseEvent.RateLimits -> {
                    // Update internal state with latest rate limits
                    sess.updateRateLimits(turnContext, event.snapshot)
                }

                is ResponseEvent.Completed -> {
                    // Update token usage
                    sess.updateTokenUsageInfo(turnContext, event.tokenUsage)

                    // Collect all processed items
                    val processedItems = outputQueue.mapNotNull { deferred ->
                        try { deferred.await().getOrNull() } catch (_: Exception) { null }
                    }

                    // Get unified diff if available
                    val unifiedDiff = turnDiffTracker.computeUnifiedDiff()
                    if (unifiedDiff.isNotEmpty()) {
                        val msg = EventMsg.TurnDiff(TurnDiffEvent(unifiedDiff = unifiedDiff))
                        sess.sendEvent(turnContext, msg)
                    }

                    return Result.success(processedItems)
                }

                is ResponseEvent.OutputTextDelta -> {
                    // Emit text delta for streaming UI
                    val active = activeItem
                    if (active != null) {
                        val deltaEvent = AgentMessageContentDeltaEvent(
                            threadId = sess.conversationId,
                            turnId = turnContext.subId,
                            itemId = active.id(),
                            delta = event.delta
                        )
                        sess.sendEvent(turnContext, EventMsg.AgentMessageContentDelta(deltaEvent))
                    } else {
                        println("WARN: OutputTextDelta without active item")
                    }
                }

                is ResponseEvent.ReasoningSummaryDelta -> {
                    val active = activeItem
                    if (active != null) {
                        val deltaEvent = ReasoningContentDeltaEvent(
                            threadId = sess.conversationId,
                            turnId = turnContext.subId,
                            itemId = active.id(),
                            delta = event.delta,
                            summaryIndex = event.summaryIndex
                        )
                        sess.sendEvent(turnContext, EventMsg.ReasoningContentDelta(deltaEvent))
                    } else {
                        println("WARN: ReasoningSummaryDelta without active item")
                    }
                }

                is ResponseEvent.ReasoningSummaryPartAdded -> {
                    val active = activeItem
                    if (active != null) {
                        val breakEvent = AgentReasoningSectionBreakEvent(
                            itemId = active.id(),
                            summaryIndex = event.summaryIndex
                        )
                        sess.sendEvent(turnContext, EventMsg.AgentReasoningSectionBreak(breakEvent))
                    } else {
                        println("WARN: ReasoningSummaryPartAdded without active item")
                    }
                }

                is ResponseEvent.ReasoningContentDelta -> {
                    val active = activeItem
                    if (active != null) {
                        val deltaEvent = ReasoningRawContentDeltaEvent(
                            threadId = sess.conversationId,
                            turnId = turnContext.subId,
                            itemId = active.id(),
                            delta = event.delta,
                            contentIndex = event.contentIndex
                        )
                        sess.sendEvent(turnContext, EventMsg.ReasoningRawContentDelta(deltaEvent))
                    } else {
                        println("WARN: ReasoningContentDelta without active item")
                    }
                }
            }
        }
    } catch (e: Exception) {
        // Stream error - return failure for retry
        return Result.failure(CodexError.Stream(e.message ?: "Stream error").toException())
    }

    // If we exit the loop without Completed event, return what we have
    val processedItems = outputQueue.mapNotNull { deferred ->
        try { deferred.await().getOrNull() } catch (_: Exception) { null }
    }
    return Result.success(processedItems)
}

// ResponseEvent is now imported from ai.solace.coder.protocol.models

/**
 * Handle a non-tool response item.
 */
private fun handleNonToolResponseItem(item: ResponseItem): TurnItem? {
    return when (item) {
        is ResponseItem.Message,
        is ResponseItem.Reasoning,
        is ResponseItem.WebSearchCall -> parseTurnItem(item)
        is ResponseItem.FunctionCallOutput,
        is ResponseItem.CustomToolCallOutput -> {
            println("DEBUG: unexpected tool output from stream")
            null
        }
        else -> null
    }
}

/**
 * Parse a ResponseItem into a TurnItem for event emission.
 */
private fun parseTurnItem(item: ResponseItem): TurnItem? {
    return when (item) {
        is ResponseItem.Message -> {
            val content = item.content.mapNotNull { ci ->
                when (ci) {
                    is ContentItem.OutputText -> ProtocolUserInput.Text(text = ci.text)
                    else -> null
                }
            }
            TurnItem.UserMessage(item = UserMessageItem(id = item.id ?: "msg_${item.hashCode()}", content = content))
        }
        is ResponseItem.Reasoning -> {
            TurnItem.Reasoning(item = ReasoningItem(
                id = item.id,
                summaryText = item.summary.mapNotNull { (it as? ReasoningItemReasoningSummary.SummaryText)?.text },
                rawContent = item.content?.mapNotNull {
                    when (it) {
                        is ReasoningItemContent.ReasoningText -> it.text
                        is ReasoningItemContent.Text -> it.text
                    }
                } ?: emptyList()
            ))
        }
        is ResponseItem.WebSearchCall -> {
            TurnItem.WebSearch(item = WebSearchItem(
                id = item.id ?: "search_${item.hashCode()}",
                query = "" // WebSearchCall doesn't have query info
            ))
        }
        else -> null
    }
}

/**
 * Get the last assistant message from turn responses.
 */
fun getLastAssistantMessageFromTurn(responses: List<ResponseItem>): String? {
    return responses.asReversed().firstNotNullOfOrNull { item ->
        if (item is ResponseItem.Message && item.role == "assistant") {
            item.content.asReversed().firstNotNullOfOrNull { ci ->
                if (ci is ContentItem.OutputText) ci.text else null
            }
        } else {
            null
        }
    }
}

/**
 * Spawn a review thread for code review functionality.
 *
 * Ported from Rust codex-rs/core/src/codex.rs spawn_review_thread (lines 1803-1893)
 */
private suspend fun spawnReviewThread(
    sess: Session,
    config: Config,
    parentTurnContext: TurnContext,
    subId: String,
    reviewRequest: ReviewRequest
) {
    // Get review model (use config review_model or fall back to main model)
    val reviewModel = config.model // In full impl, would use config.reviewModel
    val reviewModelFamily = findFamilyForModel(reviewModel)
        ?: ModelFamily.default()

    // For reviews, disable web_search and view_image regardless of global settings
    val reviewFeatures = config.features.copy().apply {
        disable(Feature.WebSearchRequest)
        disable(Feature.ViewImageTool)
    }

    // Create tools config for review
    val toolsConfig = ToolsConfig(
        modelFamily = reviewModelFamily,
        features = reviewFeatures,
        webSearchRequest = false,
        includeViewImageTool = false
    )

    // Use review-specific base instructions
    val baseInstructions = REVIEW_PROMPT
    val reviewPrompt = reviewRequest.prompt

    // Build per-turn config with review settings
    val reviewReasoningEffort = ReasoningEffort.Low
    val reviewReasoningSummary = ReasoningSummary.Auto

    // Create review turn context
    val reviewTurnContext = TurnContext(
        subId = subId,
        cwd = parentTurnContext.cwd,
        developerInstructions = null,
        baseInstructions = baseInstructions,
        compactPrompt = parentTurnContext.compactPrompt,
        userInstructions = null,
        approvalPolicy = parentTurnContext.approvalPolicy,
        sandboxPolicy = parentTurnContext.sandboxPolicy,
        shellEnvironmentPolicy = parentTurnContext.shellEnvironmentPolicy,
        toolsConfig = toolsConfig,
        finalOutputJsonSchema = null,
        codexLinuxSandboxExe = parentTurnContext.codexLinuxSandboxExe,
        toolCallGate = ReadinessFlag(),
        execPolicy = parentTurnContext.execPolicy,
        truncationPolicy = TruncationPolicy.Tokens(8000),
        model = reviewModel,
        modelFamily = reviewModelFamily.slug,
        modelContextWindow = reviewModelFamily.contextWindow.toLong(),
        reasoningEffort = reviewReasoningEffort,
        reasoningSummary = reviewReasoningSummary
    )

    // Seed the child task with the review prompt as the initial user message
    val input = listOf(UserInput.Text(reviewPrompt))

    // Spawn review task
    sess.spawnTask(
        reviewTurnContext,
        input,
        ReviewTask(appendToOriginalThread = reviewRequest.appendToOriginalThread)
    )

    // Announce entering review mode so UIs can switch modes
    sess.sendEvent(reviewTurnContext, EventMsg.EnteredReviewMode(reviewRequest))
}

/**
 * Base prompt for code review functionality.
 * Ported from Rust codex-rs/core/src/client_common.rs REVIEW_PROMPT
 */
private const val REVIEW_PROMPT = """
You are a code review assistant. Your task is to review the code changes and provide feedback.

Guidelines:
- Focus on code quality, correctness, and best practices
- Point out potential bugs or issues
- Suggest improvements where appropriate
- Be constructive and helpful in your feedback
- Consider security implications
- Check for proper error handling
- Verify the code follows the project's conventions

Provide your review in a clear, organized format.
"""

/**
 * Template for successful review exit message.
 * Ported from Rust codex-rs/core/templates/review/exit_success.xml
 */
private const val REVIEW_EXIT_SUCCESS_TMPL = """<user_action>
  <context>User initiated a review task. Here's the full review output from reviewer model. User may select one or more comments to resolve.</context>
  <action>review</action>
  <results>
  {results}
  </results>
  </user_action>
"""

/**
 * Template for interrupted review exit message.
 * Ported from Rust codex-rs/core/templates/review/exit_interrupted.xml
 */
private const val REVIEW_EXIT_INTERRUPTED_TMPL = """<user_action>
  <context>User initiated a review task, but was interrupted. If user asks about this, tell them to re-initiate a review with `/review` and wait for it to complete.</context>
  <action>review</action>
  <results>
  None.
  </results>
</user_action>
"""

// =============================================================================
// Supporting Types
// =============================================================================

/**
 * Holder for the active turn with mutex protection.
 */
class ActiveTurnHolder {
    private val mutex = Mutex()
    private var turn: ActiveTurn? = null

    suspend fun <T> withLock(block: suspend (ActiveTurn?) -> T): T {
        return mutex.withLock {
            val result = block(turn)
            if (result is ActiveTurn?) {
                @Suppress("UNCHECKED_CAST")
                turn = result as ActiveTurn?
            }
            result
        }
    }

    suspend fun set(newTurn: ActiveTurn?) {
        mutex.withLock {
            turn = newTurn
        }
    }

    suspend fun get(): ActiveTurn? {
        return mutex.withLock {
            turn
        }
    }
}

/**
 * Session services container.
 *
 * Ported from Rust codex-rs/core/src/state.rs SessionServices
 */
data class SessionServices(
    val mcpConnectionManager: McpConnectionManager,
    val mcpStartupCancellationToken: CancellationToken,
    val unifiedExecManager: UnifiedExecSessionManager,
    val notifier: UserNotifier,
    val rollout: RolloutRecorder?,
    val userShell: Shell,
    val showRawAgentReasoning: Boolean,
    val authManager: AuthManager,
    val toolApprovals: ApprovalStore
)

/**
 * Session state container.
 *
 * Ported from Rust codex-rs/core/src/state.rs SessionState
 */
class SessionState(
    var sessionConfiguration: SessionConfiguration
) {
    private val contextManager = ContextManager()
    var tokenInfo: TokenUsageInfo? = null
    var rateLimits: RateLimitSnapshot? = null

    fun getTotalTokenUsage(): Long {
        return tokenInfo?.totalTokenUsage?.totalTokens ?: 0L
    }

    fun recordItems(items: Iterable<ResponseItem>, truncationPolicy: TruncationPolicy) {
        contextManager.recordItems(items.toList(), truncationPolicy)
    }

    fun replaceHistory(items: List<ResponseItem>) {
        contextManager.replace(items)
    }

    fun cloneHistory(): ContextManager {
        val clone = ContextManager()
        clone.replace(contextManager.contents())
        return clone
    }

    fun updateTokenInfoFromUsage(usage: TokenUsage, contextWindow: Long?) {
        val currentInfo = tokenInfo
        val newTotal = if (currentInfo != null) {
            TokenUsage(
                inputTokens = currentInfo.totalTokenUsage.inputTokens + usage.inputTokens,
                cachedInputTokens = currentInfo.totalTokenUsage.cachedInputTokens + usage.cachedInputTokens,
                outputTokens = currentInfo.totalTokenUsage.outputTokens + usage.outputTokens,
                reasoningOutputTokens = currentInfo.totalTokenUsage.reasoningOutputTokens + usage.reasoningOutputTokens,
                totalTokens = currentInfo.totalTokenUsage.totalTokens + usage.totalTokens
            )
        } else {
            usage
        }

        tokenInfo = TokenUsageInfo(
            totalTokenUsage = newTotal,
            lastTokenUsage = usage,
            modelContextWindow = currentInfo?.modelContextWindow ?: contextWindow
        )
    }

    fun setTokenUsageFull(contextWindow: Long) {
        tokenInfo = TokenUsageInfo.fullContextWindow(contextWindow)
    }
}

// Note: SessionConfiguration, SessionSettingsUpdate are defined in TurnContext.kt
// InitialHistory and SessionSource are imported from ai.solace.coder.protocol

/**
 * Rich session configuration for internal use.
 * Note: TurnContext.kt has a simpler SessionConfiguration
 */
data class CodexSessionConfiguration(
    val provider: ModelProviderInfo,
    val model: String,
    val modelReasoningEffort: ReasoningEffort?,
    val modelReasoningSummary: ReasoningSummary,
    val developerInstructions: String?,
    val userInstructions: String?,
    val baseInstructions: String?,
    val compactPrompt: String?,
    val approvalPolicy: AskForApproval,
    val sandboxPolicy: SandboxPolicy,
    val cwd: String,
    val features: Features,
    val execPolicy: ExecPolicy,
    val sessionSource: SessionSource,
    val shellEnvironmentPolicy: ShellEnvironmentPolicy = ShellEnvironmentPolicy.Inherit(),
    val codexLinuxSandboxExe: String? = null,
    val modelFamily: ModelFamily = ModelFamily.default()
)

/**
 * Prompt for model.
 */
data class Prompt(
    val input: List<ResponseItem>,
    val tools: List<ToolSpec>,
    val parallelToolCalls: Boolean,
    val baseInstructionsOverride: String?,
    val outputSchema: JsonElement?
)

/**
 * Tool specification.
 *
 * When serialized as JSON, this produces a valid "Tool" in the OpenAI Responses API.
 * Matches Rust's ToolSpec enum from core/src/client_common.rs.
 */
sealed class ToolSpec {
    /**
     * A function tool with schema-defined parameters.
     */
    data class Function(
        val name: String,
        val description: String,
        val strict: Boolean = false,
        val parameters: JsonElement?
    ) : ToolSpec()

    /**
     * A local shell tool (no parameters needed).
     */
    data object LocalShell : ToolSpec()

    /**
     * A web search tool (no parameters needed).
     */
    data object WebSearch : ToolSpec()

    /**
     * A freeform/custom tool with format specification.
     */
    data class Freeform(
        val name: String,
        val description: String,
        val format: FreeformToolFormat
    ) : ToolSpec()

    /**
     * Get the name of this tool.
     */
    fun name(): String = when (this) {
        is Function -> name
        is LocalShell -> "local_shell"
        is WebSearch -> "web_search"
        is Freeform -> name
    }
}

/**
 * Format specification for freeform tools.
 */
data class FreeformToolFormat(
    val type: String,
    val syntax: String,
    val definition: String
)

/**
 * A tool for the OpenAI Responses API.
 * Helper class for serializing Function tools.
 */
data class ResponsesApiTool(
    val name: String,
    val description: String,
    val strict: Boolean = false,
    val parameters: JsonElement?
)

// =============================================================================
// Placeholder Types (to be implemented)
// =============================================================================

class Config(
    val modelProvider: ModelProviderInfo = ModelProviderInfo(),
    val model: String = "gpt-4",
    val modelProviderId: String? = null,
    val modelReasoningEffort: ReasoningEffort? = null,
    val modelReasoningSummary: ReasoningSummary = ReasoningSummary.Detailed,
    val developerInstructions: String? = null,
    val baseInstructions: String? = null,
    val compactPrompt: String? = null,
    val approvalPolicy: AskForApproval = AskForApproval.OnFailure,
    val sandboxPolicy: SandboxPolicy = SandboxPolicy.ReadOnly,
    val cwd: String = "/",
    val features: Features = Features.withDefaults(),
    val codexHome: String? = null,
    val mcpServers: Map<String, McpServerConfig> = emptyMap(),
    val notify: NotifyConfig? = null,
    val showRawAgentReasoning: Boolean = false
)

data class ModelProviderInfo(
    val name: String = "openai",
    val apiBase: String? = null
)

data class ModelFamily(
    val slug: String = "gpt-4",
    val baseInstructions: String = "",
    val contextWindow: Int = 128000,
    val supportsParallelToolCalls: Boolean = true
) {
    companion object {
        fun default() = ModelFamily()
    }
}

// ReasoningEffort and ReasoningSummary are imported from ai.solace.coder.protocol.ConfigTypes

// McpServerConfig is now imported from ai.solace.coder.mcp.connection

data class NotifyConfig(
    val enabled: Boolean = false
)

class RolloutRecorder(
    val rolloutPath: String?
) {
    suspend fun recordItems(items: List<RolloutItem>) {}
    suspend fun flush() {}
    suspend fun shutdown() {}

    companion object {
        fun new(config: Config, conversationId: ConversationId): RolloutRecorder? {
            return RolloutRecorder(null)
        }
    }
}

class UserNotifier(config: NotifyConfig?) {
    fun notify(notification: UserNotification) {}
}

sealed class UserNotification {
    data class AgentTurnComplete(
        val threadId: String,
        val turnId: String,
        val cwd: String,
        val inputMessages: List<String>,
        val lastAssistantMessage: String?
    ) : UserNotification()
}

class UnifiedExecSessionManager {
    suspend fun terminateAllSessions() {}
}

// CancellationToken is now imported from ai.solace.coder.utils.concurrent

// ReadinessFlag and ReadinessToken are now imported from ai.solace.coder.utils.readiness

// Helper functions

private suspend fun getUserInstructions(config: Config): String? = null
private fun execPolicyFor(features: Features, codexHome: String?): ExecPolicy = ExecPolicy()
private fun findFamilyForModel(model: String): ModelFamily? = null

/**
 * Convert content items to text, joining non-empty text pieces with newlines.
 * Ignores image content items.
 *
 * Ported from Rust codex-rs/core/src/compact.rs content_items_to_text
 */
fun contentItemsToText(content: List<ContentItem>): String? {
    val pieces = mutableListOf<String>()
    for (item in content) {
        when (item) {
            is ContentItem.InputText -> {
                if (item.text.isNotEmpty()) {
                    pieces.add(item.text)
                }
            }
            is ContentItem.OutputText -> {
                if (item.text.isNotEmpty()) {
                    pieces.add(item.text)
                }
            }
            is ContentItem.InputImage -> {
                // Ignore images
            }
        }
    }
    return if (pieces.isEmpty()) null else pieces.joinToString("\n")
}

/**
 * Check if a message is a summary message (starts with SUMMARY_PREFIX).
 *
 * Ported from Rust codex-rs/core/src/compact.rs is_summary_message
 */
fun isSummaryMessage(message: String): Boolean {
    return message.startsWith("$SUMMARY_PREFIX\n")
}

/**
 * Collect user messages from history for context compaction.
 * Extracts text content from user role messages, filtering out summary messages.
 *
 * Ported from Rust codex-rs/core/src/compact.rs collect_user_messages
 */
private fun collectUserMessages(history: List<ResponseItem>): List<String> {
    return history.filterIsInstance<ResponseItem.Message>()
        .filter { it.role == "user" }
        .mapNotNull { message ->
            val text = contentItemsToText(message.content)
            // Filter out summary messages
            if (text != null && !isSummaryMessage(text)) text else null
        }
}

/**
 * Build compacted history with initial context, preserved user messages, and summary.
 *
 * Creates a new history consisting of:
 * 1. Initial context (system instructions, environment)
 * 2. Preserved user messages
 * 3. Compaction summary as assistant message
 *
 * Ported from Rust codex-rs/core/src/tasks/compact.rs
 */
/**
 * Build compacted history with token-limited user messages.
 * Ported from Rust codex-rs/core/src/compact.rs build_compacted_history
 */
private fun buildCompactedHistory(
    initial: List<ResponseItem>,
    userMessages: List<String>,
    summary: String
): List<ResponseItem> {
    return buildCompactedHistoryWithLimit(
        initial,
        userMessages,
        summary,
        COMPACT_USER_MESSAGE_MAX_TOKENS
    )
}

/**
 * Build compacted history with configurable token limit for user messages.
 * Ported from Rust codex-rs/core/src/compact.rs build_compacted_history_with_limit
 */
private fun buildCompactedHistoryWithLimit(
    initial: List<ResponseItem>,
    userMessages: List<String>,
    summary: String,
    maxTokens: Int
): List<ResponseItem> {
    val result = mutableListOf<ResponseItem>()

    // Add initial context
    result.addAll(initial)

    // Select user messages within token limit (most recent first)
    val selectedMessages = mutableListOf<String>()
    if (maxTokens > 0) {
        var remaining = maxTokens
        for (message in userMessages.asReversed()) {
            if (remaining == 0) break
            val tokens = TruncationPolicy.approxTokenCount(message)
            if (tokens <= remaining) {
                selectedMessages.add(message)
                remaining -= tokens
            } else {
                // Truncate the message to fit within remaining tokens
                val truncated = truncateText(message, TruncationPolicy.Tokens(remaining))
                selectedMessages.add(truncated)
                break
            }
        }
    }

    // Add selected user messages (reverse back to original order)
    for (message in selectedMessages.asReversed()) {
        result.add(ResponseItem.Message(
            role = "user",
            content = listOf(ContentItem.InputText(text = message))
        ))
    }

    // Handle empty summary - Rust uses "(no summary available)" as fallback
    val summaryText = if (summary.isEmpty()) {
        "(no summary available)"
    } else {
        summary
    }

    // Add summary as a user message (matching Rust's ResponseItem::Message with role="user")
    result.add(ResponseItem.Message(
        id = null,
        role = "user",
        content = listOf(ContentItem.InputText(text = summaryText))
    ))

    return result
}
/**
 * Check if remote compaction should be used.
 * Returns true if auth mode is ChatGPT and RemoteCompaction feature is enabled.
 *
 * Ported from Rust codex-rs/core/src/compact.rs should_use_remote_compact_task
 */
private suspend fun shouldUseRemoteCompactTask(sess: Session): Boolean {
    val auth = sess.services.authManager.auth() ?: return false
    return auth.mode == AuthMode.ChatGPT && sess.enabled(Feature.RemoteCompaction)
}

/**
 * Remote compaction task (ChatGPT-backed).
 * Currently a stub - remote compaction uses server-side infrastructure.
 *
 * Ported from Rust codex-rs/core/src/compact.rs (remote compaction path)
 */
private suspend fun runInlineRemoteAutoCompactTask(sess: Session, turnContext: TurnContext) {
    // Remote compaction would use the ChatGPT backend for server-side compaction.
    // For now, fall back to local compaction.
    runInlineAutoCompactTask(sess, turnContext)
}

/**
 * Run inline auto-compaction using the configured compact prompt.
 *
 * Ported from Rust codex-rs/core/src/compact.rs run_inline_auto_compact_task
 */
private suspend fun runInlineAutoCompactTask(sess: Session, turnContext: TurnContext) {
    val prompt = turnContext.compactPromptOrDefault()
    val input = listOf(UserInput.Text(prompt))
    runCompactTaskInner(sess, turnContext, input)
}

/**
 * Run a compaction task with TaskStarted event.
 *
 * Ported from Rust codex-rs/core/src/compact.rs run_compact_task
 */
private suspend fun runCompactTask(sess: Session, turnContext: TurnContext, input: List<UserInput>) {
    val startEvent = EventMsg.TaskStarted(TaskStartedEvent(
        modelContextWindow = turnContext.modelContextWindow
    ))
    sess.sendEvent(turnContext, startEvent)
    runCompactTaskInner(sess, turnContext, input)
}

/**
 * Inner implementation of compaction task.
 * Performs the actual context compaction by summarizing conversation history.
 *
 * Ported from Rust codex-rs/core/src/compact.rs run_compact_task_inner
 */
private suspend fun runCompactTaskInner(sess: Session, turnContext: TurnContext, input: List<UserInput>) {
    // Build the input item from user input
    val inputText = input.filterIsInstance<UserInput.Text>().joinToString("\n") { it.content }
    val initialInputForTurn = ResponseItem.Message(
        role = "user",
        content = listOf(ContentItem.InputText(text = inputText))
    )

    // Clone history and record the compaction request
    val history = sess.cloneHistory()
    history.recordItems(
        listOf(initialInputForTurn),
        turnContext.truncationPolicy
    )

    // For inline compaction, we simulate the summarization by:
    // 1. Using the existing history
    // 2. Creating a summary from the last assistant message
    val historySnapshot = history.getHistory()
    val summaryContent = getLastAssistantMessageFromTurn(historySnapshot) ?: ""
    val summaryText = "$SUMMARY_PREFIX\n$summaryContent"
    val userMessages = collectUserMessages(historySnapshot)

    // Build new compacted history
    val initialContext = sess.buildInitialContext(turnContext)
    val newHistory = buildCompactedHistory(initialContext, userMessages, summaryText).toMutableList()

    // Preserve ghost snapshots
    val ghostSnapshots = historySnapshot.filter { it is ResponseItem.GhostSnapshot }
    newHistory.addAll(ghostSnapshots)

    // Replace session history with compacted version
    sess.replaceHistory(newHistory)
    sess.recomputeTokenUsage(turnContext)

    // Persist rollout item
    val rolloutItem = RolloutItem.Compacted(
        ai.solace.coder.protocol.CompactedItem(
            message = summaryText,
            replacementHistory = null
        )
    )
    sess.persistRolloutItems(listOf(rolloutItem))

    // Send compaction event
    val event = EventMsg.ContextCompacted(ContextCompactedEvent())
    sess.sendEvent(turnContext, event)

    // Send warning about compaction limitations
    val warning = EventMsg.Warning(WarningEvent(
        message = "Heads up: Long conversations and multiple compactions can cause the model to be less accurate. Start a new conversation when possible to keep conversations small and targeted."
    ))
    sess.sendEvent(turnContext, warning)
}
private fun discoverCustomPrompts(): List<CustomPrompt> = emptyList()
private fun backoff(retries: Int): Long = minOf(1000L * (1 shl retries), 30000L)

private const val PARALLEL_INSTRUCTIONS = """
When multiple tool calls are independent and can be executed in parallel,
invoke them in a single response to improve efficiency.
"""

// Exception types for turn execution
class TurnAbortedException(val danglingArtifacts: List<ProcessedResponseItem>) : Exception()
class InterruptedException : Exception()
class ContextWindowExceededException : Exception()

class DeveloperInstructions(val text: String) {
    fun toResponseItem(): ResponseItem {
        return ResponseItem.Message(
            role = "system",
            content = listOf(ContentItem.InputText(text = text))
        )
    }
}

class UserInstructions(val text: String, val directory: String) {
    fun toResponseItem(): ResponseItem {
        return ResponseItem.Message(
            role = "system",
            content = listOf(ContentItem.InputText(text = text))
        )
    }
}

class EnvironmentContext(
    val cwd: String?,
    val approvalPolicy: AskForApproval?,
    val sandboxPolicy: SandboxPolicy?,
    val shell: Shell
) {
    fun equalsExceptShell(other: EnvironmentContext): Boolean {
        return cwd == other.cwd &&
            approvalPolicy == other.approvalPolicy &&
            sandboxPolicy == other.sandboxPolicy
    }

    fun toResponseItem(): ResponseItem {
        return ResponseItem.Message(
            role = "system",
            content = listOf(ContentItem.InputText(
                text = "Environment: cwd=$cwd, policy=$approvalPolicy, sandbox=$sandboxPolicy"
            ))
        )
    }

    companion object {
        fun from(context: TurnContext): EnvironmentContext {
            return EnvironmentContext(
                cwd = context.cwd,
                approvalPolicy = context.approvalPolicy,
                sandboxPolicy = context.sandboxPolicy,
                shell = ShellDetector().defaultUserShell()
            )
        }

        fun diff(prev: TurnContext, next: TurnContext): EnvironmentContext {
            return EnvironmentContext(
                cwd = if (prev.cwd != next.cwd) next.cwd else null,
                approvalPolicy = if (prev.approvalPolicy != next.approvalPolicy) next.approvalPolicy else null,
                sandboxPolicy = if (prev.sandboxPolicy != next.sandboxPolicy) next.sandboxPolicy else null,
                shell = ShellDetector().defaultUserShell()
            )
        }
    }
}

// Note: SessionTask, SessionTaskContext are defined in Turn.kt

class RegularTask : SessionTask {
    override fun kind() = TaskKind.Regular
    override suspend fun run(
        sessionContext: SessionTaskContext,
        turnContext: TurnContext,
        input: List<UserInput>,
        cancellationToken: CancellationToken
    ): String? {
        return runTask(sessionContext.getSession(), turnContext, input, cancellationToken)
    }
}

class UserShellCommandTask(private val command: String) : SessionTask {
    private val processExecutor = ProcessExecutor()

    override fun kind() = TaskKind.Regular

    override suspend fun run(
        sessionContext: SessionTaskContext,
        turnContext: TurnContext,
        input: List<UserInput>,
        cancellationToken: CancellationToken
    ): String? {
        val session = sessionContext.getSession()
        val callId = "user-shell-${kotlin.random.Random.nextLong()}"

        // Use login shell for user commands
        val shell = session.userShell()
        val execArgs = shell.deriveExecArgs(command, useLoginShell = true)

        // Send TaskStarted event
        session.sendEvent(
            turnContext,
            EventMsg.TaskStarted(TaskStartedEvent(modelContextWindow = null))
        )

        // Check for cancellation before execution
        if (cancellationToken.isCancelled()) {
            return null
        }

        val cwd = turnContext.cwd
        val parsedCmd = listOf(ParsedCommand.Unknown(cmd = execArgs.first()))

        // Send ExecCommandBegin event
        session.sendEvent(
            turnContext,
            EventMsg.ExecCommandBegin(ExecCommandBeginEvent(
                callId = callId,
                processId = null,
                turnId = turnContext.subId,
                command = execArgs,
                cwd = cwd,
                parsedCmd = parsedCmd,
                source = ExecCommandSource.UserShell,
                interactionInput = command
            ))
        )

        // Execute the command with 1-hour timeout (Rust: USER_SHELL_TIMEOUT_MS)
        val execParams = ExecParams(
            command = execArgs,
            cwd = cwd,
            expiration = ExecExpiration.Timeout(USER_SHELL_TIMEOUT),
            env = emptyMap()
        )

        var stdout = ""
        var stderr = ""
        var exitCode = 0
        var execDuration: Duration = Duration.ZERO

        execDuration = measureTime {
            val result = processExecutor.execute(
                params = execParams,
                sandboxPolicy = ai.solace.coder.protocol.SandboxPolicy.DangerFullAccess,
                sandboxCwd = cwd
            )

            result.fold(
                onSuccess = { output ->
                    stdout = output.stdout.text
                    stderr = output.stderr.text
                    exitCode = output.exitCode
                },
                onFailure = { error ->
                    stderr = error.toString()
                    exitCode = 1
                }
            )
        }

        // Format output for display
        val formattedOutput = buildString {
            if (stdout.isNotEmpty()) {
                append(stdout)
            }
            if (stderr.isNotEmpty()) {
                if (isNotEmpty()) append("\n")
                append(stderr)
            }
        }

        val aggregatedOutput = buildString {
            if (stdout.isNotEmpty()) append(stdout)
            if (stderr.isNotEmpty()) {
                if (isNotEmpty()) append("\n")
                append(stderr)
            }
        }

        // Send ExecCommandEnd event
        session.sendEvent(
            turnContext,
            EventMsg.ExecCommandEnd(ExecCommandEndEvent(
                callId = callId,
                processId = null,
                turnId = turnContext.subId,
                command = execArgs,
                cwd = cwd,
                parsedCmd = parsedCmd,
                source = ExecCommandSource.UserShell,
                interactionInput = command,
                stdout = stdout,
                stderr = stderr,
                aggregatedOutput = aggregatedOutput,
                exitCode = exitCode,
                duration = execDuration.toString(),
                formattedOutput = formattedOutput
            ))
        )

        return null
    }
}

class UndoTask : SessionTask {
    private val gitOperations = ShellGitOperations()

    override fun kind() = TaskKind.Regular

    override suspend fun run(
        sessionContext: SessionTaskContext,
        turnContext: TurnContext,
        input: List<UserInput>,
        cancellationToken: CancellationToken
    ): String? {
        val session = sessionContext.getSession()

        // Send undo started event
        session.sendEvent(
            turnContext,
            EventMsg.UndoStarted(UndoStartedEvent(message = "Undo in progress..."))
        )

        // Check for cancellation
        if (cancellationToken.isCancelled()) {
            session.sendEvent(
                turnContext,
                EventMsg.UndoCompleted(UndoCompletedEvent(
                    success = false,
                    message = "Undo cancelled."
                ))
            )
            return null
        }

        // Get history and find the most recent ghost snapshot
        val history = session.cloneHistory()
        val items = history.getHistory().toMutableList()

        // Find the most recent ghost snapshot (search in reverse)
        var foundIdx: Int? = null
        var foundCommit: ai.solace.coder.utils.git.GhostCommit? = null

        for (i in items.indices.reversed()) {
            val item = items[i]
            if (item is ResponseItem.GhostSnapshot) {
                foundIdx = i
                foundCommit = item.ghostCommit
                break
            }
        }

        if (foundIdx == null || foundCommit == null) {
            session.sendEvent(
                turnContext,
                EventMsg.UndoCompleted(UndoCompletedEvent(
                    success = false,
                    message = "No ghost snapshot available to undo."
                ))
            )
            return null
        }

        val commitId = foundCommit.id
        val repoPath = turnContext.cwd

        // Restore the ghost commit
        val restoreResult = gitOperations.restoreGhostCommit(repoPath, foundCommit)

        restoreResult.fold(
            onSuccess = {
                // Remove the snapshot from history
                items.removeAt(foundIdx)
                session.replaceHistory(items)

                val shortId = commitId.take(7)
                println("INFO: Undo restored ghost snapshot $commitId")

                session.sendEvent(
                    turnContext,
                    EventMsg.UndoCompleted(UndoCompletedEvent(
                        success = true,
                        message = "Undo restored snapshot $shortId."
                    ))
                )
            },
            onFailure = { error ->
                val message = "Failed to restore snapshot $commitId: $error"
                println("WARN: $message")

                session.sendEvent(
                    turnContext,
                    EventMsg.UndoCompleted(UndoCompletedEvent(
                        success = false,
                        message = message
                    ))
                )
            }
        )

        return null
    }
}

/**
 * Task that compacts conversation history to reduce token usage.
 *
 * The compact task:
 * 1. Clones current history
 * 2. Collects user messages for preservation
 * 3. Sends compaction request to model for summarization
 * 4. Builds compacted history with initial context + user messages + summary
 * 5. Replaces session history with compacted version
 * 6. Persists compacted state to rollout
 * 7. Sends ContextCompacted event
 *
 * Ported from Rust codex-rs/core/src/tasks/compact.rs
 */
class CompactTask : SessionTask {
    override fun kind() = TaskKind.Compact

    override suspend fun run(
        sessionContext: SessionTaskContext,
        turnContext: TurnContext,
        input: List<UserInput>,
        cancellationToken: CancellationToken
    ): String? {
        val session = sessionContext.getSession()

        // Check for cancellation
        if (cancellationToken.isCancelled()) {
            return null
        }

        // Clone current history
        val history = session.cloneHistory()
        val snapshot = history.getHistory()

        // If history is empty or too short, nothing to compact
        if (snapshot.size <= 3) {
            println("INFO: compact task skipped - history too short (${snapshot.size} items)")
            return null
        }

        // Collect user messages for preservation
        val userMessages = collectUserMessages(snapshot)

        // Build the compaction prompt
        val compactPrompt = turnContext.getCompactPrompt()

        // Format history for summarization
        val historyText = formatHistoryForCompaction(snapshot)

        // Build the compaction request message
        val compactionRequest = buildString {
            appendLine(compactPrompt)
            appendLine()
            appendLine("=== Conversation History ===")
            appendLine(historyText)
            appendLine()
            appendLine("=== End of History ===")
            appendLine()
            appendLine("Please provide a concise summary that captures the essential context for continuing this conversation.")
        }

        // Send to model for summarization using regular task flow
        val compactionInput = listOf(UserInput.Text(compactionRequest))
        val summaryResult = runTask(session, turnContext, compactionInput, cancellationToken)

        // Use the summary or a default if model didn't respond
        val summary = summaryResult ?: "[Previous conversation context compacted]"

        // Build initial context
        val initialContext = session.buildInitialContext(turnContext)

        // Build compacted history
        val compactedHistory = buildCompactedHistory(initialContext, userMessages, summary)

        // Replace session history
        session.replaceHistory(compactedHistory)

        // Persist compacted state to rollout
        val rolloutItem = ai.solace.coder.protocol.RolloutItem.Compacted(
            payload = ai.solace.coder.protocol.CompactedItem(
                message = summary,
                replacementHistory = compactedHistory
            )
        )
        session.persistRolloutItems(listOf(rolloutItem))

        // Send ContextCompacted event
        session.sendEvent(
            turnContext,
            EventMsg.ContextCompacted(ContextCompactedEvent())
        )

        println("INFO: compact task completed - history compacted from ${snapshot.size} to ${compactedHistory.size} items")

        return null
    }
}

/**
 * Format history items as text for compaction prompt.
 */
private fun formatHistoryForCompaction(history: List<ResponseItem>): String {
    return buildString {
        for (item in history) {
            when (item) {
                is ResponseItem.Message -> {
                    val role = item.role.uppercase()
                    val content = item.content.joinToString("\n") { contentItem ->
                        when (contentItem) {
                            is ContentItem.InputText -> contentItem.text
                            is ContentItem.OutputText -> contentItem.text
                            else -> ""
                        }
                    }
                    if (content.isNotBlank()) {
                        appendLine("[$role]: $content")
                        appendLine()
                    }
                }
                is ResponseItem.FunctionCall -> {
                    appendLine("[TOOL CALL: ${item.name}]")
                }
                is ResponseItem.FunctionCallOutput -> {
                    val output = item.output.content.take(200)
                    val truncated = if (item.output.content.length > 200) "..." else ""
                    appendLine("[TOOL OUTPUT]: $output$truncated")
                }
                is ResponseItem.Reasoning -> {
                    item.summary.firstOrNull()?.let { summary ->
                        if (summary is ai.solace.coder.protocol.ReasoningItemReasoningSummary.SummaryText) {
                            appendLine("[REASONING]: ${summary.text.take(100)}...")
                        }
                    }
                }
                else -> {
                    // Skip other item types
                }
            }
        }
    }
}

class GhostSnapshotTask(
    private val token: ReadinessToken,
    private val readinessFlag: ReadinessFlag
) : SessionTask {
    private val gitOperations = ShellGitOperations()

    override fun kind() = TaskKind.Regular

    override suspend fun run(
        sessionContext: SessionTaskContext,
        turnContext: TurnContext,
        input: List<UserInput>,
        cancellationToken: CancellationToken
    ): String? {
        // Spawn as independent coroutine so the main flow can continue
        val session = sessionContext.getSession()
        val scope = CoroutineScope(Dispatchers.Default + SupervisorJob())

        scope.launch {
            try {
                val repoPath = turnContext.cwd

                // First, compute a snapshot report to warn about large untracked directories
                val reportResult = gitOperations.captureGhostSnapshotReport(
                    CreateGhostCommitOptions.new(repoPath)
                )

                reportResult.onSuccess { report ->
                    formatLargeUntrackedWarning(report)?.let { message ->
                        session.sendEvent(
                            turnContext,
                            EventMsg.Warning(WarningEvent(message))
                        )
                    }
                }

                // Check for cancellation
                if (cancellationToken.isCancelled()) {
                    println("INFO: ghost snapshot task cancelled")
                    markReadyBestEffort()
                    return@launch
                }

                // Create the ghost commit
                val options = CreateGhostCommitOptions.new(repoPath)
                val commitResult = gitOperations.createGhostCommit(options)

                commitResult.fold(
                    onSuccess = { ghostCommit ->
                        println("INFO: ghost snapshot blocking task finished")
                        session.recordConversationItems(
                            turnContext,
                            listOf(ResponseItem.GhostSnapshot(ghostCommit = ghostCommit))
                        )
                        println("INFO: ghost commit captured: ${ghostCommit.id}")
                    },
                    onFailure = { error ->
                        when (error) {
                            is GitToolingError.NotAGitRepository -> {
                                println("INFO: skipping ghost snapshot because current directory is not a Git repository")
                            }
                            else -> {
                                println("WARN: failed to capture ghost snapshot: $error")
                            }
                        }
                    }
                )
            } catch (e: Exception) {
                println("WARN: ghost snapshot task failed: $e")
                val message = "Snapshots disabled after ghost snapshot error: $e."
                session.notifyBackgroundEvent(turnContext, message)
            } finally {
                markReadyBestEffort()
            }
        }

        // Return null - this task runs in background and doesn't produce a direct response
        return null
    }

    private suspend fun markReadyBestEffort() {
        val result = readinessFlag.markReady(token)
        result.fold(
            onSuccess = { marked ->
                if (marked) {
                    println("INFO: ghost snapshot gate marked ready")
                } else {
                    println("WARN: ghost snapshot gate already ready")
                }
            },
            onFailure = { error ->
                println("WARN: failed to mark ghost snapshot ready: $error")
            }
        )
    }

    companion object {
        private const val MAX_DIRS = 3

        /**
         * Format a warning message about large untracked directories.
         */
        fun formatLargeUntrackedWarning(report: GhostSnapshotReport): String? {
            if (report.largeUntrackedDirs.isEmpty()) {
                return null
            }

            val parts = mutableListOf<String>()
            for (dir in report.largeUntrackedDirs.take(MAX_DIRS)) {
                parts.add("${dir.path} (${dir.fileCount} files)")
            }

            if (report.largeUntrackedDirs.size > MAX_DIRS) {
                val remaining = report.largeUntrackedDirs.size - MAX_DIRS
                parts.add("$remaining more")
            }

            return "Repository snapshot encountered large untracked directories: ${parts.joinToString(", ")}. " +
                "This can slow Codex; consider adding these paths to .gitignore or disabling undo in your config."
        }
    }
}

/**
 * Task that performs code review on conversation history.
 *
 * The review task:
 * 1. Runs the review prompt against the model
 * 2. Parses the response for review findings
 * 3. Optionally appends findings to the original thread
 * 4. Sends ExitedReviewMode event with review output
 *
 * Ported from Rust codex-rs/core/src/tasks/review.rs
 */
class ReviewTask(private val appendToOriginalThread: Boolean) : SessionTask {
    override fun kind() = TaskKind.Review

    /**
     * Cleanup on abort - ensures ExitedReviewMode event is sent.
     * Ported from Rust codex-rs/core/src/tasks/review.rs ReviewTask::abort
     */
    override suspend fun abort(sessionContext: SessionTaskContext, turnContext: TurnContext) {
        exitReviewMode(sessionContext.getSession(), turnContext, null)
    }

    override suspend fun run(
        sessionContext: SessionTaskContext,
        turnContext: TurnContext,
        input: List<UserInput>,
        cancellationToken: CancellationToken
    ): String? {
        val session = sessionContext.getSession()

        // Check for cancellation
        if (cancellationToken.isCancelled()) {
            exitReviewMode(session, turnContext, null)
            return null
        }

        // Run the review task using the regular task flow
        val reviewResponse = runTask(session, turnContext, input, cancellationToken)

        // Parse the review output from the response
        val reviewOutput = parseReviewOutputEvent(reviewResponse)

        // Exit review mode (handles history recording and event sending)
        if (!cancellationToken.isCancelled()) {
            exitReviewMode(session, turnContext, reviewOutput)
        }

        return null // Rust returns None
    }

    /**
     * Parse a ReviewOutputEvent from a text blob returned by the reviewer model.
     * If the text is valid JSON matching ReviewOutputEvent, deserialize it.
     * Otherwise, attempt to extract the first JSON object substring and parse it.
     * If parsing still fails, return a structured fallback carrying the plain text
     * in `overall_explanation`.
     *
     * Ported from Rust codex-rs/core/src/tasks/review.rs parse_review_output_event
     */
    private fun parseReviewOutputEvent(text: String?): ReviewOutputEvent? {
        if (text.isNullOrBlank()) {
            return null
        }

        // Try direct JSON parsing first
        try {
            return kotlinx.serialization.json.Json { ignoreUnknownKeys = true }
                .decodeFromString<ReviewOutputEvent>(text)
        } catch (_: Exception) {
            // Continue to fallback parsing
        }

        // Try to extract JSON object from text
        val startBrace = text.indexOf('{')
        val endBrace = text.lastIndexOf('}')
        if (startBrace >= 0 && endBrace > startBrace) {
            val jsonSlice = text.substring(startBrace, endBrace + 1)
            try {
                return kotlinx.serialization.json.Json { ignoreUnknownKeys = true }
                    .decodeFromString<ReviewOutputEvent>(jsonSlice)
            } catch (_: Exception) {
                // Continue to fallback
            }
        }

        // Fallback: create structured output from plain text
        return ReviewOutputEvent(
            overallExplanation = text,
            findings = emptyList(),
            overallCorrectness = "",
            overallConfidenceScore = 0.0f
        )
    }

    /**
     * Format review findings into a readable block.
     * Ported from Rust codex-rs/core/src/review_format.rs format_review_findings_block
     */
    private fun formatReviewFindingsBlock(findings: List<ReviewFinding>): String {
        if (findings.isEmpty()) return ""
        return buildString {
            appendLine("### Review Findings")
            appendLine()
            for ((index, finding) in findings.withIndex()) {
                appendLine("${index + 1}. **${finding.title}**")
                if (finding.body.isNotBlank()) {
                    appendLine("   ${finding.body.replace("\n", "\n   ")}")
                }
                val loc = finding.codeLocation
                if (loc.absoluteFilePath.isNotEmpty()) {
                    appendLine("   Location: ${loc.absoluteFilePath}:${loc.lineRange.start}-${loc.lineRange.end}")
                }
                appendLine()
            }
        }
    }

    /**
     * Emits an ExitedReviewMode Event with optional ReviewOutput,
     * and optionally records a user message with the review output.
     *
     * Ported from Rust codex-rs/core/src/tasks/review.rs exit_review_mode
     */
    private suspend fun exitReviewMode(
        session: Session,
        turnContext: TurnContext,
        reviewOutput: ReviewOutputEvent?
    ) {
        // Record to original thread if requested
        if (appendToOriginalThread) {
            val userMessage = if (reviewOutput != null) {
                val findingsStr = buildString {
                    val explanation = reviewOutput.overallExplanation.trim()
                    if (explanation.isNotEmpty()) {
                        append(explanation)
                    }
                    if (reviewOutput.findings.isNotEmpty()) {
                        val block = formatReviewFindingsBlock(reviewOutput.findings)
                        append("\n$block")
                    }
                }
                REVIEW_EXIT_SUCCESS_TMPL.replace("{results}", findingsStr)
            } else {
                REVIEW_EXIT_INTERRUPTED_TMPL
            }

            // Record as user message (matching Rust)
            session.recordConversationItems(
                turnContext,
                listOf(ResponseItem.Message(
                    id = null,
                    role = "user",
                    content = listOf(ContentItem.InputText(text = userMessage))
                ))
            )
        }

        // Send ExitedReviewMode event
        session.sendEvent(
            turnContext,
            EventMsg.ExitedReviewMode(ExitedReviewModeEvent(reviewOutput = reviewOutput))
        )
    }
}

// Extension functions
fun ResponseInputItem.toResponseItem(): ResponseItem {
    return when (this) {
        is ResponseInputItem.FunctionCallOutput -> ResponseItem.FunctionCallOutput(
            callId = callId,
            output = output
        )
        is ResponseInputItem.CustomToolCallOutput -> ResponseItem.CustomToolCallOutput(
            callId = callId,
            output = output
        )
        else -> ResponseItem.Message(role = "user", content = emptyList())
    }
}

fun ResponseInputItem.Companion.fromUserInput(input: List<UserInput>): ResponseInputItem {
    val content = input.map { userInput ->
        when (userInput) {
            is UserInput.Text -> ContentItem.InputText(text = userInput.content)
            is UserInput.Image -> ContentItem.InputImage(imageUrl = "data:${userInput.mimeType};base64,...")
            is UserInput.FileRef -> ContentItem.InputText(text = "[File: ${userInput.path}]")
        }
    }
    return ResponseInputItem.Message(
        role = "user",
        content = content
    )
}

fun UserInput.toResponseInputItem(): ResponseInputItem {
    return when (this) {
        is UserInput.Text -> ResponseInputItem.Message(
            role = "user",
            content = listOf(ContentItem.InputText(text = content))
        )
        is UserInput.Image -> ResponseInputItem.Message(
            role = "user",
            content = listOf(ContentItem.InputImage(imageUrl = "data:$mimeType;base64,..."))
        )
        is UserInput.FileRef -> ResponseInputItem.Message(
            role = "user",
            content = listOf(ContentItem.InputText(text = "[File: $path]"))
        )
    }
}

fun CodexError.httpStatusCodeValue(): Int? = null
