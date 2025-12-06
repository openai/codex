package kasuari

/**
 * A result type that represents either success ([Ok]) or failure ([Err]).
 *
 * This type mirrors Rust's `Result<T, E>` and provides an alternative to exception-based
 * error handling for users who prefer explicit error handling.
 *
 * ## Creating Results
 *
 * ```kotlin
 * // Create a success result
 * val success: Result<Int, String> = Result.ok(42)
 *
 * // Create an error result
 * val failure: Result<Int, String> = Result.err("Something went wrong")
 * ```
 *
 * ## Checking Results
 *
 * ```kotlin
 * val result: Result<Int, String> = someOperation()
 *
 * // Pattern matching with when
 * when (result) {
 *     is Result.Ok -> println("Success: ${result.value}")
 *     is Result.Err -> println("Error: ${result.error}")
 * }
 *
 * // Using properties
 * if (result.isOk) {
 *     println("Value: ${result.ok()}")
 * }
 * ```
 *
 * ## Extracting Values
 *
 * ```kotlin
 * val result: Result<Int, String> = someOperation()
 *
 * // Get value or throw
 * val value = result.unwrap()  // Throws if Err
 *
 * // Get value or default
 * val valueOrDefault = result.unwrapOr(0)
 *
 * // Get value or compute from error
 * val valueOrComputed = result.unwrapOrElse { error -> error.length }
 * ```
 *
 * ## Transforming Results
 *
 * ```kotlin
 * val result: Result<Int, String> = someOperation()
 *
 * // Map the success value
 * val doubled = result.map { it * 2 }
 *
 * // Map the error
 * val upperError = result.mapErr { it.uppercase() }
 *
 * // Chain operations
 * val chained = result.andThen { value ->
 *     if (value > 0) Result.ok(value.toString())
 *     else Result.err("Value must be positive")
 * }
 * ```
 *
 * ## Using with Solver
 *
 * The solver provides `try*` extension functions that return [Result] instead of throwing:
 *
 * ```kotlin
 * val solver = Solver.new()
 * val x = Variable.new()
 *
 * val result = solver.tryAddConstraint(
 *     x with WeightedRelation.EQ(Strength.REQUIRED) to 100.0
 * )
 *
 * when (result) {
 *     is Result.Ok -> println("Constraint added")
 *     is Result.Err -> println("Failed: ${result.error}")
 * }
 * ```
 *
 * @param T The type of the success value.
 * @param E The type of the error value.
 * @see Solver.tryAddConstraint
 * @see Solver.tryRemoveConstraint
 * @see Solver.tryAddEditVariable
 * @see Solver.tryRemoveEditVariable
 * @see Solver.trySuggestValue
 */
sealed class Result<out T, out E> {
    /**
     * Represents a successful result containing a value.
     *
     * @property value The success value.
     */
    data class Ok<T>(val value: T) : Result<T, Nothing>()

    /**
     * Represents a failed result containing an error.
     *
     * @property error The error value.
     */
    data class Err<E>(val error: E) : Result<Nothing, E>()

    /**
     * Returns `true` if this is an [Ok] result, `false` otherwise.
     */
    val isOk: Boolean get() = this is Ok

    /**
     * Returns `true` if this is an [Err] result, `false` otherwise.
     */
    val isErr: Boolean get() = this is Err

    /**
     * Returns the success value if this is [Ok], or `null` if this is [Err].
     *
     * @return The success value, or `null`.
     */
    fun ok(): T? = when (this) {
        is Ok -> value
        is Err -> null
    }

    /**
     * Returns the error value if this is [Err], or `null` if this is [Ok].
     *
     * @return The error value, or `null`.
     */
    fun err(): E? = when (this) {
        is Ok -> null
        is Err -> error
    }

    /**
     * Returns the success value if this is [Ok], or throws if this is [Err].
     *
     * @return The success value.
     * @throws RuntimeException If this is an [Err] result.
     */
    fun unwrap(): T = when (this) {
        is Ok -> value
        is Err -> throw RuntimeException("Called unwrap on an Err value: $error")
    }

    /**
     * Returns the success value if this is [Ok], or the provided default if this is [Err].
     *
     * @param default The default value to return if this is [Err].
     * @return The success value or the default.
     */
    fun unwrapOr(default: @UnsafeVariance T): T = when (this) {
        is Ok -> value
        is Err -> default
    }

    /**
     * Returns the success value if this is [Ok], or computes a value from the error if this is [Err].
     *
     * @param f A function that computes a value from the error.
     * @return The success value or the computed value.
     */
    inline fun unwrapOrElse(f: (E) -> @UnsafeVariance T): T = when (this) {
        is Ok -> value
        is Err -> f(error)
    }

    /**
     * Transforms the success value using the provided function, leaving errors unchanged.
     *
     * @param f The function to apply to the success value.
     * @return A new [Result] with the transformed value.
     */
    inline fun <U> map(f: (T) -> U): Result<U, E> = when (this) {
        is Ok -> Ok(f(value))
        is Err -> Err(error)
    }

    /**
     * Transforms the error value using the provided function, leaving success values unchanged.
     *
     * @param f The function to apply to the error value.
     * @return A new [Result] with the transformed error.
     */
    inline fun <F> mapErr(f: (E) -> F): Result<T, F> = when (this) {
        is Ok -> Ok(value)
        is Err -> Err(f(error))
    }

    /**
     * Chains another operation that returns a [Result].
     *
     * If this is [Ok], applies the function and returns its result.
     * If this is [Err], returns the error unchanged.
     *
     * @param f The function to apply to the success value.
     * @return The result of the function, or the original error.
     */
    inline fun <U> andThen(f: (T) -> Result<U, @UnsafeVariance E>): Result<U, E> = when (this) {
        is Ok -> f(value)
        is Err -> Err(error)
    }

