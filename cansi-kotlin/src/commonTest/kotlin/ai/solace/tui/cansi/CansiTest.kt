package ai.solace.tui.cansi

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class CansiTest {

    // ========== Parsing Tests ==========

    @Test
    fun parseBasicAnsiSequences() {
        val ansiText = "Hello, \u001b[31;4mworld\u001b[0m!"
        val parsed = parse(ansiText)

        assertEquals(2, parsed.size)
        assertEquals(7, parsed[0].start)
        assertEquals(14, parsed[0].end)
        assertEquals("\u001b[31;4m", parsed[0].text)

        assertEquals(19, parsed[1].start)
        assertEquals(23, parsed[1].end)
        assertEquals("\u001b[0m", parsed[1].text)
    }

    @Test
    fun parseStringWithEmoji() {
        val text = "\uD83D\uDC4B, \u001b[31;4m\uD83C\uDF0D\u001b[0m!"
        val parsed = parse(text)

        assertEquals(2, parsed.size)
        // Wave emoji is 4 bytes, comma+space is 2 bytes = 6 bytes offset
        assertEquals(6, parsed[0].start)
        assertEquals(13, parsed[0].end)
        assertEquals("\u001b[31;4m", parsed[0].text)

        // After first escape (13) + globe emoji (4) = 17
        assertEquals(17, parsed[1].start)
        assertEquals(21, parsed[1].end)
        assertEquals("\u001b[0m", parsed[1].text)
    }

    @Test
    fun parseMalformedEscape() {
        val result = parse("oops\u001b[\n")
        assertEquals(0, result.size)
    }

    @Test
    fun parseNoEscapeSequences() {
        val result = parse("Hello, world!")
        assertEquals(0, result.size)
    }

    // ========== Categorisation Tests ==========

    @Test
    fun categoriseNoEscapeSequences() {
        val text = "test"
        val result = categoriseText(text)

        assertEquals(1, result.size)
        assertEquals("test", result[0].text)
        assertEquals(0, result[0].start)
        assertEquals(4, result[0].end)
        assertEquals(null, result[0].fg)
        assertEquals(null, result[0].bg)
        assertEquals(null, result[0].intensity)
    }

    @Test
    fun categoriseEmptySequences() {
        val text = "\u001b[;mtest"
        val result = categoriseText(text)

        assertEquals(1, result.size)
        assertEquals("test", result[0].text)
        assertEquals(4, result[0].start)
        assertEquals(8, result[0].end)
    }

    @Test
    fun categoriseMultipleEmptySequences() {
        val text = "\u001b[;mtest\u001b[;m\u001b[;m"
        val result = categoriseText(text)

        assertEquals(1, result.size)
        assertEquals("test", result[0].text)
    }

    @Test
    fun categoriseBoldThenNormal() {
        val text = "\u001b[1;22mtest"
        val result = categoriseText(text)

        assertEquals(1, result.size)
        assertEquals("test", result[0].text)
        assertEquals(Intensity.Normal, result[0].intensity)
    }

    @Test
    fun categoriseItalicThenNotItalic() {
        val text = "\u001b[3;23mtest"
        val result = categoriseText(text)

        assertEquals(1, result.size)
        assertEquals(false, result[0].italic)
    }

    @Test
    fun categoriseUnderlineThenNoUnderline() {
        val text = "\u001b[4;24mtest"
        val result = categoriseText(text)

        assertEquals(1, result.size)
        assertEquals(false, result[0].underline)
    }

    @Test
    fun categoriseBlinkThenNoBlink() {
        val text = "\u001b[5;25mtest"
        val result = categoriseText(text)

        assertEquals(1, result.size)
        assertEquals(false, result[0].blink)
    }

    @Test
    fun categoriseReversedThenNotReversed() {
        val text = "\u001b[7;27mtest"
        val result = categoriseText(text)

        assertEquals(1, result.size)
        assertEquals(false, result[0].reversed)
    }

    @Test
    fun categoriseHiddenThenNotHidden() {
        val text = "\u001b[8;28mtest"
        val result = categoriseText(text)

        assertEquals(1, result.size)
        assertEquals(false, result[0].hidden)
    }

    @Test
    fun categoriseStrikethroughThenNoStrikethrough() {
        val text = "\u001b[9;29mtest"
        val result = categoriseText(text)

        assertEquals(1, result.size)
        assertEquals(false, result[0].strikethrough)
    }

    @Test
    fun categoriseForegroundColors() {
        // Test all basic foreground colors (30-37)
        val colors = listOf(
            "30" to Color.Black,
            "31" to Color.Red,
            "32" to Color.Green,
            "33" to Color.Yellow,
            "34" to Color.Blue,
            "35" to Color.Magenta,
            "36" to Color.Cyan,
            "37" to Color.White
        )

        for ((code, expectedColor) in colors) {
            val text = "\u001b[${code}mtest"
            val result = categoriseText(text)
            assertEquals(expectedColor, result[0].fg, "Expected $expectedColor for code $code")
        }
    }

    @Test
    fun categoriseBackgroundColors() {
        // Test all basic background colors (40-47)
        val colors = listOf(
            "40" to Color.Black,
            "41" to Color.Red,
            "42" to Color.Green,
            "43" to Color.Yellow,
            "44" to Color.Blue,
            "45" to Color.Magenta,
            "46" to Color.Cyan,
            "47" to Color.White
        )

        for ((code, expectedColor) in colors) {
            val text = "\u001b[${code}mtest"
            val result = categoriseText(text)
            assertEquals(expectedColor, result[0].bg, "Expected $expectedColor for code $code")
        }
    }

    @Test
    fun categoriseBrightForegroundColors() {
        // Test bright foreground colors (90-97)
        val colors = listOf(
            "90" to Color.BrightBlack,
            "91" to Color.BrightRed,
            "92" to Color.BrightGreen,
            "93" to Color.BrightYellow,
            "94" to Color.BrightBlue,
            "95" to Color.BrightMagenta,
            "96" to Color.BrightCyan,
            "97" to Color.BrightWhite
        )

        for ((code, expectedColor) in colors) {
            val text = "\u001b[${code}mtest"
            val result = categoriseText(text)
            assertEquals(expectedColor, result[0].fg, "Expected $expectedColor for code $code")
        }
    }

    @Test
    fun categoriseBrightBackgroundColors() {
        // Test bright background colors (100-107)
        val colors = listOf(
            "100" to Color.BrightBlack,
            "101" to Color.BrightRed,
            "102" to Color.BrightGreen,
            "103" to Color.BrightYellow,
            "104" to Color.BrightBlue,
            "105" to Color.BrightMagenta,
            "106" to Color.BrightCyan,
            "107" to Color.BrightWhite
        )

        for ((code, expectedColor) in colors) {
            val text = "\u001b[${code}mtest"
            val result = categoriseText(text)
            assertEquals(expectedColor, result[0].bg, "Expected $expectedColor for code $code")
        }
    }

    @Test
    fun categoriseRedAndUnderlined() {
        val text = "\u001b[31;4m\uD83C\uDF0D\u001b[0m!"
        val result = categoriseText(text)

        assertEquals(2, result.size)

        // First slice: globe emoji with red and underline
        assertEquals("\uD83C\uDF0D", result[0].text)
        assertEquals(Color.Red, result[0].fg)
        assertEquals(true, result[0].underline)

        // Second slice: exclamation with reset
        assertEquals("!", result[1].text)
        assertEquals(null, result[1].fg)
        assertEquals(null, result[1].underline)
    }

    @Test
    fun categoriseMalformedEscapes() {
        val result = categoriseText("oops\u001b[\n")

        assertEquals(1, result.size)
        assertEquals("oops\u001b[\n", result[0].text)
        assertEquals(0, result[0].start)
        assertEquals(7, result[0].end)
    }

    @Test
    fun categoriseWithEmoji() {
        val text = "\uD83D\uDC4B, \u001b[31;4m\uD83C\uDF0D\u001b[0m!"
        val result = categoriseText(text)

        assertEquals(constructTextNoCodes(result), "\uD83D\uDC4B, \uD83C\uDF0D!")

        assertEquals(3, result.size)

        // Wave emoji and comma-space
        assertEquals("\uD83D\uDC4B, ", result[0].text)
        assertEquals(0, result[0].start)
        assertEquals(6, result[0].end)
        assertEquals(null, result[0].fg)

        // Globe emoji with red underline
        assertEquals("\uD83C\uDF0D", result[1].text)
        assertEquals(13, result[1].start)
        assertEquals(17, result[1].end)
        assertEquals(Color.Red, result[1].fg)
        assertEquals(true, result[1].underline)

        // Exclamation mark
        assertEquals("!", result[2].text)
        assertEquals(21, result[2].start)
        assertEquals(22, result[2].end)
    }

    // ========== Construct Text No Codes Tests ==========

    @Test
    fun constructTextNoCodesBasic() {
        val categorized = categoriseText("\u001b[30mH\u001b[31me\u001b[32ml\u001b[33ml\u001b[34mo")
        assertEquals("Hello", constructTextNoCodes(categorized))
    }

    @Test
    fun constructTextNoCodesWithPlainText() {
        val categorized = categoriseText("Hello, world!")
        assertEquals("Hello, world!", constructTextNoCodes(categorized))
    }

    // ========== Line Iterator Tests ==========

    @Test
    fun lineIterSingleLine() {
        val slices = categoriseText("hello, world")
        val iter = lineIter(slices)

        assertTrue(iter.hasNext())
        val first = iter.next()
        assertEquals(1, first.size)
        assertEquals("hello, world", first[0].text)

        assertTrue(!iter.hasNext())
    }

    @Test
    fun lineIterTwoLines() {
        val slices = categoriseText("hello, world\nhow are you")
        val lines = lineIter(slices).asSequence().toList()

        assertEquals(2, lines.size)
        assertEquals("hello, world", lines[0][0].text)
        assertEquals("how are you", lines[1][0].text)
    }

    @Test
    fun lineIterMultipleNewlines() {
        val slices = categoriseText("\n\n\n\n")
        val lines = lineIter(slices).asSequence().toList()

        assertEquals(5, lines.size)
        for (line in lines) {
            assertEquals(1, line.size)
            assertEquals("", line[0].text)
        }
    }

    @Test
    fun lineIterCrLf() {
        val slices = categoriseText("\r\n\r\n\r\n\r\n")
        val lines = lineIter(slices).asSequence().toList()

        assertEquals(5, lines.size)
    }

    @Test
    fun lineIterMixedNewlines() {
        val slices = categoriseText("hello, world\nhow are you\r\ntoday")
        val lines = lineIter(slices).asSequence().toList()

        assertEquals(3, lines.size)
        assertEquals("hello, world", constructTextNoCodes(lines[0]))
        assertEquals("how are you", constructTextNoCodes(lines[1]))
        assertEquals("today", constructTextNoCodes(lines[2]))
    }

    @Test
    fun lineIterNewlineStartsWithEscape() {
        // Simulating "hello\n" followed by green "world"
        val text = "hello\n\u001b[32mworld\u001b[0m"
        val slices = categoriseText(text)
        val lines = lineIter(slices).asSequence().toList()

        assertEquals(2, lines.size)
        assertEquals("hello", lines[0][0].text)
        assertEquals("world", lines[1][0].text)
        assertEquals(Color.Green, lines[1][0].fg)
    }

    // ========== CategorisedSlice Tests ==========

    @Test
    fun cloneStyleTest() {
        val original = CategorisedSlice(
            text = "hello",
            start = 0,
            end = 5,
            fg = Color.Green,
            bg = Color.Black,
            intensity = Intensity.Bold
        )

        val cloned = original.cloneStyle("why", 10, 13)

        assertEquals("why", cloned.text)
        assertEquals(10, cloned.start)
        assertEquals(13, cloned.end)
        assertEquals(Color.Green, cloned.fg)
        assertEquals(Color.Black, cloned.bg)
        assertEquals(Intensity.Bold, cloned.intensity)
    }

    @Test
    fun defaultStyleTest() {
        val slice = CategorisedSlice.defaultStyle("test", 0, 4)

        assertEquals("test", slice.text)
        assertEquals(0, slice.start)
        assertEquals(4, slice.end)
        assertEquals(null, slice.fg)
        assertEquals(null, slice.bg)
        assertEquals(null, slice.intensity)
        assertEquals(null, slice.italic)
        assertEquals(null, slice.underline)
    }

    // ========== Unicode Handling Tests ==========

    @Test
    fun byteBug() {
        val s = "\uFEAE" // Half-width katakana letter WO
        val matches = parse(s)
        assertEquals(0, matches.size)

        val result = categoriseText(s)
        val constructed = constructTextNoCodes(result)
        assertEquals(s, constructed)
    }

    // ========== Match and Slice Count Relationship ==========

    @Test
    fun categorisedSlicesLessThanOrEqualToMatchesPlusOne() {
        val tests = listOf(
            "hello",
            "\u001b[32mhello\u001b[0m",
            "prefix\u001b[32mmiddle\u001b[31mend\u001b[0mfinal"
        )

        for (text in tests) {
            val matches = parse(text)
            val slices = categoriseText(text)
            assertTrue(
                slices.size <= matches.size + 1,
                "Expected slices (${slices.size}) <= matches + 1 (${matches.size + 1}) for: $text"
            )
        }
    }

    // ========== Intensity Tests ==========

    @Test
    fun intensityBold() {
        val text = "\u001b[1mtest"
        val result = categoriseText(text)
        assertEquals(Intensity.Bold, result[0].intensity)
    }

    @Test
    fun intensityFaint() {
        val text = "\u001b[2mtest"
        val result = categoriseText(text)
        assertEquals(Intensity.Faint, result[0].intensity)
    }

    @Test
    fun resetClearsAllStyles() {
        val text = "\u001b[1;3;4;31;42mstyledtext\u001b[0mreset"
        val result = categoriseText(text)

        assertEquals(2, result.size)

        // First slice has all styles
        assertEquals(Intensity.Bold, result[0].intensity)
        assertEquals(true, result[0].italic)
        assertEquals(true, result[0].underline)
        assertEquals(Color.Red, result[0].fg)
        assertEquals(Color.Green, result[0].bg)

        // Second slice after reset has no styles
        assertEquals(null, result[1].intensity)
        assertEquals(null, result[1].italic)
        assertEquals(null, result[1].underline)
        assertEquals(null, result[1].fg)
        assertEquals(null, result[1].bg)
    }
}
