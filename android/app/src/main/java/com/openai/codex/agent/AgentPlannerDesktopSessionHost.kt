package com.openai.codex.agent

import android.app.agent.AgentManager
import android.content.Context
import android.util.Log
import com.openai.codex.bridge.FrameworkEventBridge
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
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger
import kotlin.concurrent.thread
import org.json.JSONArray
import org.json.JSONObject

internal class AgentPlannerDesktopSessionHost(
    private val context: Context,
    private val sessionController: AgentSessionController,
    private val sessionId: String,
    private val onClosed: () -> Unit,
) : Closeable {
    companion object {
        private const val TAG = "AgentPlannerDesktop"
        private const val REQUEST_TIMEOUT_MS = 30_000L
        private const val POLL_TIMEOUT_MS = 250L
        private const val REMOTE_REQUEST_ID_PREFIX = "remote:"
        private const val REMOTE_SERVER_VERSION = "0.1.0"
        private const val DEFAULT_HOSTED_MODEL = "gpt-5.3-codex"
        private val DISALLOWED_TARGET_PACKAGES = setOf(
            "com.android.shell",
            "com.android.systemui",
            "com.openai.codex.agent",
            "com.openai.codex.genie",
        )
    }

    private data class DesktopProxy(
        val connectionId: String,
        val onMessage: (String) -> Unit,
        val onClosed: (String?) -> Unit,
    )

    private data class RemoteProxyState(
        val connectionId: String,
        val optOutNotificationMethods: Set<String>,
    )

    private data class RemotePendingRequest(
        val connectionId: String,
        val remoteRequestId: Any,
    )

    private data class PendingDesktopMessage(
        val connectionId: String,
        val message: String,
    )

    private val requestIdSequence = AtomicInteger(1)
    private val pendingResponses = ConcurrentHashMap<String, LinkedBlockingQueue<JSONObject>>()
    private val remotePendingRequests = ConcurrentHashMap<String, RemotePendingRequest>()
    private val inboundMessages = LinkedBlockingQueue<JSONObject>()
    private val pendingDesktopMessages = LinkedBlockingQueue<PendingDesktopMessage>()
    private val writerLock = Any()
    private val proxyLock = Any()
    private val streamedAgentMessages = mutableMapOf<String, StringBuilder>()
    private val closing = AtomicBoolean(false)

    private lateinit var process: Process
    private lateinit var writer: BufferedWriter
    private lateinit var codexHome: File
    private lateinit var executionSettings: SessionExecutionSettings
    private var stdoutThread: Thread? = null
    private var stderrThread: Thread? = null
    private var eventLoopThread: Thread? = null
    private var desktopDispatchThread: Thread? = null
    private var localProxy: AgentLocalCodexProxy? = null
    private var runtimeStatus: AgentCodexAppServerClient.RuntimeStatus? = null
    private var finalAgentMessage: String? = null
    private var currentObjective: String? = null
    private var pendingDirectSessionStart: PendingDirectSessionStart? = null
    @Volatile
    private var currentThreadId: String? = null
    @Volatile
    private var currentDesktopProxy: DesktopProxy? = null
    @Volatile
    private var remoteProxyState: RemoteProxyState? = null
    @Volatile
    private var lastReportedFrameworkEventCount = 0

    fun start() {
        executionSettings = sessionController.executionSettingsForSession(sessionId)
        runtimeStatus = runCatching {
            AgentCodexAppServerClient.readRuntimeStatus(context)
        }.getOrNull()
        startProcess()
        initialize()
        currentThreadId = startThread()
        desktopDispatchThread = thread(
            start = true,
            name = "AgentPlannerDesktopDispatch-$sessionId",
        ) {
            dispatchDesktopMessages()
        }
        eventLoopThread = thread(
            start = true,
            name = "AgentPlannerDesktopEventLoop-$sessionId",
        ) {
            eventLoop()
        }
    }

    override fun close() {
        if (!closing.compareAndSet(false, true)) {
            return
        }
        DesktopInspectionRegistry.markPlannerDetached(sessionId)
        val proxy = synchronized(proxyLock) {
            currentDesktopProxy.also {
                currentDesktopProxy = null
            }
        }
        runCatching {
            proxy?.onClosed("Planner desktop session closed")
        }
        stdoutThread?.interrupt()
        stderrThread?.interrupt()
        eventLoopThread?.interrupt()
        desktopDispatchThread?.interrupt()
        synchronized(writerLock) {
            runCatching { writer.close() }
        }
        localProxy?.close()
        if (::codexHome.isInitialized) {
            runCatching { codexHome.deleteRecursively() }
        }
        if (::process.isInitialized) {
            process.destroy()
        }
        onClosed()
    }

    fun activeThreadId(): String? = currentThreadId

    fun openDesktopProxy(
        onMessage: (String) -> Unit,
        onClosed: (String?) -> Unit,
    ): String? {
        val threadId = currentThreadId ?: return null
        if (threadId.isBlank()) {
            return null
        }
        val connectionId = java.util.UUID.randomUUID().toString()
        val replacement = synchronized(proxyLock) {
            currentDesktopProxy.also {
                currentDesktopProxy = DesktopProxy(connectionId, onMessage, onClosed)
            }
        }
        runCatching {
            replacement?.onClosed("Replaced by a newer desktop attach")
        }
        DesktopInspectionRegistry.markPlannerAttached(sessionId)
        return connectionId
    }

    fun sendDesktopProxyInput(
        connectionId: String,
        message: String,
    ): Boolean {
        val proxy = currentDesktopProxy
        if (proxy?.connectionId != connectionId) {
            return false
        }
        handleRemoteProxyMessage(message)
        return true
    }

    fun closeDesktopProxy(
        connectionId: String,
        reason: String? = null,
        detachPlanner: Boolean = false,
    ) {
        val proxy = synchronized(proxyLock) {
            currentDesktopProxy?.takeIf { it.connectionId == connectionId }?.also {
                currentDesktopProxy = null
            }
        } ?: return
        if (remoteProxyState?.connectionId == connectionId) {
            remoteProxyState = null
            lastReportedFrameworkEventCount = 0
        }
        if (detachPlanner) {
            DesktopInspectionRegistry.markPlannerDetached(sessionId)
        }
        runCatching {
            proxy.onClosed(reason)
        }
    }

    private fun startProcess() {
        codexHome = File(context.cacheDir, "planner-desktop-codex-home/$sessionId").apply {
            deleteRecursively()
            mkdirs()
        }
        localProxy = AgentLocalCodexProxy { requestBody ->
            forwardResponsesRequest(requestBody)
        }.also(AgentLocalCodexProxy::start)
        HostedCodexConfig.write(
            context,
            codexHome,
            localProxy?.baseUrl ?: throw IOException("planner desktop local proxy did not start"),
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
            environment()["RUST_LOG"] = "warn"
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
                        .put("name", "android_agent_planner_desktop")
                        .put("title", "Android Agent Planner Desktop")
                        .put("version", "0.1.0"),
                )
                .put("capabilities", JSONObject().put("experimentalApi", true)),
        )
        notify("initialized", JSONObject())
    }

    private fun startThread(): String {
        val params = JSONObject()
            .put("approvalPolicy", "never")
            .put("sandbox", "read-only")
            .put("cwd", context.filesDir.absolutePath)
            .put("serviceName", "android_agent_planner")
            .put("baseInstructions", AgentTaskPlanner.plannerInstructions())
            .put(
                "model",
                executionSettings.model
                    ?.takeIf(String::isNotBlank)
                    ?: DEFAULT_HOSTED_MODEL,
            )
        val result = request(
            method = "thread/start",
            params = params,
        )
        return result.getJSONObject("thread").getString("id")
    }

    private fun eventLoop() {
        try {
            while (!closing.get()) {
                val message = inboundMessages.poll(POLL_TIMEOUT_MS, TimeUnit.MILLISECONDS)
                if (message == null) {
                    maybeEmitFrameworkEvents()
                    if (!process.isAlive) {
                        throw IOException("Planner app-server exited with code ${process.exitValue()}")
                    }
                    continue
                }
                if (message.has("method") && message.has("id")) {
                    handleServerRequest(message)
                    continue
                }
                if (message.has("method") && handleNotification(message)) {
                    return
                }
            }
        } catch (err: Exception) {
            if (!closing.get()) {
                Log.w(TAG, "Planner desktop runtime failed for $sessionId", err)
            }
        } finally {
            close()
        }
    }

    private fun handleServerRequest(message: JSONObject) {
        val requestId = message.opt("id") ?: return
        val method = message.optString("method")
        when (method) {
            "item/tool/requestUserInput" -> {
                val questions = message.optJSONObject("params")?.optJSONArray("questions") ?: JSONArray()
                val result = runCatching {
                    AgentPlannerQuestionRegistry.requestUserInput(
                        context = context,
                        sessionController = sessionController,
                        sessionId = sessionId,
                        questions = questions,
                    )
                }.getOrElse { err ->
                    sendError(
                        requestId = requestId,
                        code = -32000,
                        message = err.message ?: "Planner user input request failed",
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

    private fun handleNotification(message: JSONObject): Boolean {
        val method = message.optString("method")
        val params = message.optJSONObject("params") ?: JSONObject()
        return when (method) {
            "turn/started" -> {
                finalAgentMessage = null
                streamedAgentMessages.clear()
                if (pendingDirectSessionStart == null) {
                    val objective = currentObjective?.takeIf(String::isNotBlank)
                    if (objective != null) {
                        pendingDirectSessionStart = sessionController.prepareDirectSessionDraftForStart(
                            sessionId = sessionId,
                            objective = objective,
                            executionSettings = executionSettings,
                        )
                    }
                }
                false
            }
            "item/agentMessage/delta" -> {
                val itemId = params.optString("itemId")
                if (itemId.isNotBlank()) {
                    streamedAgentMessages.getOrPut(itemId, ::StringBuilder)
                        .append(params.optString("delta"))
                }
                false
            }
            "item/completed" -> {
                val item = params.optJSONObject("item")
                if (item?.optString("type") == "agentMessage") {
                    val itemId = item.optString("id")
                    val text = item.optString("text").ifBlank {
                        streamedAgentMessages[itemId]?.toString().orEmpty()
                    }
                    if (text.isNotBlank()) {
                        finalAgentMessage = text
                    }
                }
                false
            }
            "turn/completed" -> handleTurnCompleted(params.optJSONObject("turn") ?: JSONObject())
            else -> false
        }
    }

    private fun handleTurnCompleted(turn: JSONObject): Boolean {
        return when (turn.optString("status")) {
            "completed" -> {
                val objective = currentObjective?.takeIf(String::isNotBlank)
                    ?: run {
                        publishTrace("Planner turn completed without a captured objective.")
                        return false
                    }
                val pending = pendingDirectSessionStart
                    ?: run {
                        publishTrace("Planner turn completed before the direct session moved to RUNNING.")
                        return false
                    }
                val plannerText = finalAgentMessage?.takeIf(String::isNotBlank)
                    ?: run {
                        publishTrace("Planner turn completed without an assistant message.")
                        return false
                    }
                val plannerRequest = runCatching {
                    AgentTaskPlanner.parsePlannerResponse(
                        responseText = plannerText,
                        userObjective = objective,
                        isEligibleTargetPackage = ::isEligibleTargetPackage,
                    )
                }.getOrElse { err ->
                    publishTrace("Planner output rejected: ${err.message ?: err::class.java.simpleName}")
                    return false
                }
                runCatching {
                    sessionController.startDirectSessionChildren(
                        parentSessionId = sessionId,
                        geniePackage = pending.geniePackage,
                        plan = plannerRequest.plan,
                        allowDetachedMode = plannerRequest.allowDetachedMode,
                        executionSettings = executionSettings,
                    )
                }.onFailure { err ->
                    sessionController.failDirectSession(
                        sessionId,
                        "Failed to start planned child session: ${err.message ?: err::class.java.simpleName}",
                    )
                    publishTrace("Planner child start failed: ${err.message ?: err::class.java.simpleName}")
                }.onSuccess { result ->
                    val heldForInspection = currentDesktopProxy != null &&
                        DesktopInspectionRegistry.holdChildrenForAttachedPlanner(
                            parentSessionId = sessionId,
                            childSessionIds = result.childSessionIds,
                        )
                    if (heldForInspection) {
                        val childSummary = if (result.childSessionIds.size == 1) {
                            "child session ${result.childSessionIds.single()} is paused and attachable"
                        } else {
                            "child sessions ${result.childSessionIds.joinToString(", ")} are paused and attachable"
                        }
                        publishTrace(
                            "Planner completed; $childSummary. Attach ${if (result.childSessionIds.size == 1) "it" else "one of them"} from the desktop to continue while this planner remains attached.",
                        )
                    } else {
                        publishTrace(
                            "Planner completed; started child sessions ${result.childSessionIds.joinToString(", ")}.",
                        )
                    }
                    return false
                }
                false
            }
            "interrupted" -> {
                publishTrace("Planner turn interrupted; desktop attach remains active.")
                false
            }
            else -> {
                publishTrace(
                    turn.opt("error")?.toString()
                        ?: "Planner turn failed with status ${turn.optString("status", "unknown")}",
                )
                false
            }
        }
    }

    private fun isEligibleTargetPackage(packageName: String): Boolean {
        return sessionController.canStartSessionForTarget(packageName) && packageName !in DISALLOWED_TARGET_PACKAGES
    }

    private fun publishTrace(message: String) {
        val agentManager = context.getSystemService(AgentManager::class.java) ?: return
        runCatching {
            agentManager.publishTrace(sessionId, message)
        }.onFailure { err ->
            Log.w(TAG, "Failed to publish planner desktop trace for $sessionId", err)
        }
    }

    private fun forwardResponsesRequest(requestBody: String): AgentResponsesProxy.HttpResponse {
        val agentManager = context.getSystemService(AgentManager::class.java)
            ?: throw IOException("AgentManager unavailable for framework session transport")
        return AgentResponsesProxy.sendResponsesRequestThroughFramework(
            agentManager = agentManager,
            sessionId = sessionId,
            context = context,
            requestBody = requestBody,
        )
    }

    private fun request(
        method: String,
        params: JSONObject,
    ): JSONObject {
        val requestId = "host-${requestIdSequence.getAndIncrement()}"
        val responseQueue = LinkedBlockingQueue<JSONObject>(1)
        pendingResponses[requestId] = responseQueue
        try {
            sendMessage(
                JSONObject()
                    .put("id", requestId)
                    .put("method", method)
                    .put("params", params),
            )
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
        synchronized(writerLock) {
            writer.write(message.toString())
            writer.newLine()
            writer.flush()
        }
    }

    private fun startStdoutPump() {
        stdoutThread = thread(name = "AgentPlannerDesktopStdout-$sessionId") {
            try {
                process.inputStream.bufferedReader().useLines { lines ->
                    lines.forEach { line ->
                        if (line.isBlank()) {
                            return@forEach
                        }
                        val message = runCatching { JSONObject(line) }
                            .getOrElse { err ->
                                Log.w(TAG, "Failed to parse planner desktop stdout line", err)
                                return@forEach
                            }
                        routeInbound(line, message)
                    }
                }
            } catch (err: InterruptedIOException) {
                if (!closing.get()) {
                    Log.w(TAG, "Planner desktop stdout interrupted unexpectedly", err)
                }
            } catch (err: IOException) {
                if (!closing.get()) {
                    Log.w(TAG, "Planner desktop stdout failed", err)
                }
            }
        }
    }

    private fun startStderrPump() {
        stderrThread = thread(name = "AgentPlannerDesktopStderr-$sessionId") {
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
                    Log.w(TAG, "Planner desktop stderr interrupted unexpectedly", err)
                }
            } catch (err: IOException) {
                if (!closing.get()) {
                    Log.w(TAG, "Planner desktop stderr failed", err)
                }
            }
        }
    }

    private fun routeInbound(
        rawMessage: String,
        message: JSONObject,
    ) {
        if (message.has("id") && !message.has("method")) {
            val requestId = message.get("id").toString()
            pendingResponses[requestId]?.offer(message)
            val remoteRequest = remotePendingRequests.remove(requestId)
            if (remoteRequest != null) {
                sendDesktopMessage(
                    JSONObject(message.toString())
                        .put("id", remoteRequest.remoteRequestId)
                        .toString(),
                    remoteRequest.connectionId,
                )
            }
            return
        }
        if (message.has("method") && !message.has("id")) {
            forwardRemoteNotification(rawMessage, message)
        }
        inboundMessages.offer(message)
    }

    private fun handleRemoteProxyMessage(message: String) {
        val json = runCatching { JSONObject(message) }
            .getOrElse { err ->
                sendDesktopMessage(
                    errorResponse(
                        requestId = null,
                        code = -32700,
                        message = err.message ?: "Invalid remote JSON-RPC message",
                    ),
                )
                return
            }
        when {
            json.has("method") && json.has("id") -> handleRemoteProxyRequest(json)
            json.has("method") -> handleRemoteProxyNotification(json)
            else -> Unit
        }
    }

    private fun handleRemoteProxyRequest(message: JSONObject) {
        val method = message.optString("method")
        val remoteRequestId = message.opt("id") ?: return
        when (method) {
            "initialize" -> {
                val params = message.optJSONObject("params") ?: JSONObject()
                val optOut = params
                    .optJSONObject("capabilities")
                    ?.optJSONArray("optOutNotificationMethods")
                    ?.toStringSet()
                    .orEmpty()
                val connectionId = checkNotNull(currentDesktopProxy?.connectionId) {
                    "Desktop proxy is unavailable during initialize"
                }
                remoteProxyState = RemoteProxyState(
                    connectionId = connectionId,
                    optOutNotificationMethods = optOut,
                )
                lastReportedFrameworkEventCount = 0
                sendDesktopMessage(
                    JSONObject()
                        .put("id", remoteRequestId)
                        .put(
                            "result",
                            JSONObject()
                                .put("userAgent", "android_agent_planner_bridge/$REMOTE_SERVER_VERSION")
                                .put("codexHome", codexHome.absolutePath)
                                .put("platformFamily", "unix")
                                .put("platformOs", "android"),
                        )
                        .toString(),
                    connectionId,
                )
            }
            "account/read" -> {
                sendDesktopMessage(
                    JSONObject()
                        .put("id", remoteRequestId)
                        .put("result", buildRemoteAccountReadResult())
                        .toString(),
                )
            }
            "turn/start" -> {
                val params = message.optJSONObject("params") ?: JSONObject()
                currentObjective = extractTurnPrompt(params)
                forwardRemoteRequest(message, remoteRequestId)
            }
            else -> {
                forwardRemoteRequest(message, remoteRequestId)
            }
        }
    }

    private fun forwardRemoteRequest(
        message: JSONObject,
        remoteRequestId: Any,
    ) {
        val connectionId = currentDesktopProxy?.connectionId
        if (connectionId.isNullOrBlank()) {
            sendDesktopMessage(
                errorResponse(remoteRequestId, -32000, "Remote desktop session is not attached"),
            )
            return
        }
        val forwardedRequestId = "$REMOTE_REQUEST_ID_PREFIX$connectionId:${message.get("id")}"
        remotePendingRequests[forwardedRequestId] = RemotePendingRequest(
            connectionId = connectionId,
            remoteRequestId = remoteRequestId,
        )
        sendMessage(
            JSONObject(message.toString())
                .put("id", forwardedRequestId),
        )
    }

    private fun handleRemoteProxyNotification(message: JSONObject) {
        when (message.optString("method")) {
            "initialized" -> maybeEmitFrameworkEvents()
            else -> sendMessage(JSONObject(message.toString()))
        }
    }

    private fun maybeEmitFrameworkEvents() {
        val proxyState = remoteProxyState ?: return
        if (proxyState.connectionId != currentDesktopProxy?.connectionId) {
            return
        }
        if (
            proxyState.optOutNotificationMethods.contains(
                FrameworkEventBridge.THREAD_FRAMEWORK_EVENT_METHOD,
            )
        ) {
            return
        }
        val threadId = currentThreadId ?: return
        val agentManager = context.getSystemService(AgentManager::class.java) ?: return
        val events = runCatching {
            agentManager.getSessionEvents(sessionId)
        }.getOrElse { err ->
            Log.w(TAG, "Failed to load framework events for planner $sessionId", err)
            return
        }
        if (lastReportedFrameworkEventCount > events.size) {
            lastReportedFrameworkEventCount = 0
        }
        for (index in lastReportedFrameworkEventCount until events.size) {
            val notification = FrameworkEventBridge.buildThreadFrameworkEventNotification(
                threadId = threadId,
                event = events[index],
            ) ?: continue
            sendDesktopMessage(notification, proxyState.connectionId)
        }
        lastReportedFrameworkEventCount = events.size
    }

    private fun buildRemoteAccountReadResult(): JSONObject {
        val authenticated = runtimeStatus?.authenticated == true
        val account = if (authenticated) {
            JSONObject().put("type", "apiKey")
        } else {
            JSONObject.NULL
        }
        return JSONObject()
            .put("account", account)
            .put("requiresOpenaiAuth", true)
    }

    private fun extractTurnPrompt(params: JSONObject): String? {
        val input = params.optJSONArray("input") ?: return null
        val text = buildString {
            for (index in 0 until input.length()) {
                val item = input.optJSONObject(index) ?: continue
                if (item.optString("type") != "text") {
                    continue
                }
                val value = item.optString("text").trim()
                if (value.isEmpty()) {
                    continue
                }
                if (isNotEmpty()) {
                    append('\n')
                }
                append(value)
            }
        }.trim()
        return text.ifEmpty { null }
    }

    private fun forwardRemoteNotification(
        rawMessage: String,
        message: JSONObject,
    ) {
        val proxyState = remoteProxyState ?: return
        if (proxyState.connectionId != currentDesktopProxy?.connectionId) {
            return
        }
        val method = message.optString("method")
        if (proxyState.optOutNotificationMethods.contains(method)) {
            return
        }
        sendDesktopMessage(rawMessage, proxyState.connectionId)
    }

    private fun sendDesktopMessage(
        message: String,
        connectionId: String? = currentDesktopProxy?.connectionId,
    ) {
        val proxy = currentDesktopProxy
        if (proxy == null || connectionId == null || proxy.connectionId != connectionId) {
            return
        }
        pendingDesktopMessages.offer(
            PendingDesktopMessage(
                connectionId = connectionId,
                message = message,
            ),
        )
    }

    private fun errorResponse(
        requestId: Any?,
        code: Int,
        message: String,
    ): String {
        return JSONObject()
            .put("id", requestId)
            .put(
                "error",
                JSONObject()
                    .put("code", code)
                    .put("message", message),
            )
            .toString()
    }

    private fun JSONArray.toStringSet(): Set<String> {
        return buildSet {
            for (index in 0 until length()) {
                optString(index).takeIf(String::isNotBlank)?.let(::add)
            }
        }
    }

    private fun dispatchDesktopMessages() {
        while (!closing.get()) {
            val pending = try {
                pendingDesktopMessages.take()
            } catch (_: InterruptedException) {
                return
            }
            val proxy = currentDesktopProxy
            if (proxy == null || proxy.connectionId != pending.connectionId) {
                continue
            }
            runCatching {
                proxy.onMessage(pending.message)
            }.onFailure { err ->
                Log.w(TAG, "Failed to deliver planner desktop message for $sessionId", err)
                closeDesktopProxy(
                    connectionId = pending.connectionId,
                    reason = err.message ?: err::class.java.simpleName,
                    detachPlanner = false,
                )
            }
        }
    }
}
