plugins {
    kotlin("multiplatform") version "2.2.10"
    kotlin("plugin.serialization") version "2.2.10"
}

kotlin {
    applyDefaultHierarchyTemplate()

    sourceSets.all {
        languageSettings.optIn("kotlin.time.ExperimentalTime")
    }

    macosArm64 {
        binaries {
            executable {
                entryPoint = "main"
            }
        }
    }
    macosX64 {
        binaries {
            executable {
                entryPoint = "main"
            }
        }
    }

    // Define Linux target so we can confine certain dependencies/code to Linux only
    linuxX64()

    sourceSets {
        val nativeMain by getting {
            dependencies {
                implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.10.2")
                implementation("org.jetbrains.kotlinx:kotlinx-serialization-core:1.9.0")
                implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.9.0")
                implementation("org.jetbrains.kotlinx:kotlinx-io-core:0.8.2")

                // Ktor HTTP client for native platforms
                implementation("io.ktor:ktor-client-core:2.3.7")
                implementation("io.ktor:ktor-client-curl:2.3.7")
                implementation("io.ktor:ktor-client-content-negotiation:2.3.7")
                implementation("io.ktor:ktor-serialization-kotlinx-json:2.3.7")
                implementation("io.ktor:ktor-client-auth:2.3.7")
                
                // File I/O
                implementation("com.squareup.okio:okio:3.9.0")

                // Character encoding support (for legacy codepage conversion)
                // fleeksoft-io provides JDK-like IO classes for Kotlin Multiplatform
                implementation("com.fleeksoft.io:io-core:0.0.4")
                implementation("com.fleeksoft.io:io:0.0.4")
                implementation("com.fleeksoft.charset:charset:0.0.5")
                implementation("com.fleeksoft.charset:charset-ext:0.0.5")

                // Tree-sitter parsing library bindings
                implementation("io.github.tree-sitter:ktreesitter:0.24.1")
            }
        }
        
        val nativeTest by getting {
            dependencies {
                implementation(kotlin("test"))
            }
        }
    }
}