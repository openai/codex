package com.openai.codex.agent

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.RemoteInput
import android.app.agent.AgentSessionInfo
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.drawable.Icon
import android.os.Build

object AgentQuestionNotifier {
    const val ACTION_REPLY_FROM_NOTIFICATION =
        "com.openai.codex.agent.action.REPLY_FROM_NOTIFICATION"
    const val EXTRA_SESSION_ID = "sessionId"
    const val EXTRA_ANSWER_SESSION_ID = "answerSessionId"
    const val EXTRA_ANSWER_PARENT_SESSION_ID = "answerParentSessionId"
    const val EXTRA_NOTIFICATION_TOKEN = "notificationToken"
    const val REMOTE_INPUT_KEY = "codexAgentNotificationReply"

    private const val CHANNEL_ID = "codex_agent_questions"
    private const val CHANNEL_NAME = "Codex Agent Questions"
    private const val MAX_CONTENT_PREVIEW_CHARS = 400
    private val notificationStateLock = Any()
    private val activeNotificationTokens = mutableMapOf<String, String>()
    private val retiredNotificationTokens = mutableMapOf<String, MutableSet<String>>()
    private val suppressedNotificationTokens = mutableMapOf<String, MutableSet<String>>()

    fun showQuestion(
        context: Context,
        sessionId: String,
        targetPackage: String?,
        question: String,
    ) {
        val manager = context.getSystemService(NotificationManager::class.java) ?: return
        ensureChannel(manager)
        manager.notify(notificationId(sessionId), buildNotification(context, sessionId, targetPackage, question))
    }

    fun showOrUpdateDelegatedNotification(
        context: Context,
        presentation: AgentNotificationPresentation,
        notificationToken: String,
    ): Boolean {
        if (!activateNotificationToken(presentation.notificationSessionId, notificationToken)) {
            return false
        }
        val manager = context.getSystemService(NotificationManager::class.java) ?: return false
        if (
            !shouldShowDelegatedNotification(presentation.state) ||
            isSuppressedNotificationToken(presentation.notificationSessionId, notificationToken)
        ) {
            manager.cancel(notificationId(presentation.notificationSessionId))
            return true
        }
        if (presentation.notificationText.isBlank()) {
            return false
        }
        ensureChannel(manager)
        manager.notify(
            notificationId(presentation.notificationSessionId),
            buildDelegatedNotification(
                context = context,
                presentation = presentation,
                notificationToken = notificationToken,
            ),
        )
        return true
    }

    private fun shouldShowDelegatedNotification(state: Int): Boolean {
        return when (state) {
            AgentSessionInfo.STATE_WAITING_FOR_USER,
            AgentSessionInfo.STATE_COMPLETED,
            AgentSessionInfo.STATE_FAILED,
            AgentSessionInfo.STATE_CANCELLED,
            -> true
            AgentSessionInfo.STATE_CREATED,
            AgentSessionInfo.STATE_QUEUED,
            AgentSessionInfo.STATE_RUNNING,
            -> false
            else -> false
        }
    }

    fun suppress(
        context: Context,
        sessionId: String,
        notificationToken: String,
    ) {
        if (!suppressNotificationToken(sessionId, notificationToken)) {
            return
        }
        val manager = context.getSystemService(NotificationManager::class.java) ?: return
        manager.cancel(notificationId(sessionId))
    }

    fun cancel(context: Context, sessionId: String) {
        retireActiveNotificationToken(sessionId)
        val manager = context.getSystemService(NotificationManager::class.java) ?: return
        manager.cancel(notificationId(sessionId))
    }

    fun clearSessionState(sessionId: String) {
        clearNotificationToken(sessionId)
    }

    fun cancel(
        context: Context,
        sessionId: String,
        notificationToken: String,
    ) {
        if (!retireNotificationToken(sessionId, notificationToken)) {
            return
        }
        val manager = context.getSystemService(NotificationManager::class.java) ?: return
        manager.cancel(notificationId(sessionId))
    }

