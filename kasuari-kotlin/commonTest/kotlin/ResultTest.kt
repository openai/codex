package kasuari

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertNull
import kotlin.test.assertTrue

/**
 * Tests for the Result type and Solver extension functions.
 */
class ResultTest {

    // ============================================================================
    // Basic Result operations
    // ============================================================================

    @Test
    fun okResult() {
        val result: Result<Int, String> = Result.ok(42)

        assertTrue(result.isOk)
        assertFalse(result.isErr)
        assertEquals(42, result.ok())
        assertNull(result.err())
    }

    @Test
    fun errResult() {
        val result: Result<Int, String> = Result.err("error")

        assertFalse(result.isOk)
        assertTrue(result.isErr)
        assertNull(result.ok())
        assertEquals("error", result.err())
    }

    @Test
    fun unwrapOk() {
        val result: Result<Int, String> = Result.ok(42)
        assertEquals(42, result.unwrap())
    }

    @Test
    fun unwrapOr() {
        val ok: Result<Int, String> = Result.ok(42)
        val err: Result<Int, String> = Result.err("error")

        assertEquals(42, ok.unwrapOr(0))
        assertEquals(0, err.unwrapOr(0))
    }

    @Test
    fun unwrapOrElse() {
        val ok: Result<Int, String> = Result.ok(42)
        val err: Result<Int, String> = Result.err("error")

        assertEquals(42, ok.unwrapOrElse { it.length })
        assertEquals(5, err.unwrapOrElse { it.length })  // "error".length == 5
    }

    @Test
    fun mapOk() {
        val result: Result<Int, String> = Result.ok(42)
        val mapped = result.map { it * 2 }

        assertTrue(mapped.isOk)
        assertEquals(84, mapped.ok())
    }

    @Test
    fun mapErr() {
        val result: Result<Int, String> = Result.err("error")
        val mapped = result.map { it * 2 }

        assertTrue(mapped.isErr)
        assertEquals("error", mapped.err())
    }

    @Test
    fun mapErrOnErr() {
        val result: Result<Int, String> = Result.err("error")
        val mapped = result.mapErr { it.uppercase() }

        assertTrue(mapped.isErr)
        assertEquals("ERROR", mapped.err())
    }

    @Test
    fun mapErrOnOk() {
        val result: Result<Int, String> = Result.ok(42)
        val mapped = result.mapErr { it.uppercase() }

        assertTrue(mapped.isOk)
        assertEquals(42, mapped.ok())
    }

    @Test
    fun andThenOk() {
        val result: Result<Int, String> = Result.ok(42)
        val chained = result.andThen { Result.ok(it.toString()) }

        assertTrue(chained.isOk)
        assertEquals("42", chained.ok())
    }

    @Test
    fun andThenErr() {
        val result: Result<Int, String> = Result.err("error")
        val chained = result.andThen { Result.ok(it.toString()) }

        assertTrue(chained.isErr)
        assertEquals("error", chained.err())
    }

    // ============================================================================
    // Solver try* methods - success cases
    // ============================================================================

    @Test
    fun tryAddConstraintSuccess() {
        val solver = Solver.new()
        val v = Variable.new()
        val constraint = v with WeightedRelation.EQ(Strength.REQUIRED) to 100.0

        val result = solver.tryAddConstraint(constraint)

        assertTrue(result.isOk)
        assertTrue(solver.hasConstraint(constraint))
    }

    @Test
    fun tryAddConstraintDuplicate() {
        val solver = Solver.new()
        val v = Variable.new()
        val constraint = v with WeightedRelation.EQ(Strength.REQUIRED) to 100.0

        solver.addConstraint(constraint)
        val result = solver.tryAddConstraint(constraint)

        assertTrue(result.isErr)
        assertIs<AddConstraintError.DuplicateConstraint>(result.err())
    }

    @Test
    fun tryAddConstraintsSuccess() {
        val solver = Solver.new()
        val v1 = Variable.new()
        val v2 = Variable.new()
        val constraints = listOf(
            v1 with WeightedRelation.EQ(Strength.REQUIRED) to 100.0,
            v2 with WeightedRelation.EQ(Strength.REQUIRED) to 200.0
        )

        val result = solver.tryAddConstraints(constraints)

        assertTrue(result.isOk)
        assertEquals(100.0, solver.getValue(v1))
        assertEquals(200.0, solver.getValue(v2))
    }

