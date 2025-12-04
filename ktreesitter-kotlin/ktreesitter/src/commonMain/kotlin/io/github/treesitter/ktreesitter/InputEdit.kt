package io.github.treesitter.ktreesitter

import kotlin.jvm.JvmName

/** An edit to a text document. */
data class InputEdit(
    @get:JvmName("startByte") val startByte: UInt,
    @get:JvmName("oldEndByte") val oldEndByte: UInt,
    @get:JvmName("newEndByte") val newEndByte: UInt,
    @get:JvmName("startPoint") val startPoint: Point,
    @get:JvmName("oldEndPoint") val oldEndPoint: Point,
    @get:JvmName("newEndPoint") val newEndPoint: Point
)
