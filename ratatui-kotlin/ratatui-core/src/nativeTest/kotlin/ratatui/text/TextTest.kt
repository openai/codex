package ratatui.text

import ratatui.layout.Alignment
import ratatui.style.Color
import ratatui.style.Modifier
import ratatui.style.Style
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

/**
 * Tests for the Text class.
 *
 * These tests are transliterated from the Rust ratatui-core tests.
 */
class TextTest {

    @Test
    fun raw() {
        val text = Text.raw("The first line\nThe second line")
        assertEquals(
            listOf(Line.from("The first line"), Line.from("The second line")),
            text.lines
        )
    }

    @Test
    fun styled() {
        val style = Style.new().yellow().italic()
        val styledText = Text.styled("The first line\nThe second line", style)

        val text = Text.raw("The first line\nThe second line").style(style)

        assertEquals(styledText, text)
    }

    @Test
    fun width() {
        val text = Text.from("The first line\nThe second line")
        assertEquals(15, text.width())
    }

    @Test
    fun height() {
        val text = Text.from("The first line\nThe second line")
        assertEquals(2, text.height())
    }

    @Test
    fun patchStyle() {
        val style = Style.new().yellow().italic()
        val style2 = Style.new().red().underlined()
        val text = Text.styled("The first line\nThe second line", style).patchStyle(style2)

        val expectedStyle = Style.new().red().italic().underlined()
        val expectedText = Text.styled("The first line\nThe second line", expectedStyle)

        assertEquals(text, expectedText)
    }

    @Test
    fun resetStyle() {
        val style = Style.new().yellow().italic()
        val text = Text.styled("The first line\nThe second line", style).resetStyle()

        assertEquals(Style.reset(), text.style)
    }

    @Test
    fun fromString() {
        val text = Text.from("The first line\nThe second line")
        assertEquals(
            listOf(Line.from("The first line"), Line.from("The second line")),
            text.lines
        )
    }

    @Test
    fun fromSpan() {
        val style = Style.new().yellow().italic()
        val text = Text.from(Span.styled("The first line\nThe second line", style))
        assertEquals(
            listOf(Line.from(Span.styled(
                "The first line\nThe second line",
                style
            ))),
            text.lines
        )
    }

    @Test
    fun fromLine() {
        val text = Text.from(Line.from("The first line"))
        assertEquals(listOf(Line.from("The first line")), text.lines)
    }

    @Test
    fun toTextInt() {
        assertEquals(Text.from("42"), 42.toText())
    }

    @Test
    fun toTextString() {
        assertEquals(Text.from("hello"), "hello".toText())
    }

    @Test
    fun toTextBoolean() {
        assertEquals(Text.from("true"), true.toText())
    }

    @Test
    fun fromVecLine() {
        val text = Text.from(listOf(
            Line.from("The first line"),
            Line.from("The second line")
        ))
        assertEquals(
            listOf(Line.from("The first line"), Line.from("The second line")),
            text.lines
        )
    }

    @Test
    fun fromIterator() {
        val text = Text.fromIter(listOf(
            Line.from("The first line"),
            Line.from("The second line")
        ))
        assertEquals(
            listOf(Line.from("The first line"), Line.from("The second line")),
            text.lines
        )
    }

    @Test
    fun intoIter() {
        val text = Text.from("The first line\nThe second line")
        val iter = text.iterator()
        assertEquals(Line.from("The first line"), iter.next())
        assertEquals(Line.from("The second line"), iter.next())
        assertEquals(false, iter.hasNext())
    }

    @Test
    fun addLine() {
        assertEquals(
            Text(
                lines = mutableListOf(Line.raw("Red"), Line.raw("Blue").blue()),
                style = Style.new().red(),
                alignment = null
            ),
            Text.raw("Red").red() + Line.raw("Blue").blue()
        )
    }

    @Test
    fun addText() {
        assertEquals(
            Text(
                lines = mutableListOf(Line.raw("Red"), Line.raw("Blue")),
                style = Style.new().red(),
                alignment = null
            ),
            Text.raw("Red").red() + Text.raw("Blue").blue()
        )
    }

    @Test
    fun extend() {
        val text = Text.from("The first line\nThe second line")
        text.extend(listOf(
            Line.from("The third line"),
            Line.from("The fourth line")
        ))
        assertEquals(
            listOf(
                Line.from("The first line"),
                Line.from("The second line"),
                Line.from("The third line"),
                Line.from("The fourth line")
            ),
            text.lines
        )
    }

