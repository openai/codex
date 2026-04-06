package com.openai.codex.genie

import android.app.agent.AgentSessionInfo
import android.app.agent.GenieRequest
import android.app.agent.GenieService
import android.content.Context
import android.util.Log
import com.openai.codex.bridge.CodexHomeRetention
import com.openai.codex.bridge.DesktopSessionBootstrap
import com.openai.codex.bridge.DetachedTargetCompat
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
import java.util.concurrent.atomic.AtomicInteger
import org.json.JSONArray
import org.json.JSONObject

class CodexAppServerHost(
    private val context: Context,
    private val request: GenieRequest,
    private val callback: GenieService.Callback,
    private val control: GenieSessionControl,
    private val bridgeClient: AgentBridgeClient,
    private val runtimeStatus: CodexAgentBridge.RuntimeStatus,
    private val startupContextNotes: List<String> = emptyList(),
) : Closeable {
    companion object {
        private const val TAG = "CodexAppServerHost"
        private const val APP_SERVER_BRIDGE_ENV_VAR = "CODEX_OPENAI_APP_SERVER_BRIDGE"
        private const val REQUEST_TIMEOUT_MS = 30_000L
        private const val POLL_TIMEOUT_MS = 250L
        private const val DEFAULT_HOSTED_MODEL = "gpt-5.3-codex"
        private const val REMOTE_REQUEST_ID_PREFIX = "remote:"
        private const val REMOTE_SERVER_VERSION = "0.1.0"
        private const val MAX_IO_RECOVERY_ATTEMPTS = 3
    }

    private enum class RecoveryStartMode {
        NORMAL,
        IDLE_ATTACH,
        AUTO_CONTINUE,
    }

    private data class RemoteProxyState(
        val connectionId: String,
        val optOutNotificationMethods: Set<String>,
    )

    private data class RemotePendingRequest(
        val connectionId: String,
        val remoteRequestId: Any,
    )

    private data class PendingTerminalTransition(
        val terminalState: Int,
        val errorMessage: String? = null,
    )

    private data class FrameworkEventRecord(
        val eventType: String,
        val message: String,
    )

    private val requestIdSequence = AtomicInteger(1)
    private val pendingResponses = ConcurrentHashMap<String, LinkedBlockingQueue<JSONObject>>()
    private val remotePendingRequests = ConcurrentHashMap<String, RemotePendingRequest>()
    private val inboundMessages = LinkedBlockingQueue<JSONObject>()
    private val writerLock = Any()
    private val streamedAgentMessages = mutableMapOf<String, StringBuilder>()
    private val frameworkEventLock = Any()
    private val frameworkEventHistory = mutableListOf<FrameworkEventRecord>()

    private lateinit var process: Process
    private lateinit var writer: BufferedWriter
    private lateinit var codexHome: File
    private lateinit var executionSettings: SessionExecutionSettings
    private var stdoutThread: Thread? = null
    private var stderrThread: Thread? = null
    private var finalAgentMessage: String? = null
    private var resultPublished = false
    private var pendingTerminalTransition: PendingTerminalTransition? = null
    @Volatile
    private var activeThreadId: String? = null
    @Volatile
    private var remoteProxyState: RemoteProxyState? = null
    private var announcedStagedPromptAwaitingDesktopInput = false
    private var announcedRecoveryContextAwaitingDesktopInput = false
    private val idleDesktopAttachSession = DesktopSessionBootstrap.isIdleAttachPrompt(request.prompt)
    private val stagedDelegatedPrompt = DesktopSessionBootstrap.stagedInitialPrompt(request.prompt)
    private var initialTurnStarted = false
    private var pendingRecoveryContext: String? = null
    private var recoveryStartMode = RecoveryStartMode.NORMAL
    private var exhaustedRecoveryPauseAttempted = false

    fun run() {
        bridgeClient.setAppServerProxyHandler(
            object : AgentBridgeClient.AppServerProxyHandler {
                override fun onMessage(
                    connectionId: String,
                    message: String,
                ) {
                    handleRemoteProxyMessage(connectionId, message)
                }

                override fun onClosed(
                    connectionId: String?,
                    reason: String?,
                ) {
                    if (connectionId == null || remoteProxyState?.connectionId == connectionId) {
                        remoteProxyState = null
                    }
                    if (!reason.isNullOrBlank()) {
                        runCatching {
                            publishFrameworkTrace("Desktop attach closed: $reason")
                        }.onFailure { err ->
                            Log.w(TAG, "Failed to publish desktop attach close trace for ${request.sessionId}", err)
                        }
                    }
                }
            },
        )
        executionSettings = bridgeClient.readSessionExecutionSettings()
        val model = resolveModel()
        var recoveryAttempts = 0
        while (!control.cancelled) {
            val startMode = recoveryStartMode
            val recoveryAttempt = recoveryAttempts
            val startupPrompt = when (startMode) {
                RecoveryStartMode.NORMAL -> {
                    if (idleDesktopAttachSession) {
                        null
                    } else {
                        buildDelegatedPrompt()
                    }
                }
                RecoveryStartMode.IDLE_ATTACH -> null
                RecoveryStartMode.AUTO_CONTINUE ->
                    pendingRecoveryContext
                        ?: buildRecoveryPrompt(recoveryAttempts)
            }
            try {
                startHostedRuntime(
                    model = model,
                    startupPrompt = startupPrompt,
                    recoveryAttempt = recoveryAttempt,
                    startMode = startMode,
                )
                recoveryAttempts = 0
                exhaustedRecoveryPauseAttempted = false
                eventLoop(model)
                return
            } catch (err: InterruptedException) {
                if (control.cancelled) {
                    throw IOException("Cancelled", err)
                }
                recoveryAttempts += 1
                if (recoveryAttempts > MAX_IO_RECOVERY_ATTEMPTS) {
                    if (
                        pauseAfterRecoveryExhaustion(
                            recoveryAttempt = recoveryAttempts,
                            err = IOException(
                                "Hosted session interrupted unexpectedly: ${err.message ?: err::class.java.simpleName}",
                                err,
                            ),
                        )
                    ) {
                        continue
                    }
                    throw IOException(
                        "Exceeded $MAX_IO_RECOVERY_ATTEMPTS recoverable app-server restarts after interruption: ${err.message}",
                        err,
                    )
                }
                recoverFromIoFailure(
                    IOException(
                        "Hosted session interrupted unexpectedly: ${err.message ?: err::class.java.simpleName}",
                        err,
                    ),
                    recoveryAttempts,
                )
            } catch (err: IOException) {
                if (control.cancelled) {
                    throw err
                }
                recoveryAttempts += 1
                if (recoveryAttempts > MAX_IO_RECOVERY_ATTEMPTS) {
                    if (pauseAfterRecoveryExhaustion(recoveryAttempts, err)) {
                        continue
                    }
                    throw IOException(
                        "Exceeded $MAX_IO_RECOVERY_ATTEMPTS recoverable app-server I/O restarts: ${err.message}",
                        err,
                    )
                }
                recoverFromIoFailure(err, recoveryAttempts)
            }
        }
        throw IOException("Cancelled")
    }

    override fun close() {
        shutdownHostedRuntime("Genie session ended")
        bridgeClient.setAppServerProxyHandler(null)
    }

    private fun startProcess() {
        val codexHomeRoot = File(context.cacheDir, "codex-home")
        codexHome = File(codexHomeRoot, request.sessionId).apply {
            deleteRecursively()
            mkdirs()
        }
        CodexHomeRetention.markActive(codexHome)
        CodexHomeRetention.pruneSessionHomes(
            root = codexHomeRoot,
            keepHomeNames = setOf(request.sessionId),
        )
        HostedCodexConfig.installAgentsFile(codexHome, bridgeClient.readInstalledAgentsMarkdown())
        val processBuilder = ProcessBuilder(
            listOf(
                CodexBinaryLocator.resolve(context).absolutePath,
                "-c",
                "enable_request_compression=false",
                "-c",
                "features.default_mode_request_user_input=true",
                "app-server",
                "--listen",
                "stdio://",
            ),
        )
        val env = processBuilder.environment()
        env["CODEX_HOME"] = codexHome.absolutePath
        env[APP_SERVER_BRIDGE_ENV_VAR] = "1"
        env["RUST_LOG"] = "warn"
        if (request.isDetachedModeAllowed) {
            DetachedSessionCommandShims.installAndConfigureEnvironment(
                codexHome = codexHome,
                environment = env,
                targetPackage = request.targetPackage,
            )
        }
        process = processBuilder.start()
        control.process = process
        writer = process.outputStream.bufferedWriter()
        startStdoutPump()
        startStderrPump()
    }

    private fun startStdoutPump() {
        stdoutThread = Thread {
            try {
                process.inputStream.bufferedReader().useLines { lines ->
                    lines.forEach { line ->
                        if (line.isBlank()) {
                            return@forEach
                        }
                        val message = runCatching { JSONObject(line) }
                            .getOrElse { err ->
                                Log.w(TAG, "Failed to parse codex app-server stdout line", err)
                                return@forEach
                            }
                        routeInbound(line, message)
                    }
                }
            } catch (_: InterruptedIOException) {
                // Expected when the hosted app-server exits and the stream closes underneath the reader.
            } catch (err: IOException) {
                if (!control.cancelled && process.isAlive) {
                    Log.w(TAG, "Stdout pump failed for ${request.sessionId}", err)
                }
            }
        }.also {
            it.name = "CodexAppServerStdout-${request.sessionId}"
            it.start()
        }
    }

    private fun startHostedRuntime(
        model: String,
        startupPrompt: String?,
        recoveryAttempt: Int,
        startMode: RecoveryStartMode,
    ) {
        finalAgentMessage = null
        pendingTerminalTransition = null
        streamedAgentMessages.clear()
        announcedStagedPromptAwaitingDesktopInput = false
        announcedRecoveryContextAwaitingDesktopInput = false
        initialTurnStarted = false
        startProcess()
        initialize()
        val threadId = startThread(model)
        activeThreadId = threadId
        bridgeClient.registerAppServerThread(threadId)
        if (startupPrompt != null) {
            startTurn(threadId, model, startupPrompt)
            initialTurnStarted = true
        }
        when {
            recoveryAttempt > 0 && startMode == RecoveryStartMode.IDLE_ATTACH -> {
                updateFrameworkState(AgentSessionInfo.STATE_WAITING_FOR_USER)
            }
            recoveryAttempt > 0 && startMode == RecoveryStartMode.AUTO_CONTINUE -> {
                updateFrameworkState(AgentSessionInfo.STATE_RUNNING)
            }
        }
        val hostedThreadMessage = when {
            recoveryAttempt == 0 && idleDesktopAttachSession ->
                "Hosted idle codex app-server thread $threadId for ${request.targetPackage}."
            recoveryAttempt == 0 ->
                "Hosted codex app-server thread $threadId for ${request.targetPackage}."
            startMode == RecoveryStartMode.IDLE_ATTACH ->
                "Recovered hosted idle codex app-server thread $threadId after a recoverable I/O error. Reattach and send the next prompt to continue."
            startupPrompt == null ->
                "Recovered hosted idle codex app-server thread $threadId after a recoverable I/O error."
            else ->
                "Recovered hosted codex app-server thread $threadId after a recoverable I/O error."
        }
        publishFrameworkTrace(hostedThreadMessage)
        if (recoveryAttempt == 0 && stagedDelegatedPrompt != null) {
            publishFrameworkTrace(
                "A delegated objective is staged for this Genie, but the first turn is paused while desktop attach inspection remains active.",
            )
        }
        if (recoveryAttempt > 0 && startMode == RecoveryStartMode.IDLE_ATTACH) {
            publishFrameworkTrace(
                "Recovery context is staged for the next attached turn. Review the latest framework error/trace history after reattaching, then send a recovery prompt.",
            )
        }
        if (startMode != RecoveryStartMode.IDLE_ATTACH) {
            pendingRecoveryContext = null
        }
        recoveryStartMode = RecoveryStartMode.NORMAL
    }

    private fun recoverFromIoFailure(
        err: IOException,
        recoveryAttempt: Int,
    ) {
        val errorMessage = err.message ?: err::class.java.simpleName
        val recoveryContext = buildRecoveryPrompt(recoveryAttempt)
        Log.i(
            TAG,
            "Recoverable hosted I/O failure for ${request.sessionId}: attempt=$recoveryAttempt attached=${hasActiveRemoteDesktopAttach()} error=$errorMessage",
        )
        publishRecoverableFrameworkError("Recoverable hosted I/O error: $errorMessage")
        if (hasActiveRemoteDesktopAttach()) {
            pendingRecoveryContext = recoveryContext
            recoveryStartMode = RecoveryStartMode.IDLE_ATTACH
            publishFrameworkTrace(
                "Recoverable hosted I/O error during attached session. The current desktop attach will close, but this Genie will restart into an attachable idle thread with staged recovery context. Reattach and continue from there.",
            )
            shutdownHostedRuntime("Recoverable hosted I/O error: $errorMessage")
        } else {
            pendingRecoveryContext = recoveryContext
            recoveryStartMode = RecoveryStartMode.AUTO_CONTINUE
            publishFrameworkTrace(
                "Recoverable hosted I/O error while running unattached. Restarting the hosted app-server and continuing automatically (attempt $recoveryAttempt/$MAX_IO_RECOVERY_ATTEMPTS).",
            )
            shutdownHostedRuntime(null)
        }
    }

    private fun pauseAfterRecoveryExhaustion(
        recoveryAttempt: Int,
        err: IOException,
    ): Boolean {
        if (exhaustedRecoveryPauseAttempted) {
            return false
        }
        exhaustedRecoveryPauseAttempted = true
        val errorMessage = err.message ?: err::class.java.simpleName
        pendingRecoveryContext = buildRecoveryPrompt(recoveryAttempt)
        recoveryStartMode = RecoveryStartMode.IDLE_ATTACH
        publishRecoverableFrameworkError(
            "Hosted I/O recovery attempts exhausted: $errorMessage",
        )
        publishFrameworkTrace(
            "Automatic hosted recovery attempts were exhausted. This Genie will stay alive and restart into an attachable idle recovery thread so the next turn can inspect the failure and continue manually.",
        )
        shutdownHostedRuntime("Hosted I/O recovery attempts exhausted: $errorMessage")
        return true
    }

    private fun buildRecoveryPrompt(recoveryAttempt: Int): String {
        val priorObjective = stagedDelegatedPrompt ?: request.prompt
        return """
            The previous hosted app-server encountered a recoverable I/O error and was restarted.
            Recovery attempt: $recoveryAttempt of $MAX_IO_RECOVERY_ATTEMPTS.
            Prior objective context:
            $priorObjective

            Continue the session from the current framework state.
            Review the latest framework traces and current target state before taking more actions.
            If the interrupted step may have partially completed, verify state first instead of repeating it blindly.
            Do not assume that the previous command or tool call completed successfully.
        """.trimIndent()
    }

    private fun shutdownHostedRuntime(reason: String?) {
        if (reason != null) {
            bridgeClient.closeRemoteAppServer(reason)
        }
        stdoutThread?.interrupt()
        stdoutThread = null
        stderrThread?.interrupt()
        stderrThread = null
        synchronized(writerLock) {
            if (::writer.isInitialized) {
                runCatching { writer.close() }
            }
        }
        if (::process.isInitialized) {
            runCatching { process.destroy() }
        }
        if (::codexHome.isInitialized) {
            CodexHomeRetention.clearActive(codexHome)
            runCatching { codexHome.deleteRecursively() }
            runCatching {
                CodexHomeRetention.pruneSessionHomes(
                    root = checkNotNull(codexHome.parentFile),
                    keepHomeNames = emptySet(),
                )
            }
        }
        control.process = null
        activeThreadId = null
        remoteProxyState = null
        pendingResponses.clear()
        remotePendingRequests.clear()
        inboundMessages.clear()
        streamedAgentMessages.clear()
        pendingTerminalTransition = null
        finalAgentMessage = null
        initialTurnStarted = false
        announcedStagedPromptAwaitingDesktopInput = false
        announcedRecoveryContextAwaitingDesktopInput = false
    }

    private fun startStderrPump() {
        stderrThread = Thread {
            try {
                process.errorStream.bufferedReader().useLines { lines ->
                    lines.forEach { line ->
                        if (line.isBlank()) {
                            return@forEach
                        }
                        when {
                            line.contains(" ERROR ") -> Log.e(TAG, line)
                            line.contains(" WARN ") || line.startsWith("WARNING:") -> Log.w(TAG, line)
                        }
                    }
                }
            } catch (_: InterruptedIOException) {
                // Expected when the hosted app-server exits and the stream closes underneath the reader.
            } catch (err: IOException) {
                if (!control.cancelled && process.isAlive) {
                    Log.w(TAG, "Stderr pump failed for ${request.sessionId}", err)
                }
            }
        }.also {
            it.name = "CodexAppServerStderr-${request.sessionId}"
            it.start()
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
                bridgeClient.sendRemoteAppServerMessage(
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

    private fun initialize() {
        request(
            method = "initialize",
            params = JSONObject()
                .put(
                    "clientInfo",
                    JSONObject()
                        .put("name", "android_genie")
                        .put("title", "Android Genie")
                        .put("version", "0.1.0"),
                )
                .put(
                    "capabilities",
                    JSONObject().put("experimentalApi", true),
                ),
        )
        notify("initialized", JSONObject())
    }

    private fun startThread(model: String): String {
        val params = JSONObject()
            .put("approvalPolicy", "never")
            .put("sandbox", "read-only")
            .put("cwd", context.filesDir.absolutePath)
            .put("serviceName", "android_genie")
            .put("baseInstructions", buildBaseInstructions())
            .put("dynamicTools", buildDynamicToolSpecs())
        params.put("model", model)
        val result = request(
            method = "thread/start",
            params = params,
        )
        return result.getJSONObject("thread").getString("id")
    }

    private fun startTurn(
        threadId: String,
        model: String,
        prompt: String,
    ) {
        Log.i(TAG, "Starting hosted turn for ${request.sessionId} with model=$model")
        request(
            method = "turn/start",
            params = JSONObject()
                .put("threadId", threadId)
                .put("model", model)
                .apply {
                    executionSettings.reasoningEffort
                        ?.takeIf(String::isNotBlank)
                        ?.let { put("effort", it) }
                }
                .put(
                    "input",
                    JSONArray().put(
                        JSONObject()
                            .put("type", "text")
                            .put("text", prompt),
                    ),
                ),
        )
    }

    private fun resolveModel(): String = executionSettings.model
        ?.takeIf(String::isNotBlank)
        ?: runtimeStatus.configuredModel
            ?.takeIf(String::isNotBlank)
        ?: runtimeStatus.effectiveModel
            ?.takeIf(String::isNotBlank)
        ?: DEFAULT_HOSTED_MODEL

    private fun eventLoop(model: String) {
        while (!control.cancelled) {
            val message = inboundMessages.poll(POLL_TIMEOUT_MS, TimeUnit.MILLISECONDS)
            if (message == null) {
                maybeReleaseStagedDelegatedTurn(model)
                maybeApplyPendingTerminalTransition()?.let { return }
                if (!process.isAlive) {
                    throw IOException("codex app-server exited with code ${process.exitValue()}")
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
        throw IOException("Cancelled")
    }

    private fun handleServerRequest(message: JSONObject) {
        val method = message.getString("method")
        val requestId = message.get("id")
        val params = message.optJSONObject("params") ?: JSONObject()
        Log.i(TAG, "Handling app-server request method=$method session=${request.sessionId}")
        when (method) {
            "item/tool/call" -> handleDynamicToolCall(requestId, params)
            "item/tool/requestUserInput" -> handleRequestUserInput(requestId, params)
            "response/send" -> handleResponsesBridgeRequest(requestId, params)
            else -> {
                publishFrameworkTrace("Unsupported codex app-server request: $method")
                sendError(
                    requestId = requestId,
                    code = -32601,
                    message = "Unsupported app-server request: $method",
                )
            }
        }
    }

    private fun handleDynamicToolCall(
        requestId: Any,
        params: JSONObject,
    ) {
        val toolName = params.optString("tool").trim()
        val arguments = params.optJSONObject("arguments") ?: JSONObject()
        Log.i(TAG, "Executing dynamic tool $toolName arguments=$arguments")
        val toolExecutor = AndroidGenieToolExecutor(
            callback = callback,
            sessionId = request.sessionId,
        )
        val observation = runCatching {
            toolExecutor.execute(toolName, arguments)
        }.getOrElse { err ->
            GenieToolObservation(
                name = toolName.ifBlank { "unknown" },
                summary = "Tool $toolName failed: ${err.message}",
                promptDetails = "Tool $toolName failed.\nError: ${err.message ?: err::class.java.simpleName}",
            )
        }
        publishFrameworkTrace(observation.summary)
        sendResult(
            requestId = requestId,
            result = JSONObject()
                .put("success", !observation.summary.contains(" failed:"))
                .put("contentItems", buildDynamicToolContentItems(observation)),
        )
    }

    private fun handleRequestUserInput(
        requestId: Any,
        params: JSONObject,
    ) {
        val questions = params.optJSONArray("questions") ?: JSONArray()
        val renderedQuestion = renderAgentQuestion(questions)
        Log.i(TAG, "Requesting Agent input for ${request.sessionId}: $renderedQuestion")
        publishFrameworkQuestion(renderedQuestion)
        updateFrameworkState(AgentSessionInfo.STATE_WAITING_FOR_USER)
        val answer = control.waitForUserResponse()
        updateFrameworkState(AgentSessionInfo.STATE_RUNNING)
        publishFrameworkTrace("Received Agent answer for ${request.targetPackage}.")
        Log.i(TAG, "Received Agent input for ${request.sessionId}: ${answer.take(160)}")
        sendResult(
            requestId = requestId,
            result = JSONObject().put("answers", buildQuestionAnswers(questions, answer)),
        )
    }

    private fun handleResponsesBridgeRequest(
        requestId: Any,
        params: JSONObject,
    ) {
        val requestBody = params.optString("requestBody")
        publishFrameworkTrace(
            "Framework transport executing POST ${runtimeStatus.frameworkResponsesPath} via Agent bridge.",
        )
        val httpResponse = bridgeClient.sendResponsesRequest(requestBody)
        publishFrameworkTrace(
            "Framework transport completed ${httpResponse.statusCode} for ${runtimeStatus.frameworkResponsesPath}.",
        )
        sendResult(
            requestId = requestId,
            result = JSONObject()
                .put("statusCode", httpResponse.statusCode)
                .put("body", httpResponse.body),
        )
    }

    private fun handleNotification(message: JSONObject): Boolean {
        val method = message.getString("method")
        val params = message.optJSONObject("params") ?: JSONObject()
        return when (method) {
            "turn/started" -> {
                resultPublished = false
                finalAgentMessage = null
                pendingTerminalTransition = null
                streamedAgentMessages.clear()
                initialTurnStarted = true
                pendingRecoveryContext = null
                publishFrameworkTrace("codex turn started for ${request.targetPackage}.")
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
            "item/started" -> {
                runCatching {
                    publishItemStartedTrace(params.optJSONObject("item"))
                }.onFailure { err ->
                    recordNonFatalObserverFailure("item/started", err)
                }
                false
            }
            "item/completed" -> {
                runCatching {
                    captureCompletedItem(params.optJSONObject("item"))
                }.onFailure { err ->
                    recordNonFatalObserverFailure("item/completed", err)
                }
                false
            }
            "turn/completed" -> {
                finishTurn(params)
            }
            else -> false
        }
    }

    private fun publishItemStartedTrace(item: JSONObject?) {
        if (item == null) {
            return
        }
        val command = commandForItem(item)
        Log.i(
            TAG,
            "item/started type=${item.optString("type")} tool=${item.optString("tool")} command=${command ?: ""}",
        )
        when (item.optString("type")) {
            "dynamicToolCall" -> {
                val tool = item.optString("tool")
                publishFrameworkTrace("Codex requested dynamic tool $tool.")
            }
            "commandExecution" -> {
                if (
                    request.isDetachedModeAllowed &&
                    command != null &&
                    DetachedSessionGuard.isForbiddenTargetLaunchCommand(command, request.targetPackage)
                ) {
                    publishFrameworkTrace(
                        "Detached-session guard blocked a shell relaunch attempt for ${request.targetPackage}. The command will fail with a policy error that Codex should use to recover instead of retrying the relaunch.",
                    )
                }
                publishFrameworkTrace(
                    "Codex started command execution: ${command ?: "command"}",
                )
            }
        }
    }

    private fun captureCompletedItem(item: JSONObject?) {
        if (item == null) {
            return
        }
        val command = commandForItem(item)
        val errorDetail = item.optString("aggregatedOutput").ifBlank {
            item.optString("stderr").ifBlank {
                item.optString("output").ifBlank {
                    item.optString("error")
                }
            }
        }.trim()
        Log.i(
            TAG,
            "item/completed type=${item.optString("type")} status=${item.optString("status")} tool=${item.optString("tool")} command=${command ?: ""} error=${errorDetail.take(200)}",
        )
        when (item.optString("type")) {
            "agentMessage" -> {
                val itemId = item.optString("id")
                val text = item.optString("text").ifBlank {
                    streamedAgentMessages[itemId]?.toString().orEmpty()
                }
                if (text.isNotBlank()) {
                    finalAgentMessage = text
                }
            }
            "commandExecution" -> {
                val status = item.optString("status")
                val exitCode = if (item.has("exitCode")) item.opt("exitCode") else null
                val resolvedCommand = command ?: "command"
                if (status == "failed") {
                    Log.i(TAG, "Failed command item=${item}")
                    val detailSuffix = errorDetail
                        .takeIf(String::isNotBlank)
                        ?.let { " Details: ${it.take(240)}" }
                        .orEmpty()
                    publishFrameworkTrace(
                        "Command failed: $resolvedCommand (status=$status, exitCode=${exitCode ?: "unknown"}).$detailSuffix",
                    )
                    if (errorDetail.contains("package=com.android.shell does not belong to uid=")) {
                        publishFrameworkTrace(
                            "This shell command requires com.android.shell privileges. The target is already running hidden; use detached-target dynamic tools to show or inspect it instead of retrying the same shell launch surface.",
                        )
                    }
                } else {
                    publishFrameworkTrace(
                        "Command completed: $resolvedCommand (status=$status, exitCode=${exitCode ?: "unknown"}).",
                    )
                }
            }
            "dynamicToolCall" -> {
                val tool = item.optString("tool")
                val status = item.optString("status")
                publishFrameworkTrace("Dynamic tool $tool completed with status=$status.")
            }
        }
    }

    private fun commandForItem(item: JSONObject): String? {
        return item.optString("command")
            .takeIf(String::isNotBlank)
            ?: item.optJSONArray("command")?.join(" ")
    }

    private fun finishTurn(params: JSONObject): Boolean {
        val turn = params.optJSONObject("turn") ?: JSONObject()
        Log.i(TAG, "turn/completed status=${turn.optString("status")} error=${turn.opt("error")}")
        when (turn.optString("status")) {
            "completed" -> {
                val resultText = finalAgentMessage?.takeIf(String::isNotBlank)
                    ?: "Genie completed without a final assistant message."
                publishResultOnce(resultText)
                return deferOrFinishTurn(
                    pendingTransition = PendingTerminalTransition(
                        terminalState = AgentSessionInfo.STATE_COMPLETED,
                    ),
                    keepOpenTrace = "Turn completed; desktop attach remains active for follow-up control.",
                )
            }
            "interrupted" -> {
                return deferOrFinishTurn(
                    pendingTransition = PendingTerminalTransition(
                        terminalState = AgentSessionInfo.STATE_CANCELLED,
                        errorMessage = "Genie turn interrupted",
                    ),
                    keepOpenTrace = "Turn interrupted; desktop attach remains active for follow-up control.",
                )
            }
            else -> {
                val errorDetail = turn.opt("error")?.toString()
                    ?: "Genie turn failed with status ${turn.optString("status", "unknown")}"
                return deferOrFinishTurn(
                    pendingTransition = PendingTerminalTransition(
                        terminalState = AgentSessionInfo.STATE_FAILED,
                        errorMessage = errorDetail,
                    ),
                    keepOpenTrace = "Turn failed; desktop attach remains active for inspection and follow-up control.",
                )
            }
        }
    }

    private fun deferOrFinishTurn(
        pendingTransition: PendingTerminalTransition,
        keepOpenTrace: String,
    ): Boolean {
        if (shouldKeepSessionOpenAfterTurn()) {
            pendingTerminalTransition = pendingTransition
            publishFrameworkTrace(keepOpenTrace)
            return false
        }
        applyPendingTerminalTransition(pendingTransition)
        return true
    }

    private fun maybeApplyPendingTerminalTransition(): Unit? {
        val pendingTransition = pendingTerminalTransition ?: return null
        if (shouldKeepSessionOpenAfterTurn()) {
            return null
        }
        applyPendingTerminalTransition(pendingTransition)
        return Unit
    }

    private fun applyPendingTerminalTransition(pendingTransition: PendingTerminalTransition) {
        pendingTerminalTransition = null
        when (pendingTransition.terminalState) {
            AgentSessionInfo.STATE_COMPLETED -> {
                updateFrameworkState(AgentSessionInfo.STATE_COMPLETED)
            }
            AgentSessionInfo.STATE_CANCELLED -> {
                pendingTransition.errorMessage?.let(::publishFrameworkError)
                updateFrameworkState(AgentSessionInfo.STATE_CANCELLED)
            }
            AgentSessionInfo.STATE_FAILED -> {
                pendingTransition.errorMessage?.let(::publishFrameworkError)
                updateFrameworkState(AgentSessionInfo.STATE_FAILED)
            }
        }
    }

    private fun shouldKeepSessionOpenAfterTurn(): Boolean {
        val proxyState = remoteProxyState
        if (proxyState != null && proxyState.connectionId == bridgeClient.currentRemoteConnectionId()) {
            return true
        }
        return runCatching {
            bridgeClient.readDesktopInspectionHold()
        }.getOrElse { err ->
            Log.w(TAG, "Failed to read desktop inspection hold for ${request.sessionId}", err)
            false
        }
    }

    private fun maybeReleaseStagedDelegatedTurn(model: String) {
        if (!idleDesktopAttachSession || initialTurnStarted) {
            return
        }
        val delegatedPrompt = stagedDelegatedPrompt ?: return
        if (hasActiveRemoteDesktopAttach()) {
            return
        }
        val threadId = activeThreadId ?: return
        val inspectionHold = runCatching {
            bridgeClient.readDesktopInspectionHold()
        }.getOrElse { err ->
            Log.w(TAG, "Failed to read desktop inspection hold for ${request.sessionId}", err)
            return
        }
        if (inspectionHold) {
            return
        }
        publishFrameworkTrace(
            "Planner desktop attach released this Genie; starting the staged delegated objective.",
        )
        startTurn(threadId, model, delegatedPrompt)
        initialTurnStarted = true
    }

    private fun hasActiveRemoteDesktopAttach(): Boolean {
        val proxyState = remoteProxyState ?: return false
        return proxyState.connectionId == bridgeClient.currentRemoteConnectionId()
    }

    private fun publishResultOnce(text: String) {
        if (resultPublished) {
            return
        }
        resultPublished = true
        publishFrameworkResult(text)
    }

    private fun recordNonFatalObserverFailure(
        phase: String,
        err: Throwable,
    ) {
        Log.w(TAG, "Non-fatal observer failure during $phase for ${request.sessionId}", err)
        publishFrameworkTrace(
            "Non-fatal host observer warning during $phase: ${err.message ?: err::class.java.simpleName}",
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

    private fun handleRemoteProxyMessage(
        connectionId: String,
        message: String,
    ) {
        val json = runCatching { JSONObject(message) }
            .getOrElse { err ->
                bridgeClient.sendRemoteAppServerMessage(
                    errorResponse(
                        requestId = null,
                        code = -32700,
                        message = err.message ?: "Invalid remote JSON-RPC message",
                    ),
                )
                return
        }
        when {
            json.has("method") && json.has("id") -> handleRemoteProxyRequest(connectionId, json)
            json.has("method") -> handleRemoteProxyNotification(json)
            else -> Unit
        }
    }

    private fun handleRemoteProxyRequest(
        connectionId: String,
        message: JSONObject,
    ) {
        val method = message.optString("method")
        val remoteRequestId = message.opt("id")
        if (remoteRequestId == null) {
            return
        }
        when (method) {
            "initialize" -> {
                val params = message.optJSONObject("params") ?: JSONObject()
                val optOut = params
                    .optJSONObject("capabilities")
                    ?.optJSONArray("optOutNotificationMethods")
                    ?.toStringSet()
                    .orEmpty()
                remoteProxyState = RemoteProxyState(
                    connectionId = connectionId,
                    optOutNotificationMethods = optOut,
                )
                bridgeClient.sendRemoteAppServerMessage(
                    JSONObject()
                        .put("id", remoteRequestId)
                        .put(
                            "result",
                            JSONObject()
                                .put("userAgent", "android_genie_bridge/$REMOTE_SERVER_VERSION")
                                .put("codexHome", codexHome.absolutePath)
                                .put("platformFamily", "unix")
                                .put("platformOs", "android"),
                        )
                        .toString(),
                )
            }
            "account/read" -> {
                bridgeClient.sendRemoteAppServerMessage(
                    JSONObject()
                        .put("id", remoteRequestId)
                        .put("result", buildRemoteAccountReadResult())
                        .toString(),
                )
            }
            else -> {
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
        }
    }

    private fun buildRemoteAccountReadResult(): JSONObject {
        val account = if (runtimeStatus.authenticated) {
            JSONObject().put("type", "apiKey")
        } else {
            JSONObject.NULL
        }
        return JSONObject()
            .put("account", account)
            .put("requiresOpenaiAuth", true)
    }

    private fun handleRemoteProxyNotification(message: JSONObject) {
        when (message.optString("method")) {
            "initialized" -> {
                replayFrameworkEventsToRemote()
                maybeAnnounceStagedPromptAwaitingDesktopInput()
                maybeAnnounceRecoveryContextAwaitingDesktopInput()
            }
            else -> sendMessage(JSONObject(message.toString()))
        }
    }

    private fun maybeAnnounceStagedPromptAwaitingDesktopInput() {
        if (!idleDesktopAttachSession || initialTurnStarted || stagedDelegatedPrompt == null) {
            return
        }
        if (!hasActiveRemoteDesktopAttach() || announcedStagedPromptAwaitingDesktopInput) {
            return
        }
        announcedStagedPromptAwaitingDesktopInput = true
        publishFrameworkTrace(
            "Desktop attach is active for this staged Genie. The delegated objective is loaded as context and will stay paused until you send the first prompt.",
        )
    }

    private fun maybeAnnounceRecoveryContextAwaitingDesktopInput() {
        if (initialTurnStarted || pendingRecoveryContext == null) {
            return
        }
        if (!hasActiveRemoteDesktopAttach() || announcedRecoveryContextAwaitingDesktopInput) {
            return
        }
        announcedRecoveryContextAwaitingDesktopInput = true
        publishFrameworkTrace(
            "Desktop attach is active for a recovered Genie. The recoverable error context is loaded for the next turn and will stay paused until you send the next prompt.",
        )
    }

    private fun publishFrameworkTrace(message: String) {
        callback.publishTrace(request.sessionId, message)
        recordFrameworkEvent(eventType = "trace", message = message)
    }

    private fun publishFrameworkQuestion(message: String) {
        callback.publishQuestion(request.sessionId, message)
        recordFrameworkEvent(eventType = "question", message = message)
    }

    private fun publishFrameworkResult(message: String) {
        callback.publishResult(request.sessionId, message)
        recordFrameworkEvent(eventType = "result", message = message)
    }

    private fun publishRecoverableFrameworkError(message: String) {
        recordFrameworkEvent(eventType = "error", message = message)
        publishFrameworkTrace("Recoverable error: $message")
    }

    private fun publishFrameworkError(message: String) {
        callback.publishError(request.sessionId, message)
        recordFrameworkEvent(eventType = "error", message = message)
    }

    private fun updateFrameworkState(state: Int) {
        callback.updateState(request.sessionId, state)
        recordFrameworkEvent(
            eventType = "trace",
            message = "Session state updated to ${stateLabel(state)}.",
        )
    }

    private fun recordFrameworkEvent(
        eventType: String,
        message: String,
    ) {
        val record = FrameworkEventRecord(
            eventType = eventType,
            message = message,
        )
        synchronized(frameworkEventLock) {
            frameworkEventHistory.add(record)
        }
        sendFrameworkEventToRemote(record)
    }

    private fun replayFrameworkEventsToRemote() {
        val history = synchronized(frameworkEventLock) {
            frameworkEventHistory.toList()
        }
        history.forEach(::sendFrameworkEventToRemote)
    }

    private fun sendFrameworkEventToRemote(record: FrameworkEventRecord) {
        val proxyState = remoteProxyState ?: return
        if (proxyState.connectionId != bridgeClient.currentRemoteConnectionId()) {
            return
        }
        if (
            proxyState.optOutNotificationMethods.contains(
                FrameworkEventBridge.THREAD_FRAMEWORK_EVENT_METHOD,
            )
        ) {
            return
        }
        val threadId = activeThreadId ?: return
        val notification = FrameworkEventBridge.buildThreadFrameworkEventNotification(
            threadId = threadId,
            eventType = record.eventType,
            message = record.message,
        ) ?: return
        bridgeClient.sendRemoteAppServerMessage(notification)
    }

    private fun stateLabel(state: Int): String {
        return when (state) {
            AgentSessionInfo.STATE_CREATED -> "CREATED"
            AgentSessionInfo.STATE_QUEUED -> "QUEUED"
            AgentSessionInfo.STATE_RUNNING -> "RUNNING"
            AgentSessionInfo.STATE_WAITING_FOR_USER -> "WAITING_FOR_USER"
            AgentSessionInfo.STATE_COMPLETED -> "COMPLETED"
            AgentSessionInfo.STATE_CANCELLED -> "CANCELLED"
            AgentSessionInfo.STATE_FAILED -> "FAILED"
            else -> "UNKNOWN($state)"
        }
    }

    private fun forwardRemoteNotification(
        rawMessage: String,
        message: JSONObject,
    ) {
        val proxyState = remoteProxyState ?: return
        if (proxyState.connectionId != bridgeClient.currentRemoteConnectionId()) {
            return
        }
        val method = message.optString("method")
        if (proxyState.optOutNotificationMethods.contains(method)) {
            return
        }
        bridgeClient.sendRemoteAppServerMessage(rawMessage)
    }

    private fun errorResponse(
        requestId: Any?,
        code: Int,
        message: String,
    ): String {
        val response = JSONObject().put(
            "error",
            JSONObject()
                .put("code", code)
                .put("message", message),
        )
        if (requestId != null) {
            response.put("id", requestId)
        }
        return response.toString()
    }

    private fun org.json.JSONArray.toStringSet(): Set<String> {
        val values = mutableSetOf<String>()
        for (index in 0 until length()) {
            optString(index).takeIf(String::isNotBlank)?.let(values::add)
        }
        return values
    }

    private fun buildBaseInstructions(): String {
        val startupContextInstructions = startupContextNotes
            .takeIf(List<String>::isNotEmpty)
            ?.joinToString(separator = "\n\n")
            ?.let { notes ->
                """

                Startup context from the framework:
                $notes
                """.trimIndent()
            }
            .orEmpty()
        val detachedSessionInstructions = if (request.isDetachedModeAllowed) {
            DetachedSessionGuard.instructions(request.targetPackage)
        } else {
            ""
        }
        val stagedRecoveryContextInstructions = pendingRecoveryContext?.let { recoveryContext ->
            """

            Recovery context:
            $recoveryContext
            Treat that recovery context as part of the current session state. Verify the current framework/target state before retrying any interrupted action, and prefer continuing from verified state over blindly replaying the last step.
            """.trimIndent()
        }.orEmpty()
        val stagedDelegatedObjectiveInstructions = stagedDelegatedPrompt?.let { delegatedPrompt ->
            """

            A supervising Agent already prepared a staged delegated objective for this session, but the first turn is paused while desktop attach inspection remains active.
            Staged delegated objective:
            $delegatedPrompt
            Treat that staged delegated objective as the current task context unless the first user message explicitly changes or overrides it.
            """.trimIndent()
        }.orEmpty()
        return """
            You are Codex acting as a child Android Genie bound to ${request.targetPackage}.
            The user interacts only with the supervising Agent.
            Decide your own local plan and choose tools yourself.
            Prefer direct self-targeted Android shell commands and intents first when they can satisfy the objective without UI-driving.
            In this platform build, an active Genie session may use self-targeted shell surfaces such as `am start --user 0`, `cmd activity start-activity --user 0`, `cmd package resolve-activity`, `cmd package query-activities --user 0`, `input`, `uiautomator dump`, `screencap`, and `screenrecord`.
            When using `am start`, `cmd activity start-activity`, or `cmd package query-activities`, pass `--user 0`; omitting it can fail with cross-user permission errors.
            Android shell `date` is not GNU coreutils `date`; do not rely on `date -d "+5 minutes"` or similar relative-date parsing because it fails on this platform.
            When you must convert a relative request like “in 5 minutes” into wall-clock alarm fields, compute it with shell arithmetic from `date +%H` and `date +%M`, for example: `h=$(date +%H); m=$(date +%M); total=$((10#${'$'}h * 60 + 10#${'$'}m + 5)); hour=$(((total / 60) % 24)); minute=$((total % 60))`, then pass `hour` and `minute` as integers to `am start`.
            When the objective is a timer duration rather than a wall-clock alarm, prefer direct duration-based intents like `android.intent.action.SET_TIMER` with a length in seconds instead of computing a future clock time.
            Avoid `dumpsys` and `cmd package dump` for package/activity inspection because they require `android.permission.DUMP` in the paired app UID and will not help you complete the task.
            If a direct command or intent clearly accomplishes the objective, stop and report success instead of continuing exploratory UI actions.
            The Genie may request detached target launch through the framework callback, and after that it should treat the target as already launched by the framework.
            Keep the target hidden by default. Use frame capture and shell/UI inspection against the detached target without surfacing it whenever possible.
            Use `android_target_show` or `android_target_attach` only when the delegated objective explicitly asks to show the app to the user, when the request clearly implies that a visible app handoff is part of success, or when you need to ask the user a question and showing the current UI materially helps them answer.
            If detached recovery is needed because the target disappeared, use android_target_ensure_hidden before retrying UI inspection.
            Use Android dynamic tools only for framework-only detached target operations that do not have a working shell equivalent in the paired app sandbox.
            $startupContextInstructions
            $detachedSessionInstructions
            The delegated objective may include a required final target presentation such as ATTACHED, DETACHED_HIDDEN, or DETACHED_SHOWN. Treat that as a hard completion requirement and do not report success until the framework session actually matches it.
            If the objective omits a parameter that materially changes what action you take or what user-visible outcome occurs, do not guess. Call request_user_input before acting. Examples include alarm time, timer duration, recipient, destination, account choice, destructive confirmation, and any other choice where a wrong assumption would surprise the user.
            If you need clarification or a decision from the supervising Agent, call request_user_input with concise free-form question text.
            Do not ask a plain-text clarifying question in a normal assistant message. When you need user input, use request_user_input and wait.
            Do not use hidden control protocols.
            Finish with a normal assistant message describing what you accomplished or what blocked you.
            $stagedRecoveryContextInstructions
            $stagedDelegatedObjectiveInstructions
            Detached target mode allowed: ${request.isDetachedModeAllowed}.
            Agent-owned runtime provider: ${runtimeStatus.modelProviderId}.
        """.trimIndent()
    }

    private fun buildDelegatedPrompt(): String {
        check(!idleDesktopAttachSession) {
            "Idle desktop-attach sessions do not have an initial delegated prompt"
        }
        val detachedSessionPrompt = if (request.isDetachedModeAllowed) {
            """
            
            Detached-session requirement:
            - The framework already launched ${request.targetPackage} hidden for this session.
            - Do not relaunch ${request.targetPackage} with shell launch commands. Use framework target controls plus UI inspection and input instead.
            - If the detached target disappears or looks empty, use android_target_ensure_hidden to request framework-owned recovery.
            """.trimIndent()
        } else {
            ""
        }
        return """
            Target package:
            ${request.targetPackage}

            Delegated objective:
            ${request.prompt}
            $detachedSessionPrompt
        """.trimIndent()
    }

    private fun buildDynamicToolSpecs(): JSONArray {
        return JSONArray()
            .put(dynamicToolSpec(AndroidGenieToolExecutor.ENSURE_HIDDEN_TARGET_TOOL, "Ensure the detached target exists and remains hidden. Use this to restore a missing detached target.", emptyObjectSchema()))
            .put(dynamicToolSpec(AndroidGenieToolExecutor.SHOW_TARGET_TOOL, "Show the detached target window only when the delegated objective asks for a visible app or visible user handoff. Do not call this by default just to inspect state; prefer android_target_capture_frame first.", emptyObjectSchema()))
            .put(dynamicToolSpec(AndroidGenieToolExecutor.HIDE_TARGET_TOOL, "Hide the detached target window.", emptyObjectSchema()))
            .put(dynamicToolSpec(AndroidGenieToolExecutor.ATTACH_TARGET_TOOL, "Reattach the detached target back to the main display only when the user explicitly asks to bring the app to the front or the final objective clearly requires a visible attached UI.", emptyObjectSchema()))
            .put(dynamicToolSpec(AndroidGenieToolExecutor.CLOSE_TARGET_TOOL, "Close the detached target window.", emptyObjectSchema()))
            .put(dynamicToolSpec(AndroidGenieToolExecutor.CAPTURE_TARGET_FRAME_TOOL, "Capture the detached target window as an image.", emptyObjectSchema()))
    }

    private fun dynamicToolSpec(
        name: String,
        description: String,
        inputSchema: JSONObject,
    ): JSONObject {
        return JSONObject()
            .put("name", name)
            .put("description", description)
            .put("inputSchema", inputSchema)
    }

    private fun emptyObjectSchema(): JSONObject {
        return objectSchema(emptyMap())
    }

    private fun objectSchema(
        properties: Map<String, JSONObject>,
        required: List<String> = emptyList(),
    ): JSONObject {
        val propertiesJson = JSONObject()
        properties.forEach { (name, schema) -> propertiesJson.put(name, schema) }
        return JSONObject()
            .put("type", "object")
            .put("properties", propertiesJson)
            .put("required", JSONArray(required))
            .put("additionalProperties", false)
    }

    private fun buildDynamicToolContentItems(observation: GenieToolObservation): JSONArray {
        val items = JSONArray().put(
            JSONObject()
                .put("type", "inputText")
                .put("text", observation.promptDetails),
        )
        observation.imageDataUrls.forEach { imageUrl ->
            items.put(
                JSONObject()
                    .put("type", "inputImage")
                    .put("imageUrl", imageUrl),
            )
        }
        return items
    }

    private fun renderAgentQuestion(questions: JSONArray): String {
        if (questions.length() == 0) {
            return "Genie requested input but did not provide a question."
        }
        val rendered = buildString {
            for (index in 0 until questions.length()) {
                val question = questions.optJSONObject(index) ?: continue
                if (length > 0) {
                    append("\n\n")
                }
                val header = question.optString("header").takeIf(String::isNotBlank)
                if (header != null) {
                    append(header)
                    append(":\n")
                }
                append(question.optString("question"))
                val options = question.optJSONArray("options")
                if (options != null && options.length() > 0) {
                    append("\nOptions:")
                    for (optionIndex in 0 until options.length()) {
                        val option = options.optJSONObject(optionIndex) ?: continue
                        append("\n- ")
                        append(option.optString("label"))
                        val description = option.optString("description")
                        if (description.isNotBlank()) {
                            append(": ")
                            append(description)
                        }
                    }
                }
            }
        }
        return if (questions.length() == 1) {
            rendered
        } else {
            "$rendered\n\nReply with one answer per question, separated by a blank line."
        }
    }

    private fun buildQuestionAnswers(
        questions: JSONArray,
        answer: String,
    ): JSONObject {
        val splitAnswers = answer
            .split(Regex("\\n\\s*\\n"))
            .map(String::trim)
            .filter(String::isNotEmpty)
        val answersJson = JSONObject()
        for (index in 0 until questions.length()) {
            val question = questions.optJSONObject(index) ?: continue
            val questionId = question.optString("id")
            if (questionId.isBlank()) {
                continue
            }
            val responseText = splitAnswers.getOrNull(index)
                ?: if (index == 0) answer.trim() else ""
            answersJson.put(
                questionId,
                JSONObject().put(
                    "answers",
                    JSONArray().put(responseText),
                ),
            )
        }
        return answersJson
    }
}
