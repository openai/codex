package kasuari

/**
 * Error conditions that can occur when adding a constraint to the solver.
 *
 * These errors are thrown by [Solver.addConstraint] or returned by [Solver.tryAddConstraint].
 *
 * ## Error Types
 *
 * - [DuplicateConstraint] - The constraint was already added to the solver.
 * - [UnsatisfiableConstraint] - A required constraint conflicts with existing constraints.
 * - [InternalSolver] - The solver entered an invalid state (should be reported as a bug).
 *
 * ## Example
 *
 * ```kotlin
 * val solver = Solver.new()
 * val x = Variable.new()
 * val constraint = x with WeightedRelation.EQ(Strength.REQUIRED) to 100.0
 *
 * // Using try* variant for explicit error handling
 * when (val result = solver.tryAddConstraint(constraint)) {
 *     is Result.Ok -> println("Constraint added successfully")
 *     is Result.Err -> when (result.error) {
 *         is AddConstraintError.DuplicateConstraint ->
 *             println("Constraint already exists")
 *         is AddConstraintError.UnsatisfiableConstraint ->
 *             println("Constraint conflicts with existing constraints")
 *         is AddConstraintError.InternalSolver ->
 *             println("Internal solver error: ${result.error.error}")
 *     }
 * }
 * ```
 *
 * @see Solver.addConstraint
 * @see Solver.tryAddConstraint
 */
sealed class AddConstraintError : Exception() {
    /**
     * The constraint has already been added to the solver.
     *
     * Each constraint can only be added once. If you need the same logical constraint
     * multiple times, create separate [Constraint] instances.
     */
    data object DuplicateConstraint : AddConstraintError() {
        override val message: String = "The constraint specified has already been added to the solver."
    }

    /**
     * The constraint is required but conflicts with existing constraints.
     *
     * This occurs when adding a [Strength.REQUIRED] constraint that cannot be satisfied
     * together with the other required constraints already in the solver. Consider using
     * a weaker strength ([Strength.STRONG], [Strength.MEDIUM], or [Strength.WEAK]) if
     * the constraint can be violated.
     */
    data object UnsatisfiableConstraint : AddConstraintError() {
        override val message: String = "The constraint is required, but it is unsatisfiable in conjunction with the existing constraints."
    }

    /**
     * The solver entered an invalid internal state.
     *
     * This indicates a bug in the solver implementation. If this error occurs,
     * please report it as an issue.
     *
     * @property error The underlying internal solver error.
     */
    data class InternalSolver(val error: InternalSolverError) : AddConstraintError() {
        override val message: String = "The solver entered an invalid state. If this occurs please report the issue."
    }
}

/**
 * Error conditions that can occur when removing a constraint from the solver.
 *
 * These errors are thrown by [Solver.removeConstraint] or returned by [Solver.tryRemoveConstraint].
 *
 * ## Error Types
 *
 * - [UnknownConstraint] - The constraint was not found in the solver.
 * - [InternalSolver] - The solver entered an invalid state (should be reported as a bug).
 *
 * @see Solver.removeConstraint
 * @see Solver.tryRemoveConstraint
 */
sealed class RemoveConstraintError : Exception() {
    /**
     * The constraint was not found in the solver.
     *
     * This occurs when trying to remove a constraint that was never added, or one
     * that was already removed. Remember that constraints use identity equality,
     * so you must pass the exact same [Constraint] instance that was added.
     */
    data object UnknownConstraint : RemoveConstraintError() {
        override val message: String = "The constraint specified was not already in the solver, so cannot be removed."
    }

    /**
     * The solver entered an invalid internal state.
     *
     * This indicates a bug in the solver implementation. If this error occurs,
     * please report it as an issue.
     *
     * @property error The underlying internal solver error.
     */
    data class InternalSolver(val error: InternalSolverError) : RemoveConstraintError() {
        override val message: String = "The solver entered an invalid state. If this occurs please report the issue."
    }
}

