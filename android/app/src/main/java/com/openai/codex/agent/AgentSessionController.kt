package com.openai.codex.agent

import android.app.agent.AgentManager
import android.app.agent.AgentSessionEvent
import android.app.agent.AgentSessionInfo
import android.content.Context
import android.os.Binder
import android.os.Process
import android.util.Log
import com.openai.codex.bridge.CodexHomeRetention
import com.openai.codex.bridge.DesktopSessionBootstrap
import com.openai.codex.bridge.DetachedTargetCompat
import com.openai.codex.bridge.FrameworkSessionTransportCompat
import com.openai.codex.bridge.SessionExecutionSettings
import java.io.File
import java.util.concurrent.Executor
import kotlin.concurrent.thread

class AgentSessionController(context: Context) {
    companion object {
        private const val TAG = "AgentSessionController"
        private const val BRIDGE_REQUEST_PREFIX = "__codex_bridge__ "
        private const val BRIDGE_RESPONSE_PREFIX = "__codex_bridge_result__ "
        private const val DIAGNOSTIC_NOT_LOADED = "Diagnostics not loaded."
        private const val MAX_TIMELINE_EVENTS = 12
        private const val PREFERRED_GENIE_PACKAGE = "com.openai.codex.genie"
        private const val QUESTION_ANSWER_RETRY_COUNT = 10
        private const val QUESTION_ANSWER_RETRY_DELAY_MS = 50L
        private const val MAX_RETAINED_TERMINAL_SESSION_TREES = 10
    }

    private enum class ChildSessionLaunchMode {
        IMMEDIATE,
        IDLE_ATTACH,
    }

    private val appContext = context.applicationContext
    private val agentManager = appContext.getSystemService(AgentManager::class.java)
    private val presentationPolicyStore = SessionPresentationPolicyStore(context)
    private val executionSettingsStore = SessionExecutionSettingsStore(context)

    fun isAvailable(): Boolean = agentManager != null

    fun canStartSessionForTarget(packageName: String): Boolean {
        val manager = agentManager ?: return false
        return manager.canStartSessionForTarget(packageName, currentUserId())
    }

    fun registerSessionListener(
        executor: Executor,
        listener: AgentManager.SessionListener,
    ): Boolean {
        val manager = agentManager ?: return false
        manager.registerSessionListener(currentUserId(), executor, listener)
        return true
    }

    fun unregisterSessionListener(listener: AgentManager.SessionListener) {
        agentManager?.unregisterSessionListener(listener)
    }

    fun registerSessionUiLease(parentSessionId: String, token: Binder) {
        agentManager?.registerSessionUiLease(parentSessionId, token)
    }

    fun unregisterSessionUiLease(parentSessionId: String, token: Binder) {
        agentManager?.unregisterSessionUiLease(parentSessionId, token)
    }

    fun acknowledgeSessionUi(parentSessionId: String) {
        val manager = agentManager ?: return
        val token = Binder()
        runCatching {
            manager.registerSessionUiLease(parentSessionId, token)
        }
        runCatching {
            manager.unregisterSessionUiLease(parentSessionId, token)
        }
    }

    fun loadSnapshot(focusedSessionId: String?): AgentSnapshot {
        val manager = agentManager ?: return AgentSnapshot.unavailable
        val roleHolders = manager.getGenieRoleHolders(currentUserId())
        val selectedGeniePackage = selectGeniePackage(roleHolders)
        val sessions = manager.getSessions(currentUserId())
        presentationPolicyStore.prunePolicies(sessions.map { it.sessionId }.toSet())
        executionSettingsStore.pruneSettings(sessions.map { it.sessionId }.toSet())
        var sessionDetails = sessions.map { session ->
            val targetRuntime = DetachedTargetCompat.getTargetRuntime(session)
            AgentSessionDetails(
                sessionId = session.sessionId,
                parentSessionId = session.parentSessionId,
                targetPackage = session.targetPackage,
                anchor = session.anchor,
                state = session.state,
                stateLabel = stateToString(session.state),
                targetPresentation = session.targetPresentation,
                targetPresentationLabel = targetPresentationToString(session.targetPresentation),
                targetRuntime = targetRuntime.value,
                targetRuntimeLabel = targetRuntime.label,
                targetDetached = session.isTargetDetached,
                continuationGeneration = session.continuationGeneration,
                requiredFinalPresentationPolicy = presentationPolicyStore.getPolicy(session.sessionId),
                latestQuestion = null,
                latestResult = null,
                latestError = null,
                latestTrace = null,
                timeline = DIAGNOSTIC_NOT_LOADED,
            )
        }
        val selectedSessionId = chooseSelectedSession(sessionDetails, focusedSessionId)?.sessionId
        val parentSessionId = selectedSessionId?.let { selectedId ->
            findParentSession(sessionDetails, sessionDetails.firstOrNull { it.sessionId == selectedId })?.sessionId
        }
        val diagnosticSessionIds = linkedSetOf<String>().apply {
            parentSessionId?.let(::add)
            selectedSessionId?.let(::add)
        }
        val diagnosticsBySessionId = diagnosticSessionIds.associateWith { sessionId ->
            loadSessionDiagnostics(manager, sessionId)
        }
        sessionDetails = sessionDetails.map { session ->
            diagnosticsBySessionId[session.sessionId]?.let(session::withDiagnostics) ?: session
        }
        sessionDetails = sessionDetails.map { session ->
            AgentPlannerQuestionRegistry.latestQuestion(session.sessionId)?.let { plannerQuestion ->
                session.copy(
                    latestQuestion = plannerQuestion,
                    latestTrace = session.latestTrace ?: "Planner waiting for user input.",
                )
            } ?: session
        }
        sessionDetails = deriveDirectParentUiState(sessionDetails)
        val selectedSession = chooseSelectedSession(sessionDetails, focusedSessionId)
        val parentSession = findParentSession(sessionDetails, selectedSession)
        val relatedSessions = if (parentSession == null) {
            selectedSession?.let(::listOf) ?: emptyList()
        } else {
            sessionDetails.filter { session ->
                session.sessionId == parentSession.sessionId ||
                    session.parentSessionId == parentSession.sessionId
            }.sortedWith(compareBy<AgentSessionDetails> { it.parentSessionId != null }.thenBy { it.sessionId })
        }
        return AgentSnapshot(
            available = true,
            roleHolders = roleHolders,
            selectedGeniePackage = selectedGeniePackage,
            sessions = sessionDetails,
            selectedSession = selectedSession,
            parentSession = parentSession,
            relatedSessions = relatedSessions,
        )
    }

