# KTreeSitter

Kotlin bindings to the [tree-sitter] parsing library.

## Supported platforms

- [x] JVM
- [x] Android
- [x] Native
- [ ] WASI

*JS and WasmJS will not be supported.*

## Installation

```kotlin
dependencies {
    implementation("io.github.tree-sitter:ktreesitter") version $ktreesitterVersion
}

repositories {
    mavenCentral()
}
```

## Basic usage

```kotlin
val language = Language(TreeSitterKotlin.language())
val parser = Parser(language)
val tree = parser.parse("fun main() {}")
val rootNode = tree.rootNode

assert(rootNode.type == "source_file")
assert(rootNode.startPoint.column == 0)
assert(rootNode.endPoint.column == 13)
```

[tree-sitter]: https://tree-sitter.github.io/tree-sitter/
