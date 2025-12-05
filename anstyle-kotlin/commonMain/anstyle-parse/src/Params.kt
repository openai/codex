package anstyle.parse

/**
 * Fixed size parameters list with optional subparameters.
 */

internal const val MAX_PARAMS: Int = 32

/**
 * Parameters for ANSI escape sequences.
 *
 * Stores both parameters and subparameters (separated by `:` in escape sequences).
 */
class Params : Iterable<List<UShort>> {
    /**
     * Number of subparameters for each parameter.
     *
     * For each entry in the `params` array, this stores the length of the param as number of
     * subparams at the same index as the param in the `params` array.
     *
     * At the subparam positions the length will always be `0`.
     */
    private val subparams: UByteArray = UByteArray(MAX_PARAMS)

    /**
     * All parameters and subparameters.
     */
    private val params: UShortArray = UShortArray(MAX_PARAMS)

    /**
     * Number of subparameters in the current parameter.
     */
    private var currentSubparams: UByte = 0u

    /**
     * Total number of parameters and subparameters.
     */
    private var _len: Int = 0

    /**
     * Returns the number of parameters.
     */
    fun len(): Int = _len

    /**
     * Returns `true` if there are no parameters present.
     */
    fun isEmpty(): Boolean = _len == 0

    /**
     * Returns an iterator over all parameters and subparameters.
     */
    override fun iterator(): ParamsIter = ParamsIter(this)

    /**
     * Returns `true` if there is no more space for additional parameters.
     */
    internal fun isFull(): Boolean = _len == MAX_PARAMS

    /**
     * Clear all parameters.
     */
    internal fun clear() {
        currentSubparams = 0u
        _len = 0
    }

    /**
     * Add an additional parameter.
     */
    internal fun push(item: UShort) {
        subparams[_len - currentSubparams.toInt()] = (currentSubparams + 1u).toUByte()
        params[_len] = item
        currentSubparams = 0u
        _len += 1
    }

    /**
     * Add an additional subparameter to the current parameter.
     */
    internal fun extend(item: UShort) {
        subparams[_len - currentSubparams.toInt()] = (currentSubparams + 1u).toUByte()
        params[_len] = item
        currentSubparams = (currentSubparams + 1u).toUByte()
        _len += 1
    }

    /**
     * Get the subparams array (for iterator access).
     */
    internal fun getSubparams(): UByteArray = subparams

    /**
     * Get the params array (for iterator access).
     */
    internal fun getParams(): UShortArray = params

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is Params) return false
        if (_len != other._len) return false
        for (i in 0 until _len) {
            if (params[i] != other.params[i]) return false
            if (subparams[i] != other.subparams[i]) return false
        }
        return true
    }

    override fun hashCode(): Int {
        var result = _len
        for (i in 0 until _len) {
            result = 31 * result + params[i].hashCode()
            result = 31 * result + subparams[i].hashCode()
        }
        return result
    }

    override fun toString(): String = buildString {
        append("[")
        var first = true
        for (param in this@Params) {
            if (!first) append(";")
            first = false
            var subFirst = true
            for (subparam in param) {
                if (!subFirst) append(":")
                subFirst = false
                append(subparam)
            }
        }
        append("]")
    }
}

/**
 * Immutable subparameter iterator.
 */
class ParamsIter(private val params: Params) : Iterator<List<UShort>> {
    private var index: Int = 0

    override fun hasNext(): Boolean = index < params.len()

    override fun next(): List<UShort> {
        if (!hasNext()) throw NoSuchElementException()

        // Get all subparameters for the current parameter.
        val numSubparams = params.getSubparams()[index].toInt()
        val paramsArray = params.getParams()
        val param = (index until index + numSubparams).map { paramsArray[it] }

        // Jump to the next parameter.
        index += numSubparams

        return param
    }
}

