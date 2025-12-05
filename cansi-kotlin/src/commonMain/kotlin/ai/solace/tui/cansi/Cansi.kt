/**
 * Cansi - Categorise ANSI - ANSI escape code parser and categoriser
 *
 * Kotlin Multiplatform port of the Rust cansi crate.
 *
 * This library parses text with ANSI escape sequences and returns deconstructed
 * text with metadata around coloring and styling. It focuses on CSI sequences,
 * particularly SGR (Select Graphic Rendition) parameters.
 */
package ai.solace.tui.cansi

/**
 * The 16 standard ANSI colors (8 normal + 8 bright).
 */
enum class Color {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite
}

/**
 * The emphasis (bold, faint) states.
 */
enum class Intensity {
    /** Normal intensity (no emphasis). */
    Normal,
    /** Bold. */
    Bold,
    /** Faint. */
    Faint
}

/**
 * A match representing an ANSI escape sequence found in text.
 *
 * @property start First byte index (inclusive).
 * @property end Last byte index + 1 (exclusive).
 * @property text The matched escape sequence text.
 */
data class Match(
    val start: Int,
    val end: Int,
    val text: String
)

/**
 * Data structure that holds information about coloring and styling of a text slice.
 *
 * Uses nullable properties to indicate whether a style was explicitly set.
 * This is the v3 API which uses Option-style semantics.
 *
 * @property text The text slice.
 * @property start Inclusive starting byte position.
 * @property end Exclusive ending byte position.
 * @property fg The foreground (text) color, or null if not set.
 * @property bg The background color, or null if not set.
 * @property intensity The emphasis state (bold, faint, normal), or null if not set.
 * @property italic Italicized, or null if not set.
 * @property underline Underlined, or null if not set.
 * @property blink Slow blink text, or null if not set.
 * @property reversed Inverted colors, or null if not set.
 * @property hidden Invisible text, or null if not set.
 * @property strikethrough Struck-through, or null if not set.
 */
data class CategorisedSlice(
    val text: String,
    val start: Int,
    val end: Int,
    val fg: Color? = null,
    val bg: Color? = null,
    val intensity: Intensity? = null,
    val italic: Boolean? = null,
    val underline: Boolean? = null,
    val blink: Boolean? = null,
    val reversed: Boolean? = null,
    val hidden: Boolean? = null,
    val strikethrough: Boolean? = null
) {
    /**
     * Creates a new slice with the same style but different text and positions.
     */
    fun cloneStyle(newText: String, newStart: Int, newEnd: Int): CategorisedSlice =
        copy(text = newText, start = newStart, end = newEnd)

    companion object {
        /**
         * Creates a slice with default styling (no style attributes set).
         */
        fun defaultStyle(text: String, start: Int, end: Int): CategorisedSlice =
            CategorisedSlice(text = text, start = start, end = end)
    }
}

/** Type alias for a list of categorized slices. */
typealias CategorisedSlices = List<CategorisedSlice>

/** Type alias for a single line of categorized slices. */
typealias CategorisedLine = List<CategorisedSlice>

// CSI bytes: ESC (0x1b) followed by '[' (0x5b)
private val CSI_BYTES = byteArrayOf(0x1b, 0x5b)

/**
 * Checks if a byte is a CSI sequence terminating character.
 * Terminating bytes are in the range 0x40-0x7E.
 */
private fun isTerminatedByte(byte: Byte): Boolean =
    byte in 0x40..0x7E

/**
 * Parses ANSI escape codes from the given text, returning a list of [Match].
 *
 * Only CSI (Control Sequence Introducer) sequences are detected.
 * The escape codes themselves are returned, not the styled text.
 * Positions are byte indices into the UTF-8 encoded string.
 *
 * Example:
 * ```kotlin
 * val ansiText = "Hello, \u001b[31;4mworld\u001b[0m!"
 * val parsed = parse(ansiText).map { it.start to it.end }
 * // parsed = [(7, 14), (19, 23)]
 * ```
 *
 * @param text The text to parse for ANSI escape codes.
 * @return A list of matches representing escape sequences found.
 */
