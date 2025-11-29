package ai.solace.coder.utils.readiness

import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withTimeout
import kotlin.time.Duration.Companion.milliseconds

/**
 * Opaque subscription token returned by `subscribe()`.
 */
data class ReadinessToken(val id: Int)

/**
 * Errors that can occur during readiness operations.
 */
sealed class ReadinessError : Exception() {
    data object TokenLockFailed : ReadinessError() {
        override val message: String get() = "Failed to acquire readiness token lock"
    }
    data object FlagAlreadyReady : ReadinessError() {
        override val message: String get() = "Flag is already ready. Impossible to subscribe"
    }
}

/**
 * Interface for readiness flag operations.
 */
interface Readiness {
    /**
     * Returns true if the flag is currently marked ready.
     * At least one token needs to be marked as ready before.
     * `true` is not reversible.
     */
    fun isReady(): Boolean

    /**
     * Subscribe to readiness and receive an authorization token.
     *
     * @return Success(Token) if subscription succeeded, or Failure if flag is already ready
     */
    suspend fun subscribe(): Result<ReadinessToken>

    /**
     * Attempt to mark the flag ready, validated by the provided token.
     *
     * @return Success(true) if successfully marked ready,
     *         Success(false) if token was invalid or flag already ready
     */
    suspend fun markReady(token: ReadinessToken): Result<Boolean>

    /**
     * Asynchronously wait until the flag becomes ready.
     */
    suspend fun waitReady()
}

private val LOCK_TIMEOUT = 1000.milliseconds

/**
 * Readiness flag with token-based authorization and async waiting.
 *
 * This is used to coordinate between background tasks (like ghost snapshots)
 * and the main execution flow. Tasks can subscribe to get a token, and the
 * flag is only considered ready when all tokens have been marked ready, or
 * when there are no subscribers.
 */
class ReadinessFlag : Readiness {
    // Ready state - thread safety provided by mutex synchronization
    private var ready: Boolean = false

    // Counter for generating unique token IDs
    private var nextId: Int = 1 // Reserve 0

    // Set of active subscriptions
    private val tokens = mutableSetOf<ReadinessToken>()
    private val tokensMutex = Mutex()

    // Deferred for async waiting
    private val readyDeferred = CompletableDeferred<Unit>()

    override fun isReady(): Boolean {
        if (ready) {
            return true
        }

        // If there are no tokens, mark as ready
        val noTokens = tokensMutex.tryLock()
        if (noTokens) {
            try {
                if (tokens.isEmpty()) {
                    if (!ready) {
                        ready = true
                        readyDeferred.complete(Unit)
                    }
                    return true
                }
            } finally {
                tokensMutex.unlock()
            }
        }

        return ready
    }

    override suspend fun subscribe(): Result<ReadinessToken> {
        if (ready) {
            return Result.failure(ReadinessError.FlagAlreadyReady)
        }

        return try {
            withTimeout(LOCK_TIMEOUT) {
                tokensMutex.withLock {
                    // Double-check readiness while holding the lock
                    if (ready) {
                        return@withTimeout Result.failure(ReadinessError.FlagAlreadyReady)
                    }

                    val token = ReadinessToken(nextId++)
                    tokens.add(token)
                    Result.success(token)
                }
            }
        } catch (e: kotlinx.coroutines.TimeoutCancellationException) {
            Result.failure(ReadinessError.TokenLockFailed)
        }
    }

    override suspend fun markReady(token: ReadinessToken): Result<Boolean> {
        if (ready) {
            return Result.success(false)
        }
        if (token.id == 0) {
            return Result.success(false) // Never authorize
        }

        return try {
            withTimeout(LOCK_TIMEOUT) {
                tokensMutex.withLock {
                    if (!tokens.remove(token)) {
                        return@withTimeout Result.success(false) // Invalid or already used
                    }

                    ready = true
                    tokens.clear() // No further tokens needed once ready
                    readyDeferred.complete(Unit)
                    Result.success(true)
                }
            }
        } catch (e: kotlinx.coroutines.TimeoutCancellationException) {
            Result.failure(ReadinessError.TokenLockFailed)
        }
    }

    override suspend fun waitReady() {
        if (isReady()) {
            return
        }
        readyDeferred.await()
    }

    /**
     * Creates a child token that can be used for nested readiness tracking.
     * For use in complex workflows where multiple sub-tasks need to complete.
     */
    suspend fun subscribeMultiple(count: Int): Result<List<ReadinessToken>> {
        if (ready) {
            return Result.failure(ReadinessError.FlagAlreadyReady)
        }

        return try {
            withTimeout(LOCK_TIMEOUT) {
                tokensMutex.withLock {
                    if (ready) {
                        return@withTimeout Result.failure(ReadinessError.FlagAlreadyReady)
                    }

                    val tokenList = (0 until count).map {
                        val token = ReadinessToken(nextId++)
                        tokens.add(token)
                        token
                    }
                    Result.success(tokenList)
                }
            }
        } catch (e: kotlinx.coroutines.TimeoutCancellationException) {
            Result.failure(ReadinessError.TokenLockFailed)
        }
    }

    companion object {
        /**
         * Creates a new, not-yet-ready flag.
         */
        fun new(): ReadinessFlag = ReadinessFlag()
    }
}
