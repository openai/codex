package io.github.treesitter.ktreesitter

import kotlin.jvm.JvmName

/**
 * A range of positions in a text document,
 * both in terms of bytes and of row-column points.
 *
 * @constructor
 * @throws [IllegalArgumentException]
 *  If the end point is smaller than the start point,
 *  or the end byte is smaller than the start byte.
 */
data class Range @Throws(IllegalArgumentException::class) constructor(
    @get:JvmName("startPoint") val startPoint: Point,
    @get:JvmName("endPoint") val endPoint: Point,
    @get:JvmName("startByte") val startByte: UInt,
    @get:JvmName("endByte") val endByte: UInt
) {
    init {
        require(startPoint <= endPoint) { "Invalid point range: [$startPoint, $endPoint]" }
        require(startByte <= endByte) { "Invalid byte range: [$startByte, $endByte]" }
    }
}
