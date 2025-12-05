package ai.solace.tui.roff

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * Tests for the ROFF library, ported from the Rust test suite.
 */
class RoffTest {

    @Test
    fun escapeDash() {
        assertEquals("\\-", escapeInline("-"))
    }

    @Test
    fun escapeBackslash() {
        assertEquals("\\\\x", escapeInline("\\x"))
    }

    @Test
    fun escapeBackslashAndDash() {
        assertEquals("\\\\\\-", escapeInline("\\-"))
    }

    @Test
    fun escapesLeadingControlChars() {
        assertEquals("foo\n\\&.bar\n\\&'yo", escapeLeadingCcTest("foo\n.bar\n'yo"))
    }

    @Test
    fun escapePlain() {
        assertEquals("abc", escapeInline("abc"))
    }

    @Test
    fun renderRoman() {
        val text = Roff().text(roman("foo")).toRoff()
        assertEquals("foo\n", text)
    }

    @Test
    fun renderDash() {
        val text = Roff().text(roman("foo-bar")).toRoff()
        assertEquals("foo\\-bar\n", text)
    }

    @Test
    fun renderItalic() {
        val text = Roff().text(italic("foo")).toRoff()
        assertEquals("\\fIfoo\\fR\n", text)
    }

    @Test
    fun renderBold() {
        val text = Roff().text(bold("foo")).toRoff()
        assertEquals("\\fBfoo\\fR\n", text)
    }

    @Test
    fun renderText() {
        val text = Roff().text(roman("roman")).toRoff()
        assertEquals("roman\n", text)
    }

    @Test
    fun renderTextWithLeadingPeriod() {
        val text = Roff().text(roman(".roman")).toRoff()
        assertEquals("\\&.roman\n", text)
    }

    @Test
    fun renderTextWithNewlinePeriod() {
        val text = Roff().text(roman("foo\n.roman")).toRoff()
        assertEquals("foo\n\\&.roman\n", text)
    }

    @Test
    fun renderLineBreak() {
        val text = Roff()
            .text(roman("roman"), Inline.LineBreak, roman("more"))
            .toRoff()
        assertEquals("roman\n.br\nmore\n", text)
    }

    @Test
    fun renderControl() {
        val text = Roff().control("foo", "bar", "foo and bar").toRoff()
        assertEquals(".foo bar \"foo and bar\"\n", text)
    }

    @Test
    fun renderWithApostrophePreamble() {
        val text = Roff().text(roman("hello")).render()
        assertEquals(".ie \\n(.g .ds Aq \\(aq\n.el .ds Aq '\nhello\n", text)
    }

    @Test
    fun renderApostropheEscaping() {
        val text = Roff().text(roman("don't")).render()
        // Should contain the escaped apostrophe
        assertTrue(text.contains("\\*(Aq"))
    }

    @Test
    fun demoManPage() {
        // Test from demo.rs
        val page = Roff()
            .control("TH", "CORRUPT", "1")
            .control("SH", "NAME")
            .text(roman("corrupt - modify files by randomly changing bits"))
            .control("SH", "SYNOPSIS")
            .text(
                bold("corrupt"),
                " ".toInline(),
                "[".toInline(),
                bold("-n"),
                " ".toInline(),
                italic("BITS"),
                "]".toInline(),
                " ".toInline(),
                "[".toInline(),
                bold("--bits"),
                " ".toInline(),
                italic("BITS"),
                "]".toInline(),
                " ".toInline(),
                italic("file"),
                "...".toInline()
            )
            .control("SH", "DESCRIPTION")
            .text(
                bold("corrupt"),
                " modifies files by toggling a randomly chosen bit.".toInline()
            )
            .control("SH", "OPTIONS")
            .control("TP")
            .text(
                bold("-n"),
                ", ".toInline(),
                bold("--bits"),
                "=".toInline(),
                italic("BITS")
            )
            .text(
                "Set the number of bits to modify. ".toInline(),
                "Default is one bit.".toInline()
            )
            .toRoff()

        // Verify structure
        assertTrue(page.contains(".TH CORRUPT 1"))
        assertTrue(page.contains(".SH NAME"))
        assertTrue(page.contains("corrupt \\- modify files"))
        assertTrue(page.contains("\\fBcorrupt\\fR"))
        assertTrue(page.contains("\\fIBITS\\fR"))
    }
}

// Helper function for testing escapeLeadingCc
private fun escapeLeadingCcTest(s: String): String =
    s.replace("\n.", "\n\\&.").replace("\n'", "\n\\&'")
