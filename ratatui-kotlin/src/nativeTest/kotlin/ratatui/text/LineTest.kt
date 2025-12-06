package ratatui.text

import ratatui.layout.Alignment
import ratatui.style.Color
import ratatui.style.Modifier
import ratatui.style.Style
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

/**
 * Tests for the Line class.
 *
 * These tests are transliterated from the Rust ratatui-core tests.
 */
class LineTest {

    @Test
    fun rawStr() {
        val line = Line.raw("test content")
        assertEquals(listOf(Span.raw("test content")), line.spans)
        assertNull(line.alignment)

        val lineWithNewline = Line.raw("a\nb")
        assertEquals(listOf(Span.raw("a"), Span.raw("b")), lineWithNewline.spans)
        assertNull(lineWithNewline.alignment)
    }

    @Test
    fun styledStr() {
        val style = Style.new().yellow()
        val content = "Hello, world!"
        val line = Line.styled(content, style)
        assertEquals(listOf(Span.raw(content)), line.spans)
        assertEquals(style, line.style)
    }

    @Test
    fun styledString() {
        val style = Style.new().yellow()
        val content = "Hello, world!"
        val line = Line.styled(content, style)
        assertEquals(listOf(Span.raw(content)), line.spans)
        assertEquals(style, line.style)
    }

    @Test
    fun spansVec() {
        val line = Line.default().spans(listOf(
            Span.styled("Hello", Style.new().blue()),
            Span.styled(" world!", Style.new().green())
        ))
        assertEquals(
            listOf(
                Span.styled("Hello", Style.new().blue()),
                Span.styled(" world!", Style.new().green())
            ),
            line.spans
        )
    }

    @Test
    fun spansIter() {
        val line = Line.default().spans(listOf(1, 2, 3).map { i -> Span.raw("Item $i") })
        assertEquals(
            listOf(
                Span.raw("Item 1"),
                Span.raw("Item 2"),
                Span.raw("Item 3")
            ),
            line.spans
        )
    }

    @Test
    fun style() {
        val line = Line.default().style(Style.new().red())
        assertEquals(Style.new().red(), line.style)
    }

    @Test
    fun alignment() {
        val lineLeft = Line.from("This is left").alignment(Alignment.Left)
        assertEquals(Alignment.Left, lineLeft.alignment)

        val lineDefault = Line.from("This is default")
        assertNull(lineDefault.alignment)
    }

    @Test
    fun width() {
        val line = Line.from(listOf(
            Span.styled("My", Style.default().fg(Color.Yellow)),
            Span.raw(" text")
        ))
        assertEquals(7, line.width())

        val emptyLine = Line.default()
        assertEquals(0, emptyLine.width())
    }

    @Test
    fun patchStyle() {
        val rawLine = Line.styled("foobar", Style.default().fg(Color.Yellow))
        val styledLine = Line.styled("foobar", Style.default().fg(Color.Yellow).addModifier(Modifier.ITALIC))

        val patched = rawLine.patchStyle(Style.default().addModifier(Modifier.ITALIC))
        assertEquals(styledLine, patched)
    }

    @Test
    fun resetStyle() {
        val line = Line.styled(
            "foobar",
            Style.default().yellow().onRed().italic()
        ).resetStyle()

        assertEquals(Style.reset(), line.style)
    }

    @Test
    fun fromString() {
        val s = "Hello, world!"
        val line = Line.from(s)
        assertEquals(listOf(Span.from("Hello, world!")), line.spans)

        val sWithNewline = "Hello\nworld!"
        val lineWithNewline = Line.from(sWithNewline)
        assertEquals(listOf(Span.from("Hello"), Span.from("world!")), lineWithNewline.spans)
    }

    @Test
    fun toLine() {
        val line = 42.toLine()
        assertEquals(listOf(Span.from("42")), line.spans)
    }

    @Test
    fun fromVec() {
        val spans = listOf(
            Span.styled("Hello,", Style.default().fg(Color.Red)),
            Span.styled(" world!", Style.default().fg(Color.Green))
        )
        val line = Line.from(spans)
        assertEquals(spans, line.spans)
    }

    @Test
    fun fromIter() {
        val line = Line.fromIter(listOf(
            Span.styled("Hello", Style.new().blue()),
            Span.styled(" world!", Style.new().green())
        ))
        assertEquals(
            listOf(
                Span.styled("Hello", Style.new().blue()),
                Span.styled(" world!", Style.new().green())
            ),
            line.spans
        )
    }

    @Test
    fun fromSpan() {
        val span = Span.styled("Hello, world!", Style.default().fg(Color.Yellow))
        val line = Line.from(span)
        assertEquals(listOf(span), line.spans)
    }

    @Test
    fun addSpan() {
        val line = Line.raw("Red").style(Style.new().red()) + Span.raw("blue").blue()
        assertEquals(
            Line(
                spans = mutableListOf(Span.raw("Red"), Span.raw("blue").blue()),
                style = Style.new().red(),
                alignment = null
            ),
            line
        )
    }

    @Test
    fun intoString() {
        val line = Line.from(listOf(
            Span.styled("Hello,", Style.default().fg(Color.Red)),
            Span.styled(" world!", Style.default().fg(Color.Green))
        ))
        val s = line.toString()
        assertEquals("Hello, world!", s)
    }

    @Test
    fun styledGraphemes() {
        val red = Style.new().red()
        val green = Style.new().green()
        val blue = Style.new().blue()
        val redOnWhite = Style.new().red().onWhite()
        val greenOnWhite = Style.new().green().onWhite()
        val blueOnWhite = Style.new().blue().onWhite()

        val line = Line.from(listOf(
            Span.styled("He", red),
            Span.styled("ll", green),
            Span.styled("o!", blue)
        ))
        val styledGraphemes = line.styledGraphemes(Style.new().bg(Color.White))
        assertEquals(
            listOf(
                StyledGrapheme("H", redOnWhite),
                StyledGrapheme("e", redOnWhite),
                StyledGrapheme("l", greenOnWhite),
                StyledGrapheme("l", greenOnWhite),
                StyledGrapheme("o", blueOnWhite),
                StyledGrapheme("!", blueOnWhite)
            ),
            styledGraphemes
        )
    }

    @Test
    fun displayLineFromVec() {
        val lineFromVec = Line.from(listOf(Span.raw("Hello,"), Span.raw(" world!")))
        assertEquals("Hello, world!", lineFromVec.toString())
    }

    @Test
    fun displayStyledLine() {
        val styledLine = Line.styled("Hello, world!", Style.new().green().italic())
        assertEquals("Hello, world!", styledLine.toString())
    }

    @Test
    fun displayLineFromStyledSpan() {
        val styledSpan = Span.styled("Hello, world!", Style.new().green().italic())
        val lineFromStyledSpan = Line.from(styledSpan)
        assertEquals("Hello, world!", lineFromStyledSpan.toString())
    }

    @Test
    fun leftAligned() {
        val line = Line.from("Hello, world!").leftAligned()
        assertEquals(Alignment.Left, line.alignment)
    }

    @Test
    fun centered() {
        val line = Line.from("Hello, world!").centered()
        assertEquals(Alignment.Center, line.alignment)
    }

    @Test
    fun rightAligned() {
        val line = Line.from("Hello, world!").rightAligned()
        assertEquals(Alignment.Right, line.alignment)
    }

    @Test
    fun pushSpan() {
        val line = Line.from("A")
        line.pushSpan(Span.raw("B"))
        line.pushSpan("C")
        assertEquals(
            listOf(Span.raw("A"), Span.raw("B"), Span.raw("C")),
            line.spans
        )
    }
}

/**
 * Iterator tests for Line.
 */
class LineIteratorTest {

    private fun helloWorld(): Line = Line.from(listOf(
        Span.styled("Hello ", Style.default().fg(Color.Blue)),
        Span.styled("world!", Style.default().fg(Color.Green))
    ))

    @Test
    fun iter() {
        val line = helloWorld()
        val iter = line.iterator()
        assertEquals(Span.styled("Hello ", Style.default().fg(Color.Blue)), iter.next())
        assertEquals(Span.styled("world!", Style.default().fg(Color.Green)), iter.next())
        assertEquals(false, iter.hasNext())
    }

    @Test
    fun forLoopRef() {
        val line = helloWorld()
        var result = ""
        for (span in line) {
            result += span.content
        }
        assertEquals("Hello world!", result)
    }
}
