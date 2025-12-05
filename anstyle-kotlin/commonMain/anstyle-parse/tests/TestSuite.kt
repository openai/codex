package anstyle.parse.tests

import anstyle.parse.Params
import anstyle.parse.Parser
import anstyle.parse.Perform
import anstyle.parse.AsciiParser
import kotlin.test.Test
import kotlin.test.assertEquals

// Rust original preserved as comments for reference
// use std::vec::Vec;
// use proptest::prelude::*;
// use anstyle_parse::*;

private const val MAX_PARAMS: Int = 32
private const val MAX_OSC_RAW: Int = 1024
private const val MAX_OSC_PARAMS: Int = 16

private val OSC_BYTES: UByteArray = ubyteArrayOf(
    0x1bu, 0x5du, // Begin OSC
    0x32u, 0x3bu, 0x6au, 0x77u, 0x69u, 0x6cu, 0x6du, 0x40u, 0x6au, 0x77u, 0x69u, 0x6cu, 0x6du, 0x2du, 0x64u, 0x65u,
    0x73u, 0x6bu, 0x3au, 0x20u, 0x7eu, 0x2fu, 0x63u, 0x6fu, 0x64u, 0x65u, 0x2fu, 0x61u, 0x6cu, 0x61u, 0x63u, 0x72u,
    0x69u, 0x74u, 0x74u, 0x79u, 0x07u // End OSC
)

/**
 * Sequence represents a parsed ANSI sequence event
 */
sealed class Sequence {
    data class Print(val c: Char) : Sequence()
    data class Osc(val params: List<ByteArray>, val bellTerminated: Boolean) : Sequence() {
        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is Osc) return false
            if (bellTerminated != other.bellTerminated) return false
            if (params.size != other.params.size) return false
            return params.zip(other.params).all { (a, b) -> a.contentEquals(b) }
        }
        override fun hashCode(): Int = params.fold(bellTerminated.hashCode()) { acc, arr -> 31 * acc + arr.contentHashCode() }
    }
    data class Csi(val params: List<List<UShort>>, val intermediates: List<UByte>, val ignore: Boolean, val action: UByte) : Sequence()
    data class Esc(val intermediates: List<UByte>, val ignore: Boolean, val byte: UByte) : Sequence()
    data class DcsHook(val params: List<List<UShort>>, val intermediates: List<UByte>, val ignore: Boolean, val action: UByte) : Sequence()
    data class DcsPut(val byte: UByte) : Sequence()
    data object DcsUnhook : Sequence()
}

/**
 * Dispatcher collects parsed sequences for testing
 */
class Dispatcher : Perform {
    val dispatched: MutableList<Sequence> = mutableListOf()

    override fun print(c: Char) {
        dispatched.add(Sequence.Print(c))
    }

    override fun oscDispatch(params: Array<ByteArray>, bellTerminated: Boolean) {
        dispatched.add(Sequence.Osc(params.toList(), bellTerminated))
    }

    override fun csiDispatch(params: Params, intermediates: UByteArray, ignore: Boolean, action: UByte) {
        val paramList = mutableListOf<List<UShort>>()
        for (subparams in params) {
            paramList.add(subparams.toList())
        }
        dispatched.add(Sequence.Csi(paramList, intermediates.toList(), ignore, action))
    }

    override fun escDispatch(intermediates: UByteArray, ignore: Boolean, byte: UByte) {
        dispatched.add(Sequence.Esc(intermediates.toList(), ignore, byte))
    }

    override fun hook(params: Params, intermediates: UByteArray, ignore: Boolean, action: UByte) {
        val paramList = mutableListOf<List<UShort>>()
        for (subparams in params) {
            paramList.add(subparams.toList())
        }
        dispatched.add(Sequence.DcsHook(paramList, intermediates.toList(), ignore, action))
    }

    override fun put(byte: UByte) {
        dispatched.add(Sequence.DcsPut(byte))
    }

    override fun unhook() {
        dispatched.add(Sequence.DcsUnhook)
    }

    operator fun plus(seq: Sequence): Dispatcher {
        dispatched.add(seq)
        return this
    }

    operator fun plus(other: Dispatcher): Dispatcher {
        dispatched.addAll(other.dispatched)
        return this
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is Dispatcher) return false
        return dispatched == other.dispatched
    }

    override fun hashCode(): Int = dispatched.hashCode()
}

private fun start(): Dispatcher = Dispatcher()

private fun runTest(input: UByteArray, expected: Dispatcher) {
    val dispatcher = Dispatcher()
    val parser = Parser<AsciiParser>()

    for (byte in input) {
        parser.advance(dispatcher, byte)
    }

    assertEquals(expected.dispatched, dispatcher.dispatched)
}

class TestSuite {
    @Test
    fun advanceOsc() {
        val input = OSC_BYTES
        val expected = start() + Sequence.Osc(
            listOf(
                OSC_BYTES.sliceArray(2..2).toByteArray(),
                OSC_BYTES.sliceArray(4 until (OSC_BYTES.size - 1)).toByteArray()
            ),
            bellTerminated = true
        )
        runTest(input, expected)
    }

    @Test
    fun advanceEmptyOsc() {
        val input = ubyteArrayOf(0x1bu, 0x5du, 0x07u)
        val expected = start() + Sequence.Osc(listOf(byteArrayOf()), bellTerminated = true)
        runTest(input, expected)
    }

    @Test
    fun advanceOscMaxParams() {
        val params = ";".repeat(MAX_PARAMS + 1)
        val input = "\u001b]${params}\u001b".encodeToByteArray().toUByteArray()
        val expected = start() + Sequence.Osc(List(MAX_OSC_PARAMS) { byteArrayOf() }, bellTerminated = false)
        runTest(input, expected)
    }

    @Test
    fun advanceOscBellTerminated() {
        val input = "\u001b]11;ff/00/ff\u0007".encodeToByteArray().toUByteArray()
        val expected = start() + Sequence.Osc(
            listOf(
                "11".encodeToByteArray(),
                "ff/00/ff".encodeToByteArray()
            ),
            bellTerminated = true
        )
        runTest(input, expected)
    }

    @Test
    fun advanceOscC0StTerminated() {
        val input = "\u001b]11;ff/00/ff\u001b\\".encodeToByteArray().toUByteArray()
        val expected = start() +
            Sequence.Osc(
                listOf(
                    "11".encodeToByteArray(),
                    "ff/00/ff".encodeToByteArray()
                ),
                bellTerminated = false
            ) +
            Sequence.Esc(emptyList(), ignore = false, byte = 92u)
        runTest(input, expected)
    }

    @Test
    fun advanceOscWithUtf8Arguments() {
        val input = ubyteArrayOf(
            0x0du, 0x1bu, 0x5du, 0x32u, 0x3bu, 0x65u, 0x63u, 0x68u, 0x6fu, 0x20u, 0x27u, 0xc2u, 0xafu, 0x5cu, 0x5fu,
            0x28u, 0xe3u, 0x83u, 0x84u, 0x29u, 0x5fu, 0x2fu, 0xc2u, 0xafu, 0x27u, 0x20u, 0x26u, 0x26u, 0x20u, 0x73u,
            0x6cu, 0x65u, 0x65u, 0x70u, 0x20u, 0x31u, 0x07u
        )
        val expected = start() + Sequence.Osc(
            listOf(
                byteArrayOf('2'.code.toByte()),
                input.sliceArray(5 until (input.size - 1)).toByteArray()
            ),
            bellTerminated = true
        )
        runTest(input, expected)
    }

    @Test
    fun advanceOscContainingStringTerminator() {
        val input = ubyteArrayOf(0x1bu, 0x5du, 0x32u, 0x3bu, 0xe6u, 0x9cu, 0xabu, 0x1bu, 0x5cu)
        val expected = start() +
            Sequence.Osc(
                listOf(
                    byteArrayOf('2'.code.toByte()),
                    input.sliceArray(4 until (input.size - 2)).toByteArray()
                ),
                bellTerminated = false
            ) +
            Sequence.Esc(emptyList(), ignore = false, byte = 92u)
        runTest(input, expected)
    }

    // Skip advance_exceed_max_buffer_size - depends on feature flag behavior

    @Test
    fun advanceCsiMaxParams() {
        // This will build a list of repeating '1;'s
        // The length is MAX_PARAMS - 1 because the last semicolon is interpreted
        // as an implicit zero, making the total number of parameters MAX_PARAMS
        val params = "1;".repeat(MAX_PARAMS - 1)
        val input = "\u001b[${params}p".encodeToByteArray().toUByteArray()
        val expectedParams = MutableList(MAX_PARAMS - 1) { listOf(1u.toUShort()) }
        expectedParams.add(listOf(0u.toUShort()))
        val expected = start() + Sequence.Csi(expectedParams, emptyList(), ignore = false, action = 'p'.code.toUByte())
        runTest(input, expected)
    }

    @Test
    fun advanceCsiParamsIgnoreLongParams() {
        // This will build a list of repeating '1;'s
        // The length is MAX_PARAMS because the last semicolon is interpreted
        // as an implicit zero, making the total number of parameters MAX_PARAMS + 1
        val params = "1;".repeat(MAX_PARAMS)
        val input = "\u001b[${params}p".encodeToByteArray().toUByteArray()
        val expectedParams = List(MAX_PARAMS) { listOf(1u.toUShort()) }
        val expected = start() + Sequence.Csi(expectedParams, emptyList(), ignore = true, action = 'p'.code.toUByte())
        runTest(input, expected)
    }

    @Test
    fun advanceCsiParamsTrailingSemicolon() {
        val input = "\u001b[4;m".encodeToByteArray().toUByteArray()
        val expected = start() + Sequence.Csi(
            listOf(listOf(4u.toUShort()), listOf(0u.toUShort())),
            emptyList(),
            ignore = false,
            action = 'm'.code.toUByte()
        )
        runTest(input, expected)
    }

    @Test
    fun advanceCsiParamsLeadingSemicolon() {
        val input = "\u001b[;4m".encodeToByteArray().toUByteArray()
        val expected = start() + Sequence.Csi(
            listOf(listOf(0u.toUShort()), listOf(4u.toUShort())),
            emptyList(),
            ignore = false,
            action = 'm'.code.toUByte()
        )
        runTest(input, expected)
    }

    @Test
    fun advanceCsiLongParam() {
        // The important part is the parameter, which is (i64::MAX + 1)
        val input = "\u001b[9223372036854775808m".encodeToByteArray().toUByteArray()
        val expected = start() + Sequence.Csi(
            listOf(listOf(UShort.MAX_VALUE)),
            emptyList(),
            ignore = false,
            action = 'm'.code.toUByte()
        )
        runTest(input, expected)
    }

    @Test
    fun advanceCsiReset() {
        val input = "\u001b[3;1\u001b[?1049h".encodeToByteArray().toUByteArray()
        val expected = start() + Sequence.Csi(
            listOf(listOf(1049u.toUShort())),
            listOf('?'.code.toUByte()),
            ignore = false,
            action = 'h'.code.toUByte()
        )
        runTest(input, expected)
    }

    @Test
    fun advanceCsiSubparameters() {
        val input = "\u001b[38:2:255:0:255;1m".encodeToByteArray().toUByteArray()
        val expected = start() + Sequence.Csi(
            listOf(
                listOf(38u.toUShort(), 2u.toUShort(), 255u.toUShort(), 0u.toUShort(), 255u.toUShort()),
                listOf(1u.toUShort())
            ),
            emptyList(),
            ignore = false,
            action = 'm'.code.toUByte()
        )
        runTest(input, expected)
    }

    @Test
    fun advanceDcsMaxParams() {
        val params = "1;".repeat(MAX_PARAMS + 1)
        val input = "\u001bP${params}p".encodeToByteArray().toUByteArray()
        val expectedParams = List(MAX_PARAMS) { listOf(1u.toUShort()) }
        val expected = start() + Sequence.DcsHook(expectedParams, emptyList(), ignore = true, action = 'p'.code.toUByte())
        runTest(input, expected)
    }

    @Test
    fun advanceDcsReset() {
        val input = ubyteArrayOf(
            0x1bu, '['.code.toUByte(), '3'.code.toUByte(), ';'.code.toUByte(), '1'.code.toUByte(),
            0x1bu, 'P'.code.toUByte(), '1'.code.toUByte(), '$'.code.toUByte(), 't'.code.toUByte(),
            'x'.code.toUByte(), 0x9cu
        )
        val expected = start() +
            Sequence.DcsHook(listOf(listOf(1u.toUShort())), listOf(36u), ignore = false, action = 't'.code.toUByte()) +
            Sequence.DcsPut('x'.code.toUByte()) +
            Sequence.DcsUnhook
        runTest(input, expected)
    }

    @Test
    fun advanceDcs() {
        val input = ubyteArrayOf(
            0x1bu, 'P'.code.toUByte(), '0'.code.toUByte(), ';'.code.toUByte(), '1'.code.toUByte(),
            '|'.code.toUByte(), '1'.code.toUByte(), '7'.code.toUByte(), '/'.code.toUByte(),
            'a'.code.toUByte(), 'b'.code.toUByte(), 0x9cu
        )
        val expected = start() +
            Sequence.DcsHook(listOf(listOf(0u.toUShort()), listOf(1u.toUShort())), emptyList(), ignore = false, action = '|'.code.toUByte()) +
            Sequence.DcsPut('1'.code.toUByte()) +
            Sequence.DcsPut('7'.code.toUByte()) +
            Sequence.DcsPut('/'.code.toUByte()) +
            Sequence.DcsPut('a'.code.toUByte()) +
            Sequence.DcsPut('b'.code.toUByte()) +
            Sequence.DcsUnhook
        runTest(input, expected)
    }

    @Test
    fun advanceIntermediateResetOnDcsExit() {
        val input = ubyteArrayOf(
            0x1bu, 'P'.code.toUByte(), '='.code.toUByte(), '1'.code.toUByte(), 's'.code.toUByte(),
            'Z'.code.toUByte(), 'Z'.code.toUByte(), 'Z'.code.toUByte(),
            0x1bu, '+'.code.toUByte(), '\\'.code.toUByte()
        )
        val expected = start() +
            Sequence.DcsHook(listOf(listOf(1u.toUShort())), listOf(61u), ignore = false, action = 's'.code.toUByte()) +
            Sequence.DcsPut('Z'.code.toUByte()) +
            Sequence.DcsPut('Z'.code.toUByte()) +
            Sequence.DcsPut('Z'.code.toUByte()) +
            Sequence.DcsUnhook +
            Sequence.Esc(listOf('+'.code.toUByte()), ignore = false, byte = '\\'.code.toUByte())
        runTest(input, expected)
    }

    @Test
    fun advanceEscReset() {
        val input = "\u001b[3;1\u001b(A".encodeToByteArray().toUByteArray()
        val expected = start() + Sequence.Esc(listOf('('.code.toUByte()), ignore = false, byte = 'A'.code.toUByte())
        runTest(input, expected)
    }

    @Test
    fun advanceParamsBufferFilledWithSubparam() {
        val input = "\u001b[::::::::::::::::::::::::::::::::x\u001b".encodeToByteArray().toUByteArray()
        val expected = start() + Sequence.Csi(
            listOf(List(32) { 0u.toUShort() }),
            emptyList(),
            ignore = true,
            action = 'x'.code.toUByte()
        )
        runTest(input, expected)
    }

    // Note: proptest UTF-8 test omitted - would require property-based testing library
}

// Helper extension
private fun UByteArray.toByteArray(): ByteArray = ByteArray(size) { this[it].toByte() }