    fun startDirectSession(
        plan: AgentDelegationPlan,
        allowDetachedMode: Boolean,
        executionSettings: SessionExecutionSettings = SessionExecutionSettings.default,
    ): SessionStartResult {
        val pendingSession = createPendingDirectSession(
            objective = plan.originalObjective,
            executionSettings = executionSettings,
        )
        return startDirectSessionChildren(
            parentSessionId = pendingSession.parentSessionId,
            geniePackage = pendingSession.geniePackage,
            plan = plan,
            allowDetachedMode = allowDetachedMode,
            executionSettings = executionSettings,
            cancelParentOnFailure = true,
        )
    }

    fun createPendingDirectSession(
        objective: String,
        executionSettings: SessionExecutionSettings = SessionExecutionSettings.default,
    ): PendingDirectSessionStart {
        val parentSessionId = createDirectSessionDraft(executionSettings)
        try {
            return prepareDirectSessionDraftForStart(
                sessionId = parentSessionId,
                objective = objective,
                executionSettings = executionSettings,
            )
        } catch (err: RuntimeException) {
            runCatching { cancelSession(parentSessionId) }
            executionSettingsStore.removeSettings(parentSessionId)
            throw err
        }
    }

    fun createDirectSessionDraft(executionSettings: SessionExecutionSettings = SessionExecutionSettings.default): String {
        val manager = requireAgentManager()
        val parentSession = manager.createDirectSession(currentUserId())
        try {
            executionSettingsStore.saveSettings(parentSession.sessionId, executionSettings)
            return parentSession.sessionId
        } catch (err: RuntimeException) {
            runCatching { manager.cancelSession(parentSession.sessionId) }
            executionSettingsStore.removeSettings(parentSession.sessionId)
            throw err
        }
    }

    fun prepareDirectSessionDraftForStart(
        sessionId: String,
        objective: String,
        executionSettings: SessionExecutionSettings = SessionExecutionSettings.default,
    ): PendingDirectSessionStart {
        val manager = requireAgentManager()
        val session = manager.getSessions(currentUserId()).firstOrNull { it.sessionId == sessionId }
            ?: throw IllegalArgumentException("Unknown direct draft session: $sessionId")
        check(isDirectParentSession(session)) {
            "Session $sessionId is not an AGENT parent draft"
        }
        check(session.state == AgentSessionInfo.STATE_CREATED) {
            "Session $sessionId is not in CREATED state"
        }
        check(
            manager.getSessions(currentUserId()).none { childSession ->
                childSession.parentSessionId == sessionId
            },
        ) {
            "Session $sessionId already has child sessions"
        }
        val geniePackage = selectGeniePackage(manager.getGenieRoleHolders(currentUserId()))
            ?: throw IllegalStateException("No GENIE role holder configured")
        executionSettingsStore.saveSettings(sessionId, executionSettings)
        manager.publishTrace(
            sessionId,
            "Planning Codex direct session for objective: $objective",
        )
        manager.updateSessionState(sessionId, AgentSessionInfo.STATE_RUNNING)
        return PendingDirectSessionStart(
            parentSessionId = sessionId,
            geniePackage = geniePackage,
        )
    }

    private fun prepareDirectSessionForContinuation(
        sessionId: String,
        objective: String,
        executionSettings: SessionExecutionSettings = SessionExecutionSettings.default,
    ): PendingDirectSessionStart {
        val manager = requireAgentManager()
        val session = manager.getSessions(currentUserId()).firstOrNull { it.sessionId == sessionId }
            ?: throw IllegalArgumentException("Unknown direct session: $sessionId")
        check(isDirectParentSession(session)) {
            "Session $sessionId is not an AGENT parent session"
        }
        val geniePackage = selectGeniePackage(manager.getGenieRoleHolders(currentUserId()))
            ?: throw IllegalStateException("No GENIE role holder configured")
        executionSettingsStore.saveSettings(sessionId, executionSettings)
        if (isTerminalState(session.state)) {
            manager.resumeDirectSession(sessionId)
        } else {
            manager.updateSessionState(sessionId, AgentSessionInfo.STATE_RUNNING)
        }
        manager.publishTrace(
            sessionId,
            "Planning Codex direct follow-up for objective: $objective",
        )
        return PendingDirectSessionStart(
            parentSessionId = sessionId,
            geniePackage = geniePackage,
        )
    }

    fun createHomeSessionDraft(
        targetPackage: String,
        finalPresentationPolicy: SessionFinalPresentationPolicy,
        executionSettings: SessionExecutionSettings = SessionExecutionSettings.default,
    ): String {
        val manager = requireAgentManager()
        check(canStartSessionForTarget(targetPackage)) {
            "Target package $targetPackage is not eligible for session start"
        }
        val session = manager.createAppScopedSession(targetPackage, currentUserId())
        try {
            presentationPolicyStore.savePolicy(session.sessionId, finalPresentationPolicy)
            executionSettingsStore.saveSettings(session.sessionId, executionSettings)
            return session.sessionId
        } catch (err: RuntimeException) {
            presentationPolicyStore.removePolicy(session.sessionId)
            executionSettingsStore.removeSettings(session.sessionId)
            runCatching { manager.cancelSession(session.sessionId) }
            throw err
        }
    }

