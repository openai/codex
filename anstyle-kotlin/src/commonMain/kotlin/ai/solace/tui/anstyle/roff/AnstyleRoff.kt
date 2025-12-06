/**
 * # anstyle-roff
 *
 * Convert from ANSI stylings to ROFF Control Lines.
 *
 * This module bridges [ai.solace.tui.anstyle] with [ai.solace.tui.roff] for generating
 * ROFF output from ANSI-styled text. This is particularly useful for generating man pages
 * from colored terminal output, such as help text from CLI applications.
 *
 * ## Usage
 *
 * ```kotlin
 * import ai.solace.tui.anstyle.roff.toRoff
 *
 * // Convert ANSI-styled text to ROFF
 * val ansiText = "\u001b[31mError:\u001b[0m Something went wrong"
 * val roffDoc = toRoff(ansiText)
 *
 * // Render to ROFF format
 * val roffOutput = roffDoc.render()
 * ```
 *
 * ## Color Mapping
 *
 * ANSI colors are mapped to ROFF color names:
 * - 4-bit colors (e.g., red, blue) map directly to ROFF color names
 * - 8-bit (256) colors are converted to the nearest 4-bit color or RGB
 * - 24-bit RGB colors are defined using ROFF's `defcolor` request
 *
 * ## Effect Mapping
 *
 * - **Bold** (`\u001b[1m`) renders as ROFF bold (`\fB...\fR`)
 * - **Italic** (`\u001b[3m`) renders as ROFF italic (`\fI...\fR`)
 * - **Bright colors** (e.g., `\u001b[91m`) render as bold text
 * - Other effects (underline, blink, etc.) are not supported by ROFF
 *
 * ## Dependencies
 *
 * This module depends on:
 * - [ai.solace.tui.cansi] for parsing ANSI escape codes
 * - [ai.solace.tui.roff] for generating ROFF documents
 * - [ai.solace.tui.anstyle] for style representation
 *
 * @see toRoff Main conversion function
 * @see styledStream Parse ANSI text into styled segments
 *
 * Copyright (C) 2024-2025 Sydney Renee / The Solace Project
 * Licensed under Apache-2.0 OR MIT
 */
package ai.solace.tui.anstyle.roff

import ai.solace.tui.anstyle.Ansi256Color
import ai.solace.tui.anstyle.AnsiColor
import ai.solace.tui.anstyle.Color
import ai.solace.tui.anstyle.Effects
import ai.solace.tui.anstyle.RgbColor
import ai.solace.tui.anstyle.Style
import ai.solace.tui.anstyle.lossy.Palette
import ai.solace.tui.anstyle.lossy.xtermToRgb
import ai.solace.tui.roff.Inline
import ai.solace.tui.roff.Roff
import ai.solace.tui.roff.bold
import ai.solace.tui.roff.italic
import ai.solace.tui.roff.roman

/**
 * Static strings defining ROFF control requests.
 */
internal object ControlRequests {
    /** Control to create a color definition. */
    const val CREATE_COLOR = "defcolor"

    /** ROFF control request to set background color (fill color). */
    const val BACKGROUND = "fcolor"

    /** ROFF control request to set foreground color (glyph color). */
    const val FOREGROUND = "gcolor"
}

/**
 * A styled string segment parsed from ANSI escape codes.
 *
 * Represents a contiguous piece of text with consistent styling. When parsing
 * ANSI-styled text, the input is broken into these segments wherever the style changes.
 *
 * @property text The text content (without ANSI escape codes).
 * @property style The [Style] applied to this text segment.
 */
data class StyledStr(
    val text: String,
    val style: Style
)

/**
 * Parse ANSI-styled text into a stream of [StyledStr] segments.
 *
 * This function uses [ai.solace.tui.cansi.categoriseText] to parse the ANSI escape codes
 * and converts the results to anstyle [Style] objects. Each segment represents a piece
 * of text with consistent styling.
 *
 * Example:
 * ```kotlin
 * val text = "\u001b[31mred\u001b[0m normal \u001b[32mgreen\u001b[0m"
 * val segments = styledStream(text).toList()
 * // segments[0]: StyledStr(text="red", style=Style(fg=Red))
 * // segments[1]: StyledStr(text=" normal ", style=Style())
 * // segments[2]: StyledStr(text="green", style=Style(fg=Green))
 * ```
 *
 * @param text The ANSI escape code formatted text to parse.
 * @return A [Sequence] of [StyledStr] segments representing the styled text.
 */
fun styledStream(text: String): Sequence<StyledStr> = sequence {
    val slices = ai.solace.tui.cansi.categoriseText(text)
    for (slice in slices) {
        val style = buildStyle(slice)
        yield(StyledStr(slice.text, style))
    }
}

/**
 * Build an anstyle [Style] from a cansi [CategorisedSlice].
 */
private fun buildStyle(slice: ai.solace.tui.cansi.CategorisedSlice): Style {
    var style = Style()

    // Foreground color
    slice.fg?.let { fg ->
        style = style.fgColor(cansiColorToAnstyle(fg))
    }

    // Background color
    slice.bg?.let { bg ->
        style = style.bgColor(cansiColorToAnstyle(bg))
    }

    // Effects
    var effects = Effects.PLAIN

    slice.intensity?.let { intensity ->
        when (intensity) {
            ai.solace.tui.cansi.Intensity.Bold -> effects = effects.insert(Effects.BOLD)
            ai.solace.tui.cansi.Intensity.Faint -> effects = effects.insert(Effects.DIMMED)
            ai.solace.tui.cansi.Intensity.Normal -> { /* No effect */ }
        }
    }

    if (slice.italic == true) {
        effects = effects.insert(Effects.ITALIC)
    }

    if (slice.underline == true) {
        effects = effects.insert(Effects.UNDERLINE)
    }

    if (slice.blink == true) {
        effects = effects.insert(Effects.BLINK)
    }

    if (slice.reversed == true) {
        effects = effects.insert(Effects.INVERT)
    }

    if (slice.hidden == true) {
        effects = effects.insert(Effects.HIDDEN)
    }

    if (slice.strikethrough == true) {
        effects = effects.insert(Effects.STRIKETHROUGH)
    }

    return style.effects(effects)
}

/**
 * Convert a cansi [Color] to an anstyle [Color].
 */
private fun cansiColorToAnstyle(color: ai.solace.tui.cansi.Color): Color {
    val ansiColor = when (color) {
        ai.solace.tui.cansi.Color.Black -> AnsiColor.Black
        ai.solace.tui.cansi.Color.Red -> AnsiColor.Red
        ai.solace.tui.cansi.Color.Green -> AnsiColor.Green
        ai.solace.tui.cansi.Color.Yellow -> AnsiColor.Yellow
        ai.solace.tui.cansi.Color.Blue -> AnsiColor.Blue
        ai.solace.tui.cansi.Color.Magenta -> AnsiColor.Magenta
        ai.solace.tui.cansi.Color.Cyan -> AnsiColor.Cyan
        ai.solace.tui.cansi.Color.White -> AnsiColor.White
        ai.solace.tui.cansi.Color.BrightBlack -> AnsiColor.BrightBlack
        ai.solace.tui.cansi.Color.BrightRed -> AnsiColor.BrightRed
        ai.solace.tui.cansi.Color.BrightGreen -> AnsiColor.BrightGreen
        ai.solace.tui.cansi.Color.BrightYellow -> AnsiColor.BrightYellow
        ai.solace.tui.cansi.Color.BrightBlue -> AnsiColor.BrightBlue
        ai.solace.tui.cansi.Color.BrightMagenta -> AnsiColor.BrightMagenta
        ai.solace.tui.cansi.Color.BrightCyan -> AnsiColor.BrightCyan
        ai.solace.tui.cansi.Color.BrightWhite -> AnsiColor.BrightWhite
    }
    return Color.Ansi(ansiColor)
}

/**
 * Generate a [Roff] document from ANSI escape codes.
 *
 * This is the main entry point for converting ANSI-styled terminal output to ROFF format.
 * The resulting [Roff] document can be rendered to a string using [Roff.render] (with
 * apostrophe handling) or [Roff.toRoff] (raw output).
 *
 * ## Color Handling
 *
 * Foreground colors are set using `.gcolor` and background colors using `.fcolor`.
 * When a color changes, the appropriate control request is emitted. When color is
 * reset to default, `.gcolor default` or `.fcolor default` is emitted.
 *
 * ## Effect Handling
 *
 * - Bold text is wrapped with `\fB...\fR`
 * - Italic text is wrapped with `\fI...\fR`
 * - Bright foreground colors are treated as bold
 * - Plain text uses roman font (no wrapping)
 *
 * Example:
 * ```kotlin
 * val text = "\u001b[31;1mError:\u001b[0m File not found"
 * val roffDoc = toRoff(text)
 * println(roffDoc.render())
 * // Output:
 * // .gcolor red
 * // \fBError:\fR
 * // .gcolor default
 * // File not found
 * ```
 *
 * @param styledText The ANSI escape code formatted text to convert.
 * @return A [Roff] document representing the styled content.
 * @see styledStream For lower-level access to styled segments
 * @see Roff.render To render with apostrophe handling
 * @see Roff.toRoff To render raw ROFF output
 */
