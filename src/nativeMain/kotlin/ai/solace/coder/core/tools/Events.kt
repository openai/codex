// port-lint: source core/src/tools/events.rs
package ai.solace.coder.core.tools

import ai.solace.coder.core.error.CodexError
import ai.solace.coder.core.error.SandboxError
import ai.solace.coder.core.exec.ExecToolCallOutput
import ai.solace.coder.core.function_tool.FunctionCallError
import ai.solace.coder.core.session.Session
import ai.solace.coder.core.session.TurnContext
import ai.solace.coder.core.tools.sandboxing.ToolError
import ai.solace.coder.protocol.EventMsg
import ai.solace.coder.protocol.ExecCommandBeginEvent
import ai.solace.coder.protocol.ExecCommandEndEvent
import ai.solace.coder.protocol.ExecCommandSource
import ai.solace.coder.protocol.FileChange
import ai.solace.coder.protocol.ParsedCommand
import ai.solace.coder.protocol.PatchApplyBeginEvent
import ai.solace.coder.protocol.PatchApplyEndEvent
import ai.solace.coder.protocol.TurnDiffEvent
import ai.solace.coder.protocol.parseCommand
import kotlinx.coroutines.sync.withLock
import kotlin.time.Duration

class ToolEventCtx(
    val session: Session,
    val turn: TurnContext,
    val callId: String,
    val turnDiffTracker: SharedTurnDiffTracker?
)

sealed class ToolEventStage {
    object Begin : ToolEventStage()
    data class Success(val output: ExecToolCallOutput) : ToolEventStage()
    data class Failure(val failure: ToolEventFailure) : ToolEventStage()
}

sealed class ToolEventFailure {
    data class Output(val output: ExecToolCallOutput) : ToolEventFailure()
    data class Message(val message: String) : ToolEventFailure()
}

suspend fun emitExecCommandBegin(
    ctx: ToolEventCtx,
    command: List<String>,
    cwd: String, // PathBuf -> String
    parsedCmd: List<ParsedCommand>,
    source: ExecCommandSource,
    interactionInput: String?,
    processId: String?
) {
    ctx.session.sendEvent(
        ctx.turn,
        EventMsg.ExecCommandBegin(
            ExecCommandBeginEvent(
                callId = ctx.callId,
                processId = processId,
                turnId = ctx.turn.subId,
                command = command,
                cwd = cwd,
                parsedCmd = parsedCmd,
                source = source,
                interactionInput = interactionInput
            )
        )
    )
}

sealed class ToolEmitter {
    data class Shell(
        val command: List<String>,
        val cwd: String,
        val source: ExecCommandSource,
        val parsedCmd: List<ParsedCommand>,
        val freeform: Boolean
    ) : ToolEmitter()

    data class ApplyPatch(
        val changes: Map<String, FileChange>, // PathBuf -> String
        val autoApproved: Boolean
    ) : ToolEmitter()

    data class UnifiedExec(
        val command: List<String>,
        val cwd: String,
        val source: ExecCommandSource,
        val interactionInput: String?,
        val parsedCmd: List<ParsedCommand>,
        val processId: String?
    ) : ToolEmitter()

    companion object {
        fun shell(
            command: List<String>,
            cwd: String,
            source: ExecCommandSource,
            freeform: Boolean
        ): ToolEmitter {
            val parsedCmd = parseCommand(command)
            return Shell(
                command = command,
                cwd = cwd,
                source = source,
                parsedCmd = parsedCmd,
                freeform = freeform
            )
        }

        fun applyPatch(changes: Map<String, FileChange>, autoApproved: Boolean): ToolEmitter {
            return ApplyPatch(changes, autoApproved)
        }

        fun unifiedExec(
            command: List<String>,
            cwd: String,
            source: ExecCommandSource,
            interactionInput: String?,
            processId: String?
        ): ToolEmitter {
            val parsedCmd = parseCommand(command)
            return UnifiedExec(
                command = command,
                cwd = cwd,
                source = source,
                interactionInput = interactionInput,
                parsedCmd = parsedCmd,
                processId = processId
            )
        }
    }