    fun startDirectSessionChildren(
        parentSessionId: String,
        geniePackage: String,
        plan: AgentDelegationPlan,
        allowDetachedMode: Boolean,
        executionSettings: SessionExecutionSettings = SessionExecutionSettings.default,
        cancelParentOnFailure: Boolean = false,
    ): SessionStartResult {
        val manager = requireAgentManager()
        requireActiveDirectParentSession(manager, parentSessionId)
        val childLaunchMode = childSessionLaunchMode(parentSessionId)
        val detachedPolicyTargets = plan.targets.filter { it.finalPresentationPolicy.requiresDetachedMode() }
        check(allowDetachedMode || detachedPolicyTargets.isEmpty()) {
            "Detached final presentation requires detached mode for ${detachedPolicyTargets.joinToString(", ") { it.packageName }}"
        }
        val childSessionIds = mutableListOf<String>()
        try {
            manager.publishTrace(
                parentSessionId,
                "Starting Codex direct session for objective: ${plan.originalObjective}",
            )
            plan.rationale?.let { rationale ->
                manager.publishTrace(parentSessionId, "Planning rationale: $rationale")
            }
            plan.targets.forEach { target ->
                requireActiveDirectParentSession(manager, parentSessionId)
                val childSession = manager.createChildSession(parentSessionId, target.packageName)
                childSessionIds += childSession.sessionId
                AgentSessionBridgeServer.ensureStarted(appContext, manager, childSession.sessionId)
                presentationPolicyStore.savePolicy(childSession.sessionId, target.finalPresentationPolicy)
                executionSettingsStore.saveSettings(childSession.sessionId, executionSettings)
                provisionSessionNetworkConfig(childSession.sessionId)
                if (childLaunchMode == ChildSessionLaunchMode.IDLE_ATTACH) {
                    DesktopInspectionRegistry.holdChildrenForAttachedPlanner(parentSessionId, listOf(childSession.sessionId))
                    manager.publishTrace(
                        childSession.sessionId,
                        "Planner launched this Genie in idle desktop-attach mode. The delegated objective is staged, but the first turn will wait until the attached client sends a prompt or the planner detaches.",
                    )
                }
                manager.publishTrace(
                    parentSessionId,
                    "Created child session ${childSession.sessionId} for ${target.packageName} with required final presentation ${target.finalPresentationPolicy.wireValue}.",
                )
                requireActiveDirectParentSession(manager, parentSessionId)
                manager.startGenieSession(
                    childSession.sessionId,
                    geniePackage,
                    childSessionStartupPrompt(target, childLaunchMode),
                    allowDetachedMode,
                )
            }
            return SessionStartResult(
                parentSessionId = parentSessionId,
                childSessionIds = childSessionIds,
                plannedTargets = plan.targets.map(AgentDelegationTarget::packageName),
                geniePackage = geniePackage,
                anchor = AgentSessionInfo.ANCHOR_AGENT,
            )
        } catch (err: RuntimeException) {
            childSessionIds.forEach { childSessionId ->
                runCatching { manager.cancelSession(childSessionId) }
                presentationPolicyStore.removePolicy(childSessionId)
                executionSettingsStore.removeSettings(childSessionId)
            }
            if (cancelParentOnFailure) {
                runCatching { manager.cancelSession(parentSessionId) }
                executionSettingsStore.removeSettings(parentSessionId)
            }
            throw err
        }
    }

    fun startHomeSession(
        targetPackage: String,
        prompt: String,
        allowDetachedMode: Boolean,
        finalPresentationPolicy: SessionFinalPresentationPolicy,
        executionSettings: SessionExecutionSettings = SessionExecutionSettings.default,
    ): SessionStartResult {
        val manager = requireAgentManager()
        check(canStartSessionForTarget(targetPackage)) {
            "Target package $targetPackage is not eligible for session start"
        }
        val geniePackage = selectGeniePackage(manager.getGenieRoleHolders(currentUserId()))
            ?: throw IllegalStateException("No GENIE role holder configured")
        val session = manager.createAppScopedSession(targetPackage, currentUserId())
        presentationPolicyStore.savePolicy(session.sessionId, finalPresentationPolicy)
        executionSettingsStore.saveSettings(session.sessionId, executionSettings)
        try {
            provisionSessionNetworkConfig(session.sessionId)
            manager.publishTrace(
                session.sessionId,
                "Starting Codex app-scoped session for $targetPackage with required final presentation ${finalPresentationPolicy.wireValue}.",
            )
            manager.startGenieSession(
                session.sessionId,
                geniePackage,
                buildDelegatedPrompt(
                    AgentDelegationTarget(
                        packageName = targetPackage,
                        objective = prompt,
                        finalPresentationPolicy = finalPresentationPolicy,
                    ),
                ),
                allowDetachedMode,
            )
            return SessionStartResult(
                parentSessionId = session.sessionId,
                childSessionIds = listOf(session.sessionId),
                plannedTargets = listOf(targetPackage),
                geniePackage = geniePackage,
                anchor = AgentSessionInfo.ANCHOR_HOME,
            )
        } catch (err: RuntimeException) {
            presentationPolicyStore.removePolicy(session.sessionId)
            executionSettingsStore.removeSettings(session.sessionId)
            runCatching { manager.cancelSession(session.sessionId) }
            throw err
        }
    }

