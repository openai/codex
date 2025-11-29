package ai.solace.coder.utils.readiness

import kotlinx.coroutines.async
import kotlinx.coroutines.runBlocking
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertTrue

class ReadinessFlagTest {

    @Test
    fun testSubscribeAndMarkReadyRoundtrip() = runBlocking {
        val flag = ReadinessFlag.new()
        val tokenResult = flag.subscribe()

        assertTrue(tokenResult.isSuccess)
        val token = tokenResult.getOrThrow()

        val markResult = flag.markReady(token)
        assertTrue(markResult.isSuccess)
        assertTrue(markResult.getOrThrow())
        assertTrue(flag.isReady())
    }

    @Test
    fun testSubscribeAfterReadyReturnsError(): Unit = runBlocking {
        val flag = ReadinessFlag.new()
        val token = flag.subscribe().getOrThrow()
        flag.markReady(token)

        val result = flag.subscribe()
        assertTrue(result.isFailure)
        assertIs<ReadinessError.FlagAlreadyReady>(result.exceptionOrNull())
    }

    @Test
    fun testMarkReadyRejectsUnknownToken() = runBlocking {
        val flag = ReadinessFlag.new()

        // Try to mark ready with an unknown token
        val result = flag.markReady(ReadinessToken(42))
        assertTrue(result.isSuccess)
        assertFalse(result.getOrThrow())

        // Flag should now be ready because there are no subscribers
        assertTrue(flag.isReady())
    }

    @Test
    fun testWaitReadyUnblocksAfterMarkReady() = runBlocking {
        val flag = ReadinessFlag.new()
        val token = flag.subscribe().getOrThrow()

        // Start waiting in a coroutine
        val waiter = async {
            flag.waitReady()
            true
        }

        // Mark ready
        flag.markReady(token)

        // Waiter should complete
        assertTrue(waiter.await())
    }

    @Test
    fun testMarkReadyTwiceUsesSingleToken() = runBlocking {
        val flag = ReadinessFlag.new()
        val token = flag.subscribe().getOrThrow()

        val firstMark = flag.markReady(token)
        assertTrue(firstMark.isSuccess)
        assertTrue(firstMark.getOrThrow())

        // Second mark should return false (already ready)
        val secondMark = flag.markReady(token)
        assertTrue(secondMark.isSuccess)
        assertFalse(secondMark.getOrThrow())
    }

    @Test
    fun testIsReadyWithoutSubscribersMarksReady(): Unit = runBlocking {
        val flag = ReadinessFlag.new()

        // Without any subscribers, isReady() should mark the flag ready
        assertTrue(flag.isReady())
        assertTrue(flag.isReady())

        // Can't subscribe after ready
        val result = flag.subscribe()
        assertTrue(result.isFailure)
        assertIs<ReadinessError.FlagAlreadyReady>(result.exceptionOrNull())
    }

    @Test
    fun testTokenIdZeroNeverAuthorizes() = runBlocking {
        val flag = ReadinessFlag.new()

        val result = flag.markReady(ReadinessToken(0))
        assertTrue(result.isSuccess)
        assertFalse(result.getOrThrow())
    }

    @Test
    fun testSubscribeMultiple() = runBlocking {
        val flag = ReadinessFlag.new()

        val result = flag.subscribeMultiple(3)
        assertTrue(result.isSuccess)

        val tokens = result.getOrThrow()
        assertEquals(3, tokens.size)

        // Mark all tokens ready
        for (token in tokens) {
            flag.markReady(token)
        }

        assertTrue(flag.isReady())
    }

    @Test
    fun testSubscribeMultipleAfterReadyFails(): Unit = runBlocking {
        val flag = ReadinessFlag.new()

        // Mark ready by having no subscribers
        assertTrue(flag.isReady())

        val result = flag.subscribeMultiple(2)
        assertTrue(result.isFailure)
        assertIs<ReadinessError.FlagAlreadyReady>(result.exceptionOrNull())
    }
}

class ReadinessTokenTest {

    @Test
    fun testTokenEquality() {
        val token1 = ReadinessToken(1)
        val token2 = ReadinessToken(1)
        val token3 = ReadinessToken(2)

        assertEquals(token1, token2)
        assertTrue(token1 != token3)
    }

    @Test
    fun testTokenId() {
        val token = ReadinessToken(42)
        assertEquals(42, token.id)
    }
}

class ReadinessErrorTest {

    @Test
    fun testTokenLockFailedMessage() {
        val error = ReadinessError.TokenLockFailed
        assertEquals("Failed to acquire readiness token lock", error.message)
    }

    @Test
    fun testFlagAlreadyReadyMessage() {
        val error = ReadinessError.FlagAlreadyReady
        assertEquals("Flag is already ready. Impossible to subscribe", error.message)
    }
}
