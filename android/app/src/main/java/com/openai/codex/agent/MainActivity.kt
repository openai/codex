package com.openai.codex.agent

import android.Manifest
import android.app.Activity
import android.app.agent.AgentManager
import android.app.agent.AgentSessionInfo
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.util.Base64
import android.util.Log
import android.view.View
import android.widget.Button
import android.widget.ListView
import android.widget.TextView
import android.widget.Toast
import com.openai.codex.bridge.SessionExecutionSettings
import kotlin.concurrent.thread

class MainActivity : Activity() {
    companion object {
        private const val TAG = "CodexMainActivity"
        private const val ACTION_DEBUG_START_AGENT_SESSION =
            "com.openai.codex.agent.action.DEBUG_START_AGENT_SESSION"
        private const val ACTION_DEBUG_CANCEL_ALL_AGENT_SESSIONS =
            "com.openai.codex.agent.action.DEBUG_CANCEL_ALL_AGENT_SESSIONS"
        private const val EXTRA_DEBUG_PROMPT = "prompt"
        private const val EXTRA_DEBUG_PROMPT_BASE64 = "promptBase64"
        private const val EXTRA_DEBUG_TARGET_PACKAGE = "targetPackage"
        private const val EXTRA_DEBUG_FINAL_PRESENTATION_POLICY = "finalPresentationPolicy"
    }

    @Volatile
    private var isAuthenticated = false
    @Volatile
    private var agentRefreshInFlight = false
    @Volatile
    private var latestAgentRuntimeStatus: AgentCodexAppServerClient.RuntimeStatus? = null
    @Volatile
    private var pendingAuthMessage: String? = null

    private val agentSessionController by lazy { AgentSessionController(this) }
    private val dismissedSessionStore by lazy { DismissedSessionStore(this) }
    private val sessionListAdapter by lazy { TopLevelSessionListAdapter(this) }
    private var latestSnapshot: AgentSnapshot = AgentSnapshot.unavailable

    private val runtimeStatusListener = AgentCodexAppServerClient.RuntimeStatusListener { status ->
        latestAgentRuntimeStatus = status
        if (status != null) {
            pendingAuthMessage = null
        }
        runOnUiThread {
            updateAuthUi(renderAuthStatus(), status?.authenticated == true)
            updateRuntimeStatusUi()
        }
    }
    private val sessionListener = object : AgentManager.SessionListener {
        override fun onSessionChanged(session: AgentSessionInfo) {
            refreshAgentSessions()
        }

        override fun onSessionRemoved(sessionId: String, userId: Int) {
            refreshAgentSessions()
        }
    }