    fun startExistingHomeSession(
        sessionId: String,
        targetPackage: String,
        prompt: String,
        allowDetachedMode: Boolean,
        finalPresentationPolicy: SessionFinalPresentationPolicy,
        executionSettings: SessionExecutionSettings = SessionExecutionSettings.default,
    ): SessionStartResult {
        val manager = requireAgentManager()
        val session = manager.getSessions(currentUserId()).firstOrNull { it.sessionId == sessionId }
            ?: throw IllegalArgumentException("Unknown HOME draft session: $sessionId")
        check(
            session.anchor == AgentSessionInfo.ANCHOR_HOME &&
                session.parentSessionId == null &&
                session.targetPackage == targetPackage,
        ) {
            "Session $sessionId is not a HOME draft for $targetPackage"
        }
        check(session.state == AgentSessionInfo.STATE_CREATED) {
            "Session $sessionId is not in CREATED state"
        }
        check(canStartSessionForTarget(targetPackage)) {
            "Target package $targetPackage is not eligible for session start"
        }
        val geniePackage = selectGeniePackage(manager.getGenieRoleHolders(currentUserId()))
            ?: throw IllegalStateException("No GENIE role holder configured")
        presentationPolicyStore.savePolicy(sessionId, finalPresentationPolicy)
        executionSettingsStore.saveSettings(sessionId, executionSettings)
        try {
            provisionSessionNetworkConfig(sessionId)
            manager.publishTrace(
                sessionId,
                "Starting Codex app-scoped session for $targetPackage with required final presentation ${finalPresentationPolicy.wireValue}.",
            )
            manager.startGenieSession(
                sessionId,
                geniePackage,
                buildDelegatedPrompt(
                    AgentDelegationTarget(
                        packageName = targetPackage,
                        objective = prompt,
                        finalPresentationPolicy = finalPresentationPolicy,
                    ),
                ),
                allowDetachedMode,
            )
            return SessionStartResult(
                parentSessionId = sessionId,
                childSessionIds = listOf(sessionId),
                plannedTargets = listOf(targetPackage),
                geniePackage = geniePackage,
                anchor = AgentSessionInfo.ANCHOR_HOME,
            )
        } catch (err: RuntimeException) {
            presentationPolicyStore.removePolicy(sessionId)
            executionSettingsStore.removeSettings(sessionId)
            throw err
        }
    }

    fun startExistingHomeSessionIdle(
        sessionId: String,
        targetPackage: String,
        allowDetachedMode: Boolean,
        finalPresentationPolicy: SessionFinalPresentationPolicy,
        executionSettings: SessionExecutionSettings = SessionExecutionSettings.default,
    ): SessionStartResult {
        val manager = requireAgentManager()
        val session = manager.getSessions(currentUserId()).firstOrNull { it.sessionId == sessionId }
            ?: throw IllegalArgumentException("Unknown HOME draft session: $sessionId")
        check(
            session.anchor == AgentSessionInfo.ANCHOR_HOME &&
                session.parentSessionId == null &&
                session.targetPackage == targetPackage,
        ) {
            "Session $sessionId is not a HOME draft for $targetPackage"
        }
        check(session.state == AgentSessionInfo.STATE_CREATED) {
            "Session $sessionId is not in CREATED state"
        }
        check(canStartSessionForTarget(targetPackage)) {
            "Target package $targetPackage is not eligible for session start"
        }
        val geniePackage = selectGeniePackage(manager.getGenieRoleHolders(currentUserId()))
            ?: throw IllegalStateException("No GENIE role holder configured")
        presentationPolicyStore.savePolicy(sessionId, finalPresentationPolicy)
        executionSettingsStore.saveSettings(sessionId, executionSettings)
        try {
            provisionSessionNetworkConfig(sessionId)
            manager.publishTrace(
                sessionId,
                "Starting idle Codex app-scoped desktop attach session for $targetPackage.",
            )
            manager.startGenieSession(
                sessionId,
                geniePackage,
                DesktopSessionBootstrap.idleAttachPrompt(),
                allowDetachedMode,
            )
            return SessionStartResult(
                parentSessionId = sessionId,
                childSessionIds = listOf(sessionId),
                plannedTargets = listOf(targetPackage),
                geniePackage = geniePackage,
                anchor = AgentSessionInfo.ANCHOR_HOME,
            )
        } catch (err: RuntimeException) {
            presentationPolicyStore.removePolicy(sessionId)
            executionSettingsStore.removeSettings(sessionId)
            throw err
        }
    }

    fun continueDirectSessionWithPlanner(
        parentSessionId: String,
        objective: String,
        executionSettings: SessionExecutionSettings = SessionExecutionSettings.default,
    ): SessionStartResult {
        val pendingSession = prepareDirectSessionForContinuation(
            sessionId = parentSessionId,
            objective = objective,
            executionSettings = executionSettings,
        )
        thread(name = "CodexAgentPlannerContinuation-${pendingSession.parentSessionId}") {
            runCatching {
                AgentTaskPlanner.planSession(
                    context = appContext,
                    userObjective = objective,
                    executionSettings = executionSettings,
                    sessionController = this,
                    requestUserInputHandler = { questions ->
                        AgentPlannerQuestionRegistry.requestUserInput(
                            context = appContext,
                            sessionController = this,
                            sessionId = pendingSession.parentSessionId,
                            questions = questions,
                        )
                    },
                    frameworkSessionId = pendingSession.parentSessionId,
                )
            }.onFailure { err ->
                if (!isTerminalSession(pendingSession.parentSessionId)) {
                    failDirectSession(
                        pendingSession.parentSessionId,
                        "Planning follow-up failed: ${err.message ?: err::class.java.simpleName}",
                    )
                }
            }.onSuccess { plannedRequest ->
                if (!isTerminalSession(pendingSession.parentSessionId)) {
                    runCatching {
                        startDirectSessionChildren(
                            parentSessionId = pendingSession.parentSessionId,
                            geniePackage = pendingSession.geniePackage,
                            plan = plannedRequest.plan,
                            allowDetachedMode = plannedRequest.allowDetachedMode,
                            executionSettings = executionSettings,
                        )
                    }.onFailure { err ->
                        if (!isTerminalSession(pendingSession.parentSessionId)) {
                            failDirectSession(
                                pendingSession.parentSessionId,
                                "Failed to start planned follow-up child session: ${err.message ?: err::class.java.simpleName}",
                            )
                        }
                    }
                }
            }
        }
        return SessionStartResult(
            parentSessionId = pendingSession.parentSessionId,
            childSessionIds = emptyList(),
            plannedTargets = emptyList(),
            geniePackage = pendingSession.geniePackage,
            anchor = AgentSessionInfo.ANCHOR_AGENT,
        )
    }

