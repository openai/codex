@file:OptIn(kotlin.time.ExperimentalTime::class)

package com.auth0.jwt.impl

import com.auth0.jwt.exceptions.JWTDecodeException
import com.auth0.jwt.interfaces.Claim
import com.auth0.jwt.interfaces.Header
import com.auth0.jwt.interfaces.JWTPartsParser
import com.auth0.jwt.interfaces.Payload
import kotlinx.datetime.Instant
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.*
import kotlin.reflect.KClass

/**
 * This class helps in decoding the Header and Payload of the JWT using
 * kotlinx.serialization.
 */
class JWTParser : JWTPartsParser {

    private val json = Json { 
        ignoreUnknownKeys = true 
        isLenient = true
    }

    @Throws(JWTDecodeException::class)
    override fun parsePayload(jsonString: String?): Payload {
        if (jsonString == null) {
            throw decodeException()
        }

        try {
            val element = json.parseToJsonElement(jsonString)
            if (element !is JsonObject) throw decodeException(jsonString)
            return PayloadImpl(element)
        } catch (e: Exception) {
            throw decodeException(jsonString)
        }
    }

    @Throws(JWTDecodeException::class)
    override fun parseHeader(jsonString: String?): Header {
        if (jsonString == null) {
            throw decodeException()
        }

        try {
            val element = json.parseToJsonElement(jsonString)
            if (element !is JsonObject) throw decodeException(jsonString)
            return HeaderImpl(element)
        } catch (e: Exception) {
            throw decodeException(jsonString)
        }
    }

    companion object {
        private fun decodeException(): JWTDecodeException {
            return decodeException(null)
        }

        private fun decodeException(json: String?): JWTDecodeException {
            return JWTDecodeException("The string '$json' doesn't have a valid JSON format.")
        }
    }
}

internal class HeaderImpl(private val tree: JsonObject) : Header {
    override val algorithm: String? get() = tree["alg"]?.jsonPrimitive?.contentOrNull
    override val type: String? get() = tree["typ"]?.jsonPrimitive?.contentOrNull
    override val contentType: String? get() = tree["cty"]?.jsonPrimitive?.contentOrNull
    override val keyId: String? get() = tree["kid"]?.jsonPrimitive?.contentOrNull

    override fun getHeaderClaim(name: String): Claim {
        return JsonClaim(tree[name])
    }
}

internal class PayloadImpl(private val tree: JsonObject) : Payload {
    override val issuer: String? get() = tree["iss"]?.jsonPrimitive?.contentOrNull
    override val subject: String? get() = tree["sub"]?.jsonPrimitive?.contentOrNull
    override val audience: List<String>? get() {
        val aud = tree["aud"]
        return when (aud) {
            is JsonArray -> aud.mapNotNull { it.jsonPrimitive.contentOrNull }
            is JsonPrimitive -> aud.contentOrNull?.let { listOf(it) }
            else -> null
        }
    }
    override val expiresAt: Instant? get() = tree["exp"]?.jsonPrimitive?.longOrNull?.let { Instant.fromEpochSeconds(it) }
    override val notBefore: Instant? get() = tree["nbf"]?.jsonPrimitive?.longOrNull?.let { Instant.fromEpochSeconds(it) }
    override val issuedAt: Instant? get() = tree["iat"]?.jsonPrimitive?.longOrNull?.let { Instant.fromEpochSeconds(it) }
    override val id: String? get() = tree["jti"]?.jsonPrimitive?.contentOrNull

    override fun getClaim(name: String): Claim {
        return JsonClaim(tree[name])
    }

    override val claims: Map<String, Claim>
        get() = tree.mapValues { JsonClaim(it.value) }
}

internal class JsonClaim(private val element: JsonElement?) : Claim {
    override fun asBoolean(): Boolean? = element?.jsonPrimitive?.booleanOrNull
    override fun asInt(): Int? = element?.jsonPrimitive?.intOrNull
    override fun asLong(): Long? = element?.jsonPrimitive?.longOrNull
    override fun asDouble(): Double? = element?.jsonPrimitive?.doubleOrNull
    override fun asString(): String? = element?.jsonPrimitive?.contentOrNull
    override fun asDate(): Instant? = element?.jsonPrimitive?.longOrNull?.let { Instant.fromEpochSeconds(it) }

    override fun <T : Any> asList(clazz: KClass<T>): List<T>? {
        if (element !is JsonArray) return null
        // Simple mapping for basic types, more complex types would need reified/serializers
        return try {
            element.mapNotNull { 
                when (clazz) {
                    String::class -> it.jsonPrimitive.contentOrNull as T
                    Int::class -> it.jsonPrimitive.intOrNull as T
                    Long::class -> it.jsonPrimitive.longOrNull as T
                    Boolean::class -> it.jsonPrimitive.booleanOrNull as T
                    else -> null // Fallback or error
                }
            }
        } catch (e: Exception) {
            null
        }
    }

    override fun asMap(): Map<String, Any>? {
        if (element !is JsonObject) return null
        // Recursive conversion not fully implemented for deep objects in this simple port
        // This is a simplification.
        return element.mapValues { entry -> 
            entry.value.jsonPrimitive.contentOrNull ?: entry.value.toString() 
        }
    }

    override fun isNull(): Boolean = element == null || element is JsonNull
}