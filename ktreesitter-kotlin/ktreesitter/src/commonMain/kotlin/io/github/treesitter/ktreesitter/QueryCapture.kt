package io.github.treesitter.ktreesitter

import kotlin.jvm.JvmName

/**
 * A [Node] that was captured with a certain capture [name].
 *
 * @property node The captured node.
 * @property name The name of the capture.
 */
@ConsistentCopyVisibility
data class QueryCapture internal constructor(
    @get:JvmName("node") val node: Node,
    @get:JvmName("name") val name: String
) {
    override fun toString() = "QueryCapture(name=$name, node=$node)"
}
