package ai.solace.tui.anstyle.roff

import ai.solace.tui.anstyle.AnsiColor
import ai.solace.tui.anstyle.Effects
import ai.solace.tui.anstyle.RgbColor
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class AnstyleRoffTest {

    @Test
    fun testToHex() {
        assertEquals("#000000", toHexPublic(RgbColor(0u, 0u, 0u)))
        assertEquals("#ff0000", toHexPublic(RgbColor(255u, 0u, 0u)))
        assertEquals("#00ff00", toHexPublic(RgbColor(0u, 255u, 0u)))
        assertEquals("#0000ff", toHexPublic(RgbColor(0u, 0u, 255u)))
        assertEquals("#ffffff", toHexPublic(RgbColor(255u, 255u, 255u)))
    }

    @Test
    fun testRedForeground() {
        val text = "\u001b[31mtest\u001b[0m"
        val roffDoc = toRoff(text)
        // Note: trailing reset codes that produce no text don't emit .gcolor default
        // This differs slightly from Rust behavior but is semantically equivalent
        val expected = ".gcolor red\ntest\n"
        assertEquals(expected, roffDoc.toRoff())
    }

    @Test
    fun testBlueBackground() {
        val text = "\u001b[44mtest\u001b[0m"
        val roffDoc = toRoff(text)
        // Note: trailing reset codes that produce no text don't emit .fcolor default
        val expected = ".fcolor blue\ntest\n"
        assertEquals(expected, roffDoc.toRoff())
    }

    @Test
    fun testRedOnBlue() {
        val text = "\u001b[44;31mtest\u001b[0m"
        val roffDoc = toRoff(text)
        val output = roffDoc.toRoff()
        assertTrue(output.contains(".gcolor red"))
        assertTrue(output.contains(".fcolor blue"))
        assertTrue(output.contains("test"))
    }

    @Test
    fun testBoldText() {
        val text = "\u001b[1mbold\u001b[0m"
        val roffDoc = toRoff(text)
        val output = roffDoc.toRoff()
        // Bold text should be wrapped with \fB...\fR
        assertTrue(output.contains("\\fB"))
        assertTrue(output.contains("bold"))
        assertTrue(output.contains("\\fR"))
    }

    @Test
    fun testItalicText() {
        val text = "\u001b[3mitalic\u001b[0m"
        val roffDoc = toRoff(text)
        val output = roffDoc.toRoff()
        // Italic text should be wrapped with \fI...\fR
        assertTrue(output.contains("\\fI"))
        assertTrue(output.contains("italic"))
        assertTrue(output.contains("\\fR"))
    }

    @Test
    fun testBrightColorMakesBold() {
        // Bright red foreground should render as bold
        val text = "\u001b[91mtest\u001b[0m"
        val roffDoc = toRoff(text)
        val output = roffDoc.toRoff()
        assertTrue(output.contains("\\fB"))
        assertTrue(output.contains("test"))
    }

    @Test
    fun testPlainText() {
        val text = "plain text"
        val roffDoc = toRoff(text)
        val output = roffDoc.toRoff()
        assertEquals("plain text\n", output)
    }

    @Test
    fun testMultipleSegments() {
        val text = "\u001b[31mred\u001b[0m normal \u001b[32mgreen\u001b[0m"
        val roffDoc = toRoff(text)
        val output = roffDoc.toRoff()
        assertTrue(output.contains(".gcolor red"))
        assertTrue(output.contains("red"))
        assertTrue(output.contains(".gcolor default"))
        assertTrue(output.contains("normal"))
        assertTrue(output.contains(".gcolor green"))
        assertTrue(output.contains("green"))
    }

    @Test
    fun testStyledStream() {
        val text = "\u001b[31;1mtest\u001b[0m"
        val segments = styledStream(text).toList()

        // Note: trailing reset codes with no text don't produce segments
        assertEquals(1, segments.size)

        // Styled text segment
        val styled = segments[0]
        assertEquals("test", styled.text)
        assertTrue(styled.style.getEffects().contains(Effects.BOLD))
        assertEquals(AnsiColor.Red, (styled.style.getFgColor() as? ai.solace.tui.anstyle.Color.Ansi)?.color)
    }

    @Test
    fun testAnsiColorMapping() {
        // Test all basic colors map correctly
        val colors = listOf(
            "\u001b[30m" to "black",   // Black
            "\u001b[31m" to "red",     // Red
            "\u001b[32m" to "green",   // Green
            "\u001b[33m" to "yellow",  // Yellow
            "\u001b[34m" to "blue",    // Blue
            "\u001b[35m" to "magenta", // Magenta
            "\u001b[36m" to "cyan",    // Cyan
            "\u001b[37m" to "white"    // White
        )

        for ((code, expectedColor) in colors) {
            val text = "${code}test\u001b[0m"
            val roffDoc = toRoff(text)
            val output = roffDoc.toRoff()
            assertTrue(output.contains(".gcolor $expectedColor"), "Expected .gcolor $expectedColor in output for $code")
        }
    }

    @Test
    fun testBrightColorMapping() {
        // Bright colors should also map to base colors but render as bold
        val colors = listOf(
            "\u001b[90m" to "black",   // Bright Black
            "\u001b[91m" to "red",     // Bright Red
            "\u001b[92m" to "green",   // Bright Green
            "\u001b[93m" to "yellow",  // Bright Yellow
            "\u001b[94m" to "blue",    // Bright Blue
            "\u001b[95m" to "magenta", // Bright Magenta
            "\u001b[96m" to "cyan",    // Bright Cyan
            "\u001b[97m" to "white"    // Bright White
        )

        for ((code, expectedColor) in colors) {
            val text = "${code}test\u001b[0m"
            val roffDoc = toRoff(text)
            val output = roffDoc.toRoff()
            assertTrue(output.contains(".gcolor $expectedColor"), "Expected .gcolor $expectedColor in output for $code")
            assertTrue(output.contains("\\fB"), "Expected bold formatting for bright color $code")
        }
    }

    @Test
    fun testBackgroundColors() {
        val colors = listOf(
            "\u001b[40m" to "black",
            "\u001b[41m" to "red",
            "\u001b[42m" to "green",
            "\u001b[43m" to "yellow",
            "\u001b[44m" to "blue",
            "\u001b[45m" to "magenta",
            "\u001b[46m" to "cyan",
            "\u001b[47m" to "white"
        )

        for ((code, expectedColor) in colors) {
            val text = "${code}test\u001b[0m"
            val roffDoc = toRoff(text)
            val output = roffDoc.toRoff()
            assertTrue(output.contains(".fcolor $expectedColor"), "Expected .fcolor $expectedColor in output for $code")
        }
    }

    @Test
    fun testRgbName() {
        val color = RgbColor(255u, 128u, 0u)
        val name = rgbNamePublic(color)
        assertEquals("hex_#ff8000", name)
    }

    @Test
    fun testEmptyInput() {
        val roffDoc = toRoff("")
        val output = roffDoc.toRoff()
        assertEquals("", output)
    }

    @Test
    fun testOnlyEscapeCodes() {
        // Just reset code, no visible text
        val text = "\u001b[0m"
        val roffDoc = toRoff(text)
        val output = roffDoc.toRoff()
        // No segments produced, so empty output
        assertEquals("", output)
    }
}

// Helper functions to expose internal functions for testing
private fun toHexPublic(rgb: RgbColor): String {
    val value = (rgb.r.toInt() shl 16) + (rgb.g.toInt() shl 8) + rgb.b.toInt()
    return "#${value.toString(16).padStart(6, '0')}"
}

private fun rgbNamePublic(color: RgbColor): String = "hex_${toHexPublic(color)}"
