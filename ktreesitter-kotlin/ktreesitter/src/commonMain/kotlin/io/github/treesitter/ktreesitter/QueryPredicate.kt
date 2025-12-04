package io.github.treesitter.ktreesitter

/**
 * A query [predicate](https://tree-sitter.github.io/tree-sitter/using-parsers#predicates)
 * that associates conditions or arbitrary metadata with a pattern.
 *
 * The following predicates are supported by default:
 *
 * - `#eq?`, `#not-eq?`, `#any-eq?`, `#any-not-eq?`
 * - `#match?`, `#not-match?`, `#any-match?`, `#any-not-match?`
 * - `#any-of?`, `#not-any-of?`
 *
 * @property name The name of the predicate.
 * @property args The arguments given to the predicate.
 */
sealed class QueryPredicate(val name: String) {
    abstract val args: List<QueryPredicateArg>

    internal abstract operator fun invoke(match: QueryMatch): Boolean

    final override fun toString() = "QueryPredicate(name=$name, args=$args)"

    internal class EqCapture(
        name: String,
        private val capture: String,
        private val value: String,
        private val isPositive: Boolean,
        private val isAny: Boolean
    ) : QueryPredicate(name) {
        override val args = listOf(
            QueryPredicateArg.Capture(capture),
            QueryPredicateArg.Capture(value)
        )

        override fun invoke(match: QueryMatch): Boolean {
            val nodes1 = match[capture]
            val nodes2 = match[value]
            val test = if (!isAny) nodes1::all else nodes1::any
            return test { n1 ->
                nodes2.any { n2 ->
                    val result = n1.text() == n2.text()
                    if (isPositive) result else !result
                }
            }
        }
    }

    internal class EqString(
        name: String,
        private val capture: String,
        private val value: String,
        private val isPositive: Boolean,
        private val isAny: Boolean
    ) : QueryPredicate(name) {
        override val args = listOf(
            QueryPredicateArg.Capture(capture),
            QueryPredicateArg.Literal(value)
        )

        override fun invoke(match: QueryMatch): Boolean {
            val nodes = match[capture]
            if (nodes.isEmpty()) return !isPositive
            val test = if (!isAny) nodes::all else nodes::any
            return test {
                val result = value == it.text()!!
                if (isPositive) result else !result
            }
        }
    }

    internal class Match(
        name: String,
        private val capture: String,
        private val pattern: Regex,
        private val isPositive: Boolean,
        private val isAny: Boolean
    ) : QueryPredicate(name) {
        override val args = listOf(
            QueryPredicateArg.Capture(capture),
            QueryPredicateArg.Literal(pattern.pattern)
        )

        override fun invoke(match: QueryMatch): Boolean {
            val nodes = match[capture]
            if (nodes.isEmpty()) return !isPositive
            val test = if (!isAny) nodes::all else nodes::any
            return test {
                val result = pattern.containsMatchIn(it.text()!!)
                if (isPositive) result else !result
            }
        }
    }

    internal class AnyOf(
        name: String,
        private val capture: String,
        private val value: List<String>,
        private val isPositive: Boolean
    ) : QueryPredicate(name) {
        override val args = List(value.size + 1) {
            if (it == 0) QueryPredicateArg.Capture(capture)
            else QueryPredicateArg.Literal(value[it - 1])
        }

        override fun invoke(match: QueryMatch) =
            match[capture].none { (it.text()!! in value) != isPositive }
    }

    internal class Generic(
        name: String,
        override val args: List<QueryPredicateArg>
    ) : QueryPredicate(name) {
        override fun invoke(match: QueryMatch) = true
    }
}
