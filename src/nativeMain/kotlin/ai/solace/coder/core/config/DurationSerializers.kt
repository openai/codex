package ai.solace.coder.core.config

import kotlin.time.Duration
import kotlin.time.DurationUnit
import kotlin.time.toDuration

// Simple helpers for converting optional seconds/milliseconds into Duration.
fun secondsToDuration(sec: Double?): Duration? = sec?.let {
    if (it.isNaN() || it.isInfinite()) null else it.toDuration(DurationUnit.SECONDS)
}

fun millisToDuration(ms: Long?): Duration? = ms?.let { it.toDuration(DurationUnit.MILLISECONDS) }
