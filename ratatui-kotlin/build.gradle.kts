plugins {
    kotlin("multiplatform") version "2.2.10"
}

group = "ai.solace.ratatui"
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
            // Uses standard src/commonMain/kotlin layout
        }

        val commonTest by getting {
            dependencies {
                implementation(kotlin("test"))
            }
        }

        val nativeMain by getting {
            // No external dependencies - ratatui-kotlin is a pure Kotlin library
        }

        val nativeTest by getting {
            dependencies {
                implementation(kotlin("test"))
            }
        }
    }
}
