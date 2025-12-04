# Contributing

## Commits

Commits must follow the [Conventional Commits] specification.
If you're developing on IntelliJ IDEA or Android Studio,
you can use the [Conventional Commit plugin].

[Conventional Commits]: https://www.conventionalcommits.org/en/v1.0.0/
[Conventional Commit plugin]: https://plugins.jetbrains.com/plugin/13389-conventional-commit

## Prerequisites

- JDK 17+
- Android SDK & NDK
- CMake
- C compiler

## Building

### Clone the repository

```shell
git clone https://github.com/tree-sitter/kotlin-tree-sitter
cd kotlin-tree-sitter
```

### Build the JNI libraries

*This step is only necessary for JVM or Android development.*

Generate the grammar files:

```shell
./gradlew generateGrammarFiles
```

Set the `CMAKE_INSTALL_LIBDIR` environment variable.
The value depends on your system:

- `lib/linux/x64`
- `lib/linux/aarch64`
- `lib/macos/x64`
- `lib/macos/aarch64`
- `lib/windows/x64`

Build the libraries (change `.sh` to `.ps1` on Windows):

```shell
./.github/scripts/build-jni.sh
```

### Run the tests

#### JVM

```shell
./gradlew :ktreesitter:jvmTest
```

#### Android

*Requires a connected device.*

```shell
./gradlew :ktreesitter:connectedDebugAndroidTest
```

#### Native

Linux:

```shell
./gradlew :ktreesitter:linuxX64Test
```

macOS:

```shell
# x64
./gradlew :ktreesitter:macosX64Test
# arm64
./gradlew :ktreesitter:macosArm64Test
```

Windows:

```shell
./gradlew :ktreesitter:mingwX64Test
```

## Linting

Code linting is performed using [Ktlint] and [detekt].
Configuration for IntelliJ IDEA and Android Studio is included
(requires the [Ktlint][Ktlint plugin] and [detekt][detekt plugin] plugins).

[Ktlint]: https://pinterest.github.io/ktlint/latest/
[detekt]: https://detekt.dev/
[Ktlint plugin]: https://plugins.jetbrains.com/plugin/15057-ktlint
[detekt plugin]: https://plugins.jetbrains.com/plugin/10761-detekt
