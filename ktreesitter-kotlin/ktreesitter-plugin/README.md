# KTreeSitter plugin

A plugin that generates code for KTreeSitter grammar packages.

## Installation

```groovy
buildscript {
  repositories {
    gradlePluginPortal()
  }
}

plugins {
  id("io.github.tree-sitter.ktreesitter-plugin")
}
```

## Configuration

```groovy
grammar {
  /* Default options */
  // The base directory of the grammar
  baseDir = projectDir.parentFile.parentFile
  // The name of the C interop def file
  interopName = "grammar"
  // The name of the JNI library
  libraryName = "ktreesitter-$grammarName"

  /* Required options */
  // The name of the grammar
  grammarName = "java"
  // The name of the class
  className = "TreeSitterJava"
  // The name of the package
  packageName = "io.github.treesitter.ktreesitter.java"
}
```
