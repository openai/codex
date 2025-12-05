pluginManagement {
    repositories {
        gradlePluginPortal()
        mavenCentral()
        google()
    }
    // Include ktreesitter's custom Gradle plugin
    includeBuild("ktreesitter-kotlin/ktreesitter-plugin")
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.PREFER_PROJECT)
    repositories {
        mavenCentral()
        google()
    }
}

rootProject.name = "codex-kotlin"

// Vendored libraries - these will eventually become separate projects
include(":ratatui-kotlin")
include(":ansi-to-tui-kotlin")
include(":anstyle-kotlin")
include(":kasuari-kotlin")
include(":roff-kotlin")
include(":cansi-kotlin")

// Tree-sitter Kotlin bindings (vendored from wip/k2 branch)
includeBuild("ktreesitter-kotlin") {
    dependencySubstitution {
        substitute(module("io.github.tree-sitter:ktreesitter")).using(project(":ktreesitter"))
        substitute(module("io.github.tree-sitter:ktreesitter-bash")).using(project(":languages:bash"))
    }
}