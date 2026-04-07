package com.openai.codex.agent

import android.app.agent.AgentManager
import android.app.agent.AgentService
import android.app.agent.AgentSessionEvent
import android.app.agent.AgentSessionInfo
import android.os.Process
import android.util.Log
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.concurrent.thread
import org.json.JSONObject

class CodexAgentService : AgentService() {
    companion object {
        private const val TAG = "CodexAgentService"
        private const val BRIDGE_REQUEST_PREFIX = "__codex_bridge__ "
        private const val BRIDGE_RESPONSE_PREFIX = "__codex_bridge_result__ "
        private const val BRIDGE_METHOD_GET_RUNTIME_STATUS = "getRuntimeStatus"
        private val handledGenieQuestions = java.util.concurrent.ConcurrentHashMap.newKeySet<String>()
        private val pendingGenieQuestions = java.util.concurrent.ConcurrentHashMap.newKeySet<String>()
        private val pendingQuestionLoads = java.util.concurrent.ConcurrentHashMap.newKeySet<String>()
        private val handledBridgeRequests = java.util.concurrent.ConcurrentHashMap.newKeySet<String>()
        private val pendingParentRollups = java.util.concurrent.ConcurrentHashMap.newKeySet<String>()
        private val retentionPruneScheduled = AtomicBoolean(false)
    }

    private val agentManager by lazy { getSystemService(AgentManager::class.java) }
    private val sessionController by lazy { AgentSessionController(this) }
    private val presentationPolicyStore by lazy { SessionPresentationPolicyStore(this) }

    override fun onCreate() {
        super.onCreate()
        scheduleRetentionPrune()
    }

    override fun onSessionChanged(session: AgentSessionInfo) {
        Log.i(TAG, "onSessionChanged $session")
        maybeRollUpParentSession(session)
        agentManager?.let { manager ->
            if (shouldServeSessionBridge(session)) {
                AgentSessionBridgeServer.ensureStarted(this, manager, session.sessionId)
            } else if (isTerminalSessionState(session.state)) {
                AgentSessionBridgeServer.closeSession(session.sessionId)
            }
        }
        if (isTerminalSessionState(session.state) && !DesktopInspectionRegistry.isPlannerAttached(session.sessionId)) {
            AgentPlannerQuestionRegistry.cancelQuestion(
                sessionId = session.sessionId,
                reason = "Planner session ended before the question was answered",
            )
            AgentPlannerRuntimeManager.closeSession(session.sessionId)
            scheduleRetentionPrune()
        }
        if (isTerminalSessionState(session.state)) {
            scheduleRetentionPrune()
        }
        if (session.state != AgentSessionInfo.STATE_WAITING_FOR_USER) {
            return
        }
        if (!pendingQuestionLoads.add(session.sessionId)) {
            return
        }
        thread(name = "CodexAgentQuestionLoad-${session.sessionId}") {
            try {
                handleWaitingSession(session)
            } finally {
                pendingQuestionLoads.remove(session.sessionId)
            }
        }
    }

    override fun onSessionRemoved(sessionId: String) {
        Log.i(TAG, "onSessionRemoved sessionId=$sessionId")
        AgentSessionBridgeServer.closeSession(sessionId)
        AgentPlannerRuntimeManager.closeSession(sessionId)
        DesktopInspectionRegistry.removeSession(sessionId)
        AgentPlannerQuestionRegistry.cancelQuestion(
            sessionId = sessionId,
            reason = "Planner session was removed before the question was answered",
        )
        AgentQuestionNotifier.cancel(this, sessionId)
        AgentQuestionNotifier.clearSessionState(sessionId)
        presentationPolicyStore.removePolicy(sessionId)
        handledGenieQuestions.removeIf { it.startsWith("$sessionId:") }
        handledBridgeRequests.removeIf { it.startsWith("$sessionId:") }
        pendingGenieQuestions.removeIf { it.startsWith("$sessionId:") }
        scheduleRetentionPrune()
    }

    private fun scheduleRetentionPrune() {
        if (!retentionPruneScheduled.compareAndSet(false, true)) {
            return
        }
        thread(name = "CodexAgentRetentionPrune") {
            try {
                sessionController.enforceRetentionPolicy()
            } finally {
                retentionPruneScheduled.set(false)
            }
        }
    }

    override fun onShowOrUpdateSessionNotification(
        session: AgentSessionInfo,
        notificationToken: String,
        notificationText: String,
    ) {
        showOrUpdateSessionNotification(
            session = session,
            notificationToken = notificationToken,
            notificationText = notificationText,
        )
    }

