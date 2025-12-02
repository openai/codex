package ai.solace.coder.core.prompt

import ai.solace.coder.core.model.ModelFamily
import kotlinx.serialization.json.JsonElement

data class Prompt(
    val input: String,
    val tools: List<Any>,
    val parallelToolCalls: Boolean,
    val outputSchema: JsonElement?
) {
    fun getFullInstructions(modelFamily: ModelFamily): String {
        return "Instructions placeholder"
    }

    fun getFormattedInput(): String {
        return input
    }
}
