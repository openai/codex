rootProject.name = "ktreesitter"

pluginManagement {
    includeBuild("ktreesitter-plugin")
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

include(":ktreesitter")

// Language modules are optional - only include if explicitly enabled
// These require additional setup (grammar submodules, Android SDK for CMake builds)
val includeLanguages = System.getenv("KTREESITTER_INCLUDE_LANGUAGES")?.toBoolean() ?: false
if (includeLanguages) {
    file("languages").listFiles { file -> file.isDirectory }?.forEach {
        include(":languages:${it.name}")
    }
}
