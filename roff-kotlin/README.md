# roff-kotlin

[![Kotlin](https://img.shields.io/badge/Kotlin-2.0+-blue.svg?logo=kotlin)](https://kotlinlang.org)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](#license)
[![GitHub](https://img.shields.io/badge/github-KotlinMania%2Froff--kotlin-blue?logo=github)](https://github.com/KotlinMania/roff-kotlin)

A **Kotlin Multiplatform Native** library for generating documents in the ROFF format (man pages).
This is a port of the Rust [roff-rs](https://github.com/rust-cli/roff-rs) library.

## Overview

[ROFF](https://en.wikipedia.org/wiki/Roff_(software)) is a family of Unix text-formatting languages,
implemented by the `nroff`, `troff`, and `groff` programs. This library provides an abstract
representation of ROFF documents, making it easy to generate manual pages programmatically.

## Supported Platforms

- macOS (arm64, x64)
- Linux (x64)
- Windows (x64 via MinGW)

## Installation

### As a Git Submodule (Recommended)

This library is not yet published to Maven Central. The recommended approach is to include it as a
git submodule or vendored dependency:

```bash
git submodule add https://github.com/KotlinMania/roff-kotlin.git
```

Then in your `settings.gradle.kts`:

```kotlin
include(":roff-kotlin")
```

And in your module's `build.gradle.kts`:

```kotlin
kotlin {
    sourceSets {
        val commonMain by getting {
            dependencies {
                implementation(project(":roff-kotlin"))
            }
        }
    }
}
```

## Quick Start

```kotlin
import ai.solace.tui.roff.*

val page = Roff()
    .control("TH", "CORRUPT", "1")
    .control("SH", "NAME")
    .text(roman("corrupt - modify files by randomly changing bits"))
    .control("SH", "SYNOPSIS")
    .text(
        bold("corrupt"), " [".toInline(), bold("-n"), " ".toInline(),
        italic("BITS"), "] [".toInline(), bold("--bits"), " ".toInline(),
        italic("BITS"), "] ".toInline(), italic("FILE"), "...".toInline()
    )
    .control("SH", "DESCRIPTION")
    .text(bold("corrupt"), " modifies files by toggling a randomly chosen bit.".toInline())
    .control("SH", "OPTIONS")
    .control("TP")
    .text(bold("-n"), ", ".toInline(), bold("--bits"), "=".toInline(), italic("BITS"))
    .text(roman("Set the number of bits to modify. Default is one bit."))
    .render()

print(page)
```

## API

### Creating Documents

```kotlin
import ai.solace.tui.roff.*

val doc = Roff()
    .control("TH", "NAME", "SECTION")  // Control line with arguments
    .text(roman("Plain text"))          // Text line
    .render()                            // Render with apostrophe handling
```

### Inline Styles

- `roman("text")` - Normal (roman) font
- `bold("text")` - Bold font
- `italic("text")` - Italic font
- `lineBreak()` - Hard line break
- `"text".toInline()` - Convert string to roman inline

### Rendering

- `render()` - Render with apostrophe preamble (recommended for man pages)
- `toRoff()` - Render without apostrophe handling (for testing)

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](./LICENSE-APACHE))
- MIT license ([LICENSE-MIT](./LICENSE-MIT))

at your option.

---

## Acknowledgments

This Kotlin Multiplatform port was created by **Sydney Renee** of [The Solace Project](mailto:sydney@solace.ofharmony.ai)
for [KotlinMania](https://github.com/KotlinMania).

Special thanks to the original authors:

- The [rust-cli](https://github.com/rust-cli) team for the original [roff-rs](https://github.com/rust-cli/roff-rs) implementation