    override fun onCancelSessionNotification(
        sessionId: String,
        notificationToken: String,
        reason: Int,
    ) {
        cancelSessionNotification(
            sessionId = sessionId,
            notificationToken = notificationToken,
            reason = reason,
        )
    }

    private fun maybeRollUpParentSession(session: AgentSessionInfo) {
        val parentSessionId = when {
            !session.parentSessionId.isNullOrBlank() -> session.parentSessionId
            isDirectParentSession(session) -> session.sessionId
            else -> null
        } ?: return
        if (!pendingParentRollups.add(parentSessionId)) {
            return
        }
        thread(name = "CodexAgentParentRollup-$parentSessionId") {
            try {
                runCatching {
            rollUpParentSession(parentSessionId)
        }.onFailure { err ->
            Log.w(TAG, "Parent session roll-up failed for $parentSessionId", err)
                }
            } finally {
                pendingParentRollups.remove(parentSessionId)
            }
        }
    }

    private fun rollUpParentSession(parentSessionId: String) {
        val manager = agentManager ?: return
        val sessions = manager.getSessions(currentUserId())
        val parentSession = sessions.firstOrNull { it.sessionId == parentSessionId } ?: return
        if (!isDirectParentSession(parentSession)) {
            return
        }
        val childSessions = sessions.filter { childSession ->
            childSession.parentSessionId == parentSessionId &&
                (
                    parentSession.continuationGeneration == AgentSessionInfo.CONTINUATION_GENERATION_NONE ||
                        childSession.continuationGeneration == parentSession.continuationGeneration
                    )
        }
        if (childSessions.isEmpty()) {
            return
        }
        val rollup = AgentParentSessionAggregator.rollup(
            childSessions.map { childSession ->
                val events = manager.getSessionEvents(childSession.sessionId)
                ParentSessionChildSummary(
                    sessionId = childSession.sessionId,
                    targetPackage = childSession.targetPackage,
                    state = childSession.state,
                    targetPresentation = childSession.targetPresentation,
                    requiredFinalPresentationPolicy = presentationPolicyStore.getPolicy(childSession.sessionId),
                    latestResult = findLastEventMessage(events, AgentSessionEvent.TYPE_RESULT),
                    latestError = findLastEventMessage(events, AgentSessionEvent.TYPE_ERROR),
                )
            },
        )
        val deferTerminalRollup =
            DesktopInspectionRegistry.isPlannerAttached(parentSessionId) &&
                isTerminalSessionState(rollup.state)
        rollup.sessionsToAttach.forEach { childSessionId ->
            runCatching {
                manager.attachTarget(childSessionId)
                manager.publishTrace(
                    parentSessionId,
                    "Requested attach for $childSessionId to satisfy the required final presentation policy.",
                )
            }.onFailure { err ->
                Log.w(TAG, "Failed to attach target for $childSessionId", err)
            }
        }
        if (!deferTerminalRollup && shouldUpdateParentSessionState(parentSession.state, rollup.state)) {
            runCatching {
                manager.updateSessionState(parentSessionId, rollup.state)
            }.onFailure { err ->
                Log.w(TAG, "Failed to update parent session state for $parentSessionId", err)
            }
        }
        val parentEvents = if (!deferTerminalRollup && (rollup.resultMessage != null || rollup.errorMessage != null)) {
            manager.getSessionEvents(parentSessionId)
        } else {
            emptyList()
        }
        if (
            !deferTerminalRollup &&
            rollup.resultMessage != null &&
            findLastEventMessage(parentEvents, AgentSessionEvent.TYPE_RESULT) != rollup.resultMessage
        ) {
            runCatching {
                manager.publishResult(parentSessionId, rollup.resultMessage)
            }.onFailure { err ->
                Log.w(TAG, "Failed to publish parent result for $parentSessionId", err)
            }
        }
        if (
            !deferTerminalRollup &&
            rollup.errorMessage != null &&
            findLastEventMessage(parentEvents, AgentSessionEvent.TYPE_ERROR) != rollup.errorMessage
        ) {
            runCatching {
                manager.publishError(parentSessionId, rollup.errorMessage)
            }.onFailure { err ->
                Log.w(TAG, "Failed to publish parent error for $parentSessionId", err)
            }
        }
    }

    private fun shouldServeSessionBridge(session: AgentSessionInfo): Boolean {
        if (session.targetPackage.isNullOrBlank()) {
            return false
        }
        return !isTerminalSessionState(session.state)
    }