    companion object {
        /**
         * Creates a success [Result] with the given value.
         *
         * @param value The success value.
         * @return An [Ok] result containing the value.
         */
        fun <T> ok(value: T): Result<T, Nothing> = Ok(value)

        /**
         * Creates a failure [Result] with the given error.
         *
         * @param error The error value.
         * @return An [Err] result containing the error.
         */
        fun <E> err(error: E): Result<Nothing, E> = Err(error)

        /**
         * Executes a block and wraps the result, catching exceptions of the specified type.
         *
         * If the block succeeds, returns [Ok] with the result.
         * If the block throws an exception of type [E], returns [Err] with that exception.
         * Other exceptions are rethrown.
         *
         * ```kotlin
         * val result = Result.catching<Int, NumberFormatException> {
         *     "42".toInt()
         * }
         * ```
         *
         * @param block The block to execute.
         * @return A [Result] containing the block's result or the caught exception.
         */
        inline fun <T, reified E : Exception> catching(block: () -> T): Result<T, E> {
            return try {
                Ok(block())
            } catch (e: Exception) {
                if (e is E) {
                    Err(e)
                } else {
                    throw e
                }
            }
        }
    }
}

// ============================================================================
// Solver extension functions that return Result instead of throwing
// ============================================================================

/**
 * Adds a constraint to the solver, returning a [Result] instead of throwing.
 *
 * This is the non-throwing alternative to [Solver.addConstraint].
 *
 * ```kotlin
 * when (val result = solver.tryAddConstraint(constraint)) {
 *     is Result.Ok -> println("Constraint added")
 *     is Result.Err -> when (result.error) {
 *         is AddConstraintError.DuplicateConstraint -> println("Already exists")
 *         is AddConstraintError.UnsatisfiableConstraint -> println("Conflicts")
 *         is AddConstraintError.InternalSolver -> println("Internal error")
 *     }
 * }
 * ```
 *
 * @param constraint The constraint to add.
 * @return [Result.Ok] if successful, [Result.Err] with the error otherwise.
 * @see Solver.addConstraint
 */
fun Solver.tryAddConstraint(constraint: Constraint): Result<Unit, AddConstraintError> {
    return try {
        addConstraint(constraint)
        Result.ok(Unit)
    } catch (e: AddConstraintError) {
        Result.err(e)
    }
}

/**
 * Adds multiple constraints to the solver, returning a [Result] instead of throwing.
 *
 * This is the non-throwing alternative to [Solver.addConstraints].
 *
 * @param constraints The constraints to add.
 * @return [Result.Ok] if all constraints were added, [Result.Err] with the first error.
 * @see Solver.addConstraints
 */
fun Solver.tryAddConstraints(constraints: Iterable<Constraint>): Result<Unit, AddConstraintError> {
    return try {
        addConstraints(constraints)
        Result.ok(Unit)
    } catch (e: AddConstraintError) {
        Result.err(e)
    }
}

/**
 * Removes a constraint from the solver, returning a [Result] instead of throwing.
 *
 * This is the non-throwing alternative to [Solver.removeConstraint].
 *
 * @param constraint The constraint to remove.
 * @return [Result.Ok] if successful, [Result.Err] with the error otherwise.
 * @see Solver.removeConstraint
 */
fun Solver.tryRemoveConstraint(constraint: Constraint): Result<Unit, RemoveConstraintError> {
    return try {
        removeConstraint(constraint)
        Result.ok(Unit)
    } catch (e: RemoveConstraintError) {
        Result.err(e)
    }
}

/**
 * Adds an edit variable to the solver, returning a [Result] instead of throwing.
 *
 * This is the non-throwing alternative to [Solver.addEditVariable].
 *
 * @param variable The variable to make editable.
 * @param strength The strength of the edit constraint.
 * @return [Result.Ok] if successful, [Result.Err] with the error otherwise.
 * @see Solver.addEditVariable
 */
fun Solver.tryAddEditVariable(variable: Variable, strength: Strength): Result<Unit, AddEditVariableError> {
    return try {
        addEditVariable(variable, strength)
        Result.ok(Unit)
    } catch (e: AddEditVariableError) {
        Result.err(e)
    }
}

/**
 * Removes an edit variable from the solver, returning a [Result] instead of throwing.
 *
 * This is the non-throwing alternative to [Solver.removeEditVariable].
 *
 * @param variable The variable to remove as an edit variable.
 * @return [Result.Ok] if successful, [Result.Err] with the error otherwise.
 * @see Solver.removeEditVariable
 */
fun Solver.tryRemoveEditVariable(variable: Variable): Result<Unit, RemoveEditVariableError> {
    return try {
        removeEditVariable(variable)
        Result.ok(Unit)
    } catch (e: RemoveEditVariableError) {
        Result.err(e)
    }
}

/**
 * Suggests a value for an edit variable, returning a [Result] instead of throwing.
 *
 * This is the non-throwing alternative to [Solver.suggestValue].
 *
 * @param variable The edit variable to suggest a value for.
 * @param value The suggested value.
 * @return [Result.Ok] if successful, [Result.Err] with the error otherwise.
 * @see Solver.suggestValue
 */
fun Solver.trySuggestValue(variable: Variable, value: Double): Result<Unit, SuggestValueError> {
    return try {
        suggestValue(variable, value)
        Result.ok(Unit)
    } catch (e: SuggestValueError) {
        Result.err(e)
    }
}
