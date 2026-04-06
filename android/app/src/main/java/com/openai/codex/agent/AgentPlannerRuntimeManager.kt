package com.openai.codex.agent

import android.app.agent.AgentManager
import android.content.Context
import android.util.Log
import com.openai.codex.bridge.HostedCodexConfig
import com.openai.codex.bridge.SessionExecutionSettings
import java.io.BufferedWriter
import java.io.Closeable
import java.io.File
import java.io.IOException
import java.io.InterruptedIOException
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.concurrent.thread
import org.json.JSONArray
import org.json.JSONObject

object AgentPlannerRuntimeManager {
    private const val TAG = "AgentPlannerRuntime"
    private val activePlannerSessions = ConcurrentHashMap<String, Boolean>()
    private val desktopPlannerSessions = ConcurrentHashMap<String, AgentPlannerDesktopSessionHost>()

    fun requestText(
        context: Context,
        instructions: String,
        prompt: String,
        outputSchema: JSONObject? = null,
        requestUserInputHandler: ((JSONArray) -> JSONObject)? = null,
        executionSettings: SessionExecutionSettings = SessionExecutionSettings.default,
        requestTimeoutMs: Long = 90_000L,
        frameworkSessionId: String? = null,
    ): String {
        val applicationContext = context.applicationContext
        val plannerSessionId = frameworkSessionId?.trim()?.ifEmpty { null }
            ?: throw IOException("Planner runtime requires a parent session id")
        check(activePlannerSessions.putIfAbsent(plannerSessionId, true) == null) {
            "Planner runtime already active for parent session $plannerSessionId"
        }
        try {
            AgentPlannerRuntime(
                context = applicationContext,
                frameworkSessionId = plannerSessionId,
            ).use { runtime ->
                return runtime.requestText(
                    instructions = instructions,
                    prompt = prompt,
                    outputSchema = outputSchema,
                    requestUserInputHandler = requestUserInputHandler,
                    executionSettings = executionSettings,
                    requestTimeoutMs = requestTimeoutMs,
                )
            }
        } finally {
            activePlannerSessions.remove(plannerSessionId)
        }
    }

    fun ensureIdleDesktopSession(
        context: Context,
        sessionController: AgentSessionController,
        sessionId: String,
    ) {
        check(!activePlannerSessions.containsKey(sessionId)) {
            "Planner runtime already active for parent session $sessionId"
        }
        desktopPlannerSessions.computeIfAbsent(sessionId) {
            AgentPlannerDesktopSessionHost(
                context = context.applicationContext,
                sessionController = sessionController,
                sessionId = sessionId,
                onClosed = {
                    desktopPlannerSessions.remove(sessionId)
                },
            ).also(AgentPlannerDesktopSessionHost::start)
        }
    }

    fun activeThreadId(sessionId: String): String? = desktopPlannerSessions[sessionId]?.activeThreadId()

    fun openDesktopProxy(
        sessionId: String,
        onMessage: (String) -> Unit,
        onClosed: (String?) -> Unit,
    ): String? = desktopPlannerSessions[sessionId]?.openDesktopProxy(onMessage, onClosed)

    fun sendDesktopProxyInput(
        sessionId: String,
        connectionId: String,
        message: String,
    ): Boolean = desktopPlannerSessions[sessionId]?.sendDesktopProxyInput(connectionId, message) ?: false

    fun closeDesktopProxy(
        sessionId: String,
        connectionId: String,
        reason: String? = null,
        detachPlanner: Boolean = false,
    ) {
        desktopPlannerSessions[sessionId]?.closeDesktopProxy(connectionId, reason, detachPlanner)
    }

    fun closeSession(sessionId: String) {
        desktopPlannerSessions.remove(sessionId)?.close()
    }

