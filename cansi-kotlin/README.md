# cansi-kotlin

[![Kotlin](https://img.shields.io/badge/Kotlin-2.0+-blue.svg?logo=kotlin)](https://kotlinlang.org)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](#license)
[![GitHub](https://img.shields.io/badge/github-KotlinMania%2Fcansi--kotlin-blue?logo=github)](https://github.com/KotlinMania/cansi-kotlin)

A **Kotlin Multiplatform Native** library for parsing and categorising ANSI escape codes.
This is a port of the Rust [cansi](https://github.com/kurtlawrence/cansi) crate.

## Overview

**C**ategorise **ANSI** - ANSI escape code parser and categoriser

`cansi-kotlin` parses text with ANSI escape sequences and returns deconstructed text with
metadata about coloring and styling. The library focuses on CSI (Control Sequence Introducer)
sequences, particularly SGR (Select Graphic Rendition) parameters.

## Supported Platforms

- macOS (arm64, x64)
- Linux (x64)
- Windows (x64 via MinGW)

## Installation

### As a Git Submodule (Recommended)

This library is not yet published to Maven Central. The recommended approach is to include it as a
git submodule or vendored dependency:

```bash
git submodule add https://github.com/KotlinMania/cansi-kotlin.git
```

Then in your `settings.gradle.kts`:

```kotlin
include(":cansi-kotlin")
```

And in your module's `build.gradle.kts`:

```kotlin
kotlin {
    sourceSets {
        val commonMain by getting {
            dependencies {
                implementation(project(":cansi-kotlin"))
            }
        }
    }
}
```

## Quick Start

```kotlin
import cansi.*

// Parse text with ANSI escape sequences
val text = "\u001b[31mHello\u001b[0m, \u001b[32mWorld\u001b[0m!"
val slices = categoriseText(text)

// Each slice contains the text and its styling
for (slice in slices) {
    println("Text: '${slice.text}', Color: ${slice.fg}")
}
// Output:
// Text: 'Hello', Color: Red
// Text: ', ', Color: null
// Text: 'World', Color: Green
// Text: '!', Color: null

// Reconstruct the plain text without escape codes
val plainText = constructTextNoCodes(slices)
println(plainText) // "Hello, World!"
```

## API

### Core Functions

- `parse(text)` - Find all ANSI escape sequences and return their positions
- `categoriseText(text)` - Parse text and return styled slices with color/formatting info
- `constructTextNoCodes(slices)` - Reconstruct plain text from categorized slices
- `lineIter(slices)` - Iterate over slices line by line

### Data Types

- `CategorisedSlice` - A text slice with styling information (colors, bold, italic, etc.)
- `Color` - The 16 standard ANSI colors (8 normal + 8 bright variants)
- `Intensity` - Text intensity (Normal, Bold, Faint)
- `Match` - An ANSI escape sequence match with byte positions

### Supported SGR Parameters

- **Colors**: All 16 standard colors (foreground 30-37, 90-97; background 40-47, 100-107)
- **Intensity**: Bold (1), Faint (2), Normal (22)
- **Styles**: Italic (3), Underline (4), Blink (5), Reversed (7), Hidden (8), Strikethrough (9)
- **Reset**: Code 0 resets all attributes

## Example: Styled Text Analysis

```kotlin
import cansi.*

val styledText = "\u001b[1;31;4mError:\u001b[0m Something went wrong"
val slices = categoriseText(styledText)

// First slice: "Error:" with bold, red, underline
val errorSlice = slices[0]
println("Text: ${errorSlice.text}")           // "Error:"
println("Bold: ${errorSlice.intensity}")      // Bold
println("Color: ${errorSlice.fg}")            // Red
println("Underline: ${errorSlice.underline}") // true

// Second slice: " Something went wrong" with default styling
val msgSlice = slices[1]
println("Text: ${msgSlice.text}")             // " Something went wrong"
println("Color: ${msgSlice.fg}")              // null (default)
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](./LICENSE-APACHE))
- MIT license ([LICENSE-MIT](./LICENSE-MIT))

at your option.

---

## Acknowledgments

This Kotlin Multiplatform port was created by **Sydney Renee** of [The Solace Project](mailto:sydney@solace.ofharmony.ai)
for [KotlinMania](https://github.com/KotlinMania).

Special thanks to the original author:

- [Kurt Lawrence](https://github.com/kurtlawrence) for the original [cansi](https://github.com/kurtlawrence/cansi) Rust implementation
