package com.openai.codex.agent

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SessionTapRoutingTest {
    @Test
    fun opensTopLevelRunningTargets() {
        val session = sessionDetails(
            parentSessionId = null,
            anchor = AgentSessionAnchorValues.HOME,
            state = AgentSessionStateValues.RUNNING,
        )

        assertTrue(SessionTapRouting.shouldOpenRunningTarget(session))
    }

    @Test
    fun opensParentedRunningHomeTargets() {
        val session = sessionDetails(
            parentSessionId = "planner-session",
            anchor = AgentSessionAnchorValues.HOME,
            state = AgentSessionStateValues.RUNNING,
        )

        assertTrue(SessionTapRouting.shouldOpenRunningTarget(session))
    }

    @Test
    fun opensParentedRunningAgentTargets() {
        val session = sessionDetails(
            parentSessionId = "planner-session",
            anchor = AgentSessionAnchorValues.AGENT,
            state = AgentSessionStateValues.RUNNING,
        )

        assertTrue(SessionTapRouting.shouldOpenRunningTarget(session))
    }

    @Test
    fun keepsHomeQuestionsAndResultsInCodexUi() {
        val waiting = sessionDetails(
            parentSessionId = "planner-session",
            anchor = AgentSessionAnchorValues.HOME,
            state = AgentSessionStateValues.WAITING_FOR_USER,
        )
        val completed = sessionDetails(
            parentSessionId = "planner-session",
            anchor = AgentSessionAnchorValues.HOME,
            state = AgentSessionStateValues.COMPLETED,
        )

        assertFalse(SessionTapRouting.shouldOpenRunningTarget(waiting))
        assertFalse(SessionTapRouting.shouldOpenRunningTarget(completed))
    }

    @Test
    fun keepsAgentSessionTapsInCodexUi() {
        val session = sessionDetails(
            parentSessionId = null,
            anchor = AgentSessionAnchorValues.AGENT,
            state = AgentSessionStateValues.RUNNING,
        )

        assertFalse(SessionTapRouting.shouldOpenRunningTarget(session))
    }

    private fun sessionDetails(
        parentSessionId: String?,
        anchor: Int,
        state: Int,
    ): AgentSessionDetails {
        return AgentSessionDetails(
            sessionId = "session-id",
            parentSessionId = parentSessionId,
            targetPackage = if (anchor == AgentSessionAnchorValues.AGENT && parentSessionId == null) {
                null
            } else {
                "com.example.target"
            },
            anchor = anchor,
            state = state,
            stateLabel = state.toString(),
            targetPresentation = AgentTargetPresentationValues.ATTACHED,
            targetPresentationLabel = "ATTACHED",
            targetRuntime = null,
            targetRuntimeLabel = "NONE",
            targetDetached = true,
            continuationGeneration = 0,
            requiredFinalPresentationPolicy = null,
            latestQuestion = null,
            latestResult = null,
            latestError = null,
            latestTrace = null,
            timeline = "",
        )
    }
}