    @Test
    fun displayRawTextOneLine() {
        val text = Text.raw("The first line")
        assertEquals("The first line", text.toString())
    }

    @Test
    fun displayRawTextMultipleLines() {
        val text = Text.raw("The first line\nThe second line")
        assertEquals("The first line\nThe second line", text.toString())
    }

    @Test
    fun displayStyledText() {
        val styledText = Text.styled(
            "The first line\nThe second line",
            Style.new().yellow().italic()
        )

        assertEquals("The first line\nThe second line", styledText.toString())
    }

    @Test
    fun displayTextFromVec() {
        val textFromVec = Text.from(listOf(
            Line.from("The first line"),
            Line.from("The second line")
        ))

        assertEquals("The first line\nThe second line", textFromVec.toString())
    }

    @Test
    fun displayExtendedText() {
        val text = Text.from("The first line\nThe second line")

        assertEquals("The first line\nThe second line", text.toString())

        text.extend(listOf(
            Line.from("The third line"),
            Line.from("The fourth line")
        ))

        assertEquals(
            "The first line\nThe second line\nThe third line\nThe fourth line",
            text.toString()
        )
    }

    @Test
    fun stylize() {
        assertEquals(Style.default().fg(Color.Green), Text.default().green().style)
        assertEquals(
            Style.new().bg(Color.Green),
            Text.default().onGreen().style
        )
        assertEquals(Style.default().addModifier(Modifier.ITALIC), Text.default().italic().style)
    }

    @Test
    fun leftAligned() {
        val text = Text.from("Hello, world!").leftAligned()
        assertEquals(Alignment.Left, text.alignment)
    }

    @Test
    fun centered() {
        val text = Text.from("Hello, world!").centered()
        assertEquals(Alignment.Center, text.alignment)
    }

    @Test
    fun rightAligned() {
        val text = Text.from("Hello, world!").rightAligned()
        assertEquals(Alignment.Right, text.alignment)
    }

    @Test
    fun pushLine() {
        val text = Text.from("A")
        text.pushLine(Line.from("B"))
        text.pushLine(Span.from("C"))
        text.pushLine("D")
        assertEquals(
            listOf(
                Line.raw("A"),
                Line.raw("B"),
                Line.raw("C"),
                Line.raw("D")
            ),
            text.lines
        )
    }

    @Test
    fun pushLineEmpty() {
        val text = Text.default()
        text.pushLine(Line.from("Hello, world!"))
        assertEquals(listOf(Line.from("Hello, world!")), text.lines)
    }

    @Test
    fun pushSpan() {
        val text = Text.from("A")
        text.pushSpan(Span.raw("B"))
        text.pushSpan("C")
        assertEquals(
            listOf(Line.from(listOf(
                Span.raw("A"),
                Span.raw("B"),
                Span.raw("C")
            ))),
            text.lines
        )
    }

    @Test
    fun pushSpanEmpty() {
        val text = Text.default()
        text.pushSpan(Span.raw("Hello, world!"))
        assertEquals(listOf(Line.from(Span.raw("Hello, world!"))), text.lines)
    }

    @Test
    fun defaultText() {
        val text = Text.default()
        assertEquals(Style.default(), text.style)
        assertNull(text.alignment)
        assertEquals(emptyList<Line>(), text.lines)
    }

    @Test
    fun alignmentSetter() {
        val text = Text.from("Hi, what's up?")
        assertNull(text.alignment)
        assertEquals(
            Alignment.Right,
            text.alignment(Alignment.Right).alignment
        )
    }
}

/**
 * Iterator tests for Text.
 */
class TextIteratorTest {

    private fun helloWorld(): Text = Text.from(listOf(
        Line.styled("Hello ", Style.default().fg(Color.Blue)),
        Line.styled("world!", Style.default().fg(Color.Green))
    ))

    @Test
    fun iter() {
        val text = helloWorld()
        val iter = text.iterator()
        assertEquals(Line.styled("Hello ", Style.default().fg(Color.Blue)), iter.next())
        assertEquals(Line.styled("world!", Style.default().fg(Color.Green)), iter.next())
        assertEquals(false, iter.hasNext())
    }

    @Test
    fun forLoopRef() {
        val text = helloWorld()
        var result = ""
        for (line in text) {
            result += line.toString()
        }
        assertEquals("Hello world!", result)
    }
}
