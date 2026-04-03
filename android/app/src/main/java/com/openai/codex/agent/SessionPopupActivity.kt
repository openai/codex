package com.openai.codex.agent

import android.app.Activity
import android.app.agent.AgentManager
import android.app.agent.AgentSessionInfo
import android.content.Context
import android.content.Intent
import android.graphics.drawable.Drawable
import android.os.Bundle
import android.view.WindowManager
import android.widget.Button
import android.widget.EditText
import android.widget.ImageView
import android.widget.TextView
import android.widget.Toast
import kotlin.concurrent.thread

class SessionPopupActivity : Activity() {
    companion object {
        const val EXTRA_SESSION_ID = "sessionId"
        private const val HOME_FOLLOW_UP_SETTLE_TIMEOUT_MS = 2_000L
        private const val HOME_FOLLOW_UP_SETTLE_POLL_MS = 50L

        fun intent(
            context: Context,
            sessionId: String,
        ): Intent {
            return Intent(context, SessionPopupActivity::class.java)
                .putExtra(EXTRA_SESSION_ID, sessionId)
        }
    }

    private val sessionController by lazy { AgentSessionController(this) }
    private var requestedSessionId: String? = null
    private var fallbackLaunched = false
    private var popupRendered = false
    private var refreshInFlight = false
    private var sessionListenerRegistered = false
    @Volatile
    private var answerSubmissionInFlight = false
    @Volatile
    private var followUpSubmissionInFlight = false

