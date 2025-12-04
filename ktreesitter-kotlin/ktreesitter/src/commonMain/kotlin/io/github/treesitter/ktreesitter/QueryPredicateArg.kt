package io.github.treesitter.ktreesitter

import kotlin.jvm.JvmInline

/**
 * An argument to a [QueryPredicate].
 *
 * @property value The value of the argument.
 */
sealed interface QueryPredicateArg {
    val value: String

    /** A capture argument (`@value`). */
    @JvmInline
    value class Capture(override val value: String) : QueryPredicateArg {
        override fun toString() = "@$value"
    }

    /** A literal string argument (`"value"`). */
    @JvmInline
    value class Literal(override val value: String) : QueryPredicateArg {
        override fun toString() = "\"$value\""
    }
}
