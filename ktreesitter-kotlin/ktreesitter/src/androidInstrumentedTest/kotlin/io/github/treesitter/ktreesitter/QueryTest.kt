package io.github.treesitter.ktreesitter

import br.com.colman.kotest.KotestRunnerAndroid
import io.github.treesitter.ktreesitter.java.TreeSitterJava
import io.kotest.assertions.throwables.shouldNotThrowAny
import io.kotest.assertions.throwables.shouldThrow
import io.kotest.assertions.throwables.shouldThrowWithMessage
import io.kotest.core.spec.style.FunSpec
import io.kotest.inspectors.forAll
import io.kotest.inspectors.forOne
import io.kotest.inspectors.forSingle
import io.kotest.matchers.*
import io.kotest.matchers.collections.*
import org.junit.runner.RunWith

@RunWith(KotestRunnerAndroid::class)
class QueryTest :
    FunSpec({
        val language = Language(TreeSitterJava.language())
        val parser = Parser(language)
        val source = """
    (identifier) @identifier

    (class_declaration
        name: (identifier) @class
        (class_body) @body)
        """.trimIndent()

        test("constructor") {
            shouldNotThrowAny { Query(language, "") }
            shouldThrowWithMessage<QueryError.Syntax>(
                "Invalid syntax at row 0, column 0"
            ) {
                Query(language, "identifier)")
            }
            shouldThrowWithMessage<QueryError.Syntax>("Unexpected EOF") {
                Query(language, "(identifier) @")
            }
            shouldThrowWithMessage<QueryError.Capture>(
                "Invalid capture name at row 1, column 10: bar"
            ) {
                Query(language, "((identifier) @foo\n (#test? @bar))")
            }
            shouldThrowWithMessage<QueryError.NodeType>(
                "Invalid node type at row 0, column 1: foo"
            ) {
                Query(language, "(foo)")
            }
            shouldThrowWithMessage<QueryError.Field>(
                "Invalid field name at row 0, column 0: foo"
            ) {
                Query(language, "foo: (identifier)")
            }
            shouldThrowWithMessage<QueryError.Structure>(
                "Impossible pattern at row 0, column 9"
            ) {
                Query(language, "(program (identifier))")
            }
            shouldThrowWithMessage<QueryError.Predicate>(
                "Invalid predicate in pattern at row 1: #any-of? expects at least 2 arguments, got 0"
            ) {
                Query(language, "\n((identifier) @foo\n (#any-of?))")
            }
        }

        test("patternCount") {
            val query = Query(language, source)
            query.patternCount shouldBe 2U
        }

        test("captureCount") {
            val query = Query(language, source)
            query.captureCount shouldBe 3U
        }

        test("timeoutMicros") {
            val query = Query(language, source)
            query.timeoutMicros shouldBe 0UL
            query.timeoutMicros = 10UL
            query.timeoutMicros shouldBe 10UL
        }

        test("matchLimit") {
            val query = Query(language, source)
            query.matchLimit shouldBe UInt.MAX_VALUE
            query.matchLimit = 10U
            query.matchLimit shouldBe 10U
        }

        test("maxStartDepth") {
            val query = Query(language, source)
            query.maxStartDepth = 10U
            query.maxStartDepth shouldBe 10U
        }

        test("byteRange") {
            val query = Query(language, source)
            query.byteRange = 0U..10U
            query.byteRange.last shouldBe 10U
        }

        test("pointRange") {
            val query = Query(language, source)
            query.pointRange = Point(0U, 10U)..Point.MAX
            query.pointRange.start shouldBe Point(0U, 10U)
        }

        test("didExceedMatchLimit") {
            val query = Query(language, source)
            query.didExceedMatchLimit shouldBe false
        }

        test("matches()") {
            var tree = parser.parse("class Foo {}")
            var query = Query(language, source)
            var matches = query.matches(tree.rootNode).toList()
            matches.shouldHaveSize(2).shouldBeMonotonicallyIncreasingWith { a, b ->
                a.patternIndex.compareTo(b.patternIndex)
            }

            tree = parser.parse("int y = x + 1;")
            query = Query(
                language,
                """
            ((variable_declarator
              (identifier) @y
              (binary_expression
                (identifier) @x))
              (#not-eq? @y @x))
                """.trimIndent()
            )
            matches = query.matches(tree.rootNode).toList()
            matches.forSingle {
                it.captures[0].node.text() shouldBe "y"
            }

            tree = parser.parse(
                """
            class Foo {}
            class Bar {}
                """.trimIndent()
            )
            query = Query(
                language,
                """
            ((identifier) @foo
             (#eq? @foo "Foo"))
                """.trimIndent()
            )
            matches = query.matches(tree.rootNode).toList()
            matches.forSingle {
                it.captures[0].node.text() shouldBe "Foo"
            }

            query = Query(
                language,
                """
            ((identifier) @name
             (#not-any-of? @name "Foo" "Bar"))
                """.trimIndent()
            )
            matches = query.matches(tree.rootNode).toList()
            matches.shouldBeEmpty()

            query = Query(
                language,
                """
            ((identifier) @foo
             (#ieq? @foo "foo"))
                """.trimIndent()
            )
            matches = query.matches(tree.rootNode) {
                if (name != "ieq?") return@matches true
                val node = it[(args[0] as QueryPredicateArg.Capture).value].single()
                (args[1] as QueryPredicateArg.Literal).value.equals(node.text().toString(), true)
            }.toList()
            matches.forSingle {
                it.captures[0].node.text() shouldBe "Foo"
            }
        }

        test("captures()") {
            var tree = parser.parse("class Foo {}")
            var query = Query(language, source)
            var captures = query.captures(tree.rootNode).toList()
            captures.shouldHaveSize(3).take(2).forAll {
                it.second.captures[0].node.type shouldBe "identifier"
            }

            tree = parser.parse(
                """
            /// foo
            /// bar
                """.trimIndent()
            )
            query = Query(
                language,
                """
            ((line_comment)+ @foo
              (#any-match? @foo "foo"))
                """.trimIndent()
            )
            captures = query.captures(tree.rootNode).toList()
            captures.shouldHaveSize(2).forAll {
                it.second.captures[0].name shouldBe "foo"
            }
        }

        test("settings()") {
            var query = Query(
                language,
                """
            ((identifier) @foo
             (#set! foo))
                """.trimIndent()
            )
            var settings = query.settings(0U)
            settings.forOne { (key, value) ->
                key shouldBe "foo"
                value shouldBe null
            }

            query = Query(
                language,
                """
            ((identifier) @foo
             (#set! foo "FOO"))
                """.trimIndent()
            )
            settings = query.settings(0U)
            settings.forOne { (key, value) ->
                key shouldBe "foo"
                value shouldBe "FOO"
            }

            shouldThrow<IndexOutOfBoundsException> {
                query.settings(1U)
            }
        }

        test("assertions()") {
            var query = Query(
                language,
                """
            ((identifier) @foo
             (#is? foo))
                """.trimIndent()
            )
            var assertions = query.assertions(0U)
            assertions.forOne { (key, value) ->
                key shouldBe "foo"
                value shouldBe (null to true)
            }

            query = Query(
                language,
                """
            ((identifier) @foo
             (#is-not? foo "FOO"))
                """.trimIndent()
            )
            assertions = query.assertions(0U)
            assertions.forOne { (key, value) ->
                key shouldBe "foo"
                value shouldBe ("FOO" to false)
            }

            shouldThrow<IndexOutOfBoundsException> {
                query.assertions(1U)
            }
        }

        test("disablePattern()") {
            val query = Query(language, source)
            query.disablePattern(1U)
            val tree = parser.parse("class Foo {}")
            val matches = query.captures(tree.rootNode).toList()
            matches.forSingle {
                it.second.captures[0].node.type shouldBe "identifier"
            }

            shouldThrow<IndexOutOfBoundsException> {
                query.disablePattern(2U)
            }
        }

        test("disableCapture()") {
            val query = Query(language, source)
            query.disableCapture("body")
            val tree = parser.parse("class Foo {}")
            val matches = query.captures(tree.rootNode).toList()
            matches.shouldHaveSize(2).forAll {
                it.second.captures[0].node.type shouldBe "identifier"
            }

            shouldThrow<NoSuchElementException> {
                query.disableCapture("none")
            }
        }

        test("startByteForPattern()") {
            val query = Query(language, source)
            query.startByteForPattern(1U) shouldBe 26U
            shouldThrow<IndexOutOfBoundsException> {
                query.startByteForPattern(2U)
            }
        }

        test("endByteForPattern()") {
            val query = Query(language, source)
            query.endByteForPattern(0U) shouldBe 26U
            shouldThrow<IndexOutOfBoundsException> {
                query.endByteForPattern(2U)
            }
        }

        test("isPatternRooted()") {
            val query = Query(language, source)
            query.isPatternRooted(0U) shouldBe true
            shouldThrow<IndexOutOfBoundsException> {
                query.isPatternRooted(2U)
            }
        }

        test("isPatternNonLocal()") {
            val query = Query(language, source)
            query.isPatternNonLocal(1U) shouldBe false
            shouldThrow<IndexOutOfBoundsException> {
                query.isPatternNonLocal(2U)
            }
        }

        test("isPatternGuaranteedAtStep()") {
            val query = Query(language, source)
            query.isPatternGuaranteedAtStep(27U) shouldBe false
            shouldThrow<IndexOutOfBoundsException> {
                query.isPatternGuaranteedAtStep(99U)
            }
        }
    })