    private val sessionListener = object : AgentManager.SessionListener {
        override fun onSessionChanged(session: AgentSessionInfo) {
            if (answerSubmissionInFlight && session.sessionId == requestedSessionId) {
                return
            }
            if (followUpSubmissionInFlight && session.sessionId == requestedSessionId) {
                return
            }
            if (session.sessionId == requestedSessionId || session.parentSessionId == requestedSessionId) {
                refreshPopup(force = true)
            }
        }

        override fun onSessionRemoved(sessionId: String, userId: Int) {
            if (answerSubmissionInFlight && sessionId == requestedSessionId) {
                return
            }
            if (followUpSubmissionInFlight && sessionId == requestedSessionId) {
                return
            }
            if (sessionId == requestedSessionId) {
                finish()
            } else {
                refreshPopup(force = true)
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        requestedSessionId = intent.getStringExtra(EXTRA_SESSION_ID)?.trim()?.ifEmpty { null }
        if (requestedSessionId == null) {
            finish()
            return
        }
        setFinishOnTouchOutside(false)
    }

    override fun onResume() {
        super.onResume()
        registerSessionListenerIfNeeded()
        refreshPopup(force = true)
    }

    override fun onPause() {
        unregisterSessionListenerIfNeeded()
        super.onPause()
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        requestedSessionId = intent.getStringExtra(EXTRA_SESSION_ID)?.trim()?.ifEmpty { null }
        fallbackLaunched = false
        popupRendered = false
        refreshPopup(force = true)
    }

    private fun registerSessionListenerIfNeeded() {
        if (sessionListenerRegistered || !sessionController.isAvailable()) {
            return
        }
        sessionListenerRegistered = runCatching {
            sessionController.registerSessionListener(mainExecutor, sessionListener)
        }.getOrDefault(false)
    }

    private fun unregisterSessionListenerIfNeeded() {
        if (!sessionListenerRegistered) {
            return
        }
        runCatching { sessionController.unregisterSessionListener(sessionListener) }
        sessionListenerRegistered = false
    }

    private fun refreshPopup(force: Boolean = false) {
        if (!force && refreshInFlight) {
            return
        }
        val sessionId = requestedSessionId ?: return
        refreshInFlight = true
        thread(name = "CodexSessionPopupLoad-$sessionId") {
            try {
                val session = runCatching {
                    resolvePopupSession(sessionController.loadSnapshot(sessionId), sessionId)
                }.getOrNull()
                runOnUiThread {
                    renderSession(session)
                }
            } finally {
                refreshInFlight = false
            }
        }
    }

    private fun resolvePopupSession(
        snapshot: AgentSnapshot,
        sessionId: String,
    ): AgentSessionDetails? {
        return snapshot.sessions.firstOrNull { session -> session.sessionId == sessionId }
            ?: snapshot.selectedSession?.takeIf { session -> session.sessionId == sessionId }
            ?: snapshot.parentSession?.takeIf { session -> session.sessionId == sessionId }
    }

    private fun renderSession(session: AgentSessionDetails?) {
        if (session == null) {
            finish()
            return
        }
        when {
            isQuestionSession(session) -> showQuestionPopup(session)
            isResultSession(session) -> showResultPopup(session)
            isRunningHomeSession(session) -> openRunningHomeTarget(session)
            popupRendered || fallbackLaunched -> finish()
            else -> launchFallbackDetail(session.sessionId)
        }
    }

    private fun isQuestionSession(session: AgentSessionDetails): Boolean {
        return session.state == AgentSessionInfo.STATE_WAITING_FOR_USER &&
            !session.latestQuestion.isNullOrBlank()
    }

    private fun isResultSession(session: AgentSessionDetails): Boolean {
        return when (session.state) {
            AgentSessionInfo.STATE_COMPLETED,
            AgentSessionInfo.STATE_CANCELLED,
            AgentSessionInfo.STATE_FAILED,
            -> true
            else -> false
        }
    }

    private fun isRunningHomeSession(session: AgentSessionDetails): Boolean {
        return session.anchor == AgentSessionInfo.ANCHOR_HOME &&
            session.parentSessionId == null &&
            session.state == AgentSessionInfo.STATE_RUNNING
    }

    private fun showQuestionPopup(session: AgentSessionDetails) {
        popupRendered = true
        setContentView(R.layout.activity_session_popup)
        bindPopupHeader(
            session = session,
            title = "Codex needs input for ${targetDisplayName(session)}",
            body = session.latestQuestion.orEmpty(),
        )
        val answerInput = findViewById<EditText>(R.id.session_popup_prompt_input)
        answerInput.hint = "Answer"
        val cancelButton = findViewById<Button>(R.id.session_popup_secondary_button)
        val answerButton = findViewById<Button>(R.id.session_popup_primary_button)
        cancelButton.text = "Cancel"
        answerButton.text = "Answer"
        cancelButton.setOnClickListener {
            finish()
        }
        answerButton.setOnClickListener {
            submitAnswer(
                session = session,
                answerInput = answerInput,
                submitButton = answerButton,
                cancelButton = cancelButton,
            )
        }
        answerInput.requestFocus()
        window?.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_STATE_VISIBLE)
    }

    private fun showResultPopup(session: AgentSessionDetails) {
        popupRendered = true
        setContentView(R.layout.activity_session_popup)
        bindPopupHeader(
            session = session,
            title = resultTitle(session),
            body = resultBody(session),
        )
        val followUpInput = findViewById<EditText>(R.id.session_popup_prompt_input)
        followUpInput.hint = "Follow-up prompt"
        val okButton = findViewById<Button>(R.id.session_popup_secondary_button)
        val sendButton = findViewById<Button>(R.id.session_popup_primary_button)
        okButton.text = "Done"
        sendButton.text = "Send"
        okButton.setOnClickListener {
            dismissResultPopup(session, okButton, sendButton)
        }
        sendButton.setOnClickListener {
            submitFollowUpPrompt(
                session = session,
                promptInput = followUpInput,
                sendButton = sendButton,
                okButton = okButton,
            )
        }
    }

    private fun resultTitle(session: AgentSessionDetails): String {
        val targetDisplayName = targetDisplayName(session)
        return when (session.state) {
            AgentSessionInfo.STATE_COMPLETED -> "Codex finished $targetDisplayName"
            AgentSessionInfo.STATE_CANCELLED -> "Codex cancelled $targetDisplayName"
            AgentSessionInfo.STATE_FAILED -> "Codex hit an issue in $targetDisplayName"
            else -> "Codex session for $targetDisplayName"
        }
    }

    private fun resultBody(session: AgentSessionDetails): String {
        return when {
            !session.latestResult.isNullOrBlank() -> session.latestResult
            !session.latestError.isNullOrBlank() -> session.latestError
            session.state == AgentSessionInfo.STATE_CANCELLED -> "This session was cancelled."
            else -> "No final message was recorded for this session."
        }
    }

    private fun submitAnswer(
        session: AgentSessionDetails,
        answerInput: EditText,
        submitButton: Button,
        cancelButton: Button,
    ) {
        val answer = answerInput.text.toString().trim()
        if (answer.isEmpty()) {
            answerInput.error = "Enter an answer"
            return
        }
        answerSubmissionInFlight = true
        submitButton.isEnabled = false
        cancelButton.isEnabled = false
        thread(name = "CodexSessionPopupAnswer-${session.sessionId}") {
            runCatching {
                sessionController.answerQuestion(
                    session.sessionId,
                    answer,
                    session.parentSessionId,
                )
                SessionNotificationCoordinator.acknowledgeSessionTree(
                    context = this,
                    sessionController = sessionController,
                    topLevelSessionId = session.parentSessionId ?: session.sessionId,
                    sessionIds = listOf(session.sessionId),
                )
            }.onFailure { err ->
                runOnUiThread {
                    answerSubmissionInFlight = false
                    submitButton.isEnabled = true
                    cancelButton.isEnabled = true
                    Toast.makeText(
                        this,
                        "Failed to answer question: ${err.message}",
                        Toast.LENGTH_SHORT,
                    ).show()
                }
            }.onSuccess {
                runOnUiThread {
                    finish()
                }
            }
        }
    }

    private fun dismissResultPopup(
        session: AgentSessionDetails,
        okButton: Button,
        sendButton: Button,
    ) {
        AgentQuestionNotifier.cancel(this, session.sessionId)
        if (isTopLevelHomeSession(session)) {
            consumeHomeResultPresentation(
                session = session,
                okButton = okButton,
                sendButton = sendButton,
            )
            return
        }
        if (isTopLevelAgentSession(session)) {
            cancelAgentSessionTree(
                sessionId = session.sessionId,
                okButton = okButton,
                sendButton = sendButton,
            )
            return
        }
        if (session.parentSessionId != null) {
            cancelAgentSession(
                sessionId = session.sessionId,
                okButton = okButton,
                sendButton = sendButton,
            )
            return
        }
        finish()
    }

    private fun submitFollowUpPrompt(
        session: AgentSessionDetails,
        promptInput: EditText,
        sendButton: Button,
        okButton: Button,
    ) {
        val prompt = promptInput.text.toString().trim()
        if (prompt.isEmpty()) {
            promptInput.error = "Enter a follow-up prompt"
            return
        }
        followUpSubmissionInFlight = true
        sendButton.isEnabled = false
        okButton.isEnabled = false
        thread(name = "CodexSessionPopupFollowUp-${session.sessionId}") {
            runCatching {
                startFollowUpPrompt(session, prompt)
                AgentQuestionNotifier.cancel(this, session.sessionId)
            }.onFailure { err ->
                runOnUiThread {
                    followUpSubmissionInFlight = false
                    sendButton.isEnabled = true
                    okButton.isEnabled = true
                    Toast.makeText(
                        this,
                        "Failed to send follow-up: ${err.message}",
                        Toast.LENGTH_SHORT,
                    ).show()
                }
            }.onSuccess {
                runOnUiThread {
                    followUpSubmissionInFlight = false
                    finish()
                }
            }
        }
    }

    private fun startFollowUpPrompt(
        session: AgentSessionDetails,
        prompt: String,
    ) {
        val snapshot = sessionController.loadSnapshot(session.sessionId)
        val selectedSession = resolvePopupSession(snapshot, session.sessionId) ?: session
        val topLevelSession = selectedSession.parentSessionId
            ?.let { parentSessionId ->
                snapshot.sessions.firstOrNull { candidate ->
                    candidate.sessionId == parentSessionId
                }
            }
            ?: selectedSession
        if (topLevelSession.anchor == AgentSessionInfo.ANCHOR_HOME) {
            startHomeFollowUp(
                topLevelSession = topLevelSession,
                prompt = SessionContinuationPromptBuilder.build(
                    sourceTopLevelSession = topLevelSession,
                    selectedSession = selectedSession,
                    prompt = prompt,
                ),
            )
            return
        }
        val childSession = if (selectedSession.parentSessionId == topLevelSession.sessionId) {
            selectedSession
        } else {
            snapshot.sessions.lastOrNull { candidate ->
                candidate.parentSessionId == topLevelSession.sessionId
            } ?: selectedSession
        }
        AgentSessionLauncher.continueSessionInPlace(
            sourceTopLevelSession = topLevelSession,
            selectedSession = childSession,
            prompt = SessionContinuationPromptBuilder.build(
                sourceTopLevelSession = topLevelSession,
                selectedSession = childSession,
                prompt = prompt,
            ),
            sessionController = sessionController,
        )
    }

    private fun startHomeFollowUp(
        topLevelSession: AgentSessionDetails,
        prompt: String,
    ) {
        val targetPackage = checkNotNull(topLevelSession.targetPackage) {
            "No target package available for follow-up"
        }
        val executionSettings = sessionController.executionSettingsForSession(topLevelSession.sessionId)
        consumePreviousHomeSessionPresentation(topLevelSession)
        val newSessionId = AgentSessionLauncher.startSession(
            context = this,
            request = LaunchSessionRequest(
                prompt = prompt,
                targetPackage = targetPackage,
                model = executionSettings.model,
                reasoningEffort = executionSettings.reasoningEffort,
            ),
            sessionController = sessionController,
        ).parentSessionId
        val deadline = System.currentTimeMillis() + HOME_FOLLOW_UP_SETTLE_TIMEOUT_MS
        while (System.currentTimeMillis() < deadline) {
            val followUpSession = runCatching {
                resolvePopupSession(sessionController.loadSnapshot(newSessionId), newSessionId)
            }.getOrNull()
            if (followUpSession != null) {
                if (
                    followUpSession.targetDetached ||
                    followUpSession.targetPresentation != AgentSessionInfo.TARGET_PRESENTATION_ATTACHED
                ) {
                    return
                }
            }
            Thread.sleep(HOME_FOLLOW_UP_SETTLE_POLL_MS)
        }
    }

    private fun consumePreviousHomeSessionPresentation(
        topLevelSession: AgentSessionDetails,
    ) {
        runCatching {
            sessionController.consumeHomeSessionPresentation(topLevelSession.sessionId)
        }.onFailure { err ->
            if (!isUnknownSessionError(err)) {
                throw err
            }
        }
        if (!topLevelSession.targetDetached) {
            return
        }
        runCatching {
            sessionController.closeDetachedTarget(topLevelSession.sessionId)
        }.onFailure { err ->
            if (!isUnknownSessionError(err)) {
                throw err
            }
        }
    }

    private fun consumeHomeResultPresentation(
        session: AgentSessionDetails,
        okButton: Button,
        sendButton: Button,
    ) {
        okButton.isEnabled = false
        sendButton.isEnabled = false
        thread(name = "CodexSessionPopupConsume-${session.sessionId}") {
            runCatching {
                sessionController.consumeHomeSessionPresentation(session.sessionId)
                if (session.targetDetached) {
                    sessionController.closeDetachedTarget(session.sessionId)
                }
            }.onFailure { err ->
                runOnUiThread {
                    okButton.isEnabled = true
                    sendButton.isEnabled = true
                    Toast.makeText(
                        this,
                        "Failed to clear result badge: ${err.message}",
                        Toast.LENGTH_SHORT,
                    ).show()
                }
            }.onSuccess {
                runOnUiThread {
                    finish()
                }
            }
        }
    }

    private fun cancelAgentSessionTree(
        sessionId: String,
        okButton: Button,
        sendButton: Button,
    ) {
        okButton.isEnabled = false
        sendButton.isEnabled = false
        thread(name = "CodexSessionPopupCancelTree-$sessionId") {
            runCatching {
                sessionController.cancelSessionTree(sessionId)
            }.onFailure { err ->
                runOnUiThread {
                    okButton.isEnabled = true
                    sendButton.isEnabled = true
                    Toast.makeText(
                        this,
                        "Failed to clear session state: ${err.message}",
                        Toast.LENGTH_SHORT,
                    ).show()
                }
            }.onSuccess {
                runOnUiThread {
                    finish()
                }
            }
        }
    }

    private fun cancelAgentSession(
        sessionId: String,
        okButton: Button,
        sendButton: Button,
    ) {
        okButton.isEnabled = false
        sendButton.isEnabled = false
        thread(name = "CodexSessionPopupCancel-$sessionId") {
            runCatching {
                sessionController.cancelSession(sessionId)
            }.onFailure { err ->
                runOnUiThread {
                    okButton.isEnabled = true
                    sendButton.isEnabled = true
                    Toast.makeText(
                        this,
                        "Failed to clear session state: ${err.message}",
                        Toast.LENGTH_SHORT,
                    ).show()
                }
            }.onSuccess {
                runOnUiThread {
                    finish()
                }
            }
        }
    }

    private fun isTopLevelAgentSession(session: AgentSessionDetails): Boolean {
        return session.anchor == AgentSessionInfo.ANCHOR_AGENT &&
            session.parentSessionId == null
    }

    private fun isTopLevelHomeSession(session: AgentSessionDetails): Boolean {
        return session.anchor == AgentSessionInfo.ANCHOR_HOME &&
            session.parentSessionId == null
    }

    private fun launchFallbackDetail(sessionId: String) {
        fallbackLaunched = true
        startActivity(
            Intent(this, SessionDetailActivity::class.java)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                .addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP)
                .putExtra(SessionDetailActivity.EXTRA_SESSION_ID, sessionId),
        )
        finish()
    }