    fun executionSettingsForSession(sessionId: String): SessionExecutionSettings {
        return executionSettingsStore.getSettings(sessionId)
    }

    fun answerQuestion(sessionId: String, answer: String, parentSessionId: String?) {
        val manager = requireAgentManager()
        if (AgentPlannerQuestionRegistry.answerQuestion(sessionId, answer)) {
            manager.publishTrace(sessionId, "Answered planner question: $answer")
            manager.updateSessionState(sessionId, AgentSessionInfo.STATE_RUNNING)
            AgentQuestionNotifier.cancel(appContext, sessionId)
            return
        }
        repeat(QUESTION_ANSWER_RETRY_COUNT) { attempt ->
            runCatching {
                manager.answerQuestion(sessionId, answer)
            }.onSuccess {
                if (parentSessionId != null) {
                    manager.publishTrace(parentSessionId, "Answered question for $sessionId: $answer")
                }
                return
            }.onFailure { err ->
                if (attempt == QUESTION_ANSWER_RETRY_COUNT - 1 || !shouldRetryAnswerQuestion(sessionId, err)) {
                    throw err
                }
                Thread.sleep(QUESTION_ANSWER_RETRY_DELAY_MS)
            }
        }
    }

    fun ackSessionNotification(
        sessionId: String,
        notificationToken: String,
    ) {
        requireAgentManager().ackSessionNotification(sessionId, notificationToken)
    }

    fun answerQuestionFromNotification(
        sessionId: String,
        notificationToken: String,
        answer: String,
        parentSessionId: String?,
    ) {
        val manager = requireAgentManager()
        if (AgentPlannerQuestionRegistry.answerQuestion(sessionId, answer)) {
            manager.publishTrace(sessionId, "Answered planner question from notification: $answer")
            manager.updateSessionState(sessionId, AgentSessionInfo.STATE_RUNNING)
            runCatching { manager.ackSessionNotification(sessionId, notificationToken) }
            return
        }
        manager.answerQuestionFromNotification(sessionId, notificationToken, answer)
        if (parentSessionId != null) {
            manager.publishTrace(parentSessionId, "Answered question for $sessionId: $answer")
        }
    }

    fun isSessionWaitingForUser(sessionId: String): Boolean {
        val manager = agentManager ?: return false
        return manager.getSessions(currentUserId()).any { session ->
            session.sessionId == sessionId &&
                session.state == AgentSessionInfo.STATE_WAITING_FOR_USER
        }
    }

    fun attachTarget(sessionId: String) {
        requireAgentManager().attachTarget(sessionId)
    }

    fun showDetachedTarget(sessionId: String) {
        val result = requireAgentManager().showDetachedTarget(sessionId)
        check(result.isSuccess()) {
            buildString {
                append("Failed to show detached target for ")
                append(sessionId)
                append(": status=")
                append(result.status)
                result.message?.takeIf(String::isNotBlank)?.let { message ->
                    append(" ")
                    append(message)
                }
            }
        }
    }

    fun cancelSessionTree(sessionId: String) {
        val manager = requireAgentManager()
        val sessions = manager.getSessions(currentUserId())
        val session = sessions.firstOrNull { it.sessionId == sessionId }
            ?: throw IllegalArgumentException("Unknown session: $sessionId")
        if (isDirectParentSession(session)) {
            sessions.asSequence()
                .filter { childSession ->
                    childSession.parentSessionId == sessionId
                }
                .forEach { childSession ->
                    manager.cancelSession(childSession.sessionId)
                }
        }
        manager.cancelSession(session.sessionId)
    }

    fun cancelSession(sessionId: String) {
        requireAgentManager().cancelSession(sessionId)
    }

    fun consumeHomeSessionPresentation(sessionId: String) {
        requireAgentManager().consumeHomeSessionPresentation(sessionId)
    }

    fun closeDetachedTarget(sessionId: String) {
        requireAgentManager().closeDetachedTarget(sessionId)
    }

    fun failDirectSession(
        sessionId: String,
        message: String,
    ) {
        val manager = requireAgentManager()
        manager.publishError(sessionId, message)
        manager.updateSessionState(sessionId, AgentSessionInfo.STATE_FAILED)
    }

    fun isTerminalSession(sessionId: String): Boolean {
        val manager = agentManager ?: return true
        val session = manager.getSessions(currentUserId()).firstOrNull { it.sessionId == sessionId } ?: return true
        return isTerminalState(session.state)
    }

