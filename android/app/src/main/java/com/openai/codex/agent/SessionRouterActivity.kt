package com.openai.codex.agent

import android.app.Activity
import android.app.agent.AgentManager
import android.app.agent.AgentSessionInfo
import android.content.Intent
import android.os.Bundle
import android.util.Log
import kotlin.concurrent.thread

class SessionRouterActivity : Activity() {
    private val sessionController by lazy { AgentSessionController(this) }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        routeIntent(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        routeIntent(intent)
    }

    private fun routeIntent(intent: Intent?) {
        val sessionId = intent
            ?.getStringExtra(AgentManager.EXTRA_SESSION_ID)
            ?.trim()
            ?.ifEmpty { null }
        if (sessionId == null) {
            finish()
            return
        }
        thread(name = "CodexSessionRouter-$sessionId") {
            val destination = runCatching { resolveDestination(sessionId) }
                .getOrElse { err ->
                    Log.w(TAG, "Failed to route framework session $sessionId", err)
                    Destination.Popup(sessionId)
                }
            runOnUiThread {
                openDestination(destination)
            }
        }
    }

    private fun resolveDestination(sessionId: String): Destination {
        val snapshot = sessionController.loadSnapshot(sessionId)
        val session = snapshot.sessions.firstOrNull { it.sessionId == sessionId }
            ?: snapshot.selectedSession?.takeIf { it.sessionId == sessionId }
            ?: snapshot.parentSession?.takeIf { it.sessionId == sessionId }
            ?: return Destination.Popup(sessionId)
        val hasChildren = snapshot.sessions.any { it.parentSessionId == sessionId }
        if (isStandaloneHomeDraftSession(session, hasChildren)) {
            val targetPackage = checkNotNull(session.targetPackage)
            return Destination.CreateHomeDraft(
                sessionId = session.sessionId,
                targetPackage = targetPackage,
            )
        }
        if (isRunningHomeSession(session)) {
            return Destination.OpenRunningHomeTarget(session)
        }
        return Destination.Popup(sessionId)
    }

    private fun openDestination(destination: Destination) {
        when (destination) {
            is Destination.CreateHomeDraft -> {
                startActivity(
                    CreateSessionActivity.existingHomeSessionIntent(
                        context = this,
                        sessionId = destination.sessionId,
                        targetPackage = destination.targetPackage,
                        initialSettings = sessionController.executionSettingsForSession(destination.sessionId),
                    ).addFlags(Intent.FLAG_ACTIVITY_CLEAR_TASK),
                )
                finish()
            }
            is Destination.OpenRunningHomeTarget -> {
                openRunningHomeTarget(destination.session)
            }
            is Destination.Popup -> {
                startActivity(
                    SessionPopupActivity.intent(this, destination.sessionId)
                        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                        .addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP),
                )
                finish()
            }
        }
    }

    private fun openRunningHomeTarget(session: AgentSessionDetails) {
        thread(name = "CodexSessionRouterOpenTarget-${session.sessionId}") {
            val opened = runCatching {
                if (session.targetDetached) {
                    sessionController.showDetachedTarget(session.sessionId)
                } else {
                    sessionController.attachTarget(session.sessionId)
                }
            }.isSuccess
            runOnUiThread {
                if (!opened) {
                    startActivity(
                        SessionPopupActivity.intent(this, session.sessionId)
                            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                            .addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP),
                    )
                }
                finish()
            }
        }
    }

    private fun isStandaloneHomeDraftSession(
        session: AgentSessionDetails,
        hasChildren: Boolean,
    ): Boolean {
        return session.anchor == AgentSessionInfo.ANCHOR_HOME &&
            session.state == AgentSessionInfo.STATE_CREATED &&
            !hasChildren &&
            !session.targetPackage.isNullOrBlank()
    }

    private fun isRunningHomeSession(session: AgentSessionDetails): Boolean {
        return session.anchor == AgentSessionInfo.ANCHOR_HOME &&
            session.parentSessionId == null &&
            session.state == AgentSessionInfo.STATE_RUNNING
    }

    private sealed interface Destination {
        data class CreateHomeDraft(
            val sessionId: String,
            val targetPackage: String,
        ) : Destination

        data class OpenRunningHomeTarget(
            val session: AgentSessionDetails,
        ) : Destination

        data class Popup(
            val sessionId: String,
        ) : Destination
    }

    companion object {
        private const val TAG = "CodexSessionRouter"
    }
}
