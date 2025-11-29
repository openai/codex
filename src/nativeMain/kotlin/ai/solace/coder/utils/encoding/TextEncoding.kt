package ai.solace.coder.utils.encoding

import com.fleeksoft.charset.Charset
import com.fleeksoft.charset.Charsets
import com.fleeksoft.charset.decodeToString

/**
 * Text encoding detection and conversion utilities for shell output.
 *
 * Windows users frequently run into code pages such as CP1251 or CP866 when invoking commands
 * through VS Code. Those bytes show up as invalid UTF-8 and used to be replaced with the standard
 * Unicode replacement character. We now use charset detection heuristics so we can
 * automatically detect and decode the vast majority of legacy encodings before falling back to
 * lossy UTF-8 decoding.
 *
 * Ported from Rust codex-rs/core/src/text_encoding.rs
 */

/**
 * Windows-1252 byte values for smart punctuation.
 * These bytes in the 0x80-0x9F range map to curly quotes and dashes in Windows-1252,
 * but to Cyrillic letters in IBM866.
 */
private val WINDOWS_1252_PUNCT_BYTES = byteArrayOf(
    0x91.toByte(), // ' (left single quotation mark)
    0x92.toByte(), // ' (right single quotation mark)
    0x93.toByte(), // " (left double quotation mark)
    0x94.toByte(), // " (right double quotation mark)
    0x95.toByte(), // • (bullet)
    0x96.toByte(), // – (en dash)
    0x97.toByte(), // — (em dash)
    0x99.toByte(), // ™ (trade mark sign)
)

/**
 * Attempts to convert arbitrary bytes to UTF-8 with best-effort encoding detection.
 *
 * Ported from Rust codex-rs/core/src/text_encoding.rs bytes_to_string_smart
 */
fun bytesToStringSmart(bytes: ByteArray): String {
    if (bytes.isEmpty()) {
        return ""
    }

    // Fast path: try UTF-8 first
    val utf8Result = tryDecodeUtf8(bytes)
    if (utf8Result != null) {
        return utf8Result
    }

    // Detect encoding and decode
    val encoding = detectEncoding(bytes)
    return decodeBytes(bytes, encoding)
}

/**
 * Try to decode bytes as valid UTF-8.
 * Returns null if the bytes contain invalid UTF-8 sequences.
 */
private fun tryDecodeUtf8(bytes: ByteArray): String? {
    return try {
        // Use Kotlin stdlib's strict UTF-8 decoding
        bytes.decodeToString(throwOnInvalidSequence = true)
    } catch (e: CharacterCodingException) {
        null
    } catch (e: Exception) {
        null
    }
}

/**
 * Detect the encoding of the byte array using heuristics.
 *
 * This is a simplified version of chardetng's detection. We use heuristics
 * based on byte patterns common in various encodings.
 *
 * Ported from Rust codex-rs/core/src/text_encoding.rs detect_encoding
 */
private fun detectEncoding(bytes: ByteArray): Charset {
    // Check for BOM markers first
    if (bytes.size >= 3 && bytes[0] == 0xEF.toByte() && bytes[1] == 0xBB.toByte() && bytes[2] == 0xBF.toByte()) {
        return Charsets.forName("UTF-8")
    }
    if (bytes.size >= 2 && bytes[0] == 0xFF.toByte() && bytes[1] == 0xFE.toByte()) {
        return Charsets.forName("UTF-16LE")
    }
    if (bytes.size >= 2 && bytes[0] == 0xFE.toByte() && bytes[1] == 0xFF.toByte()) {
        return Charsets.forName("UTF-16BE")
    }

    // Analyze byte patterns
    val analysis = analyzeBytePatterns(bytes)

    // Windows-1252 vs IBM866 disambiguation
    // chardetng occasionally reports IBM866 for short strings that only contain Windows-1252
    // "smart punctuation" bytes (0x80-0x9F) because that range maps to Cyrillic letters in IBM866.
    if (analysis.looksLikeCyrillic && looksLikeWindows1252Punctuation(bytes)) {
        return Charsets.forName("Windows-1252")
    }

    // Use heuristics based on byte patterns
    return when {
        analysis.looksLikeCyrillic -> detectCyrillicEncoding(bytes)
        analysis.looksLikeCjk -> detectCjkEncoding(bytes)
        analysis.hasHighBytes -> Charsets.forName("Windows-1252") // Default Latin fallback
        else -> Charsets.forName("UTF-8")
    }
}

/**
 * Analysis of byte patterns in the input.
 */
private data class BytePatternAnalysis(
    val hasHighBytes: Boolean,
    val looksLikeCyrillic: Boolean,
    val looksLikeCjk: Boolean,
    val highByteCount: Int,
    val asciiCount: Int
)

/**
 * Analyze byte patterns to help determine encoding.
 */
private fun analyzeBytePatterns(bytes: ByteArray): BytePatternAnalysis {
    var highByteCount = 0
    var asciiCount = 0
    var cyrillicLikeCount = 0
    var cjkLikeCount = 0

    var i = 0
    while (i < bytes.size) {
        val b = bytes[i].toInt() and 0xFF

        when {
            b < 0x80 -> asciiCount++
            b in 0x80..0xBF -> {
                // Could be continuation byte (UTF-8) or high byte in single-byte encoding
                highByteCount++
                // Check for Cyrillic-like patterns (common in CP1251, KOI8-R)
                if (b in 0xC0..0xFF || b in 0x80..0xBF) {
                    cyrillicLikeCount++
                }
            }
            b in 0xC0..0xFF -> {
                highByteCount++
                // Check for CJK-like multi-byte sequences
                if (i + 1 < bytes.size) {
                    val next = bytes[i + 1].toInt() and 0xFF
                    if (next in 0x40..0xFE) {
                        cjkLikeCount++
                    }
                }
                cyrillicLikeCount++
            }
        }
        i++
    }

    val totalNonAscii = highByteCount
    val looksLikeCyrillic = cyrillicLikeCount > 0 && cjkLikeCount < cyrillicLikeCount
    val looksLikeCjk = cjkLikeCount > cyrillicLikeCount

    return BytePatternAnalysis(
        hasHighBytes = totalNonAscii > 0,
        looksLikeCyrillic = looksLikeCyrillic,
        looksLikeCjk = looksLikeCjk,
        highByteCount = highByteCount,
        asciiCount = asciiCount
    )
}

/**
 * Detect Cyrillic encoding variant.
 * Tries to distinguish between Windows-1251, KOI8-R, and IBM866.
 */
private fun detectCyrillicEncoding(bytes: ByteArray): Charset {
    // Count patterns specific to each encoding
    var cp1251Score = 0
    var koi8rScore = 0
    var cp866Score = 0

    for (b in bytes) {
        val unsigned = b.toInt() and 0xFF
        when (unsigned) {
            // Windows-1251 specific ranges
            in 0xC0..0xFF -> cp1251Score++
            // KOI8-R has letters in 0xC0-0xFF but with different mapping
            in 0xE0..0xFF -> koi8rScore++
            // IBM866 uses 0x80-0xAF for Cyrillic
            in 0x80..0xAF -> cp866Score++
        }
    }

    return when {
        koi8rScore > cp1251Score && koi8rScore > cp866Score -> Charsets.forName("KOI8-R")
        cp866Score > cp1251Score -> Charsets.forName("IBM866")
        else -> Charsets.forName("Windows-1251")
    }
}

/**
 * Detect CJK encoding variant.
 */
private fun detectCjkEncoding(bytes: ByteArray): Charset {
    // Simple heuristic: check for common lead byte ranges
    var gbkScore = 0
    var sjisScore = 0
    var big5Score = 0
    var eucKrScore = 0

    var i = 0
    while (i < bytes.size - 1) {
        val b1 = bytes[i].toInt() and 0xFF
        val b2 = bytes[i + 1].toInt() and 0xFF

        when {
            // GBK/GB2312 range
            b1 in 0x81..0xFE && b2 in 0x40..0xFE -> gbkScore++
            // Shift-JIS range
            (b1 in 0x81..0x9F || b1 in 0xE0..0xEF) && (b2 in 0x40..0x7E || b2 in 0x80..0xFC) -> sjisScore++
            // Big5 range
            b1 in 0xA1..0xF9 && (b2 in 0x40..0x7E || b2 in 0xA1..0xFE) -> big5Score++
            // EUC-KR range
            b1 in 0xA1..0xFE && b2 in 0xA1..0xFE -> eucKrScore++
        }
        i++
    }

    return when {
        sjisScore > gbkScore && sjisScore > big5Score && sjisScore > eucKrScore -> Charsets.forName("Shift_JIS")
        big5Score > gbkScore && big5Score > eucKrScore -> Charsets.forName("Big5")
        eucKrScore > gbkScore -> Charsets.forName("EUC-KR")
        else -> Charsets.forName("GBK")
    }
}

/**
 * Detect whether the byte stream looks like Windows-1252 "smart punctuation" wrapped around
 * otherwise-ASCII text.
 *
 * Context: IBM866 and Windows-1252 share the 0x80-0x9F slot range. In IBM866 these bytes decode to
 * Cyrillic letters, whereas Windows-1252 maps them to curly quotes and dashes. chardetng can guess
 * IBM866 for short snippets that only contain those bytes, which turns shell output such as
 * `"test"` into unreadable Cyrillic. To avoid that, we treat inputs comprising a handful of bytes
 * from the problematic range plus ASCII letters as CP1252 punctuation.
 *
 * Ported from Rust codex-rs/core/src/text_encoding.rs looks_like_windows_1252_punctuation
 */
private fun looksLikeWindows1252Punctuation(bytes: ByteArray): Boolean {
    var sawExtendedPunctuation = false
    var sawAsciiWord = false

    for (b in bytes) {
        val unsigned = b.toInt() and 0xFF

        // If we see bytes >= 0xA0, this is not Windows-1252 punctuation pattern
        if (unsigned >= 0xA0) {
            return false
        }

        // Check for bytes in the problematic 0x80-0x9F range
        if (unsigned in 0x80..0x9F) {
            if (!isWindows1252Punct(b)) {
                return false
            }
            sawExtendedPunctuation = true
        }

        // Check for ASCII alphabetic characters
        if (unsigned in 0x41..0x5A || unsigned in 0x61..0x7A) {
            sawAsciiWord = true
        }
    }

    return sawExtendedPunctuation && sawAsciiWord
}

/**
 * Check if byte is a Windows-1252 punctuation character.
 */
private fun isWindows1252Punct(byte: Byte): Boolean {
    return byte in WINDOWS_1252_PUNCT_BYTES
}

/**
 * Decode bytes using the specified charset, falling back to lossy UTF-8 if decoding fails.
 *
 * Ported from Rust codex-rs/core/src/text_encoding.rs decode_bytes
 */
private fun decodeBytes(bytes: ByteArray, charset: Charset): String {
    return try {
        // Use fleeksoft-charset extension function
        bytes.decodeToString(charset)
    } catch (e: Exception) {
        // Fall back to lossy UTF-8 decoding using Kotlin stdlib
        bytes.decodeToString(throwOnInvalidSequence = false)
    }
}

/**
 * Exception for character coding errors.
 */
class CharacterCodingException(message: String) : Exception(message)
