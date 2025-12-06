plugins {
    kotlin("multiplatform") version "2.2.10"
}

group = "ai.solace.ansi"
version = "0.1.0-SNAPSHOT"

kotlin {
    applyDefaultHierarchyTemplate()

    // Native targets
    macosArm64()
    macosX64()
    linuxX64()
    mingwX64()

    sourceSets {
        val commonMain by getting {
            // Custom source directory to match current layout
            kotlin.srcDir("commonMain/ansitotui/src")

            dependencies {
                // Depends on ratatui-kotlin for Text, Span, Style types
                implementation(project(":ratatui-kotlin"))
            }
        }

        val commonTest by getting {
            kotlin.srcDir("commonTest/kotlin")
            dependencies {
                implementation(kotlin("test"))
            }
        }

        val nativeMain by getting {
            dependencies {
                // No additional native dependencies
            }
        }

        val nativeTest by getting {
            dependencies {
                implementation(kotlin("test"))
            }
        }
    }
}
