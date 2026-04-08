package com.openai.codex.agent

data class AgentNotificationPresentation(
    val notificationSessionId: String,
    val contentSessionId: String,
    val answerSessionId: String,
    val answerParentSessionId: String?,
    val state: Int,
    val targetPackage: String?,
    val notificationText: String,
)

data class AgentNotificationChildQuestion(
    val sessionId: String,
    val targetPackage: String?,
    val question: String,
)

object AgentNotificationPresentationSelector {
    private const val BRIDGE_REQUEST_PREFIX = "__codex_bridge__ "
    private const val GENERIC_INPUT_REQUIRED_TEXT = "Codex needs input."

    fun select(
        sessionId: String,
        state: Int,
        targetPackage: String?,
        notificationText: String,
        plannerQuestion: String?,
        childQuestions: List<AgentNotificationChildQuestion>,
    ): AgentNotificationPresentation {
        val trimmedPlannerQuestion = plannerQuestion?.trim()?.takeIf(String::isNotEmpty)
        if (trimmedPlannerQuestion != null) {
            return parentPresentation(
                sessionId = sessionId,
                state = state,
                targetPackage = targetPackage,
                notificationText = trimmedPlannerQuestion,
            )
        }

        if (state == AgentSessionStateValues.WAITING_FOR_USER) {
            firstUserVisibleChildQuestion(childQuestions)?.let { childQuestion ->
                return AgentNotificationPresentation(
                    notificationSessionId = sessionId,
                    contentSessionId = sessionId,
                    answerSessionId = childQuestion.sessionId,
                    answerParentSessionId = sessionId,
                    state = state,
                    targetPackage = childQuestion.targetPackage,
                    notificationText = childQuestion.question.trim(),
                )
            }
        }

        return parentPresentation(
            sessionId = sessionId,
            state = state,
            targetPackage = targetPackage,
            notificationText = notificationText,
        )
    }

    private fun parentPresentation(
        sessionId: String,
        state: Int,
        targetPackage: String?,
        notificationText: String,
    ): AgentNotificationPresentation {
        return AgentNotificationPresentation(
            notificationSessionId = sessionId,
            contentSessionId = sessionId,
            answerSessionId = sessionId,
            answerParentSessionId = null,
            state = state,
            targetPackage = targetPackage,
            notificationText = notificationText.toUserVisibleFallbackText(),
        )
    }

    private fun firstUserVisibleChildQuestion(
        childQuestions: List<AgentNotificationChildQuestion>,
    ): AgentNotificationChildQuestion? {
        return childQuestions.firstOrNull { childQuestion ->
            childQuestion.question.isUserVisibleQuestion()
        }
    }

    private fun String.isUserVisibleQuestion(): Boolean {
        return trim().let { question ->
            question.isNotEmpty() && !question.startsWith(BRIDGE_REQUEST_PREFIX)
        }
    }

    private fun String.toUserVisibleFallbackText(): String {
        val trimmedText = trim()
        return if (trimmedText.startsWith(BRIDGE_REQUEST_PREFIX)) {
            GENERIC_INPUT_REQUIRED_TEXT
        } else {
            trimmedText
        }
    }
}