fun parse(text: String): List<Match> {
    val matches = mutableListOf<Match>()
    val bytes = text.encodeToByteArray()
    val csiLen = CSI_BYTES.size

    var start = 0
    var end = start + csiLen

    while (end <= bytes.size) {
        // Check if we have CSI at this position
        if (bytes[start] == CSI_BYTES[0] && bytes[start + 1] == CSI_BYTES[1]) {
            // Start of a CSI sequence - find the terminating byte
            while (end < bytes.size && !isTerminatedByte(bytes[end])) {
                end++
            }

            val finalEnd = end + 1

            if (finalEnd > bytes.size) {
                break
            }

            matches.add(
                Match(
                    start = start,
                    end = finalEnd,
                    text = bytes.sliceArray(start until finalEnd).decodeToString()
                )
            )

            start = finalEnd
        } else {
            // Move past current UTF-8 character
            // UTF-8 encoding: bytes starting with 0xxxxxxx or 11xxxxxx are start bytes
            // Continuation bytes start with 10xxxxxx
            start++
            // Skip continuation bytes (10xxxxxx pattern)
            while (start < bytes.size && (bytes[start].toInt() and 0xC0) == 0x80) {
                start++
            }
        }

        end = start + csiLen
    }

    return matches
}

/**
 * Internal representation of SGR (Select Graphic Rendition) parameters.
 */
private data class Sgr(
    var fg: Color? = null,
    var bg: Color? = null,
    var intensity: Intensity? = null,
    var italic: Boolean? = null,
    var underline: Boolean? = null,
    var blink: Boolean? = null,
    var reversed: Boolean? = null,
    var hidden: Boolean? = null,
    var strikethrough: Boolean? = null
)

private const val SEPARATOR = ';'

/**
 * Produces an [Sgr] from a styling sequence match.
 */
private fun handleSeq(m: Match): Sgr {
    // The slice we want to process skips first two bytes (ESC[) and last byte (terminating byte)
    val slice = m.text.substring(2, m.text.length - 1)
    return slice.split(SEPARATOR).fold(Sgr()) { sgr, seq -> adjustSgr(sgr, seq) }
}

/**
 * Applies the style sequence to the SGR. Maps decimal numbers according to
 * [ANSI escape code spec](https://en.wikipedia.org/wiki/ANSI_escape_code#Escape_sequences).
 */
private fun adjustSgr(sgr: Sgr, seq: String): Sgr {
    when (seq) {
        "0" -> return Sgr() // Reset
        "1" -> sgr.intensity = Intensity.Bold
        "2" -> sgr.intensity = Intensity.Faint
        "3" -> sgr.italic = true
        "4" -> sgr.underline = true
        "5" -> sgr.blink = true
        "7" -> sgr.reversed = true
        "8" -> sgr.hidden = true
        "9" -> sgr.strikethrough = true
        "22" -> sgr.intensity = Intensity.Normal
        "23" -> sgr.italic = false
        "24" -> sgr.underline = false
        "25" -> sgr.blink = false
        "27" -> sgr.reversed = false
        "28" -> sgr.hidden = false
        "29" -> sgr.strikethrough = false
        // Foreground colors 30-37
        "30" -> sgr.fg = Color.Black
        "31" -> sgr.fg = Color.Red
        "32" -> sgr.fg = Color.Green
        "33" -> sgr.fg = Color.Yellow
        "34" -> sgr.fg = Color.Blue
        "35" -> sgr.fg = Color.Magenta
        "36" -> sgr.fg = Color.Cyan
        "37" -> sgr.fg = Color.White
        // Background colors 40-47
        "40" -> sgr.bg = Color.Black
        "41" -> sgr.bg = Color.Red
        "42" -> sgr.bg = Color.Green
        "43" -> sgr.bg = Color.Yellow
        "44" -> sgr.bg = Color.Blue
        "45" -> sgr.bg = Color.Magenta
        "46" -> sgr.bg = Color.Cyan
        "47" -> sgr.bg = Color.White
        // Bright foreground colors 90-97
        "90" -> sgr.fg = Color.BrightBlack
        "91" -> sgr.fg = Color.BrightRed
        "92" -> sgr.fg = Color.BrightGreen
        "93" -> sgr.fg = Color.BrightYellow
        "94" -> sgr.fg = Color.BrightBlue
        "95" -> sgr.fg = Color.BrightMagenta
        "96" -> sgr.fg = Color.BrightCyan
        "97" -> sgr.fg = Color.BrightWhite
        // Bright background colors 100-107
        "100" -> sgr.bg = Color.BrightBlack
        "101" -> sgr.bg = Color.BrightRed
        "102" -> sgr.bg = Color.BrightGreen
        "103" -> sgr.bg = Color.BrightYellow
        "104" -> sgr.bg = Color.BrightBlue
        "105" -> sgr.bg = Color.BrightMagenta
        "106" -> sgr.bg = Color.BrightCyan
        "107" -> sgr.bg = Color.BrightWhite
    }
    return sgr
}

