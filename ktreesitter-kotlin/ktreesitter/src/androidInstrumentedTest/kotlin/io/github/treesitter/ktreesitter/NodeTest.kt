package io.github.treesitter.ktreesitter

import br.com.colman.kotest.KotestRunnerAndroid
import io.github.treesitter.ktreesitter.java.TreeSitterJava
import io.kotest.assertions.throwables.shouldThrow
import io.kotest.core.spec.style.FunSpec
import io.kotest.inspectors.forSingle
import io.kotest.matchers.*
import io.kotest.matchers.collections.*
import io.kotest.matchers.nulls.*
import io.kotest.matchers.types.*
import org.junit.runner.RunWith

@RunWith(KotestRunnerAndroid::class)
class NodeTest : FunSpec({
    val language = Language(TreeSitterJava.language())
    val parser = Parser(language)
    val source = "class Foo {}"
    val tree = parser.parse(source)
    val rootNode = tree.rootNode

    test("id") {
        rootNode.id shouldNotBe 0UL
    }

    test("symbol") {
        rootNode.symbol shouldNotBe UShort.MIN_VALUE
    }

    test("grammarSymbol") {
        rootNode.grammarSymbol shouldBe rootNode.symbol
    }

    test("type") {
        rootNode.type shouldBe "program"
    }

    test("grammarType") {
        rootNode.grammarType shouldBe rootNode.type
    }

    test("isNamed") {
        rootNode.isNamed shouldBe true
    }

    test("isExtra") {
        rootNode.isExtra shouldBe false
    }

    test("isError") {
        rootNode.isError shouldBe false
    }

    test("isMissing") {
        rootNode.isMissing shouldBe false
    }

    test("hasChanges") {
        rootNode.hasChanges shouldBe false
    }

    test("hasError") {
        rootNode.hasError shouldBe false
    }

    test("parseState") {
        rootNode.parseState shouldBe 0U
    }

    test("nextParseState") {
        rootNode.nextParseState shouldBe 0U
    }

    test("startByte") {
        rootNode.startByte shouldBe 0U
    }

    test("endByte") {
        rootNode.endByte shouldBe 12U
    }

    test("byteRange") {
        rootNode.byteRange shouldBe 0U..12U
    }

    test("range") {
        rootNode.range shouldBe Range(Point(0U, 0U), Point(0U, 12U), 0U, 12U)
    }

    test("startPoint") {
        rootNode.startPoint shouldBe Point(0U, 0U)
    }

    test("endPoint") {
        rootNode.endPoint shouldBe Point(0U, 12U)
    }

    test("childCount") {
        rootNode.childCount shouldBe 1U
    }

    test("namedChildCount") {
        rootNode.namedChildCount shouldBe 1U
    }

    test("descendantCount") {
        rootNode.descendantCount shouldBe 7U
    }

    test("parent") {
        rootNode.parent.shouldBeNull()
    }

    test("nextSibling") {
        rootNode.nextSibling.shouldBeNull()
    }

    test("prevSibling") {
        rootNode.prevSibling.shouldBeNull()
    }

    test("nextNamedSibling") {
        rootNode.nextNamedSibling.shouldBeNull()
    }

    test("prevNamedSibling") {
        rootNode.prevNamedSibling.shouldBeNull()
    }

    test("children") {
        val children = rootNode.children
        children.forSingle { it.type shouldBe "class_declaration" }
        rootNode.children shouldBeSameInstanceAs children
    }

    test("namedChildren") {
        val children = rootNode.namedChildren
        children shouldContainExactly rootNode.children
    }

    test("child()") {
        val node = rootNode.child(0U)
        node?.type shouldBe "class_declaration"
        shouldThrow<IndexOutOfBoundsException> { rootNode.child(1U) }
    }

    test("namedChild()") {
        rootNode.namedChild(0U) shouldBe rootNode.child(0U)
        shouldThrow<IndexOutOfBoundsException> { rootNode.namedChild(1U) }
    }

    test("firstChildForByte()") {
        rootNode.firstChildForByte(10U)!!.type shouldBe "class_declaration"
    }

    test("firstNamedChildForByte()") {
        rootNode.firstNamedChildForByte(10U)!!.type shouldBe "class_declaration"
    }

    test("childByFieldId()") {
        rootNode.childByFieldId(0U).shouldBeNull()
    }

    test("childByFieldName()") {
        val child = rootNode.child(0U)!!.childByFieldName("body")
        child?.type shouldBe "class_body"
    }

    test("childrenByFieldId()") {
        val id = language.fieldIdForName("name")
        val children = rootNode.child(0U)!!.childrenByFieldId(id)
        children.forSingle { it.type shouldBe "identifier" }
    }

    test("childrenByFieldName()") {
        val children = rootNode.child(0U)!!.childrenByFieldName("name")
        children.forSingle { it.type shouldBe "identifier" }
    }

    test("fieldNameForChild()") {
        rootNode.child(0U)!!.fieldNameForChild(2U) shouldBe "body"
    }

    test("fieldNameForNamedChild()") {
        rootNode.child(0U)!!.fieldNameForNamedChild(2U).shouldBeNull()
    }

    test("childWithDescendant()") {
        val descendant = rootNode.child(0U)!!
        val child = rootNode.childWithDescendant(descendant)
        child?.type shouldBe "class_declaration"
    }

    test("descendant()") {
        rootNode.descendant(0U, 5U)?.type shouldBe "class"
        rootNode.descendant(Point(0U, 10U), Point(0U, 12U))?.type shouldBe "class_body"
    }

    test("namedDescendant()") {
        rootNode.namedDescendant(0U, 5U)?.type shouldBe "class_declaration"
        rootNode.namedDescendant(Point(0U, 6U), Point(0U, 9U))?.type shouldBe "identifier"
    }

    test("walk()") {
        val cursor = rootNode.walk()
        cursor.currentNode shouldBeSameInstanceAs rootNode
    }

    test("copy()") {
        val copy = tree.copy()
        copy shouldNotBeSameInstanceAs tree
        copy.text() shouldBe tree.text()
    }

    @Suppress("UnnecessaryOptInAnnotation")
    @OptIn(ExperimentalMultiplatform::class)
    test("edit()") {
        val edit = InputEdit(0U, 12U, 10U, Point(0U, 0U), Point(0U, 12U), Point(0U, 10U))
        val copy = tree.copy()
        val node = copy.rootNode
        copy.edit(edit)
        node.edit(edit)
        node.hasChanges shouldBe true
    }

    test("text()") {
        rootNode.descendant(6U, 9U)?.text() shouldBe "Foo"
    }

    test("sexp()") {
        rootNode.child(0U)!!.sexp() shouldBe
            "(class_declaration name: (identifier) body: (class_body))"
    }

    test("equals()") {
        rootNode shouldNotBe rootNode.child(0U)
    }

    test("hashCode()") {
        rootNode.hashCode() shouldBe rootNode.id.toInt()
    }

    test("toString()") {
        rootNode.toString() shouldBe "Node(type=program, startByte=0, endByte=12)"
    }
})