    @Test
    fun tryRemoveConstraintSuccess() {
        val solver = Solver.new()
        val v = Variable.new()
        val constraint = v with WeightedRelation.EQ(Strength.REQUIRED) to 100.0

        solver.addConstraint(constraint)
        val result = solver.tryRemoveConstraint(constraint)

        assertTrue(result.isOk)
        assertFalse(solver.hasConstraint(constraint))
    }

    @Test
    fun tryRemoveConstraintUnknown() {
        val solver = Solver.new()
        val v = Variable.new()
        val constraint = v with WeightedRelation.EQ(Strength.REQUIRED) to 100.0

        val result = solver.tryRemoveConstraint(constraint)

        assertTrue(result.isErr)
        assertIs<RemoveConstraintError.UnknownConstraint>(result.err())
    }

    @Test
    fun tryAddEditVariableSuccess() {
        val solver = Solver.new()
        val v = Variable.new()

        val result = solver.tryAddEditVariable(v, Strength.STRONG)

        assertTrue(result.isOk)
        assertTrue(solver.hasEditVariable(v))
    }

    @Test
    fun tryAddEditVariableDuplicate() {
        val solver = Solver.new()
        val v = Variable.new()

        solver.addEditVariable(v, Strength.STRONG)
        val result = solver.tryAddEditVariable(v, Strength.STRONG)

        assertTrue(result.isErr)
        assertIs<AddEditVariableError.DuplicateEditVariable>(result.err())
    }

    @Test
    fun tryAddEditVariableBadStrength() {
        val solver = Solver.new()
        val v = Variable.new()

        val result = solver.tryAddEditVariable(v, Strength.REQUIRED)

        assertTrue(result.isErr)
        assertIs<AddEditVariableError.BadRequiredStrength>(result.err())
    }

    @Test
    fun tryRemoveEditVariableSuccess() {
        val solver = Solver.new()
        val v = Variable.new()

        solver.addEditVariable(v, Strength.STRONG)
        val result = solver.tryRemoveEditVariable(v)

        assertTrue(result.isOk)
        assertFalse(solver.hasEditVariable(v))
    }

    @Test
    fun tryRemoveEditVariableUnknown() {
        val solver = Solver.new()
        val v = Variable.new()

        val result = solver.tryRemoveEditVariable(v)

        assertTrue(result.isErr)
        assertIs<RemoveEditVariableError.UnknownEditVariable>(result.err())
    }

    @Test
    fun trySuggestValueSuccess() {
        val solver = Solver.new()
        val v = Variable.new()

        solver.addEditVariable(v, Strength.STRONG)
        val result = solver.trySuggestValue(v, 50.0)

        assertTrue(result.isOk)
        assertEquals(50.0, solver.getValue(v))
    }

    @Test
    fun trySuggestValueUnknown() {
        val solver = Solver.new()
        val v = Variable.new()

        val result = solver.trySuggestValue(v, 50.0)

        assertTrue(result.isErr)
        assertIs<SuggestValueError.UnknownEditVariable>(result.err())
    }

    // ============================================================================
    // Chaining Result operations
    // ============================================================================

    @Test
    fun chainedSolverOperations() {
        val solver = Solver.new()
        val v = Variable.new()

        // Add edit variable, then suggest value
        val addResult = solver.tryAddEditVariable(v, Strength.STRONG)
        assertTrue(addResult.isOk)

        val suggestResult = solver.trySuggestValue(v, 100.0)
        assertTrue(suggestResult.isOk)

        assertEquals(100.0, solver.getValue(v))
    }

    @Test
    fun chainedSolverOperationsWithError() {
        val solver = Solver.new()
        val v = Variable.new()

        // Try to suggest value without adding edit variable first - should fail
        val result = solver.trySuggestValue(v, 100.0)
        assertTrue(result.isErr)

        // Value should not have changed
        assertEquals(0.0, solver.getValue(v))
    }
}
