package anstyle.parse.benches

// Benchmark tests for the ANSI parser
// Note: Kotlin/Native doesn't have a standard benchmarking library like Rust's divan,
// so these are simple timing tests that can be run manually.

import anstyle.parse.AsciiParser
import anstyle.parse.Params
import anstyle.parse.Parser
import anstyle.parse.Perform
import anstyle.parse.state.State
import anstyle.parse.state.Action
import anstyle.parse.state.stateChange
import kotlin.time.measureTime
import okio.FileSystem
import okio.Path.Companion.toPath

// Rust original:
// use std::hint::black_box;
// use anstyle_parse::{DefaultCharAccumulator, Params, Parser, Perform};

/**
 * A dispatcher that does nothing (for benchmarking parser overhead)
 */
class BenchDispatcher : Perform {
    override fun print(c: Char) {
        // black_box equivalent - just consume the value
    }

    override fun execute(byte: UByte) {
        // black_box equivalent
    }

    override fun hook(params: Params, intermediates: UByteArray, ignore: Boolean, action: UByte) {
        // black_box equivalent
    }

    override fun put(byte: UByte) {
        // black_box equivalent
    }

    override fun oscDispatch(params: Array<ByteArray>, bellTerminated: Boolean) {
        // black_box equivalent
    }

    override fun csiDispatch(params: Params, intermediates: UByteArray, ignore: Boolean, action: UByte) {
        // black_box equivalent
    }

    override fun escDispatch(intermediates: UByteArray, ignore: Boolean, byte: UByte) {
        // black_box equivalent
    }
}

/**
 * A dispatcher that strips ANSI codes and collects printable text
 */
class Strip(capacity: Int = 0) : Perform {
    val result: StringBuilder = StringBuilder(capacity)

    override fun print(c: Char) {
        result.append(c)
    }

    override fun execute(byte: UByte) {
        val b = byte.toByte()
        if (b.toInt().toChar().isWhitespace()) {
            result.append(b.toInt().toChar())
        }
    }
}

/**
 * Strip ANSI codes using direct state machine (optimized version)
 */
fun stripStr(content: String): String {
    fun isUtf8Continuation(b: UByte): Boolean = b.toInt() in 0x80..0xbf

    fun isPrintable(action: Action, byte: UByte): Boolean {
        return action == Action.Print ||
            action == Action.BeginUtf8 ||
            // since we know the input is valid UTF-8, the only thing we can do with
            // continuations is to print them
            isUtf8Continuation(byte) ||
            (action == Action.Execute && byte.toByte().toInt().toChar().isWhitespace())
    }

    val stripped = mutableListOf<Byte>()
    val bytes = content.encodeToByteArray()
    var offset = 0

    while (offset < bytes.size) {
        // Find first non-printable position
        var printableEnd = offset
        while (printableEnd < bytes.size) {
            val b = bytes[printableEnd].toUByte()
            val (_, action) = stateChange(State.Ground, b)
            if (!isPrintable(action, b)) break
            printableEnd++
        }

        // Copy printable portion
        for (i in offset until printableEnd) {
            stripped.add(bytes[i])
        }
        offset = printableEnd

        // Skip non-printable portion
        var state = State.Ground
        while (offset < bytes.size) {
            val b = bytes[offset].toUByte()
            val (nextState, action) = stateChange(state, b)
            if (nextState != State.Anywhere) {
                state = nextState
            }
            if (isPrintable(action, b)) break
            offset++
        }
    }

    return stripped.toByteArray().decodeToString()
}

/**
 * Read a file into a ByteArray using Okio
 */
fun readFileBytes(path: String): ByteArray? {
    val filePath = path.toPath()
    return try {
        FileSystem.SYSTEM.read(filePath) {
            readByteArray()
        }
    } catch (e: Exception) {
        null
    }
}

/**
 * Benchmark data samples
 */
data class BenchData(val name: String, val content: ByteArray) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is BenchData) return false
        return name == other.name && content.contentEquals(other.content)
    }

    override fun hashCode(): Int = 31 * name.hashCode() + content.contentHashCode()

    override fun toString(): String = name
}

/**
 * Load benchmark data from test files.
 * Returns the list of available benchmark data sets.
 */
fun loadBenchData(testDir: String = "../tests"): List<BenchData> {
    val data = mutableListOf<BenchData>()

    // Always include the inline state_changes test
    data.add(BenchData("0-state_changes", "\u001b]2;X\u001b\\ \u001b[0m \u001bP0@\u001b\\".encodeToByteArray()))

    // Load test files if available
    readFileBytes("$testDir/demo.vte")?.let {
        data.add(BenchData("1-demo.vte", it))
    }
    readFileBytes("$testDir/rg_help.vte")?.let {
        data.add(BenchData("2-rg_help.vte", it))
    }
    readFileBytes("$testDir/rg_linus.vte")?.let {
        data.add(BenchData("3-rg_linus.vte", it))
    }

    return data
}

/**
 * Run parser advance benchmark
 */
fun benchAdvance(data: BenchData, iterations: Int = 1000): Long {
    val duration = measureTime {
        repeat(iterations) {
            val dispatcher = BenchDispatcher()
            val parser = Parser<AsciiParser>()
            for (byte in data.content) {
                parser.advance(dispatcher, byte.toUByte())
            }
        }
    }
    return duration.inWholeMilliseconds
}

/**
 * Run parser advance with stripping benchmark
 */
fun benchAdvanceStrip(data: BenchData, iterations: Int = 1000): Long {
    val duration = measureTime {
        repeat(iterations) {
            val stripped = Strip(data.content.size)
            val parser = Parser<AsciiParser>()
            for (byte in data.content) {
                parser.advance(stripped, byte.toUByte())
            }
        }
    }
    return duration.inWholeMilliseconds
}

/**
 * Run state change benchmark
 */
fun benchStateChange(data: BenchData, iterations: Int = 1000): Long {
    val duration = measureTime {
        repeat(iterations) {
            var state = State.Ground
            for (byte in data.content) {
                val (nextState, action) = stateChange(state, byte.toUByte())
                state = nextState
            }
        }
    }
    return duration.inWholeMilliseconds
}

/**
 * Verify that Strip and stripStr produce the same results
 */
fun verifyData(benchData: List<BenchData>) {
    for (data in benchData) {
        val content = data.content.decodeToString()

        val stripped = Strip(content.length)
        val parser = Parser<AsciiParser>()
        for (byte in content.encodeToByteArray()) {
            parser.advance(stripped, byte.toUByte())
        }

        val expected = stripStr(content)
        check(stripped.result.toString() == expected) {
            "Mismatch for ${data.name}: '${stripped.result}' != '$expected'"
        }
    }
    println("All data verified successfully")
}

/**
 * Main entry point for running benchmarks.
 *
 * Note: This is a simple timing-based benchmark since Kotlin/Native
 * doesn't have a standard benchmarking framework like Rust's divan.
 */
fun main() {
    val benchData = loadBenchData()
    println("Loaded ${benchData.size} benchmark datasets")

    verifyData(benchData)

    for (data in benchData) {
        println("\nBenchmark: ${data.name} (${data.content.size} bytes)")
        println("  advance:       ${benchAdvance(data)}ms for 1000 iterations")
        println("  advance_strip: ${benchAdvanceStrip(data)}ms for 1000 iterations")
        println("  state_change:  ${benchStateChange(data)}ms for 1000 iterations")
    }
}
