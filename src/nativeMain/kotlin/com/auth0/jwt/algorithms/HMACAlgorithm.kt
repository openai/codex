package com.auth0.jwt.algorithms

import com.auth0.jwt.exceptions.SignatureGenerationException
import com.auth0.jwt.exceptions.SignatureVerificationException
import com.auth0.jwt.interfaces.DecodedJWT
import okio.ByteString.Companion.decodeBase64
import okio.ByteString.Companion.encodeUtf8

/**
 * Subclass representing an Hash-based MAC signing algorithm
 *
 *
 * This class is thread-safe.
 */
internal class HMACAlgorithm(crypto: CryptoHelper, id: String, algorithm: String, secretBytes: ByteArray) :
    Algorithm(id, algorithm) {
    private val crypto: CryptoHelper
    private val secret: ByteArray

    //Visible for testing
    init {
        this.secret = secretBytes.copyOf()
        this.crypto = crypto
    }

    constructor(id: String, algorithm: String, secretBytes: ByteArray) : this(
        CryptoHelper(),
        id,
        algorithm,
        secretBytes
    )

    constructor(id: String, algorithm: String, secret: String) : this(
        CryptoHelper(),
        id,
        algorithm,
        getSecretBytes(secret)
    )

    @Throws(SignatureVerificationException::class)
    override fun verify(jwt: DecodedJWT?) {
        if (jwt == null) return
        try {
            val signatureBytes = jwt.signature?.decodeBase64()?.toByteArray() ?: return
            val valid = crypto.verifySignatureFor(
                description, secret, jwt.header!!, jwt.payload!!, signatureBytes
            )
            if (!valid) {
                throw SignatureVerificationException(this)
            }
        } catch (e: Exception) {
            throw SignatureVerificationException(this, e)
        }
    }

    @Throws(SignatureGenerationException::class)
    override fun sign(headerBytes: ByteArray?, payloadBytes: ByteArray?): ByteArray {
        try {
            return crypto.createSignatureFor(description, secret, headerBytes ?: ByteArray(0), payloadBytes ?: ByteArray(0))
        } catch (e: Exception) {
            throw SignatureGenerationException(this, e)
        }
    }

    @Throws(SignatureGenerationException::class)
    override fun sign(contentBytes: ByteArray?): ByteArray {
        try {
            return crypto.createSignatureFor(description, secret, contentBytes ?: ByteArray(0))
        } catch (e: Exception) {
            throw SignatureGenerationException(this, e)
        }
    }

    companion object {
        //Visible for testing
        @Throws(IllegalArgumentException::class)
        fun getSecretBytes(secret: String): ByteArray {
            return secret.encodeUtf8().toByteArray()
        }
    }
}