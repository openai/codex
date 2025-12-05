/**
 * A document in the ROFF format.
 *
 * [ROFF](https://en.wikipedia.org/wiki/Roff_(software)) is a family of Unix text-formatting
 * languages, implemented by the `nroff`, `troff`, and `groff` programs, among others.
 * See [groff(7)](https://manpages.debian.org/bullseye/groff/groff.7.en.html) for a description
 * of the language. This structure is an abstract representation of a document in ROFF format.
 * It is meant for writing code to generate ROFF documents, such as manual pages.
 *
 * ## Example
 *
 * ```kotlin
 * val doc = Roff().text(listOf(roman("hello, world"))).render()
 * assert(doc.endsWith("hello, world\n"))
 * ```
 */
package ai.solace.tui.roff

/**
 * A ROFF document, consisting of lines.
 *
 * Lines are either control lines (requests that are built in, or invocations of macros),
 * or text lines.
 *
 * ## Example
 *
 * ```kotlin
 * val doc = Roff()
 *     .control("TH", listOf("FOO", "1"))
 *     .control("SH", listOf("NAME"))
 *     .text(listOf(roman("foo - do a foo thing")))
 *     .render()
 * assert(doc.endsWith(".TH FOO 1\n.SH NAME\nfoo \\- do a foo thing\n"))
 * ```
 */
class Roff {
    private val lines: MutableList<Line> = mutableListOf()

    /**
     * Append a control line.
     *
     * The line consists of the name of a built-in command or macro,
     * and some number of arguments. Arguments that contain spaces
     * will be enclosed with double quotation marks.
     */
    fun control(name: String, args: List<String>): Roff {
        lines.add(Line.Control(name, args.toList()))
        return this
    }

    /**
     * Append a control line with vararg arguments.
     */
    fun control(name: String, vararg args: String): Roff = control(name, args.toList())

    /**
     * Append a text line.
     *
     * The line will be rendered in a way that ensures it can't be
     * interpreted as a control line. The caller does not need to
     * ensure, for example, that the line doesn't start with a
     * period ("`.`") or an apostrophe ("`'`").
     */
    fun text(inlines: List<Inline>): Roff {
        lines.add(Line.Text(inlines.toList()))
        return this
    }

    /**
     * Append a text line with vararg inlines.
     */
    fun text(vararg inlines: Inline): Roff = text(inlines.toList())

    /**
     * Render as ROFF source text that can be fed to a ROFF implementation.
     */
    fun render(): String = buildString {
        append(APOSTROPHE_PREAMBLE)
        for (line in lines) {
            line.render(this, handleApostrophes = true)
        }
    }

    /**
     * Render without handling apostrophes specially.
     *
     * You probably want [render] instead of this method.
     *
     * Without special handling, apostrophes get typeset as right
     * single quotes, including in words like "don't". In most
     * situations, such as in manual pages, that's unwanted. The
     * [render] method handles apostrophes specially to prevent it, but
     * for completeness, and for testing, this method is provided to
     * avoid it.
     */
    fun toRoff(): String = buildString {
        for (line in lines) {
            line.render(this, handleApostrophes = false)
        }
    }

    /**
     * Extend this document with lines from another Roff document.
     */
    fun extend(other: Roff): Roff {
        lines.addAll(other.lines)
        return this
    }

    companion object {
        /**
         * Create a new empty Roff document.
         */
        fun new(): Roff = Roff()
    }
}

/**
 * A part of a text line.
 *
 * Text will be escaped for ROFF. No inline escape sequences will be
 * passed to ROFF. The text may contain newlines, but leading periods
 * will be escaped so that they won't be interpreted by ROFF as
 * control lines.
 */
sealed class Inline {
    /**
     * Text in the "roman" font, which is the normal font if nothing
     * else is specified.
     */
    data class Roman(val text: String) : Inline()

    /**
     * Text in the italic (slanted) font.
     */
    data class Italic(val text: String) : Inline()

    /**
     * Text in a bold face font.
     */
    data class Bold(val text: String) : Inline()

    /**
     * A hard line break. This is an inline element so it's easy to
     * insert a line break in a paragraph.
     */
    data object LineBreak : Inline()
}

/**
 * Return some inline text in the "roman" font.
 *
 * The roman font is the normal font, if no other font is chosen.
 */
fun roman(input: String): Inline = Inline.Roman(input)

/**
 * Return some inline text in the bold font.
 */
fun bold(input: String): Inline = Inline.Bold(input)

/**
 * Return some inline text in the italic font.
 */
fun italic(input: String): Inline = Inline.Italic(input)

/**
 * Return an inline element for a hard line break.
 */
fun lineBreak(): Inline = Inline.LineBreak

/**
 * Convert a string to an Inline.Roman element.
 */
fun String.toInline(): Inline = Inline.Roman(this)

/**
 * A line in a ROFF document.
 */
internal sealed class Line {
    /**
     * A control line.
     */
    data class Control(
        /** Name of control request or macro being invoked. */
        val name: String,
        /** Arguments on control line. */
        val args: List<String>
    ) : Line()

    /**
     * A text line.
     */
    data class Text(val inlines: List<Inline>) : Line()

    /**
     * Generate a ROFF line.
     *
     * All the ROFF code generation and special handling happens here.
     */
    fun render(out: Appendable, handleApostrophes: Boolean) {
        when (this) {
            is Control -> {
                out.append(".$name")
                for (arg in args) {
                    out.append(" ")
                    out.append(escapeSpaces(arg))
                }
            }
            is Text -> {
                var atLineStart = true
                for (inline in inlines) {
                    when (inline) {
                        is Inline.LineBreak -> {
                            if (atLineStart) {
                                out.append(".br\n")
                            } else {
                                out.append("\n.br\n")
                            }
                        }
                        is Inline.Roman, is Inline.Italic, is Inline.Bold -> {
                            val rawText = when (inline) {
                                is Inline.Roman -> inline.text
                                is Inline.Italic -> inline.text
                                is Inline.Bold -> inline.text
                                else -> ""
                            }
                            var text = escapeInline(rawText)
                            if (handleApostrophes) {
                                text = escapeApostrophes(text)
                            }
                            text = escapeLeadingCc(text)

                            when (inline) {
                                is Inline.Bold -> {
                                    out.append("\\fB")
                                    out.append(text)
                                    out.append("\\fR")
                                }
                                is Inline.Italic -> {
                                    out.append("\\fI")
                                    out.append(text)
                                    out.append("\\fR")
                                }
                                is Inline.Roman -> {
                                    if (atLineStart && startsWithCc(text)) {
                                        // Line would start with a period, so we
                                        // insert a non-printable, zero-width glyph to
                                        // prevent it from being interpreted as such.
                                        out.append("\\&")
                                    }
                                    out.append(text)
                                }
                                else -> {}
                            }
                        }
                    }
                    atLineStart = false
                }
            }
        }
        out.append("\n")
    }
}

/**
 * Does line start with a control character?
 */
private fun startsWithCc(line: String): Boolean =
    line.startsWith('.') || line.startsWith('\'')

/**
 * This quotes strings with spaces. This doesn't handle strings with
 * quotes in any way: there doesn't seem to be a way to escape them.
 */
private fun escapeSpaces(w: String): String =
    if (w.contains(' ')) "\"$w\"" else w

/**
 * Prevent leading periods or apostrophes on lines to be interpreted
 * as control lines. Note that this needs to be done for apostrophes
 * whether they need special handling for typesetting or not: a
 * leading apostrophe on a line indicates a control line.
 */
private fun escapeLeadingCc(s: String): String =
    s.replace("\n.", "\n\\&.").replace("\n'", "\n\\&'")

/**
 * Escape anything that may be interpreted by the roff processor in a
 * text line: dashes and backslashes are escaped with a backslash.
 * Apostrophes are not handled.
 */
internal fun escapeInline(text: String): String =
    text.replace("\\", "\\\\").replace("-", "\\-")

/**
 * Handle apostrophes.
 */
private fun escapeApostrophes(text: String): String =
    text.replace("'", APOSTROPHE)

/**
 * Use the apostrophe string variable.
 */
private const val APOSTROPHE = "\\*(Aq"

/**
 * A preamble added to the start of rendered output.
 *
 * This defines a string variable that contains an apostrophe. For
 * historical reasons, there seems to be no other portable way to
 * represent apostrophes across various implementations of the ROFF
 * language.
 */
private const val APOSTROPHE_PREAMBLE = """.ie \n(.g .ds Aq \(aq
.el .ds Aq '
"""
