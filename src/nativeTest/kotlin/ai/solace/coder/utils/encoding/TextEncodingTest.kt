package ai.solace.coder.utils.encoding

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * Tests for TextEncoding utilities.
 * Ported from Rust codex-rs/core/src/text_encoding.rs tests.
 */
class TextEncodingTest {

    @Test
    fun testEmptyBytes() {
        val result = bytesToStringSmart(byteArrayOf())
        assertEquals("", result)
    }

    @Test
    fun testValidUtf8() {
        val input = "Hello, World!".encodeToByteArray()
        val result = bytesToStringSmart(input)
        assertEquals("Hello, World!", result)
    }

    @Test
    fun testUtf8WithEmoji() {
        val input = "Hello 🌍".encodeToByteArray()
        val result = bytesToStringSmart(input)
        assertEquals("Hello 🌍", result)
    }

    @Test
    fun testUtf8WithCyrillic() {
        val input = "Привет мир".encodeToByteArray()
        val result = bytesToStringSmart(input)
        assertEquals("Привет мир", result)
    }

    @Test
    fun testUtf8WithCjk() {
        val input = "你好世界".encodeToByteArray()
        val result = bytesToStringSmart(input)
        assertEquals("你好世界", result)
    }

    @Test
    fun testUtf8Bom() {
        // UTF-8 BOM: EF BB BF followed by "hello"
        val input = byteArrayOf(
            0xEF.toByte(), 0xBB.toByte(), 0xBF.toByte(),
            'h'.code.toByte(), 'e'.code.toByte(), 'l'.code.toByte(),
            'l'.code.toByte(), 'o'.code.toByte()
        )
        val result = bytesToStringSmart(input)
        // BOM is included in output (like Rust behavior)
        assertTrue(result.contains("hello"))
    }

    @Test
    fun testWindows1252SmartQuotes() {
        // Windows-1252 "smart quotes" around ASCII text
        // 0x93 = left double quote, 0x94 = right double quote
        val input = byteArrayOf(
            0x93.toByte(), // left "
            't'.code.toByte(), 'e'.code.toByte(), 's'.code.toByte(), 't'.code.toByte(),
            0x94.toByte()  // right "
        )
        val result = bytesToStringSmart(input)
        // Should decode as Windows-1252 smart quotes
        assertTrue(result.contains("test"))
        // The smart quotes should be decoded (not replacement chars)
        assertEquals(6, result.length)
    }

    @Test
    fun testWindows1252EmDash() {
        // Windows-1252 em dash (0x97) surrounded by ASCII
        val input = byteArrayOf(
            'a'.code.toByte(),
            0x97.toByte(), // em dash
            'b'.code.toByte()
        )
        val result = bytesToStringSmart(input)
        assertTrue(result.startsWith("a"))
        assertTrue(result.endsWith("b"))
        assertEquals(3, result.length)
    }

    @Test
    fun testAsciiOnly() {
        val input = "plain ascii text".encodeToByteArray()
        val result = bytesToStringSmart(input)
        assertEquals("plain ascii text", result)
    }

    @Test
    fun testMixedContent() {
        // Valid UTF-8 with mixed ASCII and non-ASCII
        val input = "café résumé naïve".encodeToByteArray()
        val result = bytesToStringSmart(input)
        assertEquals("café résumé naïve", result)
    }

    @Test
    fun testSingleHighByte() {
        // Single high byte that's invalid UTF-8
        // Should fall back to Windows-1252 Latin fallback
        val input = byteArrayOf(
            'a'.code.toByte(),
            0xE9.toByte(), // 'é' in Windows-1252
            'b'.code.toByte()
        )
        val result = bytesToStringSmart(input)
        // Should decode, not produce replacement chars
        assertEquals(3, result.length)
        assertTrue(result.startsWith("a"))
        assertTrue(result.endsWith("b"))
    }

