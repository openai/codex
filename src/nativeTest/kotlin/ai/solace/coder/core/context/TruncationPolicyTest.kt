package ai.solace.coder.core.context

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class TruncationPolicyTest {

    @Test
    fun testBytePolicyBudget() {
        val policy = TruncationPolicy.Bytes(1000)
        assertEquals(1000, policy.byteBudget())
        assertEquals(250, policy.tokenBudget()) // 1000 / 4
    }

    @Test
    fun testTokenPolicyBudget() {
        val policy = TruncationPolicy.Tokens(500)
        assertEquals(500, policy.tokenBudget())
        assertEquals(2000, policy.byteBudget()) // 500 * 4
    }

    @Test
    fun testPolicyMultiplier() {
        val policy = TruncationPolicy.Bytes(100)
        val scaled = policy.mul(1.5)
        assertTrue(scaled is TruncationPolicy.Bytes)
        assertEquals(151, (scaled as TruncationPolicy.Bytes).bytes) // 100 * 1.5 + 1
    }

    @Test
    fun testApproxTokenCount() {
        assertEquals(3, TruncationPolicy.approxTokenCount("hello world!")) // 12 chars / 4
        assertEquals(0, TruncationPolicy.approxTokenCount(""))
        assertEquals(1, TruncationPolicy.approxTokenCount("hi")) // 2 chars -> ceil(2/4) = 1
    }

    @Test
    fun testTruncateTextNoTruncationNeeded() {
        val content = "Short text"
        val policy = TruncationPolicy.Bytes(100)
        val result = truncateText(content, policy)
        assertEquals(content, result)
    }

    @Test
    fun testTruncateTextWithTruncation() {
        val content = "A".repeat(100)
        val policy = TruncationPolicy.Bytes(20)
        val result = truncateText(content, policy)
        assertTrue(result.length < content.length)
        assertTrue(result.contains("truncated"))
    }

    @Test
    fun testTruncateEmptyString() {
        val result = truncateText("", TruncationPolicy.Bytes(10))
        assertEquals("", result)
    }
}