/**
 * Error conditions that can occur when adding an edit variable to the solver.
 *
 * Edit variables allow you to dynamically change variable values during solving.
 * These errors are thrown by [Solver.addEditVariable] or returned by [Solver.tryAddEditVariable].
 *
 * ## Error Types
 *
 * - [DuplicateEditVariable] - The variable is already an edit variable.
 * - [BadRequiredStrength] - The strength was [Strength.REQUIRED], which is not allowed.
 *
 * ## Example
 *
 * ```kotlin
 * val solver = Solver.new()
 * val x = Variable.new()
 *
 * // Add x as an edit variable with STRONG strength
 * solver.addEditVariable(x, Strength.STRONG)
 *
 * // Now you can suggest values for x
 * solver.suggestValue(x, 100.0)
 * ```
 *
 * @see Solver.addEditVariable
 * @see Solver.tryAddEditVariable
 * @see Solver.suggestValue
 */
sealed class AddEditVariableError : Exception() {
    /**
     * The variable is already registered as an edit variable.
     *
     * Each variable can only be added as an edit variable once. To change the
     * strength, remove the variable first with [Solver.removeEditVariable] and
     * then add it again with the new strength.
     */
    data object DuplicateEditVariable : AddEditVariableError() {
        override val message: String = "The specified variable is already marked as an edit variable in the solver."
    }

    /**
     * The strength was [Strength.REQUIRED], which is not allowed for edit variables.
     *
     * Edit variables must have a strength less than [Strength.REQUIRED]. This is because
     * edit variables are used for interactive changes where the solver needs flexibility
     * to potentially not fully satisfy the suggested value.
     */
    data object BadRequiredStrength : AddEditVariableError() {
        override val message: String = "The specified strength was `REQUIRED`. This is illegal for edit variable strengths."
    }
}

/**
 * Error conditions that can occur when removing an edit variable from the solver.
 *
 * These errors are thrown by [Solver.removeEditVariable] or returned by [Solver.tryRemoveEditVariable].
 *
 * ## Error Types
 *
 * - [UnknownEditVariable] - The variable is not registered as an edit variable.
 * - [InternalSolver] - The solver entered an invalid state (should be reported as a bug).
 *
 * @see Solver.removeEditVariable
 * @see Solver.tryRemoveEditVariable
 */
sealed class RemoveEditVariableError : Exception() {
    /**
     * The variable is not registered as an edit variable.
     *
     * This occurs when trying to remove a variable that was never added as an edit
     * variable, or one that was already removed.
     */
    data object UnknownEditVariable : RemoveEditVariableError() {
        override val message: String = "The specified variable was not an edit variable in the solver, so cannot be removed."
    }

    /**
     * The solver entered an invalid internal state.
     *
     * This indicates a bug in the solver implementation. If this error occurs,
     * please report it as an issue.
     *
     * @property error The underlying internal solver error.
     */
    data class InternalSolver(val error: InternalSolverError) : RemoveEditVariableError() {
        override val message: String = "The solver entered an invalid state. If this occurs please report the issue."
    }
}

/**
 * Error conditions that can occur when suggesting a value for an edit variable.
 *
 * These errors are thrown by [Solver.suggestValue] or returned by [Solver.trySuggestValue].
 *
 * ## Error Types
 *
 * - [UnknownEditVariable] - The variable is not registered as an edit variable.
 * - [InternalSolver] - The solver entered an invalid state (should be reported as a bug).
 *
 * ## Example
 *
 * ```kotlin
 * val solver = Solver.new()
 * val x = Variable.new()
 *
 * // First register x as an edit variable
 * solver.addEditVariable(x, Strength.STRONG)
 *
 * // Now suggest values - this can be called repeatedly
 * solver.suggestValue(x, 100.0)
 * println(solver.getValue(x))  // Should be close to 100.0
 *
 * solver.suggestValue(x, 200.0)
 * println(solver.getValue(x))  // Should be close to 200.0
 * ```
 *
 * @see Solver.suggestValue
 * @see Solver.trySuggestValue
 * @see Solver.addEditVariable
 */
sealed class SuggestValueError : Exception() {
    /**
     * The variable is not registered as an edit variable.
     *
     * You must call [Solver.addEditVariable] before suggesting values for a variable.
     */
    data object UnknownEditVariable : SuggestValueError() {
        override val message: String = "The specified variable was not an edit variable in the solver, so cannot have its value suggested."
    }

    /**
     * The solver entered an invalid internal state.
     *
     * This indicates a bug in the solver implementation. If this error occurs,
     * please report it as an issue.
     *
     * @property error The underlying internal solver error.
     */
    data class InternalSolver(val error: InternalSolverError) : SuggestValueError() {
        override val message: String = "The solver entered an invalid state. If this occurs please report the issue."
    }
}
