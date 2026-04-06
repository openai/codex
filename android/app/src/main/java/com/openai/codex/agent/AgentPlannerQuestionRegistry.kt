package com.openai.codex.agent

import android.app.agent.AgentManager
import android.app.agent.AgentSessionInfo
import android.content.Context
import android.util.Log
import java.io.IOException
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.LinkedBlockingQueue
import org.json.JSONArray
import org.json.JSONObject

object AgentPlannerQuestionRegistry {
    private const val TAG = "AgentPlannerQuestionRegistry"

    private data class PendingPlannerQuestion(
        val questions: JSONArray,
        val renderedQuestion: String,
        val responses: LinkedBlockingQueue<PendingPlannerQuestionResponse> = LinkedBlockingQueue(1),
    )

    private data class PendingPlannerQuestionResponse(
        val answer: JSONObject? = null,
        val error: IOException? = null,
    )

    private val pendingQuestions = ConcurrentHashMap<String, PendingPlannerQuestion>()

    fun requestUserInput(
        context: Context,
        sessionController: AgentSessionController,
        sessionId: String,
        questions: JSONArray,
    ): JSONObject {
        val appContext = context.applicationContext
        val manager = appContext.getSystemService(AgentManager::class.java)
            ?: throw IOException("AgentManager unavailable for planner question")
        val pendingQuestion = PendingPlannerQuestion(
            questions = JSONArray(questions.toString()),
            renderedQuestion = AgentUserInputPrompter.renderQuestions(questions),
        )
        pendingQuestions.put(sessionId, pendingQuestion)?.responses?.offer(
            PendingPlannerQuestionResponse(error = IOException("Planner question superseded")),
        )
        runCatching {
            manager.publishTrace(sessionId, "Planner requested user input before delegating to Genies.")
        }.onFailure { err ->
            Log.w(TAG, "Failed to publish planner question trace for $sessionId", err)
        }
        manager.updateSessionState(sessionId, AgentSessionInfo.STATE_WAITING_FOR_USER)
        return try {
            val response = pendingQuestion.responses.take()
            response.error?.let { throw it }
            response.answer ?: throw IOException("Planner question completed without an answer")
        } catch (err: InterruptedException) {
            Thread.currentThread().interrupt()
            throw IOException("Interrupted while waiting for planner question answer", err)
        } finally {
            pendingQuestions.remove(sessionId, pendingQuestion)
            if (!sessionController.isTerminalSession(sessionId)) {
                runCatching {
                    manager.updateSessionState(sessionId, AgentSessionInfo.STATE_RUNNING)
                }.onFailure { err ->
                    Log.w(TAG, "Failed to restore planner session state for $sessionId", err)
                }
            }
        }
    }

    fun answerQuestion(
        sessionId: String,
        answer: String,
    ): Boolean {
        val pendingQuestion = pendingQuestions[sessionId] ?: return false
        val answerJson = JSONObject().put(
            "answers",
            AgentUserInputPrompter.buildQuestionAnswers(pendingQuestion.questions, answer),
        )
        pendingQuestion.responses.offer(PendingPlannerQuestionResponse(answer = answerJson))
        return true
    }

    fun cancelQuestion(
        sessionId: String,
        reason: String,
    ) {
        pendingQuestions.remove(sessionId)?.responses?.offer(
            PendingPlannerQuestionResponse(error = IOException(reason)),
        )
    }

    fun latestQuestion(sessionId: String): String? = pendingQuestions[sessionId]?.renderedQuestion
}