    private var sessionListenerRegistered = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)
        setupViews()
        requestNotificationPermissionIfNeeded()
        handleIncomingIntent(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        Log.i(TAG, "onNewIntent action=${intent.action}")
        setIntent(intent)
        handleIncomingIntent(intent)
    }

    override fun onResume() {
        super.onResume()
        registerSessionListenerIfNeeded()
        AgentCodexAppServerClient.registerRuntimeStatusListener(runtimeStatusListener)
        AgentCodexAppServerClient.refreshRuntimeStatusAsync(this, refreshToken = true)
        refreshAgentSessions(force = true)
    }

    override fun onPause() {
        AgentCodexAppServerClient.unregisterRuntimeStatusListener(runtimeStatusListener)
        unregisterSessionListenerIfNeeded()
        super.onPause()
    }

    private fun setupViews() {
        findViewById<ListView>(R.id.session_list).adapter = sessionListAdapter
        findViewById<ListView>(R.id.session_list).setOnItemClickListener { _, _, position, _ ->
            sessionListAdapter.getItem(position)?.let { session ->
                openSessionDetail(session.sessionId)
            }
        }
        findViewById<Button>(R.id.create_session_button).setOnClickListener {
            launchCreateSessionActivity()
        }
        findViewById<Button>(R.id.auth_action).setOnClickListener {
            authAction()
        }
        findViewById<Button>(R.id.refresh_sessions_button).setOnClickListener {
            refreshAgentSessions(force = true)
        }
        updateAuthUi("Agent auth: probing...", false)
        updateRuntimeStatusUi()
        updateSessionList(emptyList())
    }

    private fun handleIncomingIntent(intent: Intent?) {
        val sessionId = intent?.getStringExtra(AgentManager.EXTRA_SESSION_ID)
        if (!sessionId.isNullOrBlank()) {
            openSessionDetail(sessionId)
            return
        }
        maybeHandleDebugIntent(intent)
    }

    private fun maybeHandleDebugIntent(intent: Intent?) {
        when (intent?.action) {
            ACTION_DEBUG_CANCEL_ALL_AGENT_SESSIONS -> {
                thread {
                    runCatching { agentSessionController.cancelActiveSessions() }
                        .onFailure { err ->
                            Log.w(TAG, "Failed to cancel Agent sessions from debug intent", err)
                            showToast("Failed to cancel active sessions: ${err.message}")
                        }
                        .onSuccess { result ->
                            showToast(
                                "Cancelled ${result.cancelledSessionIds.size} sessions, ${result.failedSessionIds.size} failed",
                            )
                            refreshAgentSessions(force = true)
                        }
                }
                intent.action = null
            }

            ACTION_DEBUG_START_AGENT_SESSION -> {
                val prompt = extractDebugPrompt(intent)
                if (prompt.isEmpty()) {
                    intent.action = null
                    return
                }
                val targetPackage = intent.getStringExtra(EXTRA_DEBUG_TARGET_PACKAGE)?.trim()?.ifEmpty { null }
                val finalPresentationPolicy = SessionFinalPresentationPolicy.fromWireValue(
                    intent.getStringExtra(EXTRA_DEBUG_FINAL_PRESENTATION_POLICY),
                )
                startDebugSession(
                    prompt = prompt,
                    targetPackage = targetPackage,
                    finalPresentationPolicy = finalPresentationPolicy,
                )
                intent.action = null
            }
        }
    }

    private fun extractDebugPrompt(intent: Intent): String {
        intent.getStringExtra(EXTRA_DEBUG_PROMPT_BASE64)
            ?.trim()
            ?.takeIf(String::isNotEmpty)
            ?.let { encoded ->
                runCatching {
                    String(Base64.decode(encoded, Base64.DEFAULT), Charsets.UTF_8).trim()
                }.onFailure { err ->
                    Log.w(TAG, "Failed to decode debug promptBase64", err)
                }.getOrNull()
                    ?.takeIf(String::isNotEmpty)
                    ?.let { return it }
            }
        return intent.getStringExtra(EXTRA_DEBUG_PROMPT)?.trim().orEmpty()
    }

    private fun startDebugSession(
        prompt: String,
        targetPackage: String?,
        finalPresentationPolicy: SessionFinalPresentationPolicy?,
    ) {
        thread {
            val result = runCatching {
                if (targetPackage != null) {
                    agentSessionController.startHomeSession(
                        targetPackage = targetPackage,
                        prompt = prompt,
                        allowDetachedMode = true,
                        finalPresentationPolicy = finalPresentationPolicy
                            ?: SessionFinalPresentationPolicy.AGENT_CHOICE,
                        executionSettings = SessionExecutionSettings.default,
                    )
                } else {
                    AgentTaskPlanner.startSession(
                        context = this,
                        userObjective = prompt,
                        targetPackageOverride = null,
                        allowDetachedMode = true,
                        finalPresentationPolicyOverride = finalPresentationPolicy,
                        executionSettings = SessionExecutionSettings.default,
                        sessionController = agentSessionController,
                    )
                }
            }
            result.onFailure { err ->
                Log.w(TAG, "Failed to start debug Agent session", err)
                showToast("Failed to start Agent session: ${err.message}")
            }
            result.onSuccess { started ->
                showToast("Started session ${started.parentSessionId}")
                refreshAgentSessions(force = true)
            }
        }
    }

    private fun refreshAgentSessions(force: Boolean = false) {
        if (!force && agentRefreshInFlight) {
            return
        }
        agentRefreshInFlight = true
        thread {
            try {
                val result = runCatching { agentSessionController.loadSnapshot(null) }
                result.onFailure { err ->
                    latestSnapshot = AgentSnapshot.unavailable
                    runOnUiThread {
                        findViewById<TextView>(R.id.agent_status).text =
                            "Agent framework unavailable (${err.message})"
                        updateSessionList(emptyList())
                    }
                }
                result.onSuccess { snapshot ->
                    latestSnapshot = snapshot
                    dismissedSessionStore.prune(snapshot.sessions.map(AgentSessionDetails::sessionId).toSet())
                    val topLevelSessions = SessionUiFormatter.topLevelSessions(snapshot)
                        .filter { session ->
                            if (!isTerminalState(session.state)) {
                                dismissedSessionStore.clearDismissed(session.sessionId)
                                true
                            } else {
                                !dismissedSessionStore.isDismissed(session.sessionId)
                            }
                        }
                    runOnUiThread {
                        updateFrameworkStatus(snapshot)
                        updateSessionList(topLevelSessions)
                    }
                }
            } finally {
                agentRefreshInFlight = false
            }
        }
    }

    private fun updateFrameworkStatus(snapshot: AgentSnapshot) {
        val roleHolders = if (snapshot.roleHolders.isEmpty()) {
            "none"
        } else {
            snapshot.roleHolders.joinToString(", ")
        }
        findViewById<TextView>(R.id.agent_status).text =
            "Agent framework active. Genie role holders: $roleHolders"
    }

    private fun updateSessionList(sessions: List<AgentSessionDetails>) {
        sessionListAdapter.replaceItems(sessions)
        findViewById<TextView>(R.id.session_list_empty).visibility =
            if (sessions.isEmpty()) View.VISIBLE else View.GONE
    }

    private fun registerSessionListenerIfNeeded() {
        if (sessionListenerRegistered || !agentSessionController.isAvailable()) {
            return
        }
        sessionListenerRegistered = runCatching {
            agentSessionController.registerSessionListener(mainExecutor, sessionListener)
        }.getOrDefault(false)
    }

    private fun unregisterSessionListenerIfNeeded() {
        if (!sessionListenerRegistered) {
            return
        }
        runCatching { agentSessionController.unregisterSessionListener(sessionListener) }
        sessionListenerRegistered = false
    }

    private fun requestNotificationPermissionIfNeeded() {
        if (Build.VERSION.SDK_INT < 33) {
            return
        }
        if (checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS)
            == PackageManager.PERMISSION_GRANTED
        ) {
            return
        }
        requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1001)
    }

    private fun authAction() {
        if (isAuthenticated) {
            signOutAgent()
        } else {
            startAgentSignIn()
        }
    }

    private fun startAgentSignIn() {
        pendingAuthMessage = "Agent auth: opening browser for sign-in..."
        updateAuthUi(pendingAuthMessage.orEmpty(), false)
        thread {
            runCatching { AgentCodexAppServerClient.startChatGptLogin(this) }
                .onFailure { err ->
                    pendingAuthMessage = null
                    updateAuthUi("Agent auth: sign-in failed (${err.message})", false)
                }
                .onSuccess { loginSession ->
                    pendingAuthMessage = "Agent auth: complete sign-in in the browser"
                    updateAuthUi(pendingAuthMessage.orEmpty(), false)
                    runOnUiThread {
                        runCatching {
                            startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(loginSession.authUrl)))
                        }.onFailure { err ->
                            pendingAuthMessage = "Agent auth: open ${loginSession.authUrl}"
                            updateAuthUi(pendingAuthMessage.orEmpty(), false)
                            showToast("Failed to open browser: ${err.message}")
                        }.onSuccess {
                            showToast("Complete sign-in in the browser")
                        }
                    }
                }
        }
    }

    private fun signOutAgent() {
        pendingAuthMessage = "Agent auth: signing out..."
        updateAuthUi(pendingAuthMessage.orEmpty(), false)
        thread {
            runCatching { AgentCodexAppServerClient.logoutAccount(this) }
                .onFailure { err ->
                    pendingAuthMessage = null
                    updateAuthUi("Agent auth: sign out failed (${err.message})", isAuthenticated)
                }
                .onSuccess {
                    pendingAuthMessage = null
                    AgentCodexAppServerClient.refreshRuntimeStatusAsync(this)
                    showToast("Signed out")
                }
        }
    }

    private fun updateRuntimeStatusUi() {
        findViewById<TextView>(R.id.agent_runtime_status).text = renderAgentRuntimeStatus()
    }

    private fun renderAgentRuntimeStatus(): String {
        val runtimeStatus = latestAgentRuntimeStatus
        if (runtimeStatus == null) {
            return "Agent runtime: probing..."
        }
        val authSummary = if (runtimeStatus.authenticated) {
            runtimeStatus.accountEmail?.let { "signed in ($it)" } ?: "signed in"
        } else {
            "not signed in"
        }
        val configuredModelSuffix = runtimeStatus.configuredModel
            ?.takeIf { it != runtimeStatus.effectiveModel }
            ?.let { ", configured=$it" }
            ?: ""
        val effectiveModel = runtimeStatus.effectiveModel ?: "unknown"
        return "Agent runtime: $authSummary, provider=${runtimeStatus.modelProviderId}, effective=$effectiveModel$configuredModelSuffix, clients=${runtimeStatus.clientCount}, base=${runtimeStatus.upstreamBaseUrl}"
    }

    private fun renderAuthStatus(): String {
        pendingAuthMessage?.let { return it }
        val runtimeStatus = latestAgentRuntimeStatus
        if (runtimeStatus == null) {
            return "Agent auth: probing..."
        }
        if (!runtimeStatus.authenticated) {
            return "Agent auth: not signed in"
        }
        return runtimeStatus.accountEmail?.let { email ->
            "Agent auth: signed in ($email)"
        } ?: "Agent auth: signed in"
    }

    private fun updateAuthUi(
        message: String,
        authenticated: Boolean,
    ) {
        isAuthenticated = authenticated
        runOnUiThread {
            findViewById<TextView>(R.id.auth_status).text = message
            findViewById<Button>(R.id.auth_action).text =
                if (authenticated) "Sign out" else "Start sign-in"
        }
    }

    private fun isTerminalState(state: Int): Boolean {
        return state == AgentSessionInfo.STATE_COMPLETED ||
            state == AgentSessionInfo.STATE_CANCELLED ||
            state == AgentSessionInfo.STATE_FAILED
    }

    private fun openSessionDetail(sessionId: String) {
        startActivity(
            Intent(this, SessionDetailActivity::class.java)
                .putExtra(SessionDetailActivity.EXTRA_SESSION_ID, sessionId),
        )
    }

    private fun launchCreateSessionActivity() {
        startActivity(
            CreateSessionActivity.newSessionIntent(
                context = this,
                initialSettings = CreateSessionActivity.preferredInitialSettings(),
            ),
        )
        moveTaskToBack(true)
    }

    private fun showToast(message: String) {
        runOnUiThread {
            Toast.makeText(this, message, Toast.LENGTH_SHORT).show()
        }
    }
}