    private fun shouldUpdateParentSessionState(
        currentState: Int,
        proposedState: Int,
    ): Boolean {
        if (currentState == proposedState || isTerminalSessionState(currentState)) {
            return false
        }
        if (
            (currentState == AgentSessionInfo.STATE_RUNNING || currentState == AgentSessionInfo.STATE_WAITING_FOR_USER) &&
            (proposedState == AgentSessionInfo.STATE_CREATED || proposedState == AgentSessionInfo.STATE_QUEUED)
        ) {
            return false
        }
        return true
    }

    private fun isTerminalSessionState(state: Int): Boolean {
        return when (state) {
            AgentSessionInfo.STATE_COMPLETED,
            AgentSessionInfo.STATE_CANCELLED,
            AgentSessionInfo.STATE_FAILED,
            -> true
            else -> false
        }
    }

    private fun handleWaitingSession(session: AgentSessionInfo) {
        val manager = agentManager ?: return
        val events = manager.getSessionEvents(session.sessionId)
        val question = findLatestQuestion(events) ?: return
        if (!isBridgeQuestion(question)) {
            return
        }
        maybeAutoAnswerGenieQuestion(session, question)
    }

    private fun maybeAutoAnswerGenieQuestion(
        session: AgentSessionInfo,
        question: String,
    ) {
        if (!isBridgeQuestion(question)) {
            return
        }
        val questionKey = genieQuestionKey(session.sessionId, question)
        if (handledGenieQuestions.contains(questionKey) || !pendingGenieQuestions.add(questionKey)) {
            return
        }
        thread(name = "CodexAgentAutoAnswer-${session.sessionId}") {
            Log.i(TAG, "Attempting Agent bridge-answer for ${session.sessionId}")
            runCatching {
                answerBridgeQuestion(session, question)
                handledGenieQuestions.add(questionKey)
                AgentQuestionNotifier.cancel(this, session.sessionId)
                Log.i(TAG, "Answered bridge question for ${session.sessionId}")
            }.onFailure { err ->
                Log.i(TAG, "Agent bridge-answer unavailable for ${session.sessionId}: ${err.message}")
            }
            pendingGenieQuestions.remove(questionKey)
        }
    }

    private fun showOrUpdateSessionNotification(
        session: AgentSessionInfo,
        notificationToken: String,
        notificationText: String,
    ) {
        thread(name = "CodexAgentNotificationShow-${session.sessionId}") {
            if (shouldSuppressDelegatedChildNotification(session)) {
                AgentQuestionNotifier.suppress(
                    context = this,
                    sessionId = session.sessionId,
                    notificationToken = notificationToken,
                )
                runCatching {
                    sessionController.ackSessionNotification(session.sessionId, notificationToken)
                }.onFailure { err ->
                    Log.w(
                        TAG,
                        "Failed to ack suppressed child notification sessionId=${session.sessionId} token=$notificationToken",
                        err,
                    )
                }
                return@thread
            }
            val effectiveNotificationText =
                AgentPlannerQuestionRegistry.latestQuestion(session.sessionId) ?: notificationText
            val posted = runCatching {
                AgentQuestionNotifier.showOrUpdateDelegatedNotification(
                    context = this,
                    session = session,
                    notificationToken = notificationToken,
                    notificationText = effectiveNotificationText,
                )
            }.onFailure { err ->
                Log.w(
                    TAG,
                    "Failed to post delegated notification sessionId=${session.sessionId} token=$notificationToken",
                    err,
                )
            }.getOrDefault(false)
            if (!posted) {
                return@thread
            }
            runCatching {
                sessionController.ackSessionNotification(session.sessionId, notificationToken)
            }.onFailure { err ->
                Log.w(
                    TAG,
                    "Failed to ack delegated notification sessionId=${session.sessionId} token=$notificationToken",
                    err,
                )
            }
        }
    }

    private fun shouldSuppressDelegatedChildNotification(session: AgentSessionInfo): Boolean {
        val parentSessionId = session.parentSessionId?.trim()?.ifEmpty { null } ?: return false
        val manager = agentManager ?: return false
        val parentSession = runCatching {
            manager.getSessions(currentUserId()).firstOrNull { candidate ->
                candidate.sessionId == parentSessionId
            }
        }.onFailure { err ->
            Log.w(
                TAG,
                "Failed to resolve parent notification policy for child sessionId=${session.sessionId} parentSessionId=$parentSessionId",
                err,
            )
        }.getOrNull() ?: return false
        return isDirectParentSession(parentSession)
    }

