package ai.solace.coder.core.session

import ai.solace.coder.protocol.AskForApproval
import ai.solace.coder.protocol.SandboxPolicy
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class TurnContextTest {

    private fun createTestContext(cwd: String = "/test/path"): TurnContext {
        return TurnContext(
            subId = "test_turn_1",
            cwd = cwd,
            approvalPolicy = AskForApproval.OnFailure,
            sandboxPolicy = SandboxPolicy.ReadOnly,
            model = "gpt-4",
            modelFamily = "gpt-4"
        )
    }

    @Test
    fun testResolveAbsolutePath() {
        val ctx = createTestContext("/home/user")

        // Absolute Unix path
        assertEquals("/absolute/path", ctx.resolvePath("/absolute/path"))

        // Absolute Windows path
        assertEquals("C:/Users/test", ctx.resolvePath("C:/Users/test"))
    }

    @Test
    fun testResolveRelativePath() {
        val ctx = createTestContext("/home/user")

        assertEquals("/home/user/relative", ctx.resolvePath("relative"))
        assertEquals("/home/user/sub/dir", ctx.resolvePath("sub/dir"))
    }

    @Test
    fun testResolveNullPath() {
        val ctx = createTestContext("/home/user")
        assertEquals("/home/user", ctx.resolvePath(null))
    }

    @Test
    fun testGetCompactPromptDefault() {
        val ctx = createTestContext()
        val prompt = ctx.getCompactPrompt()
        assertTrue(prompt.contains("Summarize"))
    }

    @Test
    fun testGetCompactPromptCustom() {
        val customPrompt = "Custom summarization prompt"
        val ctx = TurnContext(
            subId = "test",
            cwd = "/test",
            compactPrompt = customPrompt,
            approvalPolicy = AskForApproval.OnFailure,
            sandboxPolicy = SandboxPolicy.ReadOnly,
            model = "gpt-4",
            modelFamily = "gpt-4"
        )
        assertEquals(customPrompt, ctx.getCompactPrompt())
    }
}

class ShellEnvironmentPolicyTest {

    @Test
    fun testInheritAllDefault() {
        val policy = ShellEnvironmentPolicy.Inherit()
        assertTrue(policy is ShellEnvironmentPolicy.Inherit)
        assertEquals(ShellEnvironmentInheritFilter.All, policy.filter)
    }

    @Test
    fun testInheritCore() {
        val policy = ShellEnvironmentPolicy.Inherit(ShellEnvironmentInheritFilter.Core)
        assertEquals(ShellEnvironmentInheritFilter.Core, policy.filter)
    }

    @Test
    fun testSanitize() {
        val vars = mapOf("CUSTOM_VAR" to "value")
        val policy = ShellEnvironmentPolicy.Sanitize(vars)
        assertTrue(policy is ShellEnvironmentPolicy.Sanitize)
        assertEquals("value", policy.additionalVars["CUSTOM_VAR"])
    }
}

class ToolsConfigTest {

    @Test
    fun testDefaultToolsConfig() {
        val config = ToolsConfig()
        assertEquals(ShellToolType.Default, config.shellType)
        assertEquals(null, config.applyPatchToolType)
        assertEquals(false, config.webSearchRequest)
        assertEquals(true, config.includeViewImageTool)
        assertTrue(config.experimentalSupportedTools.isEmpty())
    }

    @Test
    fun testCustomToolsConfig() {
        val config = ToolsConfig(
            shellType = ShellToolType.UnifiedExec,
            applyPatchToolType = ApplyPatchToolType.Freeform,
            webSearchRequest = true,
            includeViewImageTool = false,
            experimentalSupportedTools = listOf("custom_tool")
        )
        assertEquals(ShellToolType.UnifiedExec, config.shellType)
        assertEquals(ApplyPatchToolType.Freeform, config.applyPatchToolType)
        assertTrue(config.webSearchRequest)
        assertEquals(false, config.includeViewImageTool)
        assertEquals(listOf("custom_tool"), config.experimentalSupportedTools)
    }
}

class ExecPolicyTest {

    @Test
    fun testDefaultExecPolicy() {
        val policy = ExecPolicy()
        assertTrue(policy.enabled)
        assertEquals(ExecPolicyAction.Ask, policy.defaultAction)
    }

    @Test
    fun testDisabledExecPolicy() {
        val policy = ExecPolicy(enabled = false, defaultAction = ExecPolicyAction.Allow)
        assertEquals(false, policy.enabled)
        assertEquals(ExecPolicyAction.Allow, policy.defaultAction)
    }
}