/**
 * Converts an [Sgr] to a [CategorisedSlice].
 */
private fun Sgr.toSlice(text: String, start: Int, end: Int): CategorisedSlice =
    CategorisedSlice(
        text = text,
        start = start,
        end = end,
        fg = fg,
        bg = bg,
        intensity = intensity,
        italic = italic,
        underline = underline,
        blink = blink,
        reversed = reversed,
        hidden = hidden,
        strikethrough = strikethrough
    )

/**
 * Parses the text and returns each formatted slice in order.
 * The ANSI escape codes are not included in the text slices.
 *
 * Each different text slice is returned in order such that the text without
 * the escape characters can be reconstructed. There is a helper function
 * [constructTextNoCodes] for this.
 *
 * Example:
 * ```kotlin
 * val text = "\u001b[31mHello\u001b[0m, World!"
 * val slices = categoriseText(text)
 * // slices[0] = CategorisedSlice(text="Hello", fg=Color.Red, ...)
 * // slices[1] = CategorisedSlice(text=", World!", fg=null, ...)
 * ```
 *
 * @param text The text containing ANSI escape sequences to parse.
 * @return A list of categorized slices with styling information.
 */
fun categoriseText(text: String): CategorisedSlices {
    val matches = parse(text)
    val bytes = text.encodeToByteArray()

    var sgr = Sgr()
    var lo = 0

    // Will always be less than or equal to matches.size + 1 in length
    val slices = mutableListOf<CategorisedSlice>()

    for (m in matches) {
        // Add in the text before CSI with the previous SGR format
        if (m.start != lo) {
            val sliceText = bytes.sliceArray(lo until m.start).decodeToString()
            slices.add(sgr.toSlice(sliceText, lo, m.start))
        }

        sgr = handleSeq(m)
        lo = m.end
    }

    if (lo != bytes.size) {
        val sliceText = bytes.sliceArray(lo until bytes.size).decodeToString()
        slices.add(sgr.toSlice(sliceText, lo, bytes.size))
    }

    return slices
}

/**
 * Constructs a string of the categorized text without the ANSI escape characters.
 *
 * Example:
 * ```kotlin
 * val categorized = categoriseText("\u001b[30mH\u001b[31me\u001b[32ml\u001b[33ml\u001b[34mo")
 * val plainText = constructTextNoCodes(categorized)
 * // plainText = "Hello"
 * ```
 *
 * @param slices The categorized slices to reconstruct.
 * @return The plain text without ANSI escape codes.
 */
fun constructTextNoCodes(slices: CategorisedSlices): String =
    buildString(slices.sumOf { it.text.length }) {
        for (slice in slices) {
            append(slice.text)
        }
    }

/**
 * Splits on the first instance of `\r\n` or `\n` bytes.
 * Returns the exclusive end of the first component, and the inclusive start
 * of the remaining items if there is a split.
 *
 * Can return an empty remainder (if terminated with a new line).
 * Can return empty first slice (e.g., "\nHello").
 */
private fun splitOnNewLine(txt: String): Pair<Int, Int?> {
    val cr = txt.indexOf('\r')
    val nl = txt.indexOf('\n')

    return when {
        cr == -1 && nl == -1 -> txt.length to null
        cr != -1 && nl == -1 -> txt.length to null // Special case: CR but no NL
        cr == -1 && nl != -1 -> nl to (nl + 1)
        else -> {
            // Both CR and NL present
            if (nl > 0 && nl - 1 == cr) {
                // CRLF sequence
                cr to (nl + 1)
            } else {
                nl to (nl + 1)
            }
        }
    }
}

/**
 * Constructs an iterator over each new line (`\n` or `\r\n`) and returns
 * the categorized slices within those. [CategorisedSlice]s that include
 * a new line are split with the same style.
 *
 * Example:
 * ```kotlin
 * val text = "Hello\nWorld"
 * val slices = categoriseText(text)
 * val lines = lineIter(slices).toList()
 * // lines[0] = [CategorisedSlice(text="Hello", ...)]
 * // lines[1] = [CategorisedSlice(text="World", ...)]
 * ```
 *
 * @param slices The categorized slices to iterate over by line.
 * @return An iterator that yields lines of categorized slices.
 */
fun lineIter(slices: CategorisedSlices): CategorisedLineIterator =
    CategorisedLineIterator(slices)

/**
 * An iterator structure for [CategorisedSlices], iterating over each new line
 * (`\n` or `\r\n`) and returning the categorized slices within those.
 * [CategorisedSlice]s that include a new line are split with the same style.
 */
class CategorisedLineIterator(
    private val slices: CategorisedSlices
) : Iterator<CategorisedLine> {
    private var idx = 0
    private var prev: CategorisedSlice? = null
    private var hasMoreLines = true

    override fun hasNext(): Boolean {
        if (!hasMoreLines) return false
        // Check if we have more content
        return prev != null || idx < slices.size
    }

    override fun next(): CategorisedLine {
        if (!hasNext()) throw NoSuchElementException()

        val v = mutableListOf<CategorisedSlice>()

        prev?.let { prevSlice ->
            // Need to test splitting this, might be more new lines in remainder
            val (first, remainder) = splitOnNewLine(prevSlice.text)

            // Push first slice on -- only if not empty
            // If first == 0 it is because there is a sequence of new lines
            v.add(prevSlice.cloneStyle(
                prevSlice.text.substring(0, first),
                prevSlice.start,
                prevSlice.start + first
            ))

            if (remainder != null) {
                // There is a remainder, which means a new line was hit
                prev = prevSlice.cloneStyle(
                    prevSlice.text.substring(remainder),
                    prevSlice.start + remainder,
                    prevSlice.end
                )
                return v // Exit early
            }

            prev = null // Consumed prev
        }

        while (idx < slices.size) {
            val slice = slices[idx]
            idx++ // Increment to next slice

            val (first, remainder) = splitOnNewLine(slice.text)

            // Push first slice on -- only if not empty
            if (first > 0 || v.isEmpty()) {
                v.add(slice.cloneStyle(
                    slice.text.substring(0, first),
                    slice.start,
                    slice.start + first
                ))
            }

            if (remainder != null) {
                // There is a remainder, which means a new line was hit
                if (slice.text.substring(remainder).isNotEmpty()) {
                    // Not just a trailing new line
                    prev = slice.cloneStyle(
                        slice.text.substring(remainder),
                        slice.start + remainder,
                        slice.end
                    )
                }
                break // Exit looping
            }
        }

        if (v.isEmpty() && idx >= slices.size) {
            hasMoreLines = false
            throw NoSuchElementException()
        }

        return v
    }
}

/**
 * Extension function to convert the iterator to a sequence for easier use.
 */
fun CategorisedLineIterator.asSequence(): Sequence<CategorisedLine> =
    generateSequence { if (hasNext()) next() else null }