    private fun openRunningHomeTarget(session: AgentSessionDetails) {
        fallbackLaunched = true
        thread(name = "CodexSessionPopupAttachTarget-${session.sessionId}") {
            runCatching {
                if (session.targetDetached) {
                    sessionController.showDetachedTarget(session.sessionId)
                } else {
                    sessionController.attachTarget(session.sessionId)
                }
            }.onFailure {
                runOnUiThread {
                    launchFallbackDetail(session.sessionId)
                }
            }.onSuccess {
                runOnUiThread {
                    finish()
                }
            }
        }
    }

    private fun bindPopupHeader(
        session: AgentSessionDetails,
        title: String,
        body: String,
    ) {
        findViewById<ImageView>(R.id.session_popup_icon)
            .setImageDrawable(targetIcon(session))
        findViewById<TextView>(R.id.session_popup_title).text = title
        findViewById<TextView>(R.id.session_popup_body_text).text = body
    }

    private fun targetIcon(session: AgentSessionDetails): Drawable? {
        val targetPackage = session.targetPackage?.trim()?.ifEmpty { null }
            ?: return getDrawable(android.R.drawable.ic_dialog_info)
        return runCatching {
            InstalledAppCatalog.resolveInstalledApp(this, sessionController, targetPackage).icon
        }.getOrNull() ?: getDrawable(android.R.drawable.ic_dialog_info)
    }

    private fun targetDisplayName(session: AgentSessionDetails): String {
        val targetPackage = session.targetPackage?.trim()?.ifEmpty { null }
            ?: return "Codex Agent"
        return runCatching {
            InstalledAppCatalog.resolveInstalledApp(this, sessionController, targetPackage).label
        }.getOrDefault(targetPackage)
    }

    private fun isUnknownSessionError(err: Throwable): Boolean {
        return err is IllegalArgumentException &&
            err.message?.contains("Unknown session", ignoreCase = true) == true
    }
}