    private class AgentPlannerRuntime(
        private val context: Context,
        private val frameworkSessionId: String?,
    ) : Closeable {
        companion object {
            private const val REQUEST_TIMEOUT_MS = 30_000L
            private const val AGENT_APP_SERVER_RUST_LOG = "warn"
        }

        private val requestIdSequence = AtomicInteger(1)
        private val pendingResponses = ConcurrentHashMap<String, LinkedBlockingQueue<JSONObject>>()
        private val notifications = LinkedBlockingQueue<JSONObject>()

        private lateinit var process: Process
        private lateinit var writer: BufferedWriter
        private lateinit var codexHome: File
        private val closing = AtomicBoolean(false)
        private var stdoutThread: Thread? = null
        private var stderrThread: Thread? = null
        private var localProxy: AgentLocalCodexProxy? = null

        fun requestText(
            instructions: String,
            prompt: String,
            outputSchema: JSONObject?,
            requestUserInputHandler: ((JSONArray) -> JSONObject)?,
            executionSettings: SessionExecutionSettings,
            requestTimeoutMs: Long,
        ): String {
            startProcess()
            initialize()
            val threadId = startThread(
                instructions = instructions,
                executionSettings = executionSettings,
            )
            startTurn(
                threadId = threadId,
                prompt = prompt,
                outputSchema = outputSchema,
                executionSettings = executionSettings,
            )
            return waitForTurnCompletion(requestUserInputHandler, requestTimeoutMs)
        }

        override fun close() {
            closing.set(true)
            stdoutThread?.interrupt()
            stderrThread?.interrupt()
            if (::writer.isInitialized) {
                runCatching { writer.close() }
            }
            localProxy?.close()
            if (::codexHome.isInitialized) {
                runCatching { codexHome.deleteRecursively() }
            }
            if (::process.isInitialized) {
                runCatching { process.destroy() }
            }
        }

        private fun startProcess() {
            codexHome = File(context.cacheDir, "planner-codex-home/$frameworkSessionId").apply {
                deleteRecursively()
                mkdirs()
            }
            localProxy = AgentLocalCodexProxy { requestBody ->
                forwardResponsesRequest(requestBody)
            }.also(AgentLocalCodexProxy::start)
            HostedCodexConfig.write(
                context,
                codexHome,
                localProxy?.baseUrl
                    ?: throw IOException("planner local proxy did not start"),
            )
            process = ProcessBuilder(
                listOf(
                    CodexCliBinaryLocator.resolve(context).absolutePath,
                    "-c",
                    "enable_request_compression=false",
                    "-c",
                    "features.default_mode_request_user_input=true",
                    "app-server",
                    "--listen",
                    "stdio://",
                ),
            ).apply {
                environment()["CODEX_HOME"] = codexHome.absolutePath
                environment()["RUST_LOG"] = AGENT_APP_SERVER_RUST_LOG
            }.start()
            writer = process.outputStream.bufferedWriter()
            startStdoutPump()
            startStderrPump()
        }

        private fun initialize() {
            request(
                method = "initialize",
                params = JSONObject()
                    .put(
                        "clientInfo",
                        JSONObject()
                            .put("name", "android_agent_planner")
                            .put("title", "Android Agent Planner")
                            .put("version", "0.1.0"),
                    )
                    .put("capabilities", JSONObject().put("experimentalApi", true)),
            )
            notify("initialized", JSONObject())
        }

        private fun startThread(
            instructions: String,
            executionSettings: SessionExecutionSettings,
        ): String {
            val params = JSONObject()
                .put("approvalPolicy", "never")
                .put("sandbox", "read-only")
                .put("cwd", context.filesDir.absolutePath)
                .put("serviceName", "android_agent_planner")
                .put("baseInstructions", instructions)
            executionSettings.model
                ?.takeIf(String::isNotBlank)
                ?.let { params.put("model", it) }
            val result = request(
                method = "thread/start",
                params = params,
            )
            return result.getJSONObject("thread").getString("id")
        }

        private fun startTurn(
            threadId: String,
            prompt: String,
            outputSchema: JSONObject?,
            executionSettings: SessionExecutionSettings,
        ) {
            val turnParams = JSONObject()
                .put("threadId", threadId)
                .put(
                    "input",
                    JSONArray().put(
                        JSONObject()
                            .put("type", "text")
                            .put("text", prompt),
                    ),
                )
            executionSettings.model
                ?.takeIf(String::isNotBlank)
                ?.let { turnParams.put("model", it) }
            executionSettings.reasoningEffort
                ?.takeIf(String::isNotBlank)
                ?.let { turnParams.put("effort", it) }
            if (outputSchema != null) {
                turnParams.put("outputSchema", outputSchema)
            }
            request(
                method = "turn/start",
                params = turnParams,
            )
        }

        private fun waitForTurnCompletion(
            requestUserInputHandler: ((JSONArray) -> JSONObject)?,
            requestTimeoutMs: Long,
        ): String {
            val streamedAgentMessages = mutableMapOf<String, StringBuilder>()
            var finalAgentMessage: String? = null
            val deadline = System.nanoTime() + TimeUnit.MILLISECONDS.toNanos(requestTimeoutMs)
            while (true) {
                val remainingNanos = deadline - System.nanoTime()
                if (remainingNanos <= 0L) {
                    throw IOException("Timed out waiting for planner turn completion")
                }
                val notification = notifications.poll(remainingNanos, TimeUnit.NANOSECONDS)
                if (notification == null) {
                    checkProcessAlive()
                    continue
                }
                if (notification.has("id") && notification.has("method")) {
                    handleServerRequest(notification, requestUserInputHandler)
                    continue
                }
                val params = notification.optJSONObject("params") ?: JSONObject()
                when (notification.optString("method")) {
                    "item/agentMessage/delta" -> {
                        val itemId = params.optString("itemId")
                        if (itemId.isNotBlank()) {
                            streamedAgentMessages.getOrPut(itemId, ::StringBuilder)
                                .append(params.optString("delta"))
                        }
                    }

                    "item/completed" -> {
                        val item = params.optJSONObject("item") ?: continue
                        if (item.optString("type") == "agentMessage") {
                            val itemId = item.optString("id")
                            val text = item.optString("text").ifBlank {
                                streamedAgentMessages[itemId]?.toString().orEmpty()
                            }
                            if (text.isNotBlank()) {
                                finalAgentMessage = text
                            }
                        }
                    }

                    "turn/completed" -> {
                        val turn = params.optJSONObject("turn") ?: JSONObject()
                        return when (turn.optString("status")) {
                            "completed" -> finalAgentMessage?.takeIf(String::isNotBlank)
                                ?: throw IOException("Planner turn completed without an assistant message")

                            "interrupted" -> throw IOException("Planner turn interrupted")
                            else -> throw IOException(
                                turn.opt("error")?.toString()
                                    ?: "Planner turn failed with status ${turn.optString("status", "unknown")}",
                            )
                        }
                    }
                }
            }
        }

        private fun handleServerRequest(
            message: JSONObject,
            requestUserInputHandler: ((JSONArray) -> JSONObject)?,
        ) {
            val requestId = message.opt("id") ?: return
            val method = message.optString("method", "unknown")
            val params = message.optJSONObject("params") ?: JSONObject()
            when (method) {
                "item/tool/requestUserInput" -> {
                    if (requestUserInputHandler == null) {
                        sendError(
                            requestId = requestId,
                            code = -32601,
                            message = "No Agent user-input handler registered for $method",
                        )
                        return
                    }
                    val questions = params.optJSONArray("questions") ?: JSONArray()
                    val result = runCatching { requestUserInputHandler(questions) }
                        .getOrElse { err ->
                            sendError(
                                requestId = requestId,
                                code = -32000,
                                message = err.message ?: "Agent user input request failed",
                            )
                            return
                        }
                    sendResult(requestId, result)
                }

                else -> {
                    sendError(
                        requestId = requestId,
                        code = -32601,
                        message = "Unsupported planner app-server request: $method",
                    )
                }
            }
        }

        private fun forwardResponsesRequest(requestBody: String): AgentResponsesProxy.HttpResponse {
            val activeFrameworkSessionId = frameworkSessionId
            check(!activeFrameworkSessionId.isNullOrBlank()) {
                "Planner runtime requires a framework session id for /responses transport"
            }
            val agentManager = context.getSystemService(AgentManager::class.java)
                ?: throw IOException("AgentManager unavailable for framework session transport")
            return AgentResponsesProxy.sendResponsesRequestThroughFramework(
                agentManager = agentManager,
                sessionId = activeFrameworkSessionId,
                context = context,
                requestBody = requestBody,
            )
        }

        private fun request(
            method: String,
            params: JSONObject?,
        ): JSONObject {
            val requestId = requestIdSequence.getAndIncrement().toString()
            val responseQueue = LinkedBlockingQueue<JSONObject>(1)
            pendingResponses[requestId] = responseQueue
            try {
                val message = JSONObject()
                    .put("id", requestId)
                    .put("method", method)
                if (params != null) {
                    message.put("params", params)
                }
                sendMessage(message)
                val response = responseQueue.poll(REQUEST_TIMEOUT_MS, TimeUnit.MILLISECONDS)
                    ?: throw IOException("Timed out waiting for $method response")
                val error = response.optJSONObject("error")
                if (error != null) {
                    throw IOException("$method failed: ${error.optString("message", error.toString())}")
                }
                return response.optJSONObject("result") ?: JSONObject()
            } finally {
                pendingResponses.remove(requestId)
            }
        }

        private fun notify(
            method: String,
            params: JSONObject,
        ) {
            sendMessage(
                JSONObject()
                    .put("method", method)
                    .put("params", params),
            )
        }

        private fun sendResult(
            requestId: Any,
            result: JSONObject,
        ) {
            sendMessage(
                JSONObject()
                    .put("id", requestId)
                    .put("result", result),
            )
        }

        private fun sendError(
            requestId: Any,
            code: Int,
            message: String,
        ) {
            sendMessage(
                JSONObject()
                    .put("id", requestId)
                    .put(
                        "error",
                        JSONObject()
                            .put("code", code)
                            .put("message", message),
                    ),
            )
        }

        private fun sendMessage(message: JSONObject) {
            writer.write(message.toString())
            writer.newLine()
            writer.flush()
        }

        private fun startStdoutPump() {
            stdoutThread = thread(name = "AgentPlannerStdout-$frameworkSessionId") {
                try {
                    process.inputStream.bufferedReader().useLines { lines ->
                        lines.forEach { line ->
                            if (line.isBlank()) {
                                return@forEach
                            }
                            val message = runCatching { JSONObject(line) }
                                .getOrElse { err ->
                                    Log.w(TAG, "Failed to parse planner app-server stdout line", err)
                                    return@forEach
                                }
                            if (message.has("id") && !message.has("method")) {
                                pendingResponses[message.get("id").toString()]?.offer(message)
                            } else {
                                notifications.offer(message)
                            }
                        }
                    }
                } catch (err: InterruptedIOException) {
                    if (!closing.get()) {
                        Log.w(TAG, "Planner stdout pump interrupted unexpectedly", err)
                    }
                } catch (err: IOException) {
                    if (!closing.get()) {
                        Log.w(TAG, "Planner stdout pump failed", err)
                    }
                }
            }
        }

        private fun startStderrPump() {
            stderrThread = thread(name = "AgentPlannerStderr-$frameworkSessionId") {
                try {
                    process.errorStream.bufferedReader().useLines { lines ->
                        lines.forEach { line ->
                            if (line.contains(" ERROR ") || line.startsWith("ERROR")) {
                                Log.e(TAG, line)
                            } else if (line.contains(" WARN ") || line.startsWith("WARN")) {
                                Log.w(TAG, line)
                            }
                        }
                    }
                } catch (err: InterruptedIOException) {
                    if (!closing.get()) {
                        Log.w(TAG, "Planner stderr pump interrupted unexpectedly", err)
                    }
                } catch (err: IOException) {
                    if (!closing.get()) {
                        Log.w(TAG, "Planner stderr pump failed", err)
                    }
                }
            }
        }

        private fun checkProcessAlive() {
            if (!process.isAlive) {
                throw IOException("Planner app-server exited with code ${process.exitValue()}")
            }
        }
    }
}
