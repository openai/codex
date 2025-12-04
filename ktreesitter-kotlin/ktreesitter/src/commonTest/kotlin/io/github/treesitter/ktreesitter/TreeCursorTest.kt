package io.github.treesitter.ktreesitter

import io.github.treesitter.ktreesitter.java.TreeSitterJava
import io.kotest.core.spec.style.FunSpec
import io.kotest.matchers.*
import io.kotest.matchers.nulls.*
import io.kotest.matchers.types.*

class TreeCursorTest : FunSpec({
    val language = Language(TreeSitterJava.language())
    val parser = Parser(language)
    val source = "class Foo {}"
    val tree = parser.parse(source)
    val cursor = tree.walk()
    val rootNode = tree.rootNode

    test("currentNode") {
        val node = cursor.currentNode
        node shouldBe rootNode
        node shouldBeSameInstanceAs cursor.currentNode
    }

    test("currentDepth") {
        cursor.currentDepth shouldBe 0U
    }

    test("currentFieldId") {
        cursor.currentFieldId shouldBe 0U
    }

    test("currentFieldName") {
        cursor.currentFieldName.shouldBeNull()
    }

    test("currentDescendantIndex") {
        cursor.currentDescendantIndex shouldBe 0U
    }

    test("copy()") {
        val copy = cursor.copy()
        copy shouldNotBeSameInstanceAs cursor
        cursor.reset(copy)
        copy.currentNode shouldBe cursor.currentNode
    }

    test("gotoFirstChild()") {
        cursor.gotoFirstChild() shouldBe true
        cursor.currentNode.type shouldBe "class_declaration"
    }

    test("gotoLastChild()") {
        cursor.gotoLastChild() shouldBe true
        cursor.currentFieldName shouldBe "body"
    }

    test("gotoParent()") {
        cursor.gotoParent() shouldBe true
        cursor.currentNode.type shouldBe "class_declaration"
    }

    test("gotoNextSibling()") {
        cursor.gotoNextSibling() shouldBe false
    }

    test("gotoPreviousSibling()") {
        cursor.gotoPreviousSibling() shouldBe false
    }

    test("gotoDescendant()") {
        cursor.gotoDescendant(2U)
        cursor.currentDescendantIndex shouldBe 2U
        cursor.reset(rootNode)
    }

    test("gotoFirstChildForByte()") {
        cursor.gotoFirstChildForByte(1U) shouldBe 0U
        cursor.currentNode.type shouldBe "class_declaration"
    }

    test("gotoFirstChildForPoint()") {
        cursor.gotoFirstChildForPoint(Point(0U, 7U)) shouldBe 1U
        cursor.currentFieldName shouldBe "name"
    }
})
