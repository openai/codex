package com.openai.codex.agent

import android.app.RemoteInput
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import kotlin.concurrent.thread

class AgentNotificationReplyReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != AgentQuestionNotifier.ACTION_REPLY_FROM_NOTIFICATION) {
            return
        }
        val sessionId = intent.getStringExtra(AgentQuestionNotifier.EXTRA_SESSION_ID)?.trim().orEmpty()
        val answerSessionId = intent.getStringExtra(AgentQuestionNotifier.EXTRA_ANSWER_SESSION_ID)
            ?.trim()
            ?.ifEmpty { null }
            ?: sessionId
        val answerParentSessionId = intent.getStringExtra(AgentQuestionNotifier.EXTRA_ANSWER_PARENT_SESSION_ID)
            ?.trim()
            ?.ifEmpty { null }
        val notificationToken = intent.getStringExtra(
            AgentQuestionNotifier.EXTRA_NOTIFICATION_TOKEN,
        )?.trim().orEmpty()
        val answer = RemoteInput.getResultsFromIntent(intent)
            ?.getCharSequence(AgentQuestionNotifier.REMOTE_INPUT_KEY)
            ?.toString()
            ?.trim()
            .orEmpty()
        if (sessionId.isEmpty() || answer.isEmpty()) {
            return
        }
        val pendingResult = goAsync()
        thread(name = "CodexAgentNotificationReply-$sessionId") {
            try {
                AgentQuestionNotifier.suppress(
                    context = context,
                    sessionId = sessionId,
                    notificationToken = notificationToken,
                )
                val sessionController = AgentSessionController(context)
                runCatching {
                    if (answerSessionId == sessionId) {
                        sessionController.answerQuestionFromNotification(
                            sessionId = sessionId,
                            notificationToken = notificationToken,
                            answer = answer,
                            parentSessionId = null,
                        )
                    } else {
                        sessionController.answerQuestion(
                            sessionId = answerSessionId,
                            answer = answer,
                            parentSessionId = answerParentSessionId,
                        )
                        sessionController.ackSessionNotification(sessionId, notificationToken)
                    }
                }.onFailure { err ->
                    Log.w(TAG, "Failed to answer notification question for $answerSessionId", err)
                }
            } finally {
                pendingResult.finish()
            }
        }
    }

    private companion object {
        private const val TAG = "CodexAgentReply"
    }
}
