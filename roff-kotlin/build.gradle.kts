plugins {
    kotlin("multiplatform") version "2.2.10"
}

group = "ai.solace.tui"
version = "0.1.0-SNAPSHOT"

repositories {
    mavenCentral()
}

kotlin {
    applyDefaultHierarchyTemplate()

    // Native targets
    macosArm64()
    macosX64()
    linuxX64()
    mingwX64()

    sourceSets {
        val commonMain by getting
        val commonTest by getting {
            dependencies {
                implementation(kotlin("test"))
            }
        }
    }
}
