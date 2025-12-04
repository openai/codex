package io.github.treesitter.ktreesitter

import io.github.treesitter.ktreesitter.java.TreeSitterJava
import io.kotest.core.spec.style.FunSpec
import io.kotest.inspectors.forAll
import io.kotest.inspectors.forSingle
import io.kotest.matchers.*
import io.kotest.matchers.collections.*

class QueryCursorTest :
    FunSpec({
        val language = Language(TreeSitterJava.language())
        val parser = Parser(language)
        val source = """
    (identifier) @identifier

    (class_declaration
        name: (identifier) @class
        (class_body) @body)
        """.trimIndent()
        val query = Query(language, source)
        val tree = parser.parse("class Foo {}")

        @Suppress("DEPRECATION")
        test("timeoutMicros") {
            val cursor = query(tree.rootNode)
            cursor.timeoutMicros shouldBe 0UL
            cursor.timeoutMicros = 10UL
            cursor.timeoutMicros shouldBe 10UL
        }

        test("matchLimit") {
            val cursor = query(tree.rootNode)
            cursor.matchLimit shouldBe UInt.MAX_VALUE
            cursor.matchLimit = 10U
            cursor.matchLimit shouldBe 10U
        }

        test("maxStartDepth") {
            val cursor = query(tree.rootNode)
            cursor.maxStartDepth = 10U
            cursor.maxStartDepth shouldBe 10U
        }

        test("byteRange") {
            val cursor = query(tree.rootNode)
            cursor.byteRange = 0U..10U
            cursor.byteRange.last shouldBe 10U
        }

        test("pointRange") {
            val cursor = query(tree.rootNode)
            cursor.pointRange = Point(0U, 10U)..Point.MAX
            cursor.pointRange.start shouldBe Point(0U, 10U)
        }

        test("didExceedMatchLimit") {
            val cursor = query(tree.rootNode)
            cursor.didExceedMatchLimit shouldBe false
        }

        test("matches()") {
            var cursor = query(tree.rootNode)
            var matches = cursor.matches().toList()
            matches.shouldHaveSize(2).shouldBeMonotonicallyIncreasingWith { a, b ->
                a.patternIndex.compareTo(b.patternIndex)
            }

            var tree = parser.parse("int y = x + 1;")
            var query = Query(
                language,
                """
            ((variable_declarator
              (identifier) @y
              (binary_expression
                (identifier) @x))
              (#not-eq? @y @x))
                """.trimIndent()
            )
            cursor = query(tree.rootNode)
            matches = cursor.matches().toList()
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
            cursor = query(tree.rootNode)
            matches = cursor.matches().toList()
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
            cursor = query(tree.rootNode)
            matches = cursor.matches().toList()
            matches.shouldBeEmpty()

            query = Query(
                language,
                """
            ((identifier) @foo
             (#ieq? @foo "foo"))
                """.trimIndent()
            )
            cursor = query(tree.rootNode)
            matches = cursor.matches {
                if (name != "ieq?") return@matches true
                val node = it[(args[0] as QueryPredicateArg.Capture).value].single()
                (args[1] as QueryPredicateArg.Literal).value.equals(node.text().toString(), true)
            }.toList()
            matches.forSingle {
                it.captures[0].node.text() shouldBe "Foo"
            }
        }

        test("captures()") {
            var cursor = query(tree.rootNode)
            var captures = cursor.captures().toList()
            captures.shouldHaveSize(3).take(2).forAll {
                it.second.captures[0].node.type shouldBe "identifier"
            }

            var tree = parser.parse(
                """
            /// foo
            /// bar
                """.trimIndent()
            )
            var query = Query(
                language,
                """
            ((line_comment)+ @foo
              (#any-match? @foo "foo"))
                """.trimIndent()
            )
            cursor = query(tree.rootNode)
            captures = cursor.captures().toList()
            captures.shouldHaveSize(2).forAll {
                it.second.captures[0].name shouldBe "foo"
            }
        }
    })
