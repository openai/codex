package kasuari

/**
 * Helper class for tracking variable values in tests.
 */
class Values {
    private val values: MutableMap<Variable, Double> = mutableMapOf()

    fun valueOf(variable: Variable): Double = values[variable] ?: 0.0

    fun updateValues(changes: List<Pair<Variable, Double>>) {
        for ((variable, value) in changes) {
            println("$variable changed to $value")
            values[variable] = value
        }
    }
}

/**
 * Create a new Values instance with convenience functions.
 */
fun newValues(): Pair<(Variable) -> Double, (List<Pair<Variable, Double>>) -> Unit> {
    val values = Values()
    val valueOf: (Variable) -> Double = { v -> values.valueOf(v) }
    val updateValues: (List<Pair<Variable, Double>>) -> Unit = { changes -> values.updateValues(changes) }
    return Pair(valueOf, updateValues)
}
