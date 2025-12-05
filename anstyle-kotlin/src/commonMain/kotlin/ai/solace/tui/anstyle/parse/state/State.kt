package ai.solace.tui.anstyle.parse.state

/**
 * Parser states for the ANSI escape code parsing state machine.
 */
enum class State(val value: UByte) {
    Anywhere(0u),
    CsiEntry(1u),
    CsiIgnore(2u),
    CsiIntermediate(3u),
    CsiParam(4u),
    DcsEntry(5u),
    DcsIgnore(6u),
    DcsIntermediate(7u),
    DcsParam(8u),
    DcsPassthrough(9u),
    Escape(10u),
    EscapeIntermediate(11u),
    Ground(12u),
    OscString(13u),
    SosPmApcString(14u),
    Utf8(15u);

    companion object {
        private val VALUES = entries.toTypedArray()

        /**
         * Convert a byte value to a State, or null if invalid.
         */
        fun fromByte(raw: UByte): State? =
            VALUES.getOrNull(raw.toInt())

        /**
         * The default state.
         */
        val DEFAULT: State = Ground
    }
}

/**
 * Actions performed during state transitions.
 */
enum class Action(val value: UByte) {
    Nop(0u),
    Clear(1u),
    Collect(2u),
    CsiDispatch(3u),
    EscDispatch(4u),
    Execute(5u),
    Hook(6u),
    Ignore(7u),
    OscEnd(8u),
    OscPut(9u),
    OscStart(10u),
    Param(11u),
    Print(12u),
    Put(13u),
    Unhook(14u),
    BeginUtf8(15u);

    companion object {
        private val VALUES = entries.toTypedArray()

        /**
         * Convert a byte value to an Action, or null if invalid.
         */
        fun fromByte(raw: UByte): Action? =
            VALUES.getOrNull(raw.toInt())

        /**
         * The default action.
         */
        val DEFAULT: Action = Nop
    }
}

/**
 * Unpack a byte into a State and Action.
 *
 * The state is stored in the bottom 4 bits, the action in the top 4 bits.
 */
fun unpack(delta: UByte): Pair<State, Action> {
    val stateValue = (delta.toInt() and 0x0f).toUByte()
    val actionValue = (delta.toInt() shr 4).toUByte()
    val state = State.fromByte(stateValue) ?: State.Ground
    val action = Action.fromByte(actionValue) ?: Action.Nop
    return Pair(state, action)
}

/**
 * Pack a State and Action into a single byte.
 */
fun pack(state: State, action: Action): UByte =
    ((action.value.toInt() shl 4) or state.value.toInt()).toUByte()

/**
 * Transition to next State.
 *
 * Note: This does not directly support UTF-8.
 * - If the data is validated as UTF-8 (e.g. `String`) or single-byte C1 control codes are
 *   unsupported, then treat [Action.BeginUtf8] and [Action.Execute] for UTF-8 continuations
 *   as [Action.Print].
 * - If the data is not validated, then a UTF-8 state machine will need to be implemented on top,
 *   starting with [Action.BeginUtf8].
 *
 * Note: When [State.Anywhere] is returned, revert back to the prior state.
 */
fun stateChange(state: State, byte: UByte): Pair<State, Action> {
    // Handle state changes in the anywhere state before evaluating changes
    // for current state.
    var change = stateChangeInternal(State.Anywhere, byte)
    if (change == 0.toUByte()) {
        change = stateChangeInternal(state, byte)
    }

    // Unpack into a state and action
    return unpack(change)
}

private fun stateChangeInternal(state: State, byte: UByte): UByte {
    val stateIdx = state.value.toInt()
    val byteIdx = byte.toInt()
    return STATE_CHANGES[stateIdx][byteIdx]
}