    private fun cancelSessionNotification(
        sessionId: String,
        notificationToken: String,
        reason: Int,
    ) {
        thread(name = "CodexAgentNotificationCancel-$sessionId") {
            Log.i(
                TAG,
                "Cancelling delegated notification sessionId=$sessionId token=$notificationToken reason=${notificationCancelReasonToString(reason)}",
            )
            AgentQuestionNotifier.cancel(
                context = this,
                sessionId = sessionId,
                notificationToken = notificationToken,
            )
        }
    }

    private fun notificationCancelReasonToString(reason: Int): String {
        return when (reason) {
            NOTIFICATION_CANCEL_REASON_SUPPRESSED -> "SUPPRESSED"
            NOTIFICATION_CANCEL_REASON_REMOVED -> "REMOVED"
            else -> reason.toString()
        }
    }

    private fun findLatestQuestion(events: List<AgentSessionEvent>): String? {
        return events.lastOrNull { event ->
            event.type == AgentSessionEvent.TYPE_QUESTION &&
                !event.message.isNullOrBlank()
        }?.message
    }

    private fun findLastEventMessage(events: List<AgentSessionEvent>, type: Int): String? {
        return events.lastOrNull { event ->
            event.type == type && !event.message.isNullOrBlank()
        }?.message
    }

    private fun isBridgeQuestion(question: String): Boolean {
        return question.startsWith(BRIDGE_REQUEST_PREFIX)
    }

    private fun answerBridgeQuestion(
        session: AgentSessionInfo,
        question: String,
    ) {
        val request = JSONObject(question.removePrefix(BRIDGE_REQUEST_PREFIX))
        val requestId = request.optString("requestId")
        if (requestId.isNotBlank()) {
            val bridgeRequestKey = "${session.sessionId}:$requestId"
            if (!handledBridgeRequests.add(bridgeRequestKey)) {
                Log.i(
                    TAG,
                    "Skipping duplicate bridge question method=${request.optString("method")} requestId=$requestId session=${session.sessionId}",
                )
                return
            }
        }
        Log.i(
            TAG,
            "Answering bridge question method=${request.optString("method")} requestId=$requestId session=${session.sessionId}",
        )
        val response: JSONObject = runCatching {
            when (request.optString("method")) {
                BRIDGE_METHOD_GET_RUNTIME_STATUS -> {
                    val status = AgentCodexAppServerClient.readRuntimeStatus(this)
                    JSONObject()
                        .put("requestId", requestId)
                        .put("ok", true)
                        .put(
                            "runtimeStatus",
                            JSONObject()
                                .put("authenticated", status.authenticated)
                                .put("accountEmail", status.accountEmail)
                                .put("clientCount", status.clientCount)
                                .put("modelProviderId", status.modelProviderId)
                                .put("configuredModel", status.configuredModel)
                                .put("effectiveModel", status.effectiveModel)
                                .put("upstreamBaseUrl", status.upstreamBaseUrl)
                                .put("frameworkResponsesPath", status.frameworkResponsesPath),
                        )
                }
                else -> JSONObject()
                    .put("requestId", requestId)
                    .put("ok", false)
                    .put("error", "Unsupported bridge method: ${request.optString("method")}")
            }
        }.getOrElse { err ->
            JSONObject()
                .put("requestId", requestId)
                .put("ok", false)
                .put("error", err.message ?: err::class.java.simpleName)
        }
        sessionController.answerQuestion(
            session.sessionId,
            BRIDGE_RESPONSE_PREFIX + response.toString(),
            session.parentSessionId,
        )
    }

    private fun genieQuestionKey(sessionId: String, question: String): String {
        if (isBridgeQuestion(question)) {
            val requestId = runCatching {
                JSONObject(question.removePrefix(BRIDGE_REQUEST_PREFIX)).optString("requestId").trim()
            }.getOrNull()
            if (!requestId.isNullOrEmpty()) {
                return "$sessionId:bridge:$requestId"
            }
        }
        return "$sessionId:$question"
    }

    private fun isDirectParentSession(session: AgentSessionInfo): Boolean {
        return session.anchor == AgentSessionInfo.ANCHOR_AGENT &&
            session.parentSessionId == null &&
            session.targetPackage == null
    }

    private fun currentUserId(): Int {
        return Process.myUid() / 100000
    }
}
