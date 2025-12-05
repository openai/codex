package anstyle.examples

// Write ANSI escape code colored text

import anstyle.Ansi256Color
import anstyle.Color
import anstyle.Effects
import anstyle.Style
import anstyle.or

// Rust original:
// use std::io::Write;
//
// fn main() -> Result<(), lexopt::Error> {
//     let args = Args::parse()?;
//     ...
// }

/**
 * Layer determines where the color is applied
 */
enum class Layer {
    Fg,
    Bg,
    Underline
}

/**
 * Command-line arguments for the dump-style example
 */
data class DumpStyleArgs(
    val effects: Effects = Effects.PLAIN,
    val layer: Layer = Layer.Fg
)

/**
 * Create a style with the given color applied to the specified layer
 */
fun style(color: Color, layer: Layer, effects: Effects): Style {
    val baseStyle = when (layer) {
        Layer.Fg -> Style().fgColor(color)
        Layer.Bg -> Style().bgColor(color)
        Layer.Underline -> Style().underlineColor(color)
    }
    return baseStyle or effects
}

/**
 * Format a number with the given style applied
 */
fun formatNumber(fixed: UByte, style: Style): String {
    val render = style.render().toString()
    val reset = style.renderReset().toString()
    return "$render${fixed.toString(16).uppercase().padStart(3)}$reset"
}

/**
 * Main entry point - dumps all 256 colors with their hex codes
 */
fun dumpStyle(args: DumpStyleArgs = DumpStyleArgs()) {
    // Print 4-bit colors (0-15)
    for (fixed in 0u until 16u) {
        val color = Ansi256Color(fixed.toUByte())
            .intoAnsi()
            ?: error("4-bit range used")
        val stl = style(color.toColor(), args.layer, args.effects)
        print(formatNumber(fixed.toUByte(), stl))
        if (fixed == 7u || fixed == 15u) {
            println()
        }
    }

    // Print 6x6x6 cube (16-231)
    for (fixed in 16u until 232u) {
        val col = (fixed - 16u) % 36u
        if (col == 0u) {
            println()
        }
        val color = Ansi256Color(fixed.toUByte())
        val stl = style(color.toColor(), args.layer, args.effects)
        print(formatNumber(fixed.toUByte(), stl))
    }

    println()
    println()

    // Print grayscale ramp (232-255)
    for (fixed in 232u..255u) {
        val color = Ansi256Color(fixed.toUByte())
        val stl = style(color.toColor(), args.layer, args.effects)
        print(formatNumber(fixed.toUByte(), stl))
    }

    println()
}

/**
 * Parse command-line arguments
 *
 * Usage: --layer [fg|bg|underline] --effect [bold|italic|...]
 */
fun parseArgs(args: Array<String>): DumpStyleArgs {
    var effects = Effects.PLAIN
    var layer = Layer.Fg

    val effectsMap = mapOf(
        "bold" to Effects.BOLD,
        "dimmed" to Effects.DIMMED,
        "italic" to Effects.ITALIC,
        "underline" to Effects.UNDERLINE,
        "double_underline" to Effects.DOUBLE_UNDERLINE,
        "curly_underline" to Effects.CURLY_UNDERLINE,
        "dotted_underline" to Effects.DOTTED_UNDERLINE,
        "dashed_underline" to Effects.DASHED_UNDERLINE,
        "blink" to Effects.BLINK,
        "invert" to Effects.INVERT,
        "hidden" to Effects.HIDDEN,
        "strikethrough" to Effects.STRIKETHROUGH
    )

    var i = 0
    while (i < args.size) {
        when (args[i]) {
            "--layer" -> {
                i++
                if (i >= args.size) error("--layer requires a value")
                layer = when (args[i]) {
                    "fg" -> Layer.Fg
                    "bg" -> Layer.Bg
                    "underline" -> Layer.Underline
                    else -> error("expected values fg, bg, underline")
                }
            }
            "--effect" -> {
                i++
                if (i >= args.size) error("--effect requires a value")
                val effect = effectsMap[args[i]]
                    ?: error("expected one of ${effectsMap.keys.joinToString(", ")}")
                effects = effects.insert(effect)
            }
            else -> error("unexpected argument: ${args[i]}")
        }
        i++
    }

    return DumpStyleArgs(effects, layer)
}

/**
 * Main entry point.
 *
 * Usage: dump-style [--layer fg|bg|underline] [--effect bold|italic|...]
 */
fun main(args: Array<String>) {
    val parsedArgs = parseArgs(args)
    dumpStyle(parsedArgs)
}
