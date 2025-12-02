package ai.solace.coder.core.model

import ai.solace.coder.api.provider.WireApi
import ai.solace.coder.core.AuthMode

data class ModelProviderInfo(
    val wireApi: WireApi
) {
    fun toApiProvider(authMode: AuthMode?): Any? {
        // Placeholder implementation
        return null
    }
}
