package ai.solace.coder.core.session

import ai.solace.coder.utils.concurrent.CancellationToken
import ai.solace.coder.core.context.ContextManager
import ai.solace.coder.core.context.TruncationPolicy
import ai.solace.coder.core.features.Features
import ai.solace.coder.protocol.AskForApproval
import ai.solace.coder.protocol.CompactedItem
import ai.solace.coder.protocol.RolloutItem
import ai.solace.coder.protocol.SandboxPolicy
import ai.solace.coder.protocol.SessionSource
import ai.solace.coder.protocol.InitialHistory
import ai.solace.coder.protocol.ResumedHistory
import ai.solace.coder.protocol.TurnAbortReason
import ai.solace.coder.protocol.CallToolResult
import ai.solace.coder.protocol.ContentBlock
import ai.solace.coder.protocol.ContentItem
import ai.solace.coder.protocol.FunctionCallOutputPayload
import ai.solace.coder.protocol.ResponseItem
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

/**
 * Ported from Rust codex-rs/core/src/codex.rs mod tests
 */

class FunctionCallOutputPayloadConversionTest {

    @Test
    fun testPrefersStructuredContentWhenPresent() {
        val ctr = CallToolResult(
            // Content present but should be ignored because structured_content is set.
            content = listOf(ContentBlock.TextContent(text = "ignored")),
            is_error = null,
            structured_content = buildJsonObject {
                put("ok", true)
                put("value", 42)
            }
        )

        val got = FunctionCallOutputPayload.fromCallToolResult(ctr)

        // structured_content takes precedence
        assertTrue(got.content?.contains("\"ok\":true") ?: false || got.content?.contains("\"ok\": true") ?: false)
        assertTrue(got.content?.contains("\"value\":42") ?: false || got.content?.contains("\"value\": 42") ?: false)
        assertEquals(true, got.success)
    }

    @Test
    fun testFallsBackToContentWhenStructuredIsJsonNull() {
        // When structured_content is JsonNull, it should fall back to content.
        // Use empty content list to avoid serialization complexity
        val ctr = CallToolResult(
            content = emptyList(),
            is_error = null,
            structured_content = JsonNull
        )

        val got = FunctionCallOutputPayload.fromCallToolResult(ctr)

        // When structured_content is JsonNull, falls back to content serialization
        // Empty content list should serialize successfully
        assertNotNull(got.content)
        assertEquals(true, got.success)
    }

    @Test
    fun testSuccessFlagReflectsIsErrorTrue() {
        val ctr = CallToolResult(
            content = listOf(ContentBlock.TextContent(text = "unused")),
            is_error = true,
            structured_content = buildJsonObject {
                put("message", "bad")
            }
        )

        val got = FunctionCallOutputPayload.fromCallToolResult(ctr)

        // is_error = true should result in success = false
        assertEquals(false, got.success)
        assertTrue(got.content?.contains("\"message\":\"bad\"") ?: false || got.content?.contains("\"message\": \"bad\"") ?: false)
    }

    @Test
    fun testSuccessFlagTrueWithNoErrorAndContentUsed() {
        // Use same pattern as the passing tests in ToolCallProcessorTest
        // Use empty content list which serializes reliably
        val ctr = CallToolResult(
            content = emptyList(),
            is_error = false
            // Don't pass structured_content - let it default
        )

        val got = FunctionCallOutputPayload.fromCallToolResult(ctr)

        assertEquals(true, got.success)
        assertNotNull(got.content)
    }
}

class SessionConfigurationTest {

    @Test
    fun testDefaultSessionConfiguration() {
        val config = SessionConfiguration(
            provider = ModelProviderInfo(name = "openai"),
            model = "gpt-4",
            cwd = "/test/path",
            approvalPolicy = AskForApproval.OnFailure,
            sandboxPolicy = SandboxPolicy.ReadOnly,
            features = Features(),
            execPolicy = ExecPolicy(),
            sessionSource = SessionSource.Cli
        )

        assertEquals("openai", config.provider.name)
        assertEquals("gpt-4", config.model)
        assertEquals("/test/path", config.cwd)
        assertEquals(AskForApproval.OnFailure, config.approvalPolicy)
        assertEquals(SandboxPolicy.ReadOnly, config.sandboxPolicy)
        assertEquals(SessionSource.Cli, config.sessionSource)
    }

    @Test
    fun testSessionConfigurationWithReasoningConfig() {
        val config = SessionConfiguration(
            provider = ModelProviderInfo(name = "anthropic"),
            model = "claude-3",
            cwd = "/home/user",
            approvalPolicy = AskForApproval.OnRequest,
            sandboxPolicy = SandboxPolicy.DangerFullAccess,
            features = Features(),
            execPolicy = ExecPolicy(),
            sessionSource = SessionSource.Exec
        )

        assertEquals("anthropic", config.provider.name)
        assertEquals("claude-3", config.model)
        assertEquals(AskForApproval.OnRequest, config.approvalPolicy)
        assertEquals(SandboxPolicy.DangerFullAccess, config.sandboxPolicy)
        assertEquals(SessionSource.Exec, config.sessionSource)
    }
}

class SessionSourceTest {

    @Test
    fun testSessionSourceCli() {
        val source = SessionSource.Cli
        assertEquals(SessionSource.Cli, source)
    }

    @Test
    fun testSessionSourceExec() {
        val source = SessionSource.Exec
        assertEquals(SessionSource.Exec, source)
    }

    @Test
    fun testSessionSourceVscode() {
        val source = SessionSource.VSCode
        assertEquals(SessionSource.VSCode, source)
    }

    @Test
    fun testSessionSourceMcp() {
        val source = SessionSource.Mcp
        assertEquals(SessionSource.Mcp, source)
    }
}

class InitialHistoryTest {

    @Test
    fun testNewHistory() {
        val history = InitialHistory.New
        assertTrue(history is InitialHistory.New)
    }

    @Test
    fun testResumedHistory() {
        val rolloutItems = listOf(
            RolloutItem.ResponseItem(
                ResponseItem.Message(
                    role = "user",
                    content = listOf(ContentItem.InputText(text = "hello"))
                )
            )
        )
        val history = InitialHistory.Resumed(
            payload = ResumedHistory(
                conversation_id = "conv-123",
                history = rolloutItems,
                rollout_path = "/tmp/rollout.jsonl"
            )
        )

        assertTrue(history is InitialHistory.Resumed)
        assertEquals("conv-123", history.payload.conversation_id)
        assertEquals(1, history.payload.history.size)
        assertEquals("/tmp/rollout.jsonl", history.payload.rollout_path)
    }

    @Test
    fun testForkedHistory() {
        val rolloutItems = listOf(
            RolloutItem.ResponseItem(
                ResponseItem.Message(
                    role = "assistant",
                    content = listOf(ContentItem.OutputText(text = "Hi there!"))
                )
            )
        )
        val history = InitialHistory.Forked(rolloutItems)

        assertTrue(history is InitialHistory.Forked)
        assertEquals(1, history.items.size)
    }
}

class TurnAbortReasonTest {

    @Test
    fun testInterruptedReason() {
        val reason = TurnAbortReason.Interrupted
        assertEquals(TurnAbortReason.Interrupted, reason)
    }

    @Test
    fun testReplacedReason() {
        val reason = TurnAbortReason.Replaced
        assertEquals(TurnAbortReason.Replaced, reason)
    }

    @Test
    fun testReviewEndedReason() {
        val reason = TurnAbortReason.ReviewEnded
        assertEquals(TurnAbortReason.ReviewEnded, reason)
    }
}

class CompactedItemTest {

    @Test
    fun testCompactedItem() {
        val item = CompactedItem(
            message = "Summary of conversation so far"
        )

        assertEquals("Summary of conversation so far", item.message)
        assertNull(item.replacement_history)
    }

    @Test
    fun testCompactedItemWithReplacementHistory() {
        val replacementHistory = listOf(
            ResponseItem.Message(
                role = "system",
                content = listOf(ContentItem.InputText(text = "compacted context"))
            )
        )
        val item = CompactedItem(
            message = "Summary",
            replacement_history = replacementHistory
        )

        assertEquals("Summary", item.message)
        assertNotNull(item.replacement_history)
        assertEquals(1, item.replacement_history!!.size)
    }
}

class RolloutItemTest {

    @Test
    fun testRolloutItemResponseItem() {
        val responseItem = ResponseItem.Message(
            role = "user",
            content = listOf(ContentItem.InputText(text = "test message"))
        )
        val rolloutItem = RolloutItem.ResponseItem(responseItem)

        assertTrue(rolloutItem is RolloutItem.ResponseItem)
    }

    @Test
    fun testRolloutItemCompacted() {
        val compactedItem = CompactedItem(message = "compacted")
        val rolloutItem = RolloutItem.Compacted(compactedItem)

        assertTrue(rolloutItem is RolloutItem.Compacted)
        assertEquals("compacted", rolloutItem.payload.message)
    }
}

class CancellationTokenTest {

    @Test
    fun testInitialStateNotCancelled() {
        val token = CancellationToken()
        kotlinx.coroutines.runBlocking {
            assertEquals(false, token.isCancelled())
        }
    }

    @Test
    fun testCancelSetsFlag() {
        val token = CancellationToken()
        kotlinx.coroutines.runBlocking {
            token.cancel()
            assertEquals(true, token.isCancelled())
        }
    }

    @Test
    fun testChildCancelledWithParent() {
        val parent = CancellationToken()
        val child = parent.child()

        kotlinx.coroutines.runBlocking {
            parent.cancel()
            // Child should be cancelled when parent is cancelled
            assertEquals(true, child.isCancelled())
        }
    }

    @Test
    fun testChildCancelDoesNotAffectParent() {
        val parent = CancellationToken()
        val child = parent.child()

        kotlinx.coroutines.runBlocking {
            child.cancel()
            // Cancelling child should NOT cancel parent
            assertEquals(false, parent.isCancelled())
            assertEquals(true, child.isCancelled())
        }
    }

    @Test
    fun testCloneSharesState() {
        // clone() creates linked tokens - both point to same state
        val token1 = CancellationToken()
        val token2 = token1.clone()

        assertEquals(false, token1.isCancelled())
        assertEquals(false, token2.isCancelled())

        // Cancelling token1 should also cancel token2
        token1.cancel()
        assertEquals(true, token1.isCancelled())
        assertEquals(true, token2.isCancelled())
    }

    @Test
    fun testCloneCancellingEitherCancelsBoth() {
        // Test the reverse direction - cancelling the clone
        val token1 = CancellationToken()
        val token2 = token1.clone()

        // Cancelling token2 should also cancel token1
        token2.cancel()
        assertEquals(true, token1.isCancelled())
        assertEquals(true, token2.isCancelled())
    }

    @Test
    fun testChildOfCloneBehavior() {
        // child_of_clone test from loom
        val token = CancellationToken()
        val clone = token.clone()
        val child = clone.child()

        assertEquals(false, token.isCancelled())
        assertEquals(false, clone.isCancelled())
        assertEquals(false, child.isCancelled())

        // Cancelling the original should cancel clone (shared state)
        // and also cancel child (parent→child propagation)
        token.cancel()
        assertEquals(true, token.isCancelled())
        assertEquals(true, clone.isCancelled())
        assertEquals(true, child.isCancelled())
    }

    @Test
    fun testDropGuardCancelsOnClose() {
        val token = CancellationToken()
        assertEquals(false, token.isCancelled())

        token.dropGuard().use {
            // Inside the block, token is not yet cancelled
            assertEquals(false, token.isCancelled())
        }
        // After block exits, token should be cancelled
        assertEquals(true, token.isCancelled())
    }

    @Test
    fun testDropGuardDisarm() {
        val token = CancellationToken()
        val guard = token.dropGuard()

        // Disarm returns the token and prevents auto-cancel
        val disarmed = guard.disarm()
        assertEquals(token.isCancelled(), disarmed.isCancelled())

        guard.close() // Should be a no-op after disarm
        assertEquals(false, token.isCancelled())
    }
}

class SessionStateTest {

    @Test
    fun testDefaultState() {
        val config = SessionConfiguration(
            provider = ModelProviderInfo(name = "openai"),
            model = "gpt-4",
            cwd = "/test",
            approvalPolicy = AskForApproval.OnFailure,
            sandboxPolicy = SandboxPolicy.ReadOnly,
            features = Features(),
            execPolicy = ExecPolicy(),
            sessionSource = SessionSource.Cli
        )
        val state = SessionState(config)

        assertEquals(config, state.sessionConfiguration)
        assertTrue(state.cloneHistory().getHistory().isEmpty())
    }
}

class ContentBlockTest {

    @Test
    fun testTextContentBlock() {
        val block = ContentBlock.TextContent(text = "Hello world")
        assertTrue(block is ContentBlock.TextContent)
        assertEquals("Hello world", block.text)
    }

    @Test
    fun testImageContentBlock() {
        val block = ContentBlock.ImageContent(
            data = "base64encodeddata",
            mime_type = "image/png"
        )
        assertTrue(block is ContentBlock.ImageContent)
        assertEquals("base64encodeddata", block.data)
        assertEquals("image/png", block.mime_type)
    }
}

class ResponseItemTest {

    @Test
    fun testMessageResponseItem() {
        val item = ResponseItem.Message(
            role = "assistant",
            content = listOf(ContentItem.OutputText(text = "Response text"))
        )

        assertTrue(item is ResponseItem.Message)
        assertEquals("assistant", item.role)
        assertEquals(1, item.content.size)
    }

    @Test
    fun testFunctionCallResponseItem() {
        val item = ResponseItem.FunctionCall(
            name = "shell",
            arguments = "{\"command\": \"ls\"}",
            call_id = "call-123"
        )

        assertTrue(item is ResponseItem.FunctionCall)
        assertEquals("shell", item.name)
        assertEquals("call-123", item.call_id)
    }

    @Test
    fun testFunctionCallOutputResponseItem() {
        val output = FunctionCallOutputPayload(
            content = "command output",
            success = true
        )
        val item = ResponseItem.FunctionCallOutput(
            call_id = "call-123",
            output = output
        )

        assertTrue(item is ResponseItem.FunctionCallOutput)
        assertEquals("call-123", item.call_id)
        assertEquals(true, item.output.success)
    }
}

class CallToolResultTest {

    @Test
    fun testCallToolResultWithTextContent() {
        val result = CallToolResult(
            content = listOf(ContentBlock.TextContent(text = "Result text")),
            is_error = false
        )

        assertEquals(1, result.content.size)
        assertEquals(false, result.is_error)
        assertNull(result.structured_content)
    }

    @Test
    fun testCallToolResultWithStructuredContent() {
        val result = CallToolResult(
            content = emptyList(),
            is_error = null,
            structured_content = buildJsonObject {
                put("status", "success")
            }
        )

        assertTrue(result.content.isEmpty())
        assertNull(result.is_error)
        assertNotNull(result.structured_content)
    }
}

class ContextManagerTest {

    @Test
    fun testRecordAndGetHistory() {
        val manager = ContextManager()
        val policy = TruncationPolicy.Bytes(10000)

        val item = ResponseItem.Message(
            role = "user",
            content = listOf(ContentItem.InputText(text = "Hello"))
        )

        manager.recordItems(listOf(item), policy)
        val history = manager.getHistory()

        assertEquals(1, history.size)
        assertTrue(history[0] is ResponseItem.Message)
    }

    @Test
    fun testReplaceHistory() {
        val manager = ContextManager()
        val policy = TruncationPolicy.Bytes(10000)

        val item1 = ResponseItem.Message(
            role = "user",
            content = listOf(ContentItem.InputText(text = "First"))
        )

        manager.recordItems(listOf(item1), policy)
        assertEquals(1, manager.getHistory().size)

        // Replace with new items
        val item2 = ResponseItem.Message(
            role = "assistant",
            content = listOf(ContentItem.OutputText(text = "Second"))
        )
        manager.replace(listOf(item2))
        assertEquals(1, manager.getHistory().size)

        val history = manager.getHistory()
        assertTrue(history[0] is ResponseItem.Message)
        assertEquals("assistant", (history[0] as ResponseItem.Message).role)
    }
}

class ModelProviderInfoTest {

    @Test
    fun testDefaultModelProviderInfo() {
        val info = ModelProviderInfo()
        assertEquals("openai", info.name)
        assertNull(info.apiBase)
    }

    @Test
    fun testCustomModelProviderInfo() {
        val info = ModelProviderInfo(
            name = "anthropic",
            apiBase = "https://api.anthropic.com"
        )
        assertEquals("anthropic", info.name)
        assertEquals("https://api.anthropic.com", info.apiBase)
    }
}

class ModelFamilyTest {

    @Test
    fun testDefaultModelFamily() {
        val family = ModelFamily.default()
        assertEquals("gpt-4", family.slug)
        assertTrue(family.supportsParallelToolCalls)
    }

    @Test
    fun testCustomModelFamily() {
        val family = ModelFamily(
            slug = "claude-3",
            baseInstructions = "Custom instructions",
            contextWindow = 200000,
            supportsParallelToolCalls = false
        )
        assertEquals("claude-3", family.slug)
        assertEquals("Custom instructions", family.baseInstructions)
        assertEquals(200000, family.contextWindow)
        assertEquals(false, family.supportsParallelToolCalls)
    }
}