    fun cancelActiveSessions(): CancelActiveSessionsResult {
        val manager = requireAgentManager()
        val activeSessions = manager.getSessions(currentUserId())
            .filterNot { isTerminalState(it.state) }
            .sortedWith(
                compareByDescending<AgentSessionInfo> { it.parentSessionId != null }
                    .thenBy { it.sessionId },
            )
        val cancelledSessionIds = mutableListOf<String>()
        val failedSessionIds = mutableMapOf<String, String>()
        activeSessions.forEach { session ->
            runCatching {
                manager.cancelSession(session.sessionId)
            }.onSuccess {
                cancelledSessionIds += session.sessionId
            }.onFailure { err ->
                failedSessionIds[session.sessionId] = err.message ?: err::class.java.simpleName
            }
        }
        return CancelActiveSessionsResult(
            cancelledSessionIds = cancelledSessionIds,
            failedSessionIds = failedSessionIds,
        )
    }

    fun enforceRetentionPolicy() {
        val manager = agentManager ?: return
        runCatching {
            pruneTerminalSessionTrees(manager)
        }.onFailure { err ->
            Log.w(TAG, "Failed to prune terminal framework sessions", err)
        }
        val keepSessionIds: Set<String> = runCatching {
            manager.getSessions(currentUserId()).mapTo(mutableSetOf()) { it.sessionId }
        }.getOrElse { err ->
            Log.w(TAG, "Failed to load sessions for planner cache retention", err)
            emptySet()
        }
        prunePlannerCodexHomes(keepSessionIds)
    }

    private fun pruneTerminalSessionTrees(manager: AgentManager) {
        val sessions = manager.getSessions(currentUserId())
        val sessionsById = sessions.associateBy(AgentSessionInfo::getSessionId)
        val childrenByParent = sessions
            .filter { !it.parentSessionId.isNullOrBlank() }
            .groupBy { it.parentSessionId }
        val terminalRoots = sessions
            .filter { session ->
                session.parentSessionId.isNullOrBlank() ||
                    !sessionsById.containsKey(session.parentSessionId)
            }
            .filter { root ->
                terminalTreeFor(root, childrenByParent)
                    .all { isTerminalState(it.state) }
            }
            .sortedWith(
                compareByDescending<AgentSessionInfo> { it.createdElapsedRealtimeMillis }
                    .thenBy(AgentSessionInfo::getSessionId),
            )
        terminalRoots
            .drop(MAX_RETAINED_TERMINAL_SESSION_TREES)
            .forEach { session ->
                runCatching {
                    cancelSessionTree(session.sessionId)
                }.onFailure { err ->
                    Log.w(TAG, "Failed to prune terminal session tree ${session.sessionId}", err)
                }
            }
    }

    private fun terminalTreeFor(
        root: AgentSessionInfo,
        childrenByParent: Map<String?, List<AgentSessionInfo>>,
    ): List<AgentSessionInfo> {
        val tree = mutableListOf<AgentSessionInfo>()
        val stack = ArrayDeque<AgentSessionInfo>()
        stack.add(root)
        while (!stack.isEmpty()) {
            val session = stack.removeLast()
            tree += session
            childrenByParent[session.sessionId].orEmpty().forEach(stack::add)
        }
        return tree
    }

    private fun prunePlannerCodexHomes(keepSessionIds: Set<String>) {
        listOf(
            File(appContext.cacheDir, "planner-codex-home"),
            File(appContext.cacheDir, "planner-desktop-codex-home"),
        ).forEach { root ->
            runCatching {
                CodexHomeRetention.pruneSessionHomes(
                    root = root,
                    keepHomeNames = keepSessionIds,
                )
            }.onFailure { err ->
                Log.w(TAG, "Failed to prune planner codex homes under ${root.absolutePath}", err)
            }
        }
    }

    private fun requireAgentManager(): AgentManager {
        return checkNotNull(agentManager) { "AgentManager unavailable" }
    }

    private fun provisionSessionNetworkConfig(sessionId: String) {
        val manager = requireAgentManager()
        FrameworkSessionTransportCompat.setSessionNetworkConfig(
            agentManager = manager,
            sessionId = sessionId,
            config = AgentResponsesProxy.buildFrameworkSessionNetworkConfig(
                context = appContext,
                upstreamBaseUrl = "provider-default",
            ),
        )
        Log.i(TAG, "Configured framework-owned /responses transport for $sessionId")
    }

    private fun requireActiveDirectParentSession(
        manager: AgentManager,
        parentSessionId: String,
    ) {
        val parentSession = manager.getSessions(currentUserId()).firstOrNull { session ->
            session.sessionId == parentSessionId
        } ?: throw IllegalStateException("Parent session $parentSessionId is no longer available")
        check(isDirectParentSession(parentSession)) {
            "Session $parentSessionId is not an active direct parent session"
        }
        check(!isTerminalState(parentSession.state)) {
            "Parent session $parentSessionId is no longer active"
        }
    }

    private fun shouldRetryAnswerQuestion(
        sessionId: String,
        err: Throwable,
    ): Boolean {
        return err.message?.contains("not waiting for user input", ignoreCase = true) == true ||
            !isSessionWaitingForUser(sessionId)
    }

    private fun chooseSelectedSession(
        sessions: List<AgentSessionDetails>,
        focusedSessionId: String?,
    ): AgentSessionDetails? {
        val sessionsById = sessions.associateBy(AgentSessionDetails::sessionId)
        val focusedSession = focusedSessionId?.let(sessionsById::get)
        if (focusedSession != null) {
            if (focusedSession.parentSessionId != null) {
                return focusedSession
            }
            val childCandidate = sessions.firstOrNull { session ->
                session.parentSessionId == focusedSession.sessionId &&
                    session.state == AgentSessionInfo.STATE_WAITING_FOR_USER
            } ?: sessions.firstOrNull { session ->
                session.parentSessionId == focusedSession.sessionId &&
                    !isTerminalState(session.state)
            }
            val latestChild = sessions.lastOrNull { session ->
                session.parentSessionId == focusedSession.sessionId
            }
            return childCandidate ?: latestChild ?: focusedSession
        }
        return sessions.firstOrNull { session ->
            session.parentSessionId != null &&
                session.state == AgentSessionInfo.STATE_WAITING_FOR_USER
        } ?: sessions.firstOrNull { session ->
            session.parentSessionId != null && !isTerminalState(session.state)
        } ?: sessions.firstOrNull(::isDirectParentSession) ?: sessions.firstOrNull()
    }

