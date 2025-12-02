// port-lint: source core/src/unified_exec/mod.rs
package ai.solace.coder.core.unified_exec

import ai.solace.coder.core.session.Session as CodexSession
import ai.solace.coder.core.session.TurnContext
import kotlin.time.Duration

const val MIN_YIELD_TIME_MS: Long = 250
const val MAX_YIELD_TIME_MS: Long = 30_000
const val DEFAULT_MAX_OUTPUT_TOKENS: Int = 10_000
// const val UNIFIED_EXEC_OUTPUT_MAX_BYTES: Int = 1024 * 1024 // Defined in Session.kt or shared
// const val UNIFIED_EXEC_OUTPUT_MAX_TOKENS: Int = UNIFIED_EXEC_OUTPUT_MAX_BYTES / 4 // Defined in Session.kt or shared
const val MAX_UNIFIED_EXEC_SESSIONS: Int = 64

data class UnifiedExecContext(
    val session: CodexSession,
    val turn: TurnContext,
    val callId: String
)

data class ExecCommandRequest(
    val command: List<String>,
    val processId: String,
    val yieldTimeMs: Long,
    val maxOutputTokens: Int?,
    val workdir: String?, // PathBuf -> String
    val withEscalatedPermissions: Boolean?,
    val justification: String?
)

data class WriteStdinRequest(
    val callId: String,
    val processId: String,
    val input: String,
    val yieldTimeMs: Long,
    val maxOutputTokens: Int?
)

data class UnifiedExecResponse(
    val eventCallId: String,
    val chunkId: String,
    val wallTime: Duration,
    val output: String,
    val processId: String?,
    val exitCode: Int?,
    val originalTokenCount: Int?,
    val sessionCommand: List<String>?
)

fun clampYieldTime(yieldTimeMs: Long): Long {
    return yieldTimeMs.coerceIn(MIN_YIELD_TIME_MS, MAX_YIELD_TIME_MS)
}

fun resolveMaxTokens(maxTokens: Int?): Int {
    return maxTokens ?: DEFAULT_MAX_OUTPUT_TOKENS
}

fun generateChunkId(): String {
    // Simple random hex string generation
    val chars = "0123456789abcdef"
    return (1..6).map { chars.random() }.joinToString("")
}
