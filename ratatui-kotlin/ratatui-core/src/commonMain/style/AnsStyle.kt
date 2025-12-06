package ratatui.style

// This module contains conversion functions for styles from the `anstyle` crate.

import anstyle.Ansi256Color
import anstyle.AnsiColor
import anstyle.Effects
import anstyle.RgbColor
import anstyle.or

// Rust original:
// use anstyle::{Ansi256Color, AnsiColor, Effects, RgbColor};
// use thiserror::Error;
// use super::{Color, Modifier, Style};

/**
 * Error type for converting between `anstyle` colors and `Color`
 */
sealed class TryFromColorError : Exception() {
    data object Ansi256 : TryFromColorError() {
        override val message = "cannot convert Ratatui Color to an Ansi256Color as it is not an indexed color"
    }
    data object Ansi : TryFromColorError() {
        override val message = "cannot convert Ratatui Color to AnsiColor as it is not a 4-bit color"
    }
    data object Rgb : TryFromColorError() {
        override val message = "cannot convert Ratatui Color to RgbColor as it is not an RGB color"
    }
}

// ============================================================================
// Color conversions: anstyle -> ratatui
// ============================================================================

/**
 * Convert [Ansi256Color] to [Color]
 */
fun Ansi256Color.toRatatuiColor(): Color = Color.Indexed(index.toUByte())

/**
 * Try to convert [Color] to [Ansi256Color]
 * @throws TryFromColorError.Ansi256 if the color is not an indexed color
 */
fun Color.toAnsi256Color(): Ansi256Color = when (this) {
    is Color.Indexed -> Ansi256Color(index.toUByte())
    else -> throw TryFromColorError.Ansi256
}

/**
 * Convert [AnsiColor] to [Color]
 */
fun AnsiColor.toRatatuiColor(): Color = when (this) {
    AnsiColor.Black -> Color.Black
    AnsiColor.Red -> Color.Red
    AnsiColor.Green -> Color.Green
    AnsiColor.Yellow -> Color.Yellow
    AnsiColor.Blue -> Color.Blue
    AnsiColor.Magenta -> Color.Magenta
    AnsiColor.Cyan -> Color.Cyan
    AnsiColor.White -> Color.Gray
    AnsiColor.BrightBlack -> Color.DarkGray
    AnsiColor.BrightRed -> Color.LightRed
    AnsiColor.BrightGreen -> Color.LightGreen
    AnsiColor.BrightYellow -> Color.LightYellow
    AnsiColor.BrightBlue -> Color.LightBlue
    AnsiColor.BrightMagenta -> Color.LightMagenta
    AnsiColor.BrightCyan -> Color.LightCyan
    AnsiColor.BrightWhite -> Color.White
}

/**
 * Try to convert [Color] to [AnsiColor]
 * @throws TryFromColorError.Ansi if the color is not a 4-bit ANSI color
 */
fun Color.toAnsiColor(): AnsiColor = when (this) {
    Color.Black -> AnsiColor.Black
    Color.Red -> AnsiColor.Red
    Color.Green -> AnsiColor.Green
    Color.Yellow -> AnsiColor.Yellow
    Color.Blue -> AnsiColor.Blue
    Color.Magenta -> AnsiColor.Magenta
    Color.Cyan -> AnsiColor.Cyan
    Color.Gray -> AnsiColor.White
    Color.DarkGray -> AnsiColor.BrightBlack
    Color.LightRed -> AnsiColor.BrightRed
    Color.LightGreen -> AnsiColor.BrightGreen
    Color.LightYellow -> AnsiColor.BrightYellow
    Color.LightBlue -> AnsiColor.BrightBlue
    Color.LightMagenta -> AnsiColor.BrightMagenta
    Color.LightCyan -> AnsiColor.BrightCyan
    Color.White -> AnsiColor.BrightWhite
    else -> throw TryFromColorError.Ansi
}

/**
 * Convert [RgbColor] to [Color]
 */
fun RgbColor.toRatatuiColor(): Color = Color.Rgb(r, g, b)

/**
 * Try to convert [Color] to [RgbColor]
 * @throws TryFromColorError.Rgb if the color is not an RGB color
 */
fun Color.toRgbColor(): RgbColor = when (this) {
    is Color.Rgb -> RgbColor(red, green, blue)
    else -> throw TryFromColorError.Rgb
}

/**
 * Convert [anstyle.Color] to [Color]
 */