    suspend fun emit(ctx: ToolEventCtx, stage: ToolEventStage) {
        when (this) {
            is Shell -> {
                emitExecStage(
                    ctx,
                    ExecCommandInput(
                        command = command,
                        cwd = cwd,
                        parsedCmd = parsedCmd,
                        source = source,
                        interactionInput = null,
                        processId = null
                    ),
                    stage
                )
            }
            is ApplyPatch -> {
                when (stage) {
                    is ToolEventStage.Begin -> {
                        ctx.turnDiffTracker?.withLock {
                            it.onPatchBegin(changes)
                        }
                        ctx.session.sendEvent(
                            ctx.turn,
                            EventMsg.PatchApplyBegin(
                                PatchApplyBeginEvent(
                                    callId = ctx.callId,
                                    turnId = ctx.turn.subId,
                                    autoApproved = autoApproved,
                                    changes = changes
                                )
                            )
                        )
                    }
                    is ToolEventStage.Success -> {
                        emitPatchEnd(
                            ctx,
                            changes,
                            stage.output.stdout.text,
                            stage.output.stderr.text,
                            stage.output.exitCode == 0
                        )
                    }
                    is ToolEventStage.Failure -> {
                        when (val failure = stage.failure) {
                            is ToolEventFailure.Output -> {
                                emitPatchEnd(
                                    ctx,
                                    changes,
                                    failure.output.stdout.text,
                                    failure.output.stderr.text,
                                    failure.output.exitCode == 0
                                )
                            }
                            is ToolEventFailure.Message -> {
                                emitPatchEnd(
                                    ctx,
                                    changes,
                                    "",
                                    failure.message,
                                    false
                                )
                            }
                        }
                    }
                }
            }
            is UnifiedExec -> {
                emitExecStage(
                    ctx,
                    ExecCommandInput(
                        command = command,
                        cwd = cwd,
                        parsedCmd = parsedCmd,
                        source = source,
                        interactionInput = interactionInput,
                        processId = processId
                    ),
                    stage
                )
            }
        }
    }

    suspend fun begin(ctx: ToolEventCtx) {
        emit(ctx, ToolEventStage.Begin)
    }

    fun formatExecOutputForModel(
        output: ExecToolCallOutput,
        ctx: ToolEventCtx
    ): String {
        return when (this) {
            is Shell -> {
                if (freeform) {
                    formatExecOutputForModelFreeform(output, ctx.turn.truncationPolicy)
                } else {
                    formatExecOutputForModelStructured(output, ctx.turn.truncationPolicy)
                }
            }
            else -> formatExecOutputForModelStructured(output, ctx.turn.truncationPolicy)
        }
    }

    suspend fun finish(
        ctx: ToolEventCtx,
        out: Result<ExecToolCallOutput> // Using Result instead of specific ToolError for now
    ): Result<String> {
        val (event, result) = out.fold(
            onSuccess = { output ->
                val content = formatExecOutputForModel(output, ctx)
                val exitCode = output.exitCode
                val event = ToolEventStage.Success(output)
                val result = if (exitCode == 0) {
                    Result.success(content)
                } else {
                    Result.failure(FunctionCallError.RespondToModel(content))
                }
                Pair(event, result)
            },
            onFailure = { err ->
                when (err) {
                    is ToolError.Codex -> {
                        when (val inner = err.error) {
                            is CodexError.Sandbox -> {
                                when (val sandboxErr = inner.error) {
                                    is SandboxError.Timeout -> {
                                        val response = formatExecOutputForModel(sandboxErr.output, ctx)
                                        val event = ToolEventStage.Failure(ToolEventFailure.Output(sandboxErr.output))
                                        val result = Result.failure<String>(FunctionCallError.RespondToModel(response))
                                        Pair(event, result)
                                    }
                                    is SandboxError.Denied -> {
                                        val response = formatExecOutputForModel(sandboxErr.output, ctx)
                                        val event = ToolEventStage.Failure(ToolEventFailure.Output(sandboxErr.output))
                                        val result = Result.failure<String>(FunctionCallError.RespondToModel(response))
                                        Pair(event, result)
                                    }
                                    else -> {
                                        val message = "execution error: $err"
                                        val event = ToolEventStage.Failure(ToolEventFailure.Message(message))
                                        val result = Result.failure<String>(FunctionCallError.RespondToModel(message))
                                        Pair(event, result)
                                    }
                                }
                            }
                            else -> {
                                val message = "execution error: $err"
                                val event = ToolEventStage.Failure(ToolEventFailure.Message(message))
                                val result = Result.failure<String>(FunctionCallError.RespondToModel(message))
                                        Pair(event, result)
                            }
                        }
                    }
                    is ToolError.Rejected -> {
                        val msg = err.message
                        val normalized = if (msg == "rejected by user") {
                            "exec command rejected by user"
                        } else {
                            msg
                        }
                        val event = ToolEventStage.Failure(ToolEventFailure.Message(normalized))
                        val result = Result.failure<String>(FunctionCallError.RespondToModel(normalized))
                        Pair(event, result)
                    }
                    else -> {
                         val message = "execution error: $err"
                         val event = ToolEventStage.Failure(ToolEventFailure.Message(message))
                         val result = Result.failure<String>(FunctionCallError.RespondToModel(message))
                         Pair(event, result)
                    }
                }
            }
        )
        emit(ctx, event)
        return result
    }
}

