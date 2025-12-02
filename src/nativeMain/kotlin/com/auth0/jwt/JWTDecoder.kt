@file:OptIn(kotlin.time.ExperimentalTime::class)

package com.auth0.jwt

import com.auth0.jwt.TokenUtils
import com.auth0.jwt.interfaces.DecodedJWT
import com.auth0.jwt.interfaces.Claim
import com.auth0.jwt.interfaces.DecodedJWT
import com.auth0.jwt.exceptions.JWTDecodeException
import com.auth0.jwt.impl.JWTParser
import okio.ByteString.Companion.decodeBase64
import kotlin.time.Instant

internal class JWTDecoder(private val parser: JWTParser) {

    constructor() : this(JWTParser())

    @Throws(JWTDecodeException::class)
    fun decode(token: String): DecodedJWT {
        val parts = TokenUtils.splitToken(token)
        val headerJson: String
        val payloadJson: String
        try {
            headerJson = parts[0].decodeBase64()?.utf8() ?: throw JWTDecodeException("Invalid header encoding")
            payloadJson = parts[1].decodeBase64()?.utf8() ?: throw JWTDecodeException("Invalid payload encoding")
        } catch (e: Exception) {
            throw JWTDecodeException("The UTF8 decoding failed.", e)
        }

        val headerImpl = parser.parseHeader(headerJson)
        val payloadImpl = parser.parsePayload(payloadJson)

        return object : DecodedJWT {
            override val algorithm: String? get() = headerImpl.algorithm
            override val type: String? get() = headerImpl.type
            override val contentType: String? get() = headerImpl.contentType
            override val keyId: String? get() = headerImpl.keyId
            override fun getHeaderClaim(name: String): Claim = headerImpl.getHeaderClaim(name)

            override val issuer: String? get() = payloadImpl.issuer
            override val subject: String? get() = payloadImpl.subject
            override val audience: List<String>? get() = payloadImpl.audience
            override val expiresAt: Instant? get() = payloadImpl.expiresAt
            override val notBefore: Instant? get() = payloadImpl.notBefore
            override val issuedAt: Instant? get() = payloadImpl.issuedAt
            override val id: String? get() = payloadImpl.id
            override fun getClaim(name: String): Claim = payloadImpl.getClaim(name)
            override val claims: Map<String, Claim> get() = payloadImpl.claims

            override val header: String get() = parts[0]
            override val payload: String get() = parts[1]
            override val signature: String get() = parts[2]
            override val token: String get() = token
        }
    }
}