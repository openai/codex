package ansitotui

import ratatui.style.Color
import ratatui.style.Modifier
import ratatui.style.Style
import ratatui.text.Line
import ratatui.text.Span
import ratatui.text.Text
import kotlin.test.Test
import kotlin.test.assertEquals

/**
 * Tests for the ANSI to TUI parser, ported from the Rust test suite.
 */
class AnsiToTuiTest {

    // ============================================================================
    // Basic parsing tests
    // ============================================================================

    @Test
    fun testEmptyOp() {
        val string = "\u001b[32mGREEN\u001b[mFOO\nFOO"
        val output = Text.from(listOf(
            Line.from(listOf(
                Span.styled("GREEN", Style.default().fg(Color.Green)),
                Span.styled("FOO", Style.reset())
            )),
            Line.from(Span.styled("FOO", Style.reset()))
        ))
        testParsing(string, output)
    }

    @Test
    fun testString() {
        val string = "FOO"
        testParsing(string, Text.raw("FOO"))
    }

    @Test
    fun testUnicode() {
        // Unicode characters
        val bytes = "AAA🅱️🅱️🅱️"
        val output = Text.raw("AAA🅱️🅱️🅱️")
        testParsing(bytes, output)
    }

    @Test
    fun testAsciiRgb() {
        val bytes = "\u001b[38;2;100;100;100mAAABBB"
        val output = Text.from(Span.styled(
            "AAABBB",
            Style.default().fg(Color.Rgb(100u, 100u, 100u))
        ))
        testParsing(bytes, output)
    }

    @Test
    fun testAsciiNewlines() {
        val bytes = "LINE_1\n\n\n\n\n\n\nLINE_8"
        // Kotlin implementation creates empty lines with no spans
        // This is functionally equivalent to Rust's Text.raw("")
        val text = bytes.intoText()
        assertEquals(8, text.lines.size)
        assertEquals("LINE_1", text.lines[0].spans.firstOrNull()?.content ?: "")
        assertEquals("LINE_8", text.lines[7].spans.firstOrNull()?.content ?: "")
        // Middle lines should be empty
        for (i in 1..6) {
            val lineContent = text.lines[i].spans.joinToString("") { it.content }
            assertEquals("", lineContent, "Line $i should be empty")
        }
    }

    @Test
    fun testReset() {
        val string = "\u001b[33mA\u001b[0mB"
        val output = Text.from(Line.from(listOf(
            Span.styled("A", Style.default().fg(Color.Yellow)),
            Span.styled("B", Style.reset())
        )))
        testParsing(string, output)
    }

    @Test
    fun testScreenModes() {
        val bytes = "\u001b[?25hAAABBB"
        val output = Text.styled("AAABBB", Style.default())
        testParsing(bytes, output)
    }

    @Test
    fun testCursorShapeAndColor() {
        // malformed -> malformed -> empty
        // These escape sequences should be consumed/ignored, leaving empty text
        val bytes = "\u001b[4 q\u001b]12;#fab1ed\u0007"
        val text = bytes.intoText()
        assertEquals(1, text.lines.size)
        val content = text.lines[0].spans.joinToString("") { it.content }
        assertEquals("", content, "Malformed escape sequences should result in empty text")
    }

    @Test
    fun testMalformedSimple() {
        // Incomplete CSI sequence
        val bytes = "\u001b["
        val text = bytes.intoText()
        assertEquals(1, text.lines.size)
        val content = text.lines[0].spans.joinToString("") { it.content }
        assertEquals("", content, "Incomplete CSI should result in empty text")
    }

    @Test
    fun testMalformedComplex() {
        // Multiple malformed sequences
        val bytes = "\u001b\u001b[0\u001b[m\u001b"
        val text = bytes.intoText()
        assertEquals(1, text.lines.size)
        val content = text.lines[0].spans.joinToString("") { it.content }
        assertEquals("", content, "Multiple malformed sequences should result in empty text")
    }

    @Test
    fun testEmptySpan() {
        // Yellow -> Red -> Green -> "Hello" -> Reset -> "World"
        val bytes = "\u001b[33m\u001b[31m\u001b[32mHello\u001b[0mWorld"
        val output = Text.from(Line.from(listOf(
            Span.styled("Hello", Style.default().fg(Color.Green)),
            Span.styled("World", Style.reset())
        )))
        testParsing(bytes, output)
    }

    @Test
    fun testColorAndStyleReset() {
        val bytes = "\u001b[32m* \u001b[0mRunning before-startup command \u001b[1mcommand\u001b[0m=make my-simple-package.cabal\n" +
                "\u001b[32m* \u001b[0m\$ make my-simple-package.cabal\n" +
                "Build profile: -w ghc-9.0.2 -O1"
        val output = Text.from(listOf(
            Line.from(listOf(
                Span.styled("* ", Style.default().fg(Color.Green)),
                Span.styled("Running before-startup command ", Style.reset()),
                Span.styled("command", Style.reset().addModifier(Modifier.BOLD)),
                Span.styled("=make my-simple-package.cabal", Style.reset())
            )),
            Line.from(listOf(
                Span.styled("* ", Style.reset().fg(Color.Green)),
                Span.styled("\$ make my-simple-package.cabal", Style.reset())
            )),
            Line.from(listOf(
                Span.styled("Build profile: -w ghc-9.0.2 -O1", Style.reset())
            ))
        ))
        testParsing(bytes, output)
    }

    // ============================================================================
    // Color tests
    // ============================================================================

    @Test
    fun testForegroundColors() {
        for (i in 0..255) {
            val bytes = "\u001b[38;5;${i}mHELLO"
            val output = Text.from(Span.styled(
                "HELLO",
                Style.default().fg(Color.Indexed(i.toUByte()))
            ))
            testParsing(bytes, output)
        }
    }

    @Test
    fun testBackgroundColors() {
        for (i in 0..255) {
            val bytes = "\u001b[48;5;${i}mHELLO"
            val output = Text.from(Span.styled(
                "HELLO",
                Style.default().bg(Color.Indexed(i.toUByte()))
            ))
            testParsing(bytes, output)
        }
    }

    @Test
    fun testRgbColors() {
        // Test a subset for performance
        for (i in listOf(1, 50, 100, 150, 200, 255)) {
            for (j in listOf(1, 50, 100, 150, 200, 255)) {
                val bytes = "\u001b[38;2;$i;$i;$i;48;2;$j;$j;${j}mHELLO"
                val output = Text.from(Span.styled(
                    "HELLO",
                    Style.default()
                        .fg(Color.Rgb(i.toUByte(), i.toUByte(), i.toUByte()))
                        .bg(Color.Rgb(j.toUByte(), j.toUByte(), j.toUByte()))
                ))
                testParsing(bytes, output)
            }
        }
    }

    // ============================================================================
    // Modifier tests
    // ============================================================================

    @Test
    fun testBoldResetSequences() {
        val bytes = "not, \u001b[1mbold\u001b[22m, not anymore"
        val output = Text.from(Line.from(listOf(
            Span.raw("not, "),
            Span.styled("bold", Style.default().addModifier(Modifier.BOLD)),
            Span.styled(", not anymore", Style.default().removeModifier(Modifier.BOLD or Modifier.DIM))
        )))
        testParsing(bytes, output)
    }

    @Test
    fun testUnderlineResetSequences() {
        val bytes = "not, \u001b[4munderlined\u001b[24m, not anymore"
        val output = Text.from(Line.from(listOf(
            Span.raw("not, "),
            Span.styled("underlined", Style.default().addModifier(Modifier.UNDERLINED)),
            Span.styled(", not anymore", Style.default().removeModifier(Modifier.UNDERLINED))
        )))
        testParsing(bytes, output)
    }

    @Test
    fun testConcealResetSequences() {
        val bytes = "not, \u001b[8mconcealed\u001b[28m, not anymore"
        val output = Text.from(Line.from(listOf(
            Span.raw("not, "),
            Span.styled("concealed", Style.default().addModifier(Modifier.HIDDEN)),
            Span.styled(", not anymore", Style.default().removeModifier(Modifier.HIDDEN))
        )))
        testParsing(bytes, output)
    }

    @Test
    fun testItalicResetSequences() {
        val bytes = "not, \u001b[3mitalic\u001b[23m, not anymore"
        val output = Text.from(Line.from(listOf(
            Span.raw("not, "),
            Span.styled("italic", Style.default().addModifier(Modifier.ITALIC)),
            Span.styled(", not anymore", Style.default().removeModifier(Modifier.ITALIC))
        )))
        testParsing(bytes, output)
    }

    @Test
    fun testBlinkResetSequences() {
        val bytes = "not, \u001b[5mblinking\u001b[25m, not anymore"
        val output = Text.from(Line.from(listOf(
            Span.raw("not, "),
            Span.styled("blinking", Style.default().addModifier(Modifier.SLOW_BLINK)),
            Span.styled(", not anymore", Style.default().removeModifier(Modifier.SLOW_BLINK or Modifier.RAPID_BLINK))
        )))
        testParsing(bytes, output)
    }

