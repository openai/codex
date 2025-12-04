package io.github.treesitter.ktreesitter

import io.github.treesitter.ktreesitter.java.TreeSitterJava
import io.kotest.assertions.throwables.shouldNotThrowAny
import io.kotest.assertions.throwables.shouldThrow
import io.kotest.assertions.throwables.shouldThrowWithMessage
import io.kotest.core.spec.style.FunSpec
import io.kotest.inspectors.forOne
import io.kotest.matchers.*

class QueryTest :
    FunSpec({
        val language = Language(TreeSitterJava.language())
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