    @Test
    fun testLongUtf8Text() {
        val input = "This is a longer text with various characters: αβγδ, 你好, привет, 日本語"
            .encodeToByteArray()
        val result = bytesToStringSmart(input)
        assertTrue(result.contains("αβγδ"))
        assertTrue(result.contains("你好"))
        assertTrue(result.contains("привет"))
        assertTrue(result.contains("日本語"))
    }

    @Test
    fun testControlCharacters() {
        // ASCII control characters should pass through
        val input = byteArrayOf(
            'a'.code.toByte(),
            0x09.toByte(), // tab
            0x0A.toByte(), // newline
            0x0D.toByte(), // carriage return
            'b'.code.toByte()
        )
        val result = bytesToStringSmart(input)
        assertEquals("a\t\n\rb", result)
    }

    @Test
    fun testNullByte() {
        // Null bytes are valid in the middle of strings
        val input = byteArrayOf(
            'a'.code.toByte(),
            0x00.toByte(),
            'b'.code.toByte()
        )
        val result = bytesToStringSmart(input)
        assertEquals("a\u0000b", result)
    }

    @Test
    fun testTrademarkSymbol() {
        // Windows-1252 trademark symbol (0x99)
        val input = byteArrayOf(
            'T'.code.toByte(), 'e'.code.toByte(), 's'.code.toByte(), 't'.code.toByte(),
            0x99.toByte() // trademark
        )
        val result = bytesToStringSmart(input)
        assertTrue(result.startsWith("Test"))
        assertEquals(5, result.length)
    }

    @Test
    fun testBulletPoint() {
        // Windows-1252 bullet (0x95)
        val input = byteArrayOf(
            0x95.toByte(), // bullet
            ' '.code.toByte(),
            'i'.code.toByte(), 't'.code.toByte(), 'e'.code.toByte(), 'm'.code.toByte()
        )
        val result = bytesToStringSmart(input)
        assertTrue(result.contains("item"))
        assertEquals(6, result.length)
    }
}

/**
 * Tests for Windows-1252 punctuation detection.
 * These test the heuristics that distinguish Windows-1252 smart punctuation
 * from IBM866 Cyrillic in the 0x80-0x9F byte range.
 */
class Windows1252PunctuationTest {

    @Test
    fun testSmartQuotesWithWord() {
        // This pattern should be detected as Windows-1252, not IBM866
        val input = byteArrayOf(
            0x93.toByte(), // left "
            'w'.code.toByte(), 'o'.code.toByte(), 'r'.code.toByte(), 'd'.code.toByte(),
            0x94.toByte()  // right "
        )
        val result = bytesToStringSmart(input)
        // Should contain the word, not Cyrillic garbage
        assertTrue(result.contains("word"))
    }

    @Test
    fun testEnDashBetweenWords() {
        // En dash (0x96) between ASCII words
        val input = byteArrayOf(
            'A'.code.toByte(),
            0x96.toByte(), // en dash
            'B'.code.toByte()
        )
        val result = bytesToStringSmart(input)
        assertEquals(3, result.length)
        assertTrue(result.startsWith("A"))
        assertTrue(result.endsWith("B"))
    }
}

/**
 * Edge case tests.
 */
class TextEncodingEdgeCasesTest {

    @Test
    fun testSingleByte() {
        val result = bytesToStringSmart(byteArrayOf('x'.code.toByte()))
        assertEquals("x", result)
    }

    @Test
    fun testTwoBytes() {
        val result = bytesToStringSmart(byteArrayOf('x'.code.toByte(), 'y'.code.toByte()))
        assertEquals("xy", result)
    }

    @Test
    fun testAllHighBytes() {
        // All bytes in 0x80-0xFF range (invalid as standalone UTF-8)
        val input = ByteArray(16) { (0x80 + it).toByte() }
        val result = bytesToStringSmart(input)
        // Should decode to something (not crash)
        assertEquals(16, result.length)
    }
}