fun toRoff(styledText: String): Roff {
    val doc = Roff()
    var previousFgColor: Color? = null
    var previousBgColor: Color? = null

    for (styled in styledStream(styledText)) {
        val currentFgColor = styled.style.getFgColor()
        val currentBgColor = styled.style.getBgColor()

        if (previousFgColor != currentFgColor) {
            addColorToRoff(doc, ControlRequests.FOREGROUND, currentFgColor)
            previousFgColor = currentFgColor
        }

        if (previousBgColor != currentBgColor) {
            addColorToRoff(doc, ControlRequests.BACKGROUND, currentBgColor)
            previousBgColor = currentBgColor
        }

        setEffectsAndText(styled, doc)
    }

    return doc
}

/**
 * Add text with appropriate effects to the ROFF document.
 *
 * ROFF (the crate) only supports these inline commands:
 * - Bold
 * - Italic
 * - Roman (plain text)
 *
 * If we want more support, or even support combined formats, we would need
 * to push improvements to roff upstream or implement a more thorough roff solution.
 */
private fun setEffectsAndText(styled: StyledStr, doc: Roff) {
    val effects = styled.style.getEffects()
    val inline: Inline = when {
        effects.contains(Effects.BOLD) || hasBrightFg(styled.style) -> bold(styled.text)
        effects.contains(Effects.ITALIC) -> italic(styled.text)
        else -> roman(styled.text)
    }
    doc.text(inline)
}

/**
 * Check if the style has a bright foreground color.
 */
private fun hasBrightFg(style: Style): Boolean {
    val fgColor = style.getFgColor() ?: return false
    return isBright(fgColor)
}

/**
 * Check if a [Color] is a bright [AnsiColor] variant.
 */
private fun isBright(color: Color): Boolean {
    if (color !is Color.Ansi) return false
    return when (color.color) {
        AnsiColor.BrightRed,
        AnsiColor.BrightBlue,
        AnsiColor.BrightBlack,
        AnsiColor.BrightCyan,
        AnsiColor.BrightGreen,
        AnsiColor.BrightWhite,
        AnsiColor.BrightYellow,
        AnsiColor.BrightMagenta -> true
        else -> false
    }
}

/**
 * Add a color control request to the ROFF document.
 */
private fun addColorToRoff(doc: Roff, controlRequest: String, color: Color?) {
    when (color) {
        is Color.Rgb -> {
            // Adding Support for RGB colors, however cansi does not support
            // RGB Colors, so this is not executed. If we switch to a provider
            // that has RGB support we will also get it for Roff
            val name = rgbName(color.color)
            doc.control(ControlRequests.CREATE_COLOR, name, "rgb", toHex(color.color))
                .control(controlRequest, name)
        }
        is Color.Ansi -> {
            doc.control(controlRequest, ansiColorToRoff(color.color))
        }
        is Color.Ansi256 -> {
            // Adding Support for Ansi256 colors, however cansi does not support
            // Ansi256 Colors, so this is not executed. If we switch to a provider
            // that has Xterm support we will also get it for Roff
            val convertedColor = xtermToAnsiOrRgb(color.color)
            addColorToRoff(doc, controlRequest, convertedColor)
        }
        null -> {
            doc.control(controlRequest, "default")
        }
    }
}

/**
 * Non-lossy conversion of Xterm color to one that ROFF can handle.
 */
private fun xtermToAnsiOrRgb(color: Ansi256Color): Color {
    val ansiColor = color.intoAnsi()
    return if (ansiColor != null) {
        Color.Ansi(ansiColor)
    } else {
        Color.Rgb(xtermToRgb(color, Palette.DEFAULT))
    }
}

/**
 * Generate a color name from RGB values.
 */
private fun rgbName(color: RgbColor): String = "hex_${toHex(color)}"

/**
 * Convert RGB color to hex string.
 */
private fun toHex(rgb: RgbColor): String {
    val value = (rgb.r.toInt() shl 16) + (rgb.g.toInt() shl 8) + rgb.b.toInt()
    return "#${value.toString(16).padStart(6, '0')}"
}

/**
 * Map [AnsiColor] (including bright variants) to ROFF color names.
 */
private fun ansiColorToRoff(color: AnsiColor): String = when (color) {
    AnsiColor.Black, AnsiColor.BrightBlack -> "black"
    AnsiColor.Red, AnsiColor.BrightRed -> "red"
    AnsiColor.Green, AnsiColor.BrightGreen -> "green"
    AnsiColor.Yellow, AnsiColor.BrightYellow -> "yellow"
    AnsiColor.Blue, AnsiColor.BrightBlue -> "blue"
    AnsiColor.Magenta, AnsiColor.BrightMagenta -> "magenta"
    AnsiColor.Cyan, AnsiColor.BrightCyan -> "cyan"
    AnsiColor.White, AnsiColor.BrightWhite -> "white"
}