data class ExecCommandInput(
    val command: List<String>,
    val cwd: String,
    val parsedCmd: List<ParsedCommand>,
    val source: ExecCommandSource,
    val interactionInput: String?,
    val processId: String?
)

data class ExecCommandResult(
    val stdout: String,
    val stderr: String,
    val aggregatedOutput: String,
    val exitCode: Int,
    val duration: Duration,
    val formattedOutput: String
)

suspend fun emitExecStage(
    ctx: ToolEventCtx,
    execInput: ExecCommandInput,
    stage: ToolEventStage
) {
    when (stage) {
        is ToolEventStage.Begin -> {
            emitExecCommandBegin(
                ctx,
                execInput.command,
                execInput.cwd,
                execInput.parsedCmd,
                execInput.source,
                execInput.interactionInput,
                execInput.processId
            )
        }
        is ToolEventStage.Success -> {
            val output = stage.output
            val execResult = ExecCommandResult(
                stdout = output.stdout.text,
                stderr = output.stderr.text,
                aggregatedOutput = output.aggregatedOutput.text,
                exitCode = output.exitCode,
                duration = output.duration,
                formattedOutput = formatExecOutputStr(output, ctx.turn.truncationPolicy)
            )
            emitExecEnd(ctx, execInput, execResult)
        }
        is ToolEventStage.Failure -> {
            when (val failure = stage.failure) {
                is ToolEventFailure.Output -> {
                    val output = failure.output
                    val execResult = ExecCommandResult(
                        stdout = output.stdout.text,
                        stderr = output.stderr.text,
                        aggregatedOutput = output.aggregatedOutput.text,
                        exitCode = output.exitCode,
                        duration = output.duration,
                        formattedOutput = formatExecOutputStr(output, ctx.turn.truncationPolicy)
                    )
                    emitExecEnd(ctx, execInput, execResult)
                }
                is ToolEventFailure.Message -> {
                    val text = failure.message
                    val execResult = ExecCommandResult(
                        stdout = "",
                        stderr = text,
                        aggregatedOutput = text,
                        exitCode = -1,
                        duration = Duration.ZERO,
                        formattedOutput = text
                    )
                    emitExecEnd(ctx, execInput, execResult)
                }
            }
        }
    }
}

suspend fun emitExecEnd(
    ctx: ToolEventCtx,
    execInput: ExecCommandInput,
    execResult: ExecCommandResult
) {
    ctx.session.sendEvent(
        ctx.turn,
        EventMsg.ExecCommandEnd(
            ExecCommandEndEvent(
                callId = ctx.callId,
                processId = execInput.processId,
                turnId = ctx.turn.subId,
                command = execInput.command,
                cwd = execInput.cwd,
                parsedCmd = execInput.parsedCmd,
                source = execInput.source,
                interactionInput = execInput.interactionInput,
                stdout = execResult.stdout,
                stderr = execResult.stderr,
                aggregatedOutput = execResult.aggregatedOutput,
                exitCode = execResult.exitCode,
                duration = execResult.duration,
                formattedOutput = execResult.formattedOutput
            )
        )
    )
}

suspend fun emitPatchEnd(
    ctx: ToolEventCtx,
    changes: Map<String, FileChange>,
    stdout: String,
    stderr: String,
    success: Boolean
) {
    ctx.session.sendEvent(
        ctx.turn,
        EventMsg.PatchApplyEnd(
            PatchApplyEndEvent(
                callId = ctx.callId,
                turnId = ctx.turn.subId,
                stdout = stdout,
                stderr = stderr,
                success = success,
                changes = changes
            )
        )
    )

    ctx.turnDiffTracker?.let { tracker ->
        val unifiedDiff = tracker.withLock {
            it.getUnifiedDiff()
        }
        if (unifiedDiff != null) {
            ctx.session.sendEvent(
                ctx.turn,
                EventMsg.TurnDiff(TurnDiffEvent(unifiedDiff))
            )
        }
    }
}
