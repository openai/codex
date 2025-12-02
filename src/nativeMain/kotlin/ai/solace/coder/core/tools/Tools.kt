// port-lint: source core/src/tools/mod.rs
package ai.solace.coder.core.tools

import ai.solace.coder.core.ExecToolCallOutput
import ai.solace.coder.core.context.TruncationPolicy
import ai.solace.coder.core.truncate.formattedTruncateText
import ai.solace.coder.core.truncate.truncateText
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlin.math.round

// Telemetry preview limits: keep log events smaller than model budgets.
const val TELEMETRY_PREVIEW_MAX_BYTES = 2 * 1024 // 2 KiB
const val TELEMETRY_PREVIEW_MAX_LINES = 64 // lines
const val TELEMETRY_PREVIEW_TRUNCATION_NOTICE = "[... telemetry preview truncated ...]"

/**
 * Format the combined exec output for sending back to the model.
 * Includes exit code and duration metadata; truncates large bodies safely.
 */
fun formatExecOutputForModelStructured(
    execOutput: ExecToolCallOutput,
    truncationPolicy: TruncationPolicy
): String {
    val exitCode = execOutput.exitCode
    val duration = execOutput.duration

    @Serializable
    data class ExecMetadata(
        val exitCode1: Int,
        val durationSeconds: Float
    )

    @Serializable
    data class ExecOutput(
        val output: String,
        val metadata: ExecMetadata
    )

    // round to 1 decimal place
    val durationSeconds = (duration.inWholeMilliseconds / 1000f * 10.0f).roundToInt() / 10.0f

    val formattedOutput = formatExecOutputStr(execOutput, truncationPolicy)

    val payload = ExecOutput(
        output = formattedOutput,
        metadata = ExecMetadata(
            exitCode1 = exitCode,
            durationSeconds = durationSeconds
        )
    )

    return Json.encodeToString(payload)
}

fun formatExecOutputForModelFreeform(
    execOutput: ExecToolCallOutput,
    truncationPolicy: TruncationPolicy
): String {
    // round to 1 decimal place
    val durationSeconds = (execOutput.duration.inWholeMilliseconds / 1000f * 10.0f).roundToInt() / 10.0f

    val totalLines = execOutput.aggregatedOutput.text.lines().count()

    val formattedOutput = truncateText(execOutput.aggregatedOutput.text, truncationPolicy)

    val sections = mutableListOf<String>()

    sections.add("Exit code: ${execOutput.exitCode}")
    sections.add("Wall time: $durationSeconds seconds")
    if (totalLines != formattedOutput.lines().count()) {
        sections.add("Total output lines: $totalLines")
    }

    sections.add("Output:")
    sections.add(formattedOutput)

    return sections.joinToString("\n")
}

fun formatExecOutputStr(
    execOutput: ExecToolCallOutput,
    truncationPolicy: TruncationPolicy
): String {
    val content = execOutput.aggregatedOutput.text

    val body = if (execOutput.timedOut) {
        "command timed out after ${execOutput.duration.inWholeMilliseconds} milliseconds\n$content"
    } else {
        content
    }

    // Truncate for model consumption before serialization.
    return formattedTruncateText(body, truncationPolicy)
}

private fun Float.roundToInt(): Int = round(this).toInt()
