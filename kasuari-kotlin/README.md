# Kasuari-Kotlin

[![Kotlin](https://img.shields.io/badge/Kotlin-2.0+-blue.svg?logo=kotlin)](https://kotlinlang.org)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](#license)
[![GitHub](https://img.shields.io/badge/github-KotlinMania%2Fkasuari--kotlin-blue?logo=github)](https://github.com/KotlinMania/kasuari-kotlin)

A **Kotlin Multiplatform Native** implementation of the Cassowary constraint solving algorithm
([Badros et. al 2001]). This is a port of the Rust [kasuari] library by the Ratatui team.

`Kasuari` is the Indonesian name for the Cassowary bird.

## Overview

Cassowary is designed for solving constraints to lay out user interfaces. Constraints typically take
the form "this button must line up with this text box", or "this box should try to be 3 times the
size of this other box". Its most popular incarnation by far is in Apple's AutoLayout system for
macOS and iOS user interfaces.

This library is a low-level interface to the solving algorithm. It does not have any intrinsic
knowledge of common user interface conventions like rectangular regions or even two dimensions.
These abstractions belong in a higher-level library.

## Supported Platforms

- macOS (arm64, x64)
- Linux (x64)
- Windows (x64 via MinGW)

## Installation

### As a Git Submodule (Recommended)

This library is not yet published to Maven Central. The recommended approach is to include it as a
git submodule or vendored dependency:

```bash
git submodule add https://github.com/KotlinMania/kasuari-kotlin.git
```

Then in your `settings.gradle.kts`:

```kotlin
include(":kasuari-kotlin")
```

And in your module's `build.gradle.kts`:

```kotlin
kotlin {
    sourceSets {
        val commonMain by getting {
            dependencies {
                implementation(project(":kasuari-kotlin"))
            }
        }
    }
}
```

### Future Maven Central Publication

Once published to Maven Central, you'll be able to add it directly:

```kotlin
dependencies {
    implementation("io.github.kotlinmania:kasuari-kotlin:1.0.0")
}
```

## Quick Start

```kotlin
import kasuari.*

// Create a solver
val solver = Solver.new()

// Create variables
val left = Variable.new()
val width = Variable.new()
val right = Variable.new()

// Add constraints: right == left + width
solver.addConstraint(
    right with WeightedRelation.EQ(Strength.REQUIRED) to (left + width)
)

// left == 0
solver.addConstraint(
    left with WeightedRelation.EQ(Strength.REQUIRED) to 0.0
)

// width == 100 (strong, not required)
solver.addConstraint(
    width with WeightedRelation.EQ(Strength.STRONG) to 100.0
)

// Read the solution
println("left: ${solver.getValue(left)}")    // 0.0
println("width: ${solver.getValue(width)}")  // 100.0
println("right: ${solver.getValue(right)}")  // 100.0
```

## Edit Variables

For interactive applications, use edit variables to dynamically change values:

```kotlin
val solver = Solver.new()
val x = Variable.new()

// Add a constraint that x >= 0
solver.addConstraint(x with WeightedRelation.GE(Strength.REQUIRED) to 0.0)

// Register x as an edit variable
solver.addEditVariable(x, Strength.STRONG)

// Suggest values for x
solver.suggestValue(x, 50.0)
println(solver.getValue(x))  // 50.0

solver.suggestValue(x, -10.0)
println(solver.getValue(x))  // 0.0 (constrained to >= 0)
```

## Error Handling

The library provides two styles of error handling:

### Exception-based (default)

```kotlin
try {
    solver.addConstraint(constraint)
} catch (e: AddConstraintError.DuplicateConstraint) {
    println("Constraint already exists")
} catch (e: AddConstraintError.UnsatisfiableConstraint) {
    println("Constraint conflicts with existing constraints")
}
```

### Result-based (Rust-style)

```kotlin
when (val result = solver.tryAddConstraint(constraint)) {
    is Result.Ok -> println("Constraint added")
    is Result.Err -> when (result.error) {
        is AddConstraintError.DuplicateConstraint -> println("Already exists")
        is AddConstraintError.UnsatisfiableConstraint -> println("Conflicts")
        is AddConstraintError.InternalSolver -> println("Internal error")
    }
}
```

## Constraint Strengths

Constraints have strengths that determine priority when conflicts arise:

- `Strength.REQUIRED` - Must be satisfied (solver fails if impossible)
- `Strength.STRONG` - High priority, but can be violated
- `Strength.MEDIUM` - Medium priority
- `Strength.WEAK` - Low priority, used for defaults/preferences

```kotlin
// Required: x must equal 100
val required = x with WeightedRelation.EQ(Strength.REQUIRED) to 100.0

// Strong: x should be at least 0
val strong = x with WeightedRelation.GE(Strength.STRONG) to 0.0

// Weak: x prefers to be 50
val weak = x with WeightedRelation.EQ(Strength.WEAK) to 50.0
```

## License

Licensed under

- MIT license ([LICENSE-MIT](./LICENSE-MIT))

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the
work by you, as defined in the MIT license, shall be licensed as above, without any
additional terms or conditions.

---

## Acknowledgments

This Kotlin Multiplatform port was created by **Sydney Renee** of [The Solace Project](mailto:sydney@solace.ofharmony.ai)
for [KotlinMania](https://github.com/KotlinMania).

Special thanks to the original authors and contributors:

- **Dylan Ede** - Original [Cassowary-rs] Rust implementation (2016)
- **Josh McKinney** and the **Ratatui team** - [kasuari] fork and maintenance (2024)
- The authors of the C++ [Kiwi] library, which heavily influenced the implementation
- **Badros, Borning, and Stuckey** - Original Cassowary algorithm paper (2001)

[Badros et. al 2001]: https://constraints.cs.washington.edu/solvers/cassowary-tochi.pdf
[Kiwi]: https://github.com/nucleic/kiwi
[Cassowary-rs]: https://crates.io/crates/cassowary
[kasuari]: https://github.com/ratatui/kasuari