fun anstyle.Color.toRatatuiColor(): Color = when (this) {
    is anstyle.Color.Ansi -> color.toRatatuiColor()
    is anstyle.Color.Ansi256 -> color.toRatatuiColor()
    is anstyle.Color.Rgb -> color.toRatatuiColor()
}

/**
 * Convert [Color] to [anstyle.Color]
 */
fun Color.toAnstyleColor(): anstyle.Color = when (this) {
    is Color.Rgb -> anstyle.Color.Rgb(toRgbColor())
    is Color.Indexed -> anstyle.Color.Ansi256(toAnsi256Color())
    else -> anstyle.Color.Ansi(toAnsiColor())
}

// ============================================================================
// Modifier/Effects conversions
// ============================================================================

/**
 * Convert [Effects] to [Modifier]
 */
fun Effects.toModifier(): Modifier {
    var modifier = Modifier.empty()
    if (contains(Effects.BOLD)) {
        modifier = modifier or Modifier.BOLD
    }
    if (contains(Effects.DIMMED)) {
        modifier = modifier or Modifier.DIM
    }
    if (contains(Effects.ITALIC)) {
        modifier = modifier or Modifier.ITALIC
    }
    if (contains(Effects.UNDERLINE) ||
        contains(Effects.DOUBLE_UNDERLINE) ||
        contains(Effects.CURLY_UNDERLINE) ||
        contains(Effects.DOTTED_UNDERLINE) ||
        contains(Effects.DASHED_UNDERLINE)
    ) {
        modifier = modifier or Modifier.UNDERLINED
    }
    if (contains(Effects.BLINK)) {
        modifier = modifier or Modifier.SLOW_BLINK
    }
    if (contains(Effects.INVERT)) {
        modifier = modifier or Modifier.REVERSED
    }
    if (contains(Effects.HIDDEN)) {
        modifier = modifier or Modifier.HIDDEN
    }
    if (contains(Effects.STRIKETHROUGH)) {
        modifier = modifier or Modifier.CROSSED_OUT
    }
    return modifier
}

/**
 * Convert [Modifier] to [Effects]
 */
fun Modifier.toEffects(): Effects {
    var effects = Effects.new()
    if (contains(Modifier.BOLD)) {
        effects = effects or Effects.BOLD
    }
    if (contains(Modifier.DIM)) {
        effects = effects or Effects.DIMMED
    }
    if (contains(Modifier.ITALIC)) {
        effects = effects or Effects.ITALIC
    }
    if (contains(Modifier.UNDERLINED)) {
        effects = effects or Effects.UNDERLINE
    }
    if (contains(Modifier.SLOW_BLINK) || contains(Modifier.RAPID_BLINK)) {
        effects = effects or Effects.BLINK
    }
    if (contains(Modifier.REVERSED)) {
        effects = effects or Effects.INVERT
    }
    if (contains(Modifier.HIDDEN)) {
        effects = effects or Effects.HIDDEN
    }
    if (contains(Modifier.CROSSED_OUT)) {
        effects = effects or Effects.STRIKETHROUGH
    }
    return effects
}

// ============================================================================
// Style conversions
// ============================================================================

/**
 * Convert [anstyle.Style] to [Style]
 */
fun anstyle.Style.toRatatuiStyle(): Style {
    return Style(
        fg = getFgColor()?.toRatatuiColor(),
        bg = getBgColor()?.toRatatuiColor(),
        underlineColor = getUnderlineColor()?.toRatatuiColor(),
        addModifier = getEffects().toModifier(),
        subModifier = Modifier.empty()
    )
}

/**
 * Convert [Style] to [anstyle.Style]
 */
fun Style.toAnstyleStyle(): anstyle.Style {
    var anstyleStyle = anstyle.Style()
    fg?.let { anstyleStyle = anstyleStyle.fgColor(it.toAnstyleColor()) }
    bg?.let { anstyleStyle = anstyleStyle.bgColor(it.toAnstyleColor()) }
    underlineColor?.let { anstyleStyle = anstyleStyle.underlineColor(it.toAnstyleColor()) }
    anstyleStyle = anstyleStyle.effects(addModifier.toEffects())
    return anstyleStyle
}

// ============================================================================
// Tests
// ============================================================================

// Rust original tests:
// #[cfg(test)]
// mod tests {
//     use super::*;
//
//     #[test]
//     fn anstyle_to_color() { ... }
//     ...
// }

class AnsStyleTest {
    @kotlin.test.Test
    fun anstyleToColor() {
        val anstyleColor = Ansi256Color(42u)
        val color = anstyleColor.toRatatuiColor()
        kotlin.test.assertEquals(Color.Indexed(42u), color)
    }

