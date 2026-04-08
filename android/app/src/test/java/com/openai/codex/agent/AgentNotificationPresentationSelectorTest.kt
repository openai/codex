package com.openai.codex.agent

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class AgentNotificationPresentationSelectorTest {
    @Test
    fun plannerQuestionWinsOverChildQuestion() {
        val presentation = select(
            plannerQuestion = "Which task should I do?",
            childQuestions = listOf(clockQuestion()),
        )

        assertEquals(parentSessionId, presentation.notificationSessionId)
        assertEquals(parentSessionId, presentation.contentSessionId)
        assertEquals(parentSessionId, presentation.answerSessionId)
        assertNull(presentation.answerParentSessionId)
        assertNull(presentation.targetPackage)
        assertEquals("Which task should I do?", presentation.notificationText)
    }

    @Test
    fun waitingChildQuestionUsesChildIdentityAndAnswerDestination() {
        val presentation = select(
            notificationText = "Codex needs input for Codex Agent",
            childQuestions = listOf(clockQuestion()),
        )

        assertEquals(parentSessionId, presentation.notificationSessionId)
        assertEquals(parentSessionId, presentation.contentSessionId)
        assertEquals(clockSessionId, presentation.answerSessionId)
        assertEquals(parentSessionId, presentation.answerParentSessionId)
        assertEquals(clockPackage, presentation.targetPackage)
        assertEquals("What time should I set the alarm for?", presentation.notificationText)
    }

    @Test
    fun bridgeQuestionsAreNotSurfacedAsChildQuestions() {
        val presentation = select(
            notificationText = "__codex_bridge__ {\"method\":\"getRuntimeStatus\"}",
            childQuestions = listOf(
                clockQuestion(
                    question = "__codex_bridge__ {\"method\":\"getRuntimeStatus\"}",
                ),
            ),
        )

        assertEquals(parentSessionId, presentation.answerSessionId)
        assertNull(presentation.answerParentSessionId)
        assertNull(presentation.targetPackage)
        assertEquals("Codex needs input.", presentation.notificationText)
    }

    @Test
    fun fallbackUsesParentNotificationText() {
        val presentation = select(notificationText = "Planner result is ready")

        assertEquals(parentSessionId, presentation.notificationSessionId)
        assertEquals(parentSessionId, presentation.contentSessionId)
        assertEquals(parentSessionId, presentation.answerSessionId)
        assertNull(presentation.answerParentSessionId)
        assertNull(presentation.targetPackage)
        assertEquals("Planner result is ready", presentation.notificationText)
    }

    private fun select(
        notificationText: String = "Parent needs input",
        plannerQuestion: String? = null,
        childQuestions: List<AgentNotificationChildQuestion> = emptyList(),
    ): AgentNotificationPresentation {
        return AgentNotificationPresentationSelector.select(
            sessionId = parentSessionId,
            state = AgentSessionStateValues.WAITING_FOR_USER,
            targetPackage = null,
            notificationText = notificationText,
            plannerQuestion = plannerQuestion,
            childQuestions = childQuestions,
        )
    }

    private fun clockQuestion(
        question: String = "What time should I set the alarm for?",
    ): AgentNotificationChildQuestion {
        return AgentNotificationChildQuestion(
            sessionId = clockSessionId,
            targetPackage = clockPackage,
            question = question,
        )
    }

    private companion object {
        const val parentSessionId = "planner-session"
        const val clockSessionId = "clock-session"
        const val clockPackage = "com.android.deskclock"
    }
}