    private fun findParentSession(
        sessions: List<AgentSessionDetails>,
        selectedSession: AgentSessionDetails?,
    ): AgentSessionDetails? {
        if (selectedSession == null) {
            return null
        }
        if (selectedSession.parentSessionId == null) {
            return if (isDirectParentSession(selectedSession)) {
                selectedSession
            } else {
                null
            }
        }
        return sessions.firstOrNull { it.sessionId == selectedSession.parentSessionId }
    }

    private fun selectGeniePackage(roleHolders: List<String>): String? {
        return when {
            roleHolders.contains(PREFERRED_GENIE_PACKAGE) -> PREFERRED_GENIE_PACKAGE
            else -> roleHolders.firstOrNull()
        }
    }

    private fun deriveDirectParentUiState(sessions: List<AgentSessionDetails>): List<AgentSessionDetails> {
        val childrenByParent = sessions
            .filter { it.parentSessionId != null }
            .groupBy { it.parentSessionId }
        return sessions.map { session ->
            if (!isDirectParentSession(session)) {
                return@map session
            }
            if (!session.latestQuestion.isNullOrBlank()) {
                return@map session
            }
            val childSessions = childrenByParent[session.sessionId].orEmpty().filter { childSession ->
                session.continuationGeneration == AgentSessionInfo.CONTINUATION_GENERATION_NONE ||
                    childSession.continuationGeneration == session.continuationGeneration
            }
            if (childSessions.isEmpty()) {
                return@map session
            }
            val rollup = AgentParentSessionAggregator.rollup(
                childSessions.map { childSession ->
                    ParentSessionChildSummary(
                        sessionId = childSession.sessionId,
                        targetPackage = childSession.targetPackage,
                        state = childSession.state,
                        targetPresentation = childSession.targetPresentation,
                        requiredFinalPresentationPolicy = childSession.requiredFinalPresentationPolicy,
                        latestResult = childSession.latestResult,
                        latestError = childSession.latestError,
                    )
                },
            )
            val isRollupTerminal = isTerminalState(rollup.state)
            session.copy(
                state = rollup.state,
                stateLabel = stateToString(rollup.state),
                latestResult = rollup.resultMessage ?: session.latestResult.takeIf { isRollupTerminal },
                latestError = rollup.errorMessage ?: session.latestError.takeIf { isRollupTerminal },
                latestTrace = when (rollup.state) {
                    AgentSessionInfo.STATE_RUNNING -> "Child session running."
                    AgentSessionInfo.STATE_WAITING_FOR_USER -> "Child session waiting for user input."
                    AgentSessionInfo.STATE_QUEUED -> "Child session queued."
                    else -> session.latestTrace
                },
            )
        }
    }

    private fun buildDelegatedPrompt(target: AgentDelegationTarget): String {
        return buildString {
            appendLine(target.objective)
            appendLine()
            appendLine("Required final target presentation: ${target.finalPresentationPolicy.wireValue}")
            append(target.finalPresentationPolicy.promptGuidance())
        }.trim()
    }

    private fun childSessionLaunchMode(parentSessionId: String): ChildSessionLaunchMode {
        return if (DesktopInspectionRegistry.isPlannerAttached(parentSessionId)) {
            ChildSessionLaunchMode.IDLE_ATTACH
        } else {
            ChildSessionLaunchMode.IMMEDIATE
        }
    }

    private fun childSessionStartupPrompt(
        target: AgentDelegationTarget,
        launchMode: ChildSessionLaunchMode,
    ): String {
        val delegatedPrompt = buildDelegatedPrompt(target)
        return when (launchMode) {
            ChildSessionLaunchMode.IMMEDIATE -> delegatedPrompt
            ChildSessionLaunchMode.IDLE_ATTACH -> DesktopSessionBootstrap.idleAttachPrompt(delegatedPrompt)
        }
    }

    private fun findLastEventMessage(events: List<AgentSessionEvent>, type: Int): String? {
        for (index in events.indices.reversed()) {
            val event = events[index]
            if (event.type == type && event.message != null) {
                return normalizeEventMessage(event.message)
            }
        }
        return null
    }

    private fun loadSessionDiagnostics(manager: AgentManager, sessionId: String): SessionDiagnostics {
        val events = manager.getSessionEvents(sessionId)
        return SessionDiagnostics(
            latestQuestion = findLastEventMessage(events, AgentSessionEvent.TYPE_QUESTION),
            latestResult = findLastEventMessage(events, AgentSessionEvent.TYPE_RESULT),
            latestError = findLastEventMessage(events, AgentSessionEvent.TYPE_ERROR),
            latestTrace = findLastEventMessage(events, AgentSessionEvent.TYPE_TRACE),
            timeline = renderTimeline(events),
        )
    }

    private fun renderTimeline(events: List<AgentSessionEvent>): String {
        if (events.isEmpty()) {
            return "No framework events yet."
        }
        return events.takeLast(MAX_TIMELINE_EVENTS).joinToString("\n") { event ->
            "${eventTypeToString(event.type)}: ${normalizeEventMessage(event.message).orEmpty()}"
        }
    }

