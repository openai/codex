package com.openai.codex.agent

import android.app.agent.AgentSessionInfo

object AgentSessionAnchorValues {
    const val AGENT = AgentSessionInfo.ANCHOR_AGENT
    const val HOME = AgentSessionInfo.ANCHOR_HOME
}

internal object SessionTapRouting {
    fun shouldOpenRunningTarget(session: AgentSessionDetails): Boolean {
        return !session.targetPackage.isNullOrBlank() &&
            session.state == AgentSessionStateValues.RUNNING
    }
}
