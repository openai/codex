// port-lint: source async-utils/src/lib.rs
package ai.solace.coder.utils.concurrent

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

/**
 * A token which can be used to signal a cancellation request to one or more tasks.
 *
 * Tasks can call [cancelled] in order to obtain a suspend function which will
 * complete when cancellation is requested.
 *
 * Cancellation can be requested through the [cancel] method.
 *
 * Ported from Rust tokio_util::sync::CancellationToken
 * Source: https://github.com/tokio-rs/tokio/blob/master/tokio-util/src/sync/cancellation_token.rs
 *
 * Key semantics from Rust:
 * - Child tokens get cancelled when parent is cancelled
 * - Cancelling a child does NOT cancel the parent
 * - Clone creates a linked token (both cancel together) - use [child] for independent children
 */
class CancellationToken private constructor(
    private val node: TreeNode
) {
    /**
     * Creates a new [CancellationToken] in the non-cancelled state.
     */
    constructor() : this(TreeNode())

    /**
     * Creates a [CancellationToken] which will get cancelled whenever the
     * current token gets cancelled. Unlike a cloned [CancellationToken],
     * cancelling a child token does not cancel the parent token.
     *
     * If the current token is already cancelled, the child token will get
     * returned in cancelled state.
     */
    fun childToken(): CancellationToken {
        return CancellationToken(node.createChild())
    }

    /**
     * Alias for [childToken] matching some Rust usage patterns.
     */
    fun child(): CancellationToken = childToken()

    /**
     * Creates a clone of this [CancellationToken] that shares the same state.
     *
     * Both the original and the clone will be cancelled when either one is cancelled.
     * This is different from [childToken] where cancelling the child does NOT cancel the parent.
     *
     * Use [clone] when you need multiple handles to the same cancellation state.
     * Use [childToken] when you want hierarchical cancellation (parent→child only).
     */
    fun clone(): CancellationToken = CancellationToken(node)

    /**
     * Cancel the [CancellationToken] and all child tokens which had been
     * derived from it.
     *
     * This will wake up all tasks which are waiting for cancellation.
     */
    fun cancel() {
        node.cancel()
    }

    /**
     * Returns `true` if the [CancellationToken] is cancelled.
     */
    fun isCancelled(): Boolean = node.isCancelled()

    /**
     * Suspends until cancellation is requested.
     *
     * The function will complete immediately if the token is already cancelled
     * when this method is called.
     *
     * This is the Kotlin equivalent of Rust's `cancelled().await`.
     */
    suspend fun cancelled() {
        node.awaitCancellation()
    }

    /**
     * Creates a [DropGuard] for this token.
     *
     * The returned guard will cancel this token (and all its children) when
     * [DropGuard.close] is called, unless [DropGuard.disarm] is called first.
     *
     * Use with Kotlin's `use` extension for RAII-like behavior:
     * ```
     * token.dropGuard().use {
     *     // token will be cancelled when this block exits
     * }
     * ```
     */
    fun dropGuard(): DropGuard = DropGuard(this)

    companion object {
        /**
         * Creates a new [CancellationToken] in the non-cancelled state.
         */
        fun new(): CancellationToken = CancellationToken()
    }
}

/**
 * Internal tree node for managing parent-child cancellation relationships.
 *
 * Ported from Rust tokio_util::sync::cancellation_token::tree_node
 */
internal class TreeNode {
    private val state = MutableStateFlow(false)
    private val mutex = Mutex()
    private val children = mutableListOf<TreeNode>()
    @kotlin.concurrent.Volatile
    private var parent: TreeNode? = null

    fun isCancelled(): Boolean = state.value

    suspend fun awaitCancellation() {
        state.first { it }
    }

    fun createChild(): TreeNode {
        val child = TreeNode()

        // If already cancelled, child starts cancelled
        if (state.value) {
            child.state.value = true
            return child
        }

        // Register child
        child.parent = this
        children.add(child)
        return child
    }

    fun cancel() {
        if (state.value) return

        // Cancel self
        state.value = true

        // Cancel all children recursively
        val childrenToCancel = children.toList()
        children.clear()

        for (child in childrenToCancel) {
            child.parent = null
            child.cancel()
        }
    }
}

/**
 * A wrapper for [CancellationToken] which automatically cancels it on close.
 *
 * Implements [AutoCloseable] for use with Kotlin's `use` extension:
 * ```
 * token.dropGuard().use {
 *     // If this block exits (normally or via exception),
 *     // the token will be cancelled unless disarm() was called
 * }
 * ```
 *
 * Ported from Rust tokio_util::sync::cancellation_token::guard::DropGuard
 */
class DropGuard internal constructor(
    private var inner: CancellationToken?
) : AutoCloseable {

    /**
     * Returns the stored cancellation token and disarms this guard
     * (i.e., it will no longer cancel the token on close).
     *
     * @throws IllegalStateException if already disarmed or closed
     */
    fun disarm(): CancellationToken {
        return inner?.also { inner = null }
            ?: throw IllegalStateException("DropGuard already disarmed or closed")
    }

    /**
     * Cancels the token if this guard has not been disarmed.
     */
    override fun close() {
        inner?.cancel()
        inner = null
    }
}

/**
 * A wrapper for [CancellationToken] reference which automatically cancels it on close.
 *
 * This is a non-owning version of [DropGuard].
 *
 * Ported from Rust tokio_util::sync::cancellation_token::guard_ref::DropGuardRef
 */
class DropGuardRef internal constructor(
    private var inner: CancellationToken?
) : AutoCloseable {

    /**
     * Returns the stored cancellation token reference and disarms this guard
     * (i.e., it will no longer cancel the token on close).
     *
     * @throws IllegalStateException if already disarmed or closed
     */
    fun disarm(): CancellationToken {
        return inner?.also { inner = null }
            ?: throw IllegalStateException("DropGuardRef already disarmed or closed")
    }

    /**
     * Cancels the token if this guard has not been disarmed.
     */
    override fun close() {
        inner?.cancel()
        inner = null
    }
}

/**
 * Creates a [DropGuardRef] for this token.
 *
 * The returned guard will cancel this token (and all its children) when
 * [DropGuardRef.close] is called, unless [DropGuardRef.disarm] is called first.
 */
fun CancellationToken.dropGuardRef(): DropGuardRef = DropGuardRef(this)
