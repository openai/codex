// port-lint: source codex-rs/codex-api/src/rate_limits.rs
package ai.solace.coder.api.ratelimits

import ai.solace.coder.protocol.RateLimitSnapshot
import ai.solace.coder.protocol.RateLimitWindow
import ai.solace.coder.protocol.CreditsSnapshot
import io.ktor.http.*

/** Rate limit error. */
data class RateLimitError(val message: String)

/** Parse Codex-specific rate limit headers into a snapshot. */
fun parseRateLimit(headers: Headers): RateLimitSnapshot? {
    val primary = parseRateLimitWindow(
        headers,
        "x-codex-primary-used-percent",
        "x-codex-primary-window-minutes",
        "x-codex-primary-reset-at"
    )
    val secondary = parseRateLimitWindow(
        headers,
        "x-codex-secondary-used-percent",
        "x-codex-secondary-window-minutes",
        "x-codex-secondary-reset-at"
    )
    val credits = parseCreditsSnapshot(headers)

    return RateLimitSnapshot(primary, secondary, credits)
}

private fun parseRateLimitWindow(
    headers: Headers,
    usedPercentHeader: String,
    windowMinutesHeader: String,
    resetsAtHeader: String,
): RateLimitWindow? {
    val usedPercent = parseHeaderF64(headers, usedPercentHeader) ?: return null
    val windowMinutes = parseHeaderI64(headers, windowMinutesHeader)
    val resetsAt = parseHeaderI64(headers, resetsAtHeader)

    val hasData = usedPercent != 0.0 || windowMinutes != 0L || resetsAt != null
    return if (hasData) {
        RateLimitWindow(usedPercent, windowMinutes, resetsAt)
    } else {
        null
    }
}

private fun parseCreditsSnapshot(headers: Headers): CreditsSnapshot? {
    val hasCredits = parseHeaderBool(headers, "x-codex-credits-has-credits") ?: return null
    val unlimited = parseHeaderBool(headers, "x-codex-credits-unlimited") ?: return null
    val balance = headers["x-codex-credits-balance"]?.trim()?.takeIf { it.isNotEmpty() }
    return CreditsSnapshot(hasCredits, unlimited, balance)
}

private fun parseHeaderF64(headers: Headers, name: String): Double? {
    return headers[name]?.toDoubleOrNull()?.takeIf { it.isFinite() }
}

private fun parseHeaderI64(headers: Headers, name: String): Long? {
    return headers[name]?.toLongOrNull()
}

private fun parseHeaderBool(headers: Headers, name: String): Boolean? {
    val raw = headers[name] ?: return null
    return when {
        raw.equals("true", ignoreCase = true) || raw == "1" -> true
        raw.equals("false", ignoreCase = true) || raw == "0" -> false
        else -> null
    }
}

// RateLimitSnapshot, RateLimitWindow, and CreditsSnapshot are imported from
// ai.solace.coder.protocol - see Protocol.kt for definitions

