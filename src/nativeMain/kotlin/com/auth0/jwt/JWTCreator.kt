@file:OptIn(kotlin.time.ExperimentalTime::class)

package com.auth0.jwt

import com.auth0.jwt.algorithms.Algorithm
import com.auth0.jwt.exceptions.JWTCreationException
import com.auth0.jwt.exceptions.SignatureGenerationException
import kotlin.time.Instant
import kotlinx.serialization.json.*
import okio.ByteString.Companion.encodeUtf8

/**
 * The JWTCreator class holds the sign method to generate a complete JWT (with Signature)
 * from a given Header and Payload content.
 *
 *
 * This class is thread-safe.
 */
class JWTCreator private constructor(
    private val algorithm: Algorithm,
    headerClaims: Map<String, Any?>?,
    payloadClaims: Map<String, Any?>?
) {
    private val headerJson: String
    private val payloadJson: String

    init {
        try {
            headerJson = mapToJson(headerClaims ?: emptyMap()).toString()
            payloadJson = mapToJson(payloadClaims ?: emptyMap()).toString()
        } catch (e: Exception) {
            throw JWTCreationException("Some of the Claims couldn't be converted to a valid JSON format.", e)
        }
    }

    private fun mapToJson(map: Map<String, Any?>): JsonObject {
        val content = mutableMapOf<String, JsonElement>()
        for ((key, value) in map) {
            content[key] = toJsonElement(value)
        }
        return JsonObject(content)
    }

    private fun toJsonElement(value: Any?): JsonElement {
        return when (value) {
            null -> JsonNull
            is String -> JsonPrimitive(value)
            is Number -> JsonPrimitive(value)
            is Boolean -> JsonPrimitive(value)
            is Instant -> JsonPrimitive(value.epochSeconds)
            is List<*> -> JsonArray(value.map { toJsonElement(it) })
            is Map<*, *> -> {
                val map = mutableMapOf<String, JsonElement>()
                for ((k, v) in value) {
                    if (k is String) {
                        map[k] = toJsonElement(v)
                    }
                }
                JsonObject(map)
            }
            is Array<*> -> JsonArray(value.map { toJsonElement(it) })
            else -> JsonPrimitive(value.toString())
        }
    }

    /**
     * The Builder class holds the Claims that defines the JWT to be created.
     */
    class Builder internal constructor() {
        private val payloadClaims: MutableMap<String, Any?>
        private val headerClaims: MutableMap<String, Any?>

        init {
            this.payloadClaims = LinkedHashMap()
            this.headerClaims = LinkedHashMap()
        }

        /**
         * Add specific Claims to set as the Header.
         * If provided map is null then nothing is changed
         *
         * @param headerClaims the values to use as Claims in the token's Header.
         * @return this same Builder instance.
         */
        fun withHeader(headerClaims: Map<String, Any?>?): Builder {
            if (headerClaims == null) {
                return this
            }

            for ((key, value) in headerClaims) {
                if (value == null) {
                    this.headerClaims.remove(key)
                } else {
                    this.headerClaims[key] = value
                }
            }

            return this
        }

        /**
         * Add specific Claims to set as the Header.
         * If provided json is null then nothing is changed
         *
         * @param headerClaimsJson the values to use as Claims in the token's Header.
         * @return this same Builder instance.
         * @throws IllegalArgumentException if json value has invalid structure
         */
        @Throws(IllegalArgumentException::class)
        fun withHeader(headerClaimsJson: String?): Builder {
            if (headerClaimsJson == null) {
                return this
            }

            try {
                val json = Json.parseToJsonElement(headerClaimsJson)
                if (json is JsonObject) {
                    // Convert JsonObject to Map
                    val map = json.mapValues { entry -> 
                        when(val v = entry.value) {
                            is JsonPrimitive -> v.contentOrNull
                            else -> v.toString() // Simplification
                        }
                    }
                    return withHeader(map)
                }
                return this
            } catch (e: Exception) {
                throw IllegalArgumentException("Invalid header JSON", e)
            }
        }

        /**
         * Add a specific Key Id ("kid") claim to the Header.
         * If the [Algorithm] used to sign this token was instantiated with a KeyProvider,
         * the 'kid' value will be taken from that provider and this one will be ignored.
         *
         * @param keyId the Key Id value.
         * @return this same Builder instance.
         */
        fun withKeyId(keyId: String?): Builder {
            this.headerClaims[HeaderParams.KEY_ID] = keyId
            return this
        }

        /**
         * Add a specific Issuer ("iss") claim to the Payload.
         *
         * @param issuer the Issuer value.
         * @return this same Builder instance.
         */
        fun withIssuer(issuer: String?): Builder {
            addClaim(RegisteredClaims.ISSUER, issuer)
            return this
        }

        /**
         * Add a specific Subject ("sub") claim to the Payload.
         *
         * @param subject the Subject value.
         * @return this same Builder instance.
         */
        fun withSubject(subject: String?): Builder {
            addClaim(RegisteredClaims.SUBJECT, subject)
            return this
        }

        /**
         * Add a specific Audience ("aud") claim to the Payload.
         *
         * @param audience the Audience value.
         * @return this same Builder instance.
         */
        fun withAudience(vararg audience: String): Builder {
            addClaim(RegisteredClaims.AUDIENCE, audience.toList())
            return this
        }

        /**
         * Add a specific Expires At ("exp") claim to the payload. The claim will be written as seconds since the epoch.
         * Milliseconds will be truncated by rounding down to the nearest second.
         *
         * @param expiresAt the Expires At value.
         * @return this same Builder instance.
         */
        fun withExpiresAt(expiresAt: Instant?): Builder {
            addClaim(RegisteredClaims.EXPIRES_AT, expiresAt)
            return this
        }

        /**
         * Add a specific Not Before ("nbf") claim to the Payload. The claim will be written as seconds since the epoch;
         * Milliseconds will be truncated by rounding down to the nearest second.
         *
         * @param notBefore the Not Before value.
         * @return this same Builder instance.
         */
        fun withNotBefore(notBefore: Instant?): Builder {
            addClaim(RegisteredClaims.NOT_BEFORE, notBefore)
            return this
        }

        /**
         * Add a specific Issued At ("iat") claim to the Payload. The claim will be written as seconds since the epoch;
         * Milliseconds will be truncated by rounding down to the nearest second.
         *
         * @param issuedAt the Issued At value.
         * @return this same Builder instance.
         */
        fun withIssuedAt(issuedAt: Instant?): Builder {
            addClaim(RegisteredClaims.ISSUED_AT, issuedAt)
            return this
        }

        /**
         * Add a specific JWT Id ("jti") claim to the Payload.
         *
         * @param jwtId the Token Id value.
         * @return this same Builder instance.
         */
        fun withJWTId(jwtId: String?): Builder {
            addClaim(RegisteredClaims.JWT_ID, jwtId)
            return this
        }

        /**
         * Add a custom Claim value.
         *
         * @param name  the Claim's name.
         * @param value the Claim's value.
         * @return this same Builder instance.
         * @throws IllegalArgumentException if the name is null.
         */
        @Throws(IllegalArgumentException::class)
        fun withClaim(name: String, value: Boolean?): Builder {
            addClaim(name, value)
            return this
        }

        /**
         * Add a custom Claim value.
         *
         * @param name  the Claim's name.
         * @param value the Claim's value.
         * @return this same Builder instance.
         * @throws IllegalArgumentException if the name is null.
         */
        @Throws(IllegalArgumentException::class)
        fun withClaim(name: String, value: Int?): Builder {
            addClaim(name, value)
            return this
        }

        /**
         * Add a custom Claim value.
         *
         * @param name  the Claim's name.
         * @param value the Claim's value.
         * @return this same Builder instance.
         * @throws IllegalArgumentException if the name is null.
         */
        @Throws(IllegalArgumentException::class)
        fun withClaim(name: String, value: Long?): Builder {
            addClaim(name, value)
            return this
        }

        /**
         * Add a custom Claim value.
         *
         * @param name  the Claim's name.
         * @param value the Claim's value.
         * @return this same Builder instance.
         * @throws IllegalArgumentException if the name is null.
         */
        @Throws(IllegalArgumentException::class)
        fun withClaim(name: String, value: Double?): Builder {
            addClaim(name, value)
            return this
        }

        /**
         * Add a custom Claim value.
         *
         * @param name  the Claim's name.
         * @param value the Claim's value.
         * @return this same Builder instance.
         * @throws IllegalArgumentException if the name is null.
         */
        @Throws(IllegalArgumentException::class)
        fun withClaim(name: String, value: String?): Builder {
            addClaim(name, value)
            return this
        }

        /**
         * Add a custom Claim value. The claim will be written as seconds since the epoch.
         * Milliseconds will be truncated by rounding down to the nearest second.
         *
         * @param name  the Claim's name.
         * @param value the Claim's value.
         * @return this same Builder instance.
         * @throws IllegalArgumentException if the name is null.
         */
        @Throws(IllegalArgumentException::class)
        fun withClaim(name: String, value: Instant?): Builder {
            addClaim(name, value)
            return this
        }

        /**
         * Add a custom Map Claim with the given items.
         *
         * @param name the Claim's name.
         * @param map  the Claim's key-values.
         * @return this same Builder instance.
         * @throws IllegalArgumentException if the name is null, or if the map contents does not validate.
         */
        @Throws(IllegalArgumentException::class)
        fun withClaim(name: String, map: Map<String, *>?): Builder {
            addClaim(name, map)
            return this
        }

        /**
         * Add a custom List Claim with the given items.
         *
         * @param name the Claim's name.
         * @param list the Claim's list of values.
         * @return this same Builder instance.
         * @throws IllegalArgumentException if the name is null, or if the list contents does not validate.
         */
        @Throws(IllegalArgumentException::class)
        fun withClaim(name: String, list: List<*>?): Builder {
            addClaim(name, list)
            return this
        }
        
        @Throws(IllegalArgumentException::class)
        fun withArrayClaim(name: String, items: Array<String>?): Builder {
            addClaim(name, items)
            return this
        }

        @Throws(IllegalArgumentException::class)
        fun withArrayClaim(name: String, items: Array<Int>?): Builder {
            addClaim(name, items)
            return this
        }

        @Throws(IllegalArgumentException::class)
        fun withArrayClaim(name: String, items: Array<Long>?): Builder {
            addClaim(name, items)
            return this
        }

        /**
         * Add a custom claim with null value.
         *
         * @param name the Claim's name.
         * @return this same Builder instance.
         * @throws IllegalArgumentException if the name is null
         */
        @Throws(IllegalArgumentException::class)
        fun withNullClaim(name: String): Builder {
            addClaim(name, null)
            return this
        }

        /**
         * Add specific Claims to set as the Payload. If the provided map is null then
         * nothing is changed.
         *
         * @param payloadClaims the values to use as Claims in the token's payload.
         * @return this same Builder instance.
         * @throws IllegalArgumentException if any of the claim keys or null,
         * or if the values are not of a supported type.
         */
        @Throws(IllegalArgumentException::class)
        fun withPayload(payloadClaims: Map<String, *>?): Builder {
            if (payloadClaims == null) {
                return this
            }

            // add claims only after validating all claims so as not to corrupt the claims map of this builder
            for ((key, value) in payloadClaims) {
                addClaim(key, value)
            }

            return this
        }

        /**
         * Creates a new JWT and signs it with the given algorithm.
         *
         * @param algorithm used to sign the JWT
         * @return a new JWT token
         * @throws IllegalArgumentException if the provided algorithm is null.
         * @throws JWTCreationException     if the claims could not be converted to a valid JSON
         * or there was a problem with the signing key.
         */
        @Throws(IllegalArgumentException::class, JWTCreationException::class)
        fun sign(algorithm: Algorithm): String {
            headerClaims[HeaderParams.ALGORITHM] = algorithm.name
            if (!headerClaims.containsKey(HeaderParams.TYPE)) {
                headerClaims[HeaderParams.TYPE] = "JWT"
            }
            val signingKeyId: String? = algorithm.signingKeyId
            if (signingKeyId != null) {
                withKeyId(signingKeyId)
            }
            return JWTCreator(algorithm, headerClaims, payloadClaims).sign()
        }

        private fun addClaim(name: String?, value: Any?) {
            if (name == null) throw IllegalArgumentException("The Custom Claim's name can't be null.")
            payloadClaims[name] = value
        }
    }

    @Throws(SignatureGenerationException::class)
    private fun sign(): String {
        val header = headerJson.encodeUtf8().base64Url().trim('=')
        val payload = payloadJson.encodeUtf8().base64Url().trim('=')

        val signatureBytes = algorithm.sign(
            header.encodeUtf8().toByteArray(),
            payload.encodeUtf8().toByteArray()
        )
        val signature = signatureBytes.toByteString().base64Url().trim('=')

        return "$header.$payload.$signature"
    }

    companion object {
        /**
         * Initialize a JWTCreator instance.
         *
         * @return a JWTCreator.Builder instance to configure.
         */
        fun init(): Builder {
            return Builder()
        }
    }
}

// Helper extension for ByteString to match Okio API if needed, or just use direct methods
private fun okio.ByteString.toByteString() = this