plugins {
    kotlin("multiplatform") version "2.2.10"
}

group = "io.github.kotlinmania"
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
        val commonMain by getting {
            dependencies {
                implementation("io.github.kotlinmania:roff-kotlin")
                implementation("io.github.kotlinmania:cansi-kotlin")
            }
        }
        val commonTest by getting {
            dependencies {
                implementation(kotlin("test"))
            }
        }
    }
}
