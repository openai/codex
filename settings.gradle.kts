pluginManagement {
    repositories {
        gradlePluginPortal()
        mavenCentral()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        mavenCentral()
    }
}

rootProject.name = "codex-kotlin"

// Vendored libraries - these will eventually become separate projects
include(":ratatui-kotlin")
include(":ansi-to-tui-kotlin")
// include(":anstyle-kotlin")  // Not yet ready - needs more porting work