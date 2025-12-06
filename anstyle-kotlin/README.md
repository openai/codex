# anstyle-kotlin

[![Kotlin](https://img.shields.io/badge/Kotlin-2.0+-blue.svg?logo=kotlin)](https://kotlinlang.org)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](#license)
[![GitHub](https://img.shields.io/badge/github-KotlinMania%2Fanstyle--kotlin-blue?logo=github)](https://github.com/KotlinMania/anstyle-kotlin)

A **Kotlin Multiplatform Native** library for ANSI text styling and terminal color support.
This is a port of the Rust [anstyle](https://github.com/rust-cli/anstyle) ecosystem.

## Overview

`anstyle-kotlin` provides a complete toolkit for working with ANSI terminal styling:

- **Core styling** (`ai.solace.tui.anstyle`) - Define and render ANSI styles
- **Escape parsing** (`ai.solace.tui.anstyle.parse`) - Parse ANSI escape sequences
- **Color conversion** (`ai.solace.tui.anstyle.lossy`) - Convert between color formats
- **ROFF generation** (`ai.solace.tui.anstyle.roff`) - Convert ANSI to man page format

## Supported Platforms

- macOS (arm64, x64)
- Linux (x64)
- Windows (x64 via MinGW)

## Installation

### As Part of a Multi-Project Build

Include in your `settings.gradle.kts`:

```kotlin
include(":anstyle-kotlin")

// If using anstyle-roff, also include dependencies:
include(":roff-kotlin")
include(":cansi-kotlin")
```

And in your module's `build.gradle.kts`:

```kotlin
kotlin {
    sourceSets {
        val commonMain by getting {
            dependencies {
                implementation(project(":anstyle-kotlin"))
            }
        }
    }
}
```

### Standalone with Composite Builds

If using anstyle-kotlin standalone with the roff module, configure `settings.gradle.kts`:

```kotlin
rootProject.name = "your-project"

includeBuild("path/to/roff-kotlin") {
    dependencySubstitution {
        substitute(module("ai.solace.tui:roff-kotlin")).using(project(":"))
    }
}

includeBuild("path/to/cansi-kotlin") {
    dependencySubstitution {
        substitute(module("ai.solace.tui:cansi-kotlin")).using(project(":"))
    }
}
```

## Quick Start

### Basic Styling

```kotlin
import ai.solace.tui.anstyle.*

// Create a style with foreground color and effects
val errorStyle = Style()
    .fgColor(AnsiColor.Red.toColor())
    .bold()

// Render styled text
val message = "Error: File not found"
println("${errorStyle.render()}$message${errorStyle.renderReset()}")

// Chain colors with backgrounds
val warningStyle = AnsiColor.Yellow.on(AnsiColor.Black)
println("${warningStyle.render()}Warning${warningStyle.renderReset()}")
```

### Color Types

```kotlin
import ai.solace.tui.anstyle.*

// 4-bit ANSI colors (16 colors)
val red = AnsiColor.Red.toColor()
val brightBlue = AnsiColor.BrightBlue.toColor()

// 8-bit colors (256 colors)
val color256 = Ansi256Color(208u).toColor()  // Orange

// 24-bit RGB colors
val rgb = RgbColor(255u, 128u, 0u).toColor()  // Custom orange
```

### Effects

```kotlin
import ai.solace.tui.anstyle.*

// Individual effects
val boldStyle = Style().bold()
val italicStyle = Style().italic()
val underlineStyle = Style().underline()

// Combined effects
val emphatic = Style()
    .bold()
    .underline()
    .fgColor(AnsiColor.Red.toColor())

// Using Effects directly
val effects = Effects.BOLD or Effects.ITALIC
val style = Style().effects(effects)
```

### Converting ANSI to ROFF (Man Pages)

```kotlin
import ai.solace.tui.anstyle.roff.toRoff

// Convert ANSI-styled CLI help text to ROFF format
val helpText = "\u001b[1mUsage:\u001b[0m myapp \u001b[4m<command>\u001b[0m"
val roffDoc = toRoff(helpText)

// Render for use with groff/troff
val manPage = roffDoc.render()
```

### Lossy Color Conversion

```kotlin
import ai.solace.tui.anstyle.lossy.*
import ai.solace.tui.anstyle.*

// Convert RGB to nearest 4-bit color
val rgb = RgbColor(200u, 50u, 50u)
val ansi = rgbToAnsi(rgb)  // Approximately Red

// Convert 256-color to RGB
val xterm = Ansi256Color(208u)
val asRgb = xtermToRgb(xterm)

// Use a specific palette for conversion
val vgaRed = ansiToRgb(AnsiColor.Red, Palette.VGA)
val win10Red = ansiToRgb(AnsiColor.Red, Palette.WIN10_CONSOLE)
```

## Modules

### Core (`ai.solace.tui.anstyle`)

- `Style` - ANSI text style (colors + effects)
- `Color` - Sealed class for Ansi/Ansi256/Rgb colors
- `AnsiColor` - 16 standard ANSI colors
- `Ansi256Color` - 256-color palette
- `RgbColor` - 24-bit true color
- `Effects` - Text effects (bold, italic, underline, etc.)

### Parse (`ai.solace.tui.anstyle.parse`)

- `Parser` - State machine for parsing ANSI escape sequences
- `Params` - Parameter extraction from escape sequences

### Lossy (`ai.solace.tui.anstyle.lossy`)

- Color conversion functions between formats
- `Palette` - Color palettes (VGA, Windows 10 Console)

### Roff (`ai.solace.tui.anstyle.roff`)

- `toRoff()` - Convert ANSI-styled text to ROFF documents
- `styledStream()` - Parse ANSI text into styled segments

## Dependencies

The `anstyle-roff` module requires:
- [roff-kotlin](../roff-kotlin) - ROFF document generation
- [cansi-kotlin](../cansi-kotlin) - ANSI escape code parsing

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](./LICENSE-APACHE))
- MIT license ([LICENSE-MIT](./LICENSE-MIT))

at your option.

---

## Acknowledgments

This Kotlin Multiplatform port was created by **Sydney Renee** of [The Solace Project](mailto:sydney@thesolace.ai)
for [KotlinMania](https://github.com/KotlinMania).

Special thanks to the original authors:

- The [rust-cli](https://github.com/rust-cli) team for the original [anstyle](https://github.com/rust-cli/anstyle) ecosystem
