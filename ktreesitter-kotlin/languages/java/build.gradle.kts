import java.io.OutputStream.nullOutputStream
import org.gradle.internal.os.OperatingSystem
import org.gradle.kotlin.dsl.support.useToRun
import org.jetbrains.kotlin.gradle.ExperimentalKotlinGradlePluginApi
import org.jetbrains.kotlin.gradle.plugin.mpp.KotlinNativeTarget
import org.jetbrains.kotlin.gradle.tasks.CInteropProcess

inline val File.unixPath: String
    get() = if (!os.isWindows) path else path.replace("\\", "/")

fun KotlinNativeTarget.treesitterJava() {
    compilations.configureEach {
        cinterops.create("treesitterJava") {
            definitionFile.set(generateTask.interopFile)
            includeDirs.allHeaders(grammarDir.resolve("bindings/c"))
            extraOpts("-libraryPath", libsDir.dir(konanTarget.name))
            tasks.getByName(interopProcessingTaskName).mustRunAfter(generateTask)
        }
    }
}

val os: OperatingSystem = OperatingSystem.current()
val libsDir = layout.buildDirectory.get().dir("libs")
val grammarDir = projectDir.resolve("tree-sitter-java")

version = grammarDir.resolve("Makefile").readLines()
    .first { it.startsWith("VERSION := ") }.removePrefix("VERSION := ")

plugins {
    `maven-publish`
    signing
    alias(libs.plugins.kotlin.mpp)
    alias(libs.plugins.android.library)
    id("io.github.tree-sitter.ktreesitter-plugin")
}

grammar {
    baseDir = grammarDir
    grammarName = project.name
    className = "TreeSitterJava"
    packageName = "io.github.treesitter.ktreesitter.java"
}

val generateTask = tasks.generateGrammarFiles.get()

kotlin {
    jvm {}

    androidTarget {
        withSourcesJar(true)
        publishLibraryVariants("release")
    }

    linuxX64 { treesitterJava() }
    linuxArm64 { treesitterJava() }
    mingwX64 { treesitterJava() }
    macosArm64 { treesitterJava() }
    macosX64 { treesitterJava() }
    iosArm64 { treesitterJava() }
    iosSimulatorArm64 { treesitterJava() }

    applyDefaultHierarchyTemplate()

    jvmToolchain(17)

    sourceSets {
        val generatedSrc = generateTask.generatedSrc.get()
        configureEach {
            kotlin.srcDir(generatedSrc.dir(name).dir("kotlin"))
        }

        commonMain {
            @OptIn(ExperimentalKotlinGradlePluginApi::class)
            languageSettings {
                compilerOptions {
                    freeCompilerArgs.add("-Xexpect-actual-classes")
                }
            }

            dependencies {
                implementation(libs.kotlin.stdlib)
            }
        }

        jvmMain {
            resources.srcDir(generatedSrc.dir(name).dir("resources"))
        }
    }
}

android {
    namespace = "io.github.treesitter.ktreesitter.${grammar.grammarName.get()}"
    compileSdk = (property("sdk.version.compile") as String).toInt()
    ndkVersion = property("ndk.version") as String
    defaultConfig {
        minSdk = (property("sdk.version.min") as String).toInt()
        ndk {
            //noinspection ChromeOsAbiSupport
            abiFilters += setOf("x86_64", "arm64-v8a", "armeabi-v7a")
        }
        resValue("string", "version", version as String)
    }
    externalNativeBuild {
        cmake {
            path = generateTask.cmakeListsFile.get().asFile
            buildStagingDirectory = file(".cmake")
            version = property("cmake.version") as String
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

// TODO: replace with cmake
@Suppress("DEPRECATION")
tasks.withType<CInteropProcess>().configureEach {
    if (name.startsWith("cinteropTest")) return@configureEach

    val srcDir = grammarDir.resolve("src")
    val grammarFiles =
        if (!srcDir.resolve("scanner.c").isFile) arrayOf(srcDir.resolve("parser.c"))
        else arrayOf(srcDir.resolve("parser.c"), srcDir.resolve("scanner.c"))
    val grammarName = grammar.grammarName.get()
    val runKonan = File(konanHome.get()).resolve("bin")
        .resolve(if (os.isWindows) "run_konan.bat" else "run_konan").path
    val libFile = libsDir.dir(konanTarget.name).file("libtree-sitter-$grammarName.a").asFile
    val objectFiles = grammarFiles.map {
        srcDir.resolve(it.nameWithoutExtension + ".o").path
    }.toTypedArray()

    doFirst {
        val argsFile = File.createTempFile("args", null)
        argsFile.deleteOnExit()
        argsFile.writer().useToRun {
            write("-I" + srcDir.unixPath + "\n")
            write("-DTREE_SITTER_HIDE_SYMBOLS\n")
            write("-fvisibility=hidden\n")
            write("-std=c11\n")
            write("-O2\n")
            write("-g\n")
            write("-c\n")
            grammarFiles.forEach { write(it.unixPath + "\n") }
        }

        exec {
            executable = runKonan
            workingDir = grammarDir
            standardOutput = nullOutputStream()
            args("clang", "clang", konanTarget.name, "@" + argsFile.path)
        }

        exec {
            executable = runKonan
            workingDir = grammarDir
            standardOutput = nullOutputStream()
            args("llvm", "llvm-ar", "rcs", libFile.path, *objectFiles)
        }
    }

    inputs.files(*grammarFiles)
    outputs.file(libFile)
}

tasks.register<Jar>("javadocJar") {
    group = "documentation"
    archiveClassifier.set("javadoc")
}

publishing {
    publications.withType(MavenPublication::class) {
        val grammarName = grammar.grammarName.get()
        artifactId = grammar.libraryName.get()
        artifact(tasks["javadocJar"])
        pom {
            name.set("KTreeSitter $grammarName")
            description.set("$grammarName grammar for KTreeSitter")
            licenses {
                license {
                    name.set("MIT License")
                    url.set("https://spdx.org/licenses/MIT.html")
                }
            }
            developers {
                developer {
                    id.set("ObserverOfTime")
                    email.set("chronobserver@disroot.org")
                    url.set("https://github.com/ObserverOfTime")
                }
            }
            scm {
                url.set("https://github.com/tree-sitter/kotlin-tree-sitter")
                connection.set("scm:git:git://github.com/tree-sitter/kotlin-tree-sitter.git")
                developerConnection.set(
                    "scm:git:ssh://github.com/tree-sitter/kotlin-tree-sitter.git"
                )
            }
        }
    }

    repositories {
        maven {
            name = "local"
            url = uri(layout.buildDirectory.dir("repo"))
        }
    }
}

signing {
    isRequired = System.getenv("CI") != null
    if (isRequired) {
        val key = System.getenv("SIGNING_KEY")
        val password = System.getenv("SIGNING_PASSWORD")
        useInMemoryPgpKeys(key, password)
    }
    sign(publishing.publications)
}

tasks.withType<AbstractPublishToMaven>().configureEach {
    mustRunAfter(tasks.withType<Sign>())
}
