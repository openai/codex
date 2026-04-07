package com.openai.codex.agent

import android.app.agent.AgentSessionInfo
import android.content.Context
import com.openai.codex.bridge.SessionExecutionSettings
import kotlin.concurrent.thread
import org.json.JSONArray
import org.json.JSONObject

data class CreateSessionRequest(
    val targetPackage: String?,
    val model: String?,
    val reasoningEffort: String?,
)

data class LaunchSessionRequest(
    val prompt: String,
    val targetPackage: String?,
    val model: String?,
    val reasoningEffort: String?,
    val existingSessionId: String? = null,
)

data class StartSessionRequest(
    val sessionId: String,
    val prompt: String,
)

data class SessionDraftResult(
    val sessionId: String,
    val anchor: Int,
)

object AgentSessionLauncher {
    fun createSessionDraft(
        request: CreateSessionRequest,
        sessionController: AgentSessionController,
    ): SessionDraftResult {
        val executionSettings = SessionExecutionSettings(
            model = request.model?.trim()?.ifEmpty { null },
            reasoningEffort = request.reasoningEffort?.trim()?.ifEmpty { null },
        )
        val targetPackage = request.targetPackage?.trim()?.ifEmpty { null }
        return if (targetPackage == null) {
            SessionDraftResult(
                sessionId = sessionController.createDirectSessionDraft(executionSettings),
                anchor = AgentSessionInfo.ANCHOR_AGENT,
            )
        } else {
            SessionDraftResult(
                sessionId = sessionController.createHomeSessionDraft(
                    targetPackage = targetPackage,
                    finalPresentationPolicy = SessionFinalPresentationPolicy.AGENT_CHOICE,
                    executionSettings = executionSettings,
                ),
                anchor = AgentSessionInfo.ANCHOR_HOME,
            )
        }
    }

    fun startSessionAsync(
        context: Context,
        request: LaunchSessionRequest,
        sessionController: AgentSessionController,
        requestUserInputHandler: ((JSONArray) -> JSONObject)? = null,
    ): SessionStartResult {
        val executionSettings = SessionExecutionSettings(
            model = request.model?.trim()?.ifEmpty { null },
            reasoningEffort = request.reasoningEffort?.trim()?.ifEmpty { null },
        )
        val targetPackage = request.targetPackage?.trim()?.ifEmpty { null }
        val existingSessionId = request.existingSessionId?.trim()?.ifEmpty { null }
        if (targetPackage != null || existingSessionId != null) {
            return startSession(
                context = context,
                request = request,
                sessionController = sessionController,
                requestUserInputHandler = requestUserInputHandler,
            )
        }
        val draftSession = createSessionDraft(
            request = CreateSessionRequest(
                targetPackage = null,
                model = executionSettings.model,
                reasoningEffort = executionSettings.reasoningEffort,
            ),
            sessionController = sessionController,
        )
        return startSessionDraftAsync(
            context = context,
            request = StartSessionRequest(
                sessionId = draftSession.sessionId,
                prompt = request.prompt,
            ),
            sessionController = sessionController,
            requestUserInputHandler = requestUserInputHandler,
        )
    }

    fun startSessionDraftAsync(
        context: Context,
        request: StartSessionRequest,
        sessionController: AgentSessionController,
        requestUserInputHandler: ((JSONArray) -> JSONObject)? = null,
    ): SessionStartResult {
        val sessionId = request.sessionId.trim()
        require(sessionId.isNotEmpty()) { "Missing session id" }
        val prompt = request.prompt.trim()
        require(prompt.isNotEmpty()) { "Missing prompt" }
        val snapshot = sessionController.loadSnapshot(sessionId)
        val session = snapshot.sessions.firstOrNull { it.sessionId == sessionId }
            ?: throw IllegalArgumentException("Unknown session: $sessionId")
        if (
            session.anchor == AgentSessionInfo.ANCHOR_HOME &&
            session.parentSessionId == null &&
            !session.targetPackage.isNullOrBlank()
        ) {
            return sessionController.startExistingHomeSession(
                sessionId = sessionId,
                targetPackage = checkNotNull(session.targetPackage),
                prompt = prompt,
                allowDetachedMode = true,
                finalPresentationPolicy = session.requiredFinalPresentationPolicy
                    ?: SessionFinalPresentationPolicy.AGENT_CHOICE,
                executionSettings = sessionController.executionSettingsForSession(sessionId),
            )
        }
        check(
            session.anchor == AgentSessionInfo.ANCHOR_AGENT &&
                session.parentSessionId == null &&
                session.targetPackage == null,
        ) {
            "Session $sessionId is not a startable draft"
        }
        check(AgentPlannerRuntimeManager.activeThreadId(sessionId) == null) {
            "Session $sessionId is already attached to an idle planner runtime; send the first prompt through the attached client"
        }
        val executionSettings = sessionController.executionSettingsForSession(sessionId)
        val pendingSession = sessionController.prepareDirectSessionDraftForStart(
            sessionId = sessionId,
            objective = prompt,
            executionSettings = executionSettings,
        )
        val applicationContext = context.applicationContext
        thread(name = "CodexAgentPlanner-${pendingSession.parentSessionId}") {
            runCatching {
                val effectiveRequestUserInputHandler = requestUserInputHandler ?: { questions: JSONArray ->
                    AgentPlannerQuestionRegistry.requestUserInput(
                        context = applicationContext,
                        sessionController = sessionController,
                        sessionId = pendingSession.parentSessionId,
                        questions = questions,
                    )
                }
                AgentTaskPlanner.planSession(
                    context = applicationContext,
                    userObjective = prompt,
                    executionSettings = executionSettings,
                    sessionController = sessionController,
                    requestUserInputHandler = effectiveRequestUserInputHandler,
                    frameworkSessionId = pendingSession.parentSessionId,
                )
            }.onFailure { err ->
                if (!sessionController.isTerminalSession(pendingSession.parentSessionId)) {
                    sessionController.failDirectSession(
                        pendingSession.parentSessionId,
                        "Planning failed: ${err.message ?: err::class.java.simpleName}",
                    )
                }
            }.onSuccess { plannedRequest ->
                if (!sessionController.isTerminalSession(pendingSession.parentSessionId)) {
                    runCatching {
                        sessionController.startDirectSessionChildren(
                            parentSessionId = pendingSession.parentSessionId,
                            geniePackage = pendingSession.geniePackage,
                            plan = plannedRequest.plan,
                            allowDetachedMode = plannedRequest.allowDetachedMode,
                            executionSettings = executionSettings,
                        )
                    }.onFailure { err ->
                        if (!sessionController.isTerminalSession(pendingSession.parentSessionId)) {
                            sessionController.failDirectSession(
                                pendingSession.parentSessionId,
                                "Failed to start planned child session: ${err.message ?: err::class.java.simpleName}",
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

    fun startSession(
        context: Context,
        request: LaunchSessionRequest,
        sessionController: AgentSessionController,
        requestUserInputHandler: ((JSONArray) -> JSONObject)? = null,
    ): SessionStartResult {
        val executionSettings = SessionExecutionSettings(
            model = request.model?.trim()?.ifEmpty { null },
            reasoningEffort = request.reasoningEffort?.trim()?.ifEmpty { null },
        )
        val targetPackage = request.targetPackage?.trim()?.ifEmpty { null }
        val existingSessionId = request.existingSessionId?.trim()?.ifEmpty { null }
        return if (targetPackage == null) {
            check(existingSessionId == null) {
                "Existing HOME sessions require a target package"
            }
            AgentTaskPlanner.startSession(
                context = context,
                userObjective = request.prompt,
                targetPackageOverride = null,
                allowDetachedMode = true,
                executionSettings = executionSettings,
                sessionController = sessionController,
                requestUserInputHandler = requestUserInputHandler,
            )
        } else {
            if (existingSessionId != null) {
                sessionController.startExistingHomeSession(
                    sessionId = existingSessionId,
                    targetPackage = targetPackage,
                    prompt = request.prompt,
                    allowDetachedMode = true,
                    finalPresentationPolicy = SessionFinalPresentationPolicy.AGENT_CHOICE,
                    executionSettings = executionSettings,
                )
            } else {
                sessionController.startHomeSession(
                    targetPackage = targetPackage,
                    prompt = request.prompt,
                    allowDetachedMode = true,
                    finalPresentationPolicy = SessionFinalPresentationPolicy.AGENT_CHOICE,
                    executionSettings = executionSettings,
                )
            }
        }
    }

    fun continueSessionInPlace(
        sourceTopLevelSession: AgentSessionDetails,
        selectedSession: AgentSessionDetails,
        prompt: String,
        sessionController: AgentSessionController,
    ): SessionStartResult {
        val executionSettings = sessionController.executionSettingsForSession(sourceTopLevelSession.sessionId)
        return when (sourceTopLevelSession.anchor) {
            AgentSessionInfo.ANCHOR_HOME -> {
                throw UnsupportedOperationException(
                    "In-place continuation is not supported for app-scoped HOME sessions on the current framework",
                )
            }

            else -> {
                sessionController.continueDirectSessionWithPlanner(
                    parentSessionId = sourceTopLevelSession.sessionId,
                    objective = SessionContinuationPromptBuilder.build(
                        sourceTopLevelSession = sourceTopLevelSession,
                        selectedSession = selectedSession,
                        prompt = prompt,
                    ),
                    executionSettings = executionSettings,
                )
            }
        }
    }
}