    @kotlin.test.Test
    fun colorToAnsi256Color() {
        val color = Color.Indexed(42u)
        val anstyleColor = color.toAnsi256Color()
        kotlin.test.assertEquals(Ansi256Color(42u), anstyleColor)
    }

    @kotlin.test.Test
    fun colorToAnsi256ColorError() {
        val color = Color.Rgb(0u, 0u, 0u)
        kotlin.test.assertFailsWith<TryFromColorError.Ansi256> {
            color.toAnsi256Color()
        }
    }

    @kotlin.test.Test
    fun ansiColorToColor() {
        val ansiColor = AnsiColor.Red
        val color = ansiColor.toRatatuiColor()
        kotlin.test.assertEquals(Color.Red, color)
    }

    @kotlin.test.Test
    fun colorToAnsiColor() {
        val color = Color.Red
        val ansiColor = color.toAnsiColor()
        kotlin.test.assertEquals(AnsiColor.Red, ansiColor)
    }

    @kotlin.test.Test
    fun colorToAnsiColorError() {
        val color = Color.Rgb(0u, 0u, 0u)
        kotlin.test.assertFailsWith<TryFromColorError.Ansi> {
            color.toAnsiColor()
        }
    }

    @kotlin.test.Test
    fun rgbColorToColor() {
        val rgbColor = RgbColor(255u, 0u, 0u)
        val color = rgbColor.toRatatuiColor()
        kotlin.test.assertEquals(Color.Rgb(255u, 0u, 0u), color)
    }

    @kotlin.test.Test
    fun colorToRgbColor() {
        val color = Color.Rgb(255u, 0u, 0u)
        val rgbColor = color.toRgbColor()
        kotlin.test.assertEquals(RgbColor(255u, 0u, 0u), rgbColor)
    }

    @kotlin.test.Test
    fun colorToRgbColorError() {
        val color = Color.Indexed(42u)
        kotlin.test.assertFailsWith<TryFromColorError.Rgb> {
            color.toRgbColor()
        }
    }

    @kotlin.test.Test
    fun effectsToModifier() {
        val effects = Effects.BOLD or Effects.ITALIC
        val modifier = effects.toModifier()
        kotlin.test.assertTrue(modifier.contains(Modifier.BOLD))
        kotlin.test.assertTrue(modifier.contains(Modifier.ITALIC))
    }

    @kotlin.test.Test
    fun modifierToEffects() {
        val modifier = Modifier.BOLD or Modifier.ITALIC
        val effects = modifier.toEffects()
        kotlin.test.assertTrue(effects.contains(Effects.BOLD))
        kotlin.test.assertTrue(effects.contains(Effects.ITALIC))
    }

    @kotlin.test.Test
    fun anstyleStyleToStyle() {
        val anstyleStyle = anstyle.Style()
            .fgColor(anstyle.Color.Ansi(AnsiColor.Red))
            .bgColor(anstyle.Color.Ansi(AnsiColor.Blue))
            .underlineColor(anstyle.Color.Ansi(AnsiColor.Green))
            .effects(Effects.BOLD or Effects.ITALIC)
        val style = anstyleStyle.toRatatuiStyle()
        kotlin.test.assertEquals(Color.Red, style.fg)
        kotlin.test.assertEquals(Color.Blue, style.bg)
        kotlin.test.assertEquals(Color.Green, style.underlineColor)
        kotlin.test.assertTrue(style.addModifier.contains(Modifier.BOLD))
        kotlin.test.assertTrue(style.addModifier.contains(Modifier.ITALIC))
    }

    @kotlin.test.Test
    fun styleToAnstyleStyle() {
        val style = Style(
            fg = Color.Red,
            bg = Color.Blue,
            underlineColor = Color.Green,
            addModifier = Modifier.BOLD or Modifier.ITALIC,
            subModifier = Modifier.empty()
        )
        val anstyleStyle = style.toAnstyleStyle()
        kotlin.test.assertEquals(anstyle.Color.Ansi(AnsiColor.Red), anstyleStyle.getFgColor())
        kotlin.test.assertEquals(anstyle.Color.Ansi(AnsiColor.Blue), anstyleStyle.getBgColor())
        kotlin.test.assertEquals(anstyle.Color.Ansi(AnsiColor.Green), anstyleStyle.getUnderlineColor())
        kotlin.test.assertTrue(anstyleStyle.getEffects().contains(Effects.BOLD))
        kotlin.test.assertTrue(anstyleStyle.getEffects().contains(Effects.ITALIC))
    }
}
