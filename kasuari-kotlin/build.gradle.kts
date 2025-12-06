plugins {
    kotlin("multiplatform") version "2.2.10"
}

group = "io.github.kasuari"
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
            kotlin.srcDir("commonMain/src")
        }

        val commonTest by getting {
            kotlin.srcDir("commonTest/kotlin")
            dependencies {
                implementation(kotlin("test"))
            }
        }

        val nativeMain by getting {
            // Pure Kotlin library - no platform-specific dependencies
        }

        val nativeTest by getting {
            dependencies {
                implementation(kotlin("test"))
            }
        }
    }
}
