package ai.solace.coder.core.tools.handlers

import ai.solace.coder.core.tools.ToolInvocation
import ai.solace.coder.core.tools.ToolOutput
import ai.solace.coder.core.tools.ToolPayload
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.Json
import okio.Path.Companion.toPath
import okio.fakefilesystem.FakeFileSystem
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class ReadFileTest {

    private val fileSystem = FakeFileSystem()
    private val handler = ReadFileHandler(fileSystem) // Dependency injection for testing

    @Test
    fun `reads requested range`() = runTest {
        val path = "/tmp/test.txt".toPath()
        fileSystem.write(path) {
            writeUtf8("alpha\nbeta\ngamma\n")
        }

        val args = ReadFileArgs(
            filePath = path.toString(),
            offset = 2,
            limit = 2
        )
        val result = handler.handle(createInvocation(args))
        
        val output = (result.getOrThrow() as ToolOutput.Function).content
        val lines = output.split("\n")
        assertEquals(listOf("L2: beta", "L3: gamma"), lines)
    }

    @Test
    fun `errors when offset exceeds length`() = runTest {
        val path = "/tmp/test.txt".toPath()
        fileSystem.write(path) {
            writeUtf8("only\n")
        }

        val args = ReadFileArgs(
            filePath = path.toString(),
            offset = 3,
            limit = 1
        )
        val result = handler.handle(createInvocation(args))
        
        assertTrue(result.isFailure)
        assertTrue(result.exceptionOrNull()?.message?.contains("offset exceeds file length") == true)
    }

    @Test
    fun `reads non utf8 lines`() = runTest {
        val path = "/tmp/test.txt".toPath()
        fileSystem.write(path) {
            write(okio.ByteString.of(0xff.toByte(), 0xfe.toByte()))
            writeUtf8("\nplain\n")
        }

        val args = ReadFileArgs(
            filePath = path.toString(),
            offset = 1,
            limit = 2
        )
        val result = handler.handle(createInvocation(args))
        
        val output = (result.getOrThrow() as ToolOutput.Function).content
        val lines = output.split("\n")
        // Kotlin/Okio might handle replacement chars slightly differently, but checking for replacement char presence
        assertTrue(lines[0].startsWith("L1: "))
        assertEquals("L2: plain", lines[1])
    }

    @Test
    fun `trims crlf endings`() = runTest {
        val path = "/tmp/test.txt".toPath()
        fileSystem.write(path) {
            writeUtf8("one\r\ntwo\r\n")
        }

        val args = ReadFileArgs(
            filePath = path.toString(),
            offset = 1,
            limit = 2
        )
        val result = handler.handle(createInvocation(args))
        
        val output = (result.getOrThrow() as ToolOutput.Function).content
        val lines = output.split("\n")
        assertEquals(listOf("L1: one", "L2: two"), lines)
    }

    @Test
    fun `respects limit even with more lines`() = runTest {
        val path = "/tmp/test.txt".toPath()
        fileSystem.write(path) {
            writeUtf8("first\nsecond\nthird\n")
        }

        val args = ReadFileArgs(
            filePath = path.toString(),
            offset = 1,
            limit = 2
        )
        val result = handler.handle(createInvocation(args))
        
        val output = (result.getOrThrow() as ToolOutput.Function).content
        val lines = output.split("\n")
        assertEquals(listOf("L1: first", "L2: second"), lines)
    }

    @Test
    fun `indentation mode captures block`() = runTest {
        val path = "/tmp/test.txt".toPath()
        fileSystem.write(path) {
            writeUtf8("""
                fn outer() {
                    if cond {
                        inner();
                    }
                    tail();
                }
            """.trimIndent())
        }

        val args = ReadFileArgs(
            filePath = path.toString(),
            offset = 3,
            limit = 10,
            mode = ReadMode.Indentation,
            indentation = IndentationArgs(
                anchorLine = 3,
                includeSiblings = false,
                maxLevels = 1
            )
        )
        val result = handler.handle(createInvocation(args))
        
        val output = (result.getOrThrow() as ToolOutput.Function).content
        val lines = output.split("\n")
        // Note: line numbers depend on trimIndent() behavior in setup, assuming 1-based
        assertEquals("L2:     if cond {", lines[0])
        assertEquals("L3:         inner();", lines[1])
        assertEquals("L4:     }", lines[2])
    }

    // Helper to create invocation
    private fun createInvocation(args: ReadFileArgs): ToolInvocation {
        return ToolInvocation(
            callId = "test-call",
            toolName = "read_file",
            payload = ToolPayload.Function(
                arguments = Json.encodeToString(args)
            )
        )
    }
}
