import java.io.OutputStream.nullOutputStream
import org.gradle.internal.os.OperatingSystem
import org.gradle.kotlin.dsl.support.useToRun
import org.gradle.process.ExecOperations
import org.jetbrains.kotlin.gradle.ExperimentalKotlinGradlePluginApi
import org.jetbrains.kotlin.gradle.plugin.mpp.KotlinNativeTarget
import org.jetbrains.kotlin.gradle.tasks.CInteropProcess
import javax.inject.Inject

inline val File.unixPath: String
    get() = if (!os.isWindows) path else path.replace("\\", "/")

// Gradle 9.x interface for injecting ExecOperations
interface ExecInjected {
    @get:Inject val execOps: ExecOperations
}

val execService = objects.newInstance<ExecInjected>()

fun KotlinNativeTarget.treesitterBash() {
    compilations.configureEach {
        cinterops.create("treesitterBash") {
            definitionFile.set(generateTask.interopFile)
            // tree-sitter-bash has headers in bindings/c/tree_sitter/
            includeDirs.allHeaders(grammarDir.resolve("bindings/c/tree_sitter"))
            extraOpts("-libraryPath", libsDir.dir(konanTarget.name))
            tasks.getByName(interopProcessingTaskName).mustRunAfter(generateTask)
        }
    }
}

val os: OperatingSystem = OperatingSystem.current()
val libsDir = layout.buildDirectory.get().dir("libs")
val grammarDir = projectDir.resolve("tree-sitter-bash")

version = grammarDir.resolve("Makefile").readLines()
    .first { it.startsWith("VERSION := ") }.removePrefix("VERSION := ")

plugins {
    alias(libs.plugins.kotlin.mpp)
    id("io.github.tree-sitter.ktreesitter-plugin")
}

grammar {
    baseDir = grammarDir
    grammarName = project.name
    className = "TreeSitterBash"
    packageName = "io.github.treesitter.ktreesitter.bash"
}

val generateTask = tasks.generateGrammarFiles.get()

kotlin {
    // Native targets only - no JVM/Android for simplicity
    linuxX64 { treesitterBash() }
    linuxArm64 { treesitterBash() }
    mingwX64 { treesitterBash() }
    macosArm64 { treesitterBash() }
    macosX64 { treesitterBash() }

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
    }
}

// Build the native library for cinterop
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
    // Object files are placed in grammarDir (working directory)
    val objectFiles = grammarFiles.map {
        grammarDir.resolve(it.nameWithoutExtension + ".o").path
    }.toTypedArray()

    doFirst {
        // Ensure lib output directory exists
        libFile.parentFile.mkdirs()

        // Compile each source file separately to control output location
        grammarFiles.forEach { sourceFile ->
            val objectFile = grammarDir.resolve(sourceFile.nameWithoutExtension + ".o")
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
                write("-o\n")
                write(objectFile.unixPath + "\n")
                write(sourceFile.unixPath + "\n")
            }

            execService.execOps.exec {
                executable = runKonan
                workingDir = grammarDir
                standardOutput = nullOutputStream()
                args("clang", "clang", konanTarget.name, "@" + argsFile.path)
            }
        }

        execService.execOps.exec {
            executable = runKonan
            workingDir = grammarDir
            standardOutput = nullOutputStream()
            args("llvm", "llvm-ar", "rcs", libFile.path, *objectFiles)
        }
    }

    inputs.files(*grammarFiles)
    outputs.file(libFile)
}