    private fun buildNotification(
        context: Context,
        sessionId: String,
        targetPackage: String?,
        question: String,
    ): Notification {
        val title = targetPackage?.let { "Question for $it" } ?: "Question for Codex Agent"
        val contentIntent = PendingIntent.getActivity(
            context,
            notificationId(sessionId),
            SessionPopupActivity.intent(context, sessionId).apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
            },
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        return Notification.Builder(context, CHANNEL_ID)
            .setSmallIcon(android.R.drawable.ic_dialog_info)
            .setContentTitle(title)
            .setContentText(question)
            .setStyle(Notification.BigTextStyle().bigText(question))
            .setContentIntent(contentIntent)
            .setAutoCancel(false)
            .setOngoing(true)
            .build()
    }

    private fun buildDelegatedNotification(
        context: Context,
        presentation: AgentNotificationPresentation,
        notificationToken: String,
    ): Notification {
        val targetIdentity = resolveTargetIdentity(context, presentation.targetPackage)
        val contentIntent = PendingIntent.getActivity(
            context,
            notificationId(presentation.notificationSessionId),
            SessionPopupActivity.intent(context, presentation.contentSessionId).apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
            },
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        val contentText = presentation.notificationText.take(MAX_CONTENT_PREVIEW_CHARS)
        val builder = Notification.Builder(context, CHANNEL_ID)
            .setSmallIcon(targetIdentity.icon)
            .setLargeIcon(targetIdentity.icon)
            .setContentTitle(buildNotificationTitle(presentation.state, targetIdentity.displayName))
            .setContentText(contentText)
            .setStyle(Notification.BigTextStyle().bigText(contentText))
            .setContentIntent(contentIntent)
            .setAutoCancel(false)
            .setOngoing(true)
        buildInlineReplyAction(
            context = context,
            presentation = presentation,
            notificationToken = notificationToken,
        )?.let { replyAction ->
            builder.addAction(replyAction)
        }
        return builder.build()
    }

    private fun buildNotificationTitle(
        state: Int,
        targetDisplayName: String,
    ): String {
        return when (state) {
            AgentSessionInfo.STATE_WAITING_FOR_USER ->
                "Codex needs input for $targetDisplayName"
            AgentSessionInfo.STATE_COMPLETED ->
                "Codex finished $targetDisplayName"
            AgentSessionInfo.STATE_FAILED ->
                "Codex hit an issue in $targetDisplayName"
            AgentSessionInfo.STATE_CANCELLED ->
                "Codex cancelled $targetDisplayName"
            AgentSessionInfo.STATE_CREATED,
            AgentSessionInfo.STATE_QUEUED,
            AgentSessionInfo.STATE_RUNNING,
            -> "Codex session for $targetDisplayName"
            else -> "Codex session for $targetDisplayName"
        }
    }

    private fun buildInlineReplyAction(
        context: Context,
        presentation: AgentNotificationPresentation,
        notificationToken: String,
    ): Notification.Action? {
        if (presentation.state != AgentSessionInfo.STATE_WAITING_FOR_USER || notificationToken.isBlank()) {
            return null
        }
        val replyIntent = PendingIntent.getBroadcast(
            context,
            notificationId(presentation.notificationSessionId),
            Intent(context, AgentNotificationReplyReceiver::class.java).apply {
                action = ACTION_REPLY_FROM_NOTIFICATION
                putExtra(EXTRA_SESSION_ID, presentation.notificationSessionId)
                putExtra(EXTRA_ANSWER_SESSION_ID, presentation.answerSessionId)
                putExtra(EXTRA_ANSWER_PARENT_SESSION_ID, presentation.answerParentSessionId)
                putExtra(EXTRA_NOTIFICATION_TOKEN, notificationToken)
            },
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_MUTABLE,
        )
        val remoteInput = RemoteInput.Builder(REMOTE_INPUT_KEY)
            .setLabel("Reply")
            .build()
        return Notification.Action.Builder(
            Icon.createWithResource(context, android.R.drawable.ic_menu_send),
            "Reply",
            replyIntent,
        )
            .addRemoteInput(remoteInput)
            .setAllowGeneratedReplies(true)
            .build()
    }

    private fun ensureChannel(manager: NotificationManager) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) {
            return
        }
        if (manager.getNotificationChannel(CHANNEL_ID) != null) {
            return
        }
        val channel = NotificationChannel(
            CHANNEL_ID,
            CHANNEL_NAME,
            NotificationManager.IMPORTANCE_HIGH,
        ).apply {
            description = "Questions that need user input for Codex Agent sessions"
            setShowBadge(true)
        }
        manager.createNotificationChannel(channel)
    }

    private fun activateNotificationToken(
        sessionId: String,
        notificationToken: String,
    ): Boolean {
        if (notificationToken.isBlank()) {
            return false
        }
        synchronized(notificationStateLock) {
            if (retiredNotificationTokens[sessionId]?.contains(notificationToken) == true) {
                return false
            }
            activeNotificationTokens.put(sessionId, notificationToken)?.let { previousToken ->
                if (previousToken != notificationToken) {
                    retiredNotificationTokens.getOrPut(sessionId, ::mutableSetOf)
                        .add(previousToken)
                }
            }
            return true
        }
    }

    private fun clearNotificationToken(sessionId: String) {
        synchronized(notificationStateLock) {
            activeNotificationTokens.remove(sessionId)
            retiredNotificationTokens.remove(sessionId)
            suppressedNotificationTokens.remove(sessionId)
        }
    }

    private fun retireNotificationToken(
        sessionId: String,
        notificationToken: String,
    ): Boolean {
        if (notificationToken.isBlank()) {
            retireActiveNotificationToken(sessionId)
            return true
        }
        synchronized(notificationStateLock) {
            retiredNotificationTokens.getOrPut(sessionId, ::mutableSetOf)
                .add(notificationToken)
            suppressedNotificationTokens[sessionId]?.remove(notificationToken)
            if (activeNotificationTokens[sessionId] != notificationToken) {
                return false
            }
            activeNotificationTokens.remove(sessionId)
            return true
        }
    }

    private fun retireActiveNotificationToken(sessionId: String) {
        synchronized(notificationStateLock) {
            activeNotificationTokens.remove(sessionId)?.let { notificationToken ->
                retiredNotificationTokens.getOrPut(sessionId, ::mutableSetOf)
                    .add(notificationToken)
                suppressedNotificationTokens[sessionId]?.remove(notificationToken)
            }
        }
    }

    private fun suppressNotificationToken(
        sessionId: String,
        notificationToken: String,
    ): Boolean {
        if (!activateNotificationToken(sessionId, notificationToken)) {
            return false
        }
        synchronized(notificationStateLock) {
            suppressedNotificationTokens.getOrPut(sessionId, ::mutableSetOf)
                .add(notificationToken)
        }
        return true
    }

    private fun isSuppressedNotificationToken(
        sessionId: String,
        notificationToken: String,
    ): Boolean {
        synchronized(notificationStateLock) {
            return suppressedNotificationTokens[sessionId]?.contains(notificationToken) == true
        }
    }

    private fun resolveTargetIdentity(
        context: Context,
        targetPackage: String?,
    ): TargetIdentity {
        if (targetPackage.isNullOrBlank()) {
            return TargetIdentity(
                displayName = "Codex Agent",
                icon = Icon.createWithResource(context, android.R.drawable.ic_dialog_info),
            )
        }
        val packageManager = context.packageManager
        return runCatching {
            val appInfo = packageManager.getApplicationInfo(
                targetPackage,
                PackageManager.ApplicationInfoFlags.of(0),
            )
            val iconResId = appInfo.icon.takeIf { it != 0 }
            TargetIdentity(
                displayName = packageManager.getApplicationLabel(appInfo).toString()
                    .ifBlank { targetPackage },
                icon = if (iconResId == null) {
                    Icon.createWithResource(context, android.R.drawable.ic_dialog_info)
                } else {
                    Icon.createWithResource(targetPackage, iconResId)
                },
            )
        }.getOrDefault(
            TargetIdentity(
                displayName = targetPackage,
                icon = Icon.createWithResource(context, android.R.drawable.ic_dialog_info),
            ),
        )
    }

    private fun notificationId(sessionId: String): Int {
        return sessionId.hashCode()
    }

    private data class TargetIdentity(
        val displayName: String,
        val icon: Icon,
    )
}