    private fun normalizeEventMessage(message: String?): String? {
        val trimmed = message?.trim()?.takeIf(String::isNotEmpty) ?: return null
        if (trimmed.startsWith(BRIDGE_REQUEST_PREFIX)) {
            return summarizeBridgeRequest(trimmed)
        }
        if (trimmed.startsWith(BRIDGE_RESPONSE_PREFIX)) {
            return summarizeBridgeResponse(trimmed)
        }
        return trimmed
    }

    private fun summarizeBridgeRequest(message: String): String {
        val request = runCatching {
            org.json.JSONObject(message.removePrefix(BRIDGE_REQUEST_PREFIX))
        }.getOrNull()
        val method = request?.optString("method")?.ifEmpty { "unknown" } ?: "unknown"
        val requestId = request?.optString("requestId")?.takeIf(String::isNotBlank)
        return buildString {
            append("Bridge request: ")
            append(method)
            requestId?.let { append(" (#$it)") }
        }
    }

    private fun summarizeBridgeResponse(message: String): String {
        val response = runCatching {
            org.json.JSONObject(message.removePrefix(BRIDGE_RESPONSE_PREFIX))
        }.getOrNull()
        val requestId = response?.optString("requestId")?.takeIf(String::isNotBlank)
        val statusCode = response?.optJSONObject("httpResponse")?.optInt("statusCode")
        val ok = response?.optBoolean("ok")
        return buildString {
            append("Bridge response")
            requestId?.let { append(" (#$it)") }
            if (statusCode != null) {
                append(": HTTP $statusCode")
            } else if (ok != null) {
                append(": ")
                append(if (ok) "ok" else "error")
            }
        }
    }

    private fun eventTypeToString(type: Int): String {
        return when (type) {
            AgentSessionEvent.TYPE_TRACE -> "Trace"
            AgentSessionEvent.TYPE_QUESTION -> "Question"
            AgentSessionEvent.TYPE_RESULT -> "Result"
            AgentSessionEvent.TYPE_ERROR -> "Error"
            AgentSessionEvent.TYPE_POLICY -> "Policy"
            AgentSessionEvent.TYPE_DETACHED_ACTION -> "DetachedAction"
            AgentSessionEvent.TYPE_ANSWER -> "Answer"
            else -> "Event($type)"
        }
    }

    private fun isDirectParentSession(session: AgentSessionDetails): Boolean {
        return session.anchor == AgentSessionInfo.ANCHOR_AGENT &&
            session.parentSessionId == null &&
            session.targetPackage == null
    }

    private fun isDirectParentSession(session: AgentSessionInfo): Boolean {
        return session.anchor == AgentSessionInfo.ANCHOR_AGENT &&
            session.parentSessionId == null &&
            session.targetPackage == null
    }

    private fun isTerminalState(state: Int): Boolean {
        return state == AgentSessionInfo.STATE_COMPLETED ||
            state == AgentSessionInfo.STATE_CANCELLED ||
            state == AgentSessionInfo.STATE_FAILED
    }

    private fun stateToString(state: Int): String {
        return when (state) {
            AgentSessionInfo.STATE_CREATED -> "CREATED"
            AgentSessionInfo.STATE_RUNNING -> "RUNNING"
            AgentSessionInfo.STATE_WAITING_FOR_USER -> "WAITING_FOR_USER"
            AgentSessionInfo.STATE_QUEUED -> "QUEUED"
            AgentSessionInfo.STATE_COMPLETED -> "COMPLETED"
            AgentSessionInfo.STATE_CANCELLED -> "CANCELLED"
            AgentSessionInfo.STATE_FAILED -> "FAILED"
            else -> state.toString()
        }
    }

    private fun currentUserId(): Int = Process.myUid() / 100000
}

data class AgentSnapshot(
    val available: Boolean,
    val roleHolders: List<String>,
    val selectedGeniePackage: String?,
    val sessions: List<AgentSessionDetails>,
    val selectedSession: AgentSessionDetails?,
    val parentSession: AgentSessionDetails?,
    val relatedSessions: List<AgentSessionDetails>,
) {
    companion object {
        val unavailable = AgentSnapshot(
            available = false,
            roleHolders = emptyList(),
            selectedGeniePackage = null,
            sessions = emptyList(),
            selectedSession = null,
            parentSession = null,
            relatedSessions = emptyList(),
        )
    }
}

data class AgentSessionDetails(
    val sessionId: String,
    val parentSessionId: String?,
    val targetPackage: String?,
    val anchor: Int,
    val state: Int,
    val stateLabel: String,
    val targetPresentation: Int,
    val targetPresentationLabel: String,
    val targetRuntime: Int?,
    val targetRuntimeLabel: String,
    val targetDetached: Boolean,
    val continuationGeneration: Int,
    val requiredFinalPresentationPolicy: SessionFinalPresentationPolicy?,
    val latestQuestion: String?,
    val latestResult: String?,
    val latestError: String?,
    val latestTrace: String?,
    val timeline: String,
) {
    fun withDiagnostics(diagnostics: SessionDiagnostics): AgentSessionDetails {
        return copy(
            latestQuestion = diagnostics.latestQuestion,
            latestResult = diagnostics.latestResult,
            latestError = diagnostics.latestError,
            latestTrace = diagnostics.latestTrace,
            timeline = diagnostics.timeline,
        )
    }
}

data class SessionDiagnostics(
    val latestQuestion: String?,
    val latestResult: String?,
    val latestError: String?,
    val latestTrace: String?,
    val timeline: String,
)

data class SessionStartResult(
    val parentSessionId: String,
    val childSessionIds: List<String>,
    val plannedTargets: List<String>,
    val geniePackage: String,
    val anchor: Int,
)

data class PendingDirectSessionStart(
    val parentSessionId: String,
    val geniePackage: String,
)

data class CancelActiveSessionsResult(
    val cancelledSessionIds: List<String>,
    val failedSessionIds: Map<String, String>,
)
