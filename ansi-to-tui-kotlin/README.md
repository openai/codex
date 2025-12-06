# ansi-to-tui-kotlin

[![Kotlin](https://img.shields.io/badge/Kotlin-2.0+-blue.svg?logo=kotlin)](https://kotlinlang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](#license)
[![GitHub](https://img.shields.io/badge/github-KotlinMania%2Fansi--to--tui--kotlin-blue?logo=github)](https://github.com/KotlinMania/ansi-to-tui-kotlin)

A **Kotlin Multiplatform Native** library to parse text with ANSI color codes and turn them into
[`ratatui.text.Text`][Text]. This is a port of the Rust [ansi-to-tui] library by Uttarayan Mondal.

## Overview

Parse ANSI escape sequences from terminal output and convert them to styled text objects
compatible with ratatui-kotlin for TUI rendering.

| Color  | Supported | Examples                 |
| ------ | :-------: | ------------------------ |
| 24 bit |     ✓     | `\x1b[38;2;<R>;<G>;<B>m` |
| 8 bit  |     ✓     | `\x1b[38;5;<N>m`         |
| 4 bit  |     ✓     | `\x1b[30..37;40..47m`    |

## Supported Platforms

- macOS (arm64, x64)
- Linux (x64)
- Windows (x64 via MinGW)

## Installation

### As a Git Submodule (Recommended)

This library is not yet published to Maven Central. The recommended approach is to include it as a
git submodule or vendored dependency:

```bash
git submodule add https://github.com/KotlinMania/ansi-to-tui-kotlin.git
```

Then in your `settings.gradle.kts`:

```kotlin
include(":ansi-to-tui-kotlin")
```

And in your module's `build.gradle.kts`:

```kotlin
kotlin {
    sourceSets {
        val commonMain by getting {
            dependencies {
                implementation(project(":ansi-to-tui-kotlin"))
            }
        }
    }
}
```

### Future Maven Central Publication

Once published to Maven Central, you'll be able to add it directly:

```kotlin
dependencies {
    implementation("io.github.kotlinmania:ansi-to-tui-kotlin:1.0.0")
}
```

## Quick Start

```kotlin
import ansitotui.intoText

// Parse ANSI-colored text from a string
val text = "\u001b[38;2;225;192;203mPink Text\u001b[0m".intoText()

// Parse from a ByteArray
val bytes = someFile.readBytes()
val styledText = bytes.intoText()
```

## Supported Escape Sequences

- **Style modifiers**: Bold, Italic, Underline, Blink, Reverse, Hidden, Strikethrough
- **4-bit colors**: Standard (30-37, 40-47) and bright (90-97, 100-107)
- **8-bit colors**: 256-color palette via `38;5;N` and `48;5;N`
- **24-bit colors**: True color RGB via `38;2;R;G;B` and `48;2;R;G;B`
- **Reset codes**: Full reset (0) and individual attribute resets

## License

Licensed under MIT license ([LICENSE](./LICENSE))

---

## Acknowledgments

This Kotlin Multiplatform port was created by **Sydney Renee** of [The Solace Project](mailto:sydney@solace.ofharmony.ai)
for [KotlinMania](https://github.com/KotlinMania).

Special thanks to the original author:

- **Uttarayan Mondal** - Original [ansi-to-tui] Rust implementation

[Text]: https://docs.rs/ratatui/latest/ratatui/text/struct.Text.html
[ansi-to-tui]: https://github.com/uttarayan21/ansi-to-tui
