/**
 * Convert from ANSI stylings to ROFF Control Lines.
 *
 * This module bridges anstyle with roff for generating ROFF output from
 * ANSI-styled text. Useful for generating man pages from colored terminal output.
 *
 * Copyright (C) 2024-2025 Sydney Renee <sydney@thesolace.ai>
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
 * @property text The text content.
 * @property style The ANSI style applied to this text.
 */
data class StyledStr(
    val text: String,
    val style: Style
)

/**
 * Parse ANSI-styled text into a stream of [StyledStr] segments.
 *
 * This function converts cansi parsing results to anstyle [Style] objects.
 *
 * @param text The ANSI escape code formatted text.
 * @return Sequence of styled string segments.
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
 * Example:
 * ```kotlin
 * val text = "\u001b[44;31mtest\u001b[0m"
 *
 * val roffDoc = toRoff(text)
 * val expected = """.gcolor red
 * .fcolor blue
 * test
 * """
 *
 * assertEquals(expected, roffDoc.toRoff())
 * ```
 *
 * @param styledText The ANSI escape code formatted text.
 * @return A [Roff] document representing the styled content.
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
