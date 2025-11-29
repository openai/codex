package ai.solace.coder.exec.sandbox

import ai.solace.coder.exec.process.SandboxType
import ai.solace.coder.protocol.models.SandboxPolicy
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class SandboxManagerTest {

    @Test
    fun testSelectInitialSandboxForbid() {
        val manager = SandboxManager()
        val sandbox = manager.selectInitialSandbox(
            SandboxPolicy.ReadOnly(),
            SandboxPreference.Forbid
        )
        assertEquals(SandboxType.None, sandbox)
    }

    @Test
    fun testSelectInitialSandboxDangerFullAccess() {
        val manager = SandboxManager()
        val sandbox = manager.selectInitialSandbox(
            SandboxPolicy.DangerFullAccess,
            SandboxPreference.Auto
        )
        assertEquals(SandboxType.None, sandbox)
    }
}

class ApprovalStoreTest {

    @Test
    fun testGetPutApproval() {
        val store = ApprovalStore()

        assertNull(store.get("key1"))

        store.put("key1", ReviewDecision.ApprovedForSession)
        assertEquals(ReviewDecision.ApprovedForSession, store.get("key1"))

        store.put("key1", ReviewDecision.Rejected)
        assertEquals(ReviewDecision.Rejected, store.get("key1"))
    }

    @Test
    fun testDifferentKeys() {
        val store = ApprovalStore()

        store.put("key1", ReviewDecision.Approved)
        store.put("key2", ReviewDecision.Rejected)

        assertEquals(ReviewDecision.Approved, store.get("key1"))
        assertEquals(ReviewDecision.Rejected, store.get("key2"))
    }
}

class ApprovalRequirementTest {

    @Test
    fun testSkipRequirement() {
        val req = ApprovalRequirement.Skip(bypassSandbox = true)
        assertTrue(req is ApprovalRequirement.Skip)
        assertTrue(req.bypassSandbox)
    }

    @Test
    fun testNeedsApprovalRequirement() {
        val req = ApprovalRequirement.NeedsApproval(reason = "Test reason")
        assertTrue(req is ApprovalRequirement.NeedsApproval)
        assertEquals("Test reason", req.reason)
    }

    @Test
    fun testForbiddenRequirement() {
        val req = ApprovalRequirement.Forbidden(reason = "Not allowed")
        assertTrue(req is ApprovalRequirement.Forbidden)
        assertEquals("Not allowed", req.reason)
    }
}

class SandboxCommandAssessmentTest {

    @Test
    fun testSafeAssessment() {
        val assessment = SandboxCommandAssessment(safe = true, reason = null)
        assertTrue(assessment.safe)
        assertNull(assessment.reason)
    }

    @Test
    fun testUnsafeAssessment() {
        val assessment = SandboxCommandAssessment(safe = false, reason = "May damage system")
        assertFalse(assessment.safe)
        assertEquals("May damage system", assessment.reason)
    }
}

class ToolErrorTest {

    @Test
    fun testRejectedError() {
        val error = ToolError.Rejected("Not permitted")
        assertTrue(error is ToolError.Rejected)
        assertEquals("Not permitted", (error as ToolError.Rejected).message)
    }
}

class SandboxRetryDataTest {

    @Test
    fun testSandboxRetryData() {
        val data = SandboxRetryData(
            command = listOf("ls", "-la"),
            cwd = "/home/user"
        )
        assertEquals(listOf("ls", "-la"), data.command)
        assertEquals("/home/user", data.cwd)
    }
}