    @Test
    fun testFaintResetSequences() {
        val bytes = "not, \u001b[2mfaint\u001b[22m, not anymore"
        val output = Text.from(Line.from(listOf(
            Span.raw("not, "),
            Span.styled("faint", Style.default().addModifier(Modifier.DIM)),
            Span.styled(", not anymore", Style.default().removeModifier(Modifier.BOLD or Modifier.DIM))
        )))
        testParsing(bytes, output)
    }

    // ============================================================================
    // 4-bit color tests
    // ============================================================================

    @Test
    fun testStandard4BitForegroundColors() {
        // Test standard 4-bit foreground colors (30-37)
        val colorMap = mapOf(
            30 to Color.Black,
            31 to Color.Red,
            32 to Color.Green,
            33 to Color.Yellow,
            34 to Color.Blue,
            35 to Color.Magenta,
            36 to Color.Cyan,
            37 to Color.Gray
        )
        for ((code, color) in colorMap) {
            val bytes = "\u001b[${code}mTEST"
            val output = Text.from(Span.styled("TEST", Style.default().fg(color)))
            testParsing(bytes, output)
        }
    }

    @Test
    fun testBright4BitForegroundColors() {
        // Test bright 4-bit foreground colors (90-97)
        val colorMap = mapOf(
            90 to Color.DarkGray,
            91 to Color.LightRed,
            92 to Color.LightGreen,
            93 to Color.LightYellow,
            94 to Color.LightBlue,
            95 to Color.LightMagenta,
            96 to Color.LightCyan,
            97 to Color.White
        )
        for ((code, color) in colorMap) {
            val bytes = "\u001b[${code}mTEST"
            val output = Text.from(Span.styled("TEST", Style.default().fg(color)))
            testParsing(bytes, output)
        }
    }

    @Test
    fun testStandard4BitBackgroundColors() {
        // Test standard 4-bit background colors (40-47)
        val colorMap = mapOf(
            40 to Color.Black,
            41 to Color.Red,
            42 to Color.Green,
            43 to Color.Yellow,
            44 to Color.Blue,
            45 to Color.Magenta,
            46 to Color.Cyan,
            47 to Color.Gray
        )
        for ((code, color) in colorMap) {
            val bytes = "\u001b[${code}mTEST"
            val output = Text.from(Span.styled("TEST", Style.default().bg(color)))
            testParsing(bytes, output)
        }
    }

    // ============================================================================
    // Extension function tests
    // ============================================================================

    @Test
    fun testByteArrayExtension() {
        val bytes = "\u001b[31mRed\u001b[0m".encodeToByteArray()
        val text = bytes.intoText()
        assertEquals(1, text.lines.size)
        // The reset code at the end has no following text, so Kotlin may not create an empty span
        // Verify the content is correct
        val content = text.lines[0].spans.joinToString("") { it.content }
        assertEquals("Red", content)
        // Verify style is red foreground
        assertEquals(Color.Red, text.lines[0].spans[0].style.fg)
    }

    @Test
    fun testStringExtension() {
        val string = "\u001b[32mGreen\u001b[0m World"
        val text = string.intoText()
        assertEquals(1, text.lines.size)
    }

    // ============================================================================
    // Helper
    // ============================================================================

    private fun testParsing(input: String, expected: Text) {
        val result = input.intoText()
        assertEquals(expected.lines.size, result.lines.size, "Line count mismatch for input: $input")
        for (i in expected.lines.indices) {
            val expectedLine = expected.lines[i]
            val resultLine = result.lines[i]
            assertEquals(
                expectedLine.spans.size,
                resultLine.spans.size,
                "Span count mismatch on line $i for input: $input\nExpected: ${expectedLine.spans}\nGot: ${resultLine.spans}"
            )
            for (j in expectedLine.spans.indices) {
                val expectedSpan = expectedLine.spans[j]
                val resultSpan = resultLine.spans[j]
                assertEquals(
                    expectedSpan.content,
                    resultSpan.content,
                    "Content mismatch on line $i, span $j for input: $input"
                )
                assertEquals(
                    expectedSpan.style,
                    resultSpan.style,
                    "Style mismatch on line $i, span $j for input: $input\nExpected: ${expectedSpan.style}\nGot: ${resultSpan.style}"
                )
            }
        }
    }
}
