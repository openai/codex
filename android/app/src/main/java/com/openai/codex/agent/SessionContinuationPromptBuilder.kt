package com.openai.codex.agent

import android.app.agent.AgentSessionInfo

object SessionContinuationPromptBuilder {
    private const val MAX_TIMELINE_CHARS = 1200
    private const val MAX_DETAIL_CHARS = 600

    fun build(
        sourceTopLevelSession: AgentSessionDetails,
        selectedSession: AgentSessionDetails,
        prompt: String,
    ): String {
        return buildString {
            val selectedIsTopLevel = selectedSession.sessionId == sourceTopLevelSession.sessionId
            appendLine(prompt.trim())
            appendLine()
            appendLine("This is a follow-up continuation of an earlier attempt in the same top-level Agent session.")
            appendLine("Reuse facts learned previously instead of starting over from scratch.")
            if (sourceTopLevelSession.anchor == AgentSessionInfo.ANCHOR_AGENT) {
                appendLine("Route this follow-up through the top-level Agent planner; choose delegated Genie targets afresh instead of assuming the previous child target still applies.")
            }
            appendLine()
            appendLine("Previous session context:")
            appendLine("- Top-level session: ${sourceTopLevelSession.sessionId}")
            if (selectedIsTopLevel) {
                appendLine("- Previous focused session: top-level Agent session ${selectedSession.sessionId}")
            } else {
                appendLine("- Previous focused child session: ${selectedSession.sessionId}")
            }
            selectedSession.targetPackage?.let { appendLine("- Target package: $it") }
            appendLine("- Previous state: ${selectedSession.stateLabel}")
            appendLine("- Previous presentation: ${selectedSession.targetPresentationLabel}")
            appendLine("- Previous runtime: ${selectedSession.targetRuntimeLabel}")
            selectedSession.latestResult
                ?.takeIf(String::isNotBlank)
                ?.let { appendLine("- Previous result: ${it.take(MAX_DETAIL_CHARS)}") }
            selectedSession.latestError
                ?.takeIf(String::isNotBlank)
                ?.let { appendLine("- Previous error: ${it.take(MAX_DETAIL_CHARS)}") }
            selectedSession.latestTrace
                ?.takeIf(String::isNotBlank)
                ?.let { appendLine("- Previous trace: ${it.take(MAX_DETAIL_CHARS)}") }
            val timeline = selectedSession.timeline.trim()
            if (timeline.isNotEmpty() && timeline != "Diagnostics not loaded.") {
                appendLine()
                if (selectedIsTopLevel) {
                    appendLine("Recent timeline from the top-level Agent session:")
                } else {
                    appendLine("Recent timeline from the previous focused child session:")
                }
                appendLine(timeline.take(MAX_TIMELINE_CHARS))
            }
            val parentSummary = sourceTopLevelSession.latestResult
                ?: sourceTopLevelSession.latestError
                ?: sourceTopLevelSession.latestTrace
            parentSummary
                ?.takeIf(String::isNotBlank)
                ?.let {
                    appendLine()
                    appendLine("Top-level session summary:")
                    appendLine(it.take(MAX_DETAIL_CHARS))
                }
        }.trim()
    }
}
