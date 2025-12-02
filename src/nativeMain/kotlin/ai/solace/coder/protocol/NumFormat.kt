// port-lint: source codex-rs/protocol/src/num_format.rs
package ai.solace.coder.protocol

import kotlin.math.pow
import kotlin.math.round

/**
 * Number formatting utilities.
 *
 * Ported from Rust codex-rs/protocol/src/num_format.rs
 */

/**
 * Format an i64 with locale-aware digit separators (e.g. "12345" -> "12,345"
 * for en-US).
 */
fun formatWithSeparators(n: Long): String {
    val str = n.toString()
    val result = StringBuilder()
    var count = 0

    for (i in str.length - 1 downTo 0) {
        if (count > 0 && count % 3 == 0 && str[i] != '-') {
            result.insert(0, ',')
        }
        result.insert(0, str[i])
        if (str[i] != '-') count++
    }

    return result.toString()
}

/**
 * Format token counts to 3 significant figures, using base-10 SI suffixes.
 *
 * Examples (en-US):
 *   - 999 -> "999"
 *   - 1200 -> "1.20K"
 *   - 123456789 -> "123M"
 */
fun formatSiSuffix(n: Long): String {
    val value = n.coerceAtLeast(0)

    if (value < 1000) {
        return value.toString()
    }

    data class Unit(val scale: Long, val suffix: String)
    val units = listOf(
        Unit(1_000L, "K"),
        Unit(1_000_000L, "M"),
        Unit(1_000_000_000L, "G")
    )

    val f = value.toDouble()

    for ((scale, suffix) in units) {
        val scaled = f / scale

        if ((100.0 * scaled).toLong() < 1000) {
            return formatDouble(scaled, 2) + suffix
        } else if ((10.0 * scaled).toLong() < 1000) {
            return formatDouble(scaled, 1) + suffix
        } else if (scaled.toLong() < 1000) {
            return formatDouble(scaled, 0) + suffix
        }
    }

    // Above 1000G, keep whole-G precision
    return "${formatWithSeparators((f / 1e9).toLong())}G"
}

/**
 * Format a double with a specific number of decimal places.
 * Kotlin/Native compatible replacement for String.format().
 */
private fun formatDouble(value: Double, decimals: Int): String {
    if (decimals == 0) {
        return value.toLong().toString()
    }
    val multiplier = when (decimals) {
        1 -> 10.0
        2 -> 100.0
        else -> 10.0.pow(decimals.toDouble())
    }
    val rounded = round(value * multiplier) / multiplier
    val str = rounded.toString()
    val dotIndex = str.indexOf('.')
    return if (dotIndex < 0) {
        str + "." + "0".repeat(decimals)
    } else {
        val decimalPart = str.substring(dotIndex + 1)
        if (decimalPart.length >= decimals) {
            str.substring(0, dotIndex + 1 + decimals)
        } else {
            str + "0".repeat(decimals - decimalPart.length)
        }
    }
}
