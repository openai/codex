package com.auth0.jwt.algorithms

import com.auth0.jwt.exceptions.SignatureGenerationException
import com.auth0.jwt.exceptions.SignatureVerificationException
import com.auth0.jwt.interfaces.DecodedJWT
import com.auth0.jwt.interfaces.RSAKeyProvider

/**
 * Subclass representing an RSA signing algorithm
 *
 * This class is thread-safe.
 */
internal class RSAAlgorithm(crypto: CryptoHelper, id: String?, algorithm: String?, keyProvider: RSAKeyProvider) :
    Algorithm(id ?: "RSA256", algorithm ?: "RSA256") {
    private val keyProvider: RSAKeyProvider
    private val crypto: CryptoHelper

    init {
        this.keyProvider = keyProvider
        this.crypto = crypto
    }

    constructor(id: String?, algorithm: String?, keyProvider: RSAKeyProvider) : this(
        CryptoHelper(),
        id,
        algorithm,
        keyProvider
    )

    @Throws(SignatureVerificationException::class)
    override fun verify(jwt: DecodedJWT?) {
        throw SignatureVerificationException(this)
    }

    @Throws(SignatureGenerationException::class)
    override fun sign(headerBytes: ByteArray?, payloadBytes: ByteArray?): ByteArray {
        throw SignatureGenerationException(this, null)
    }

    @Throws(SignatureGenerationException::class)
    override fun sign(contentBytes: ByteArray?): ByteArray {
        throw SignatureGenerationException(this, null)
    }

    override val signingKeyId: String?
        get() = keyProvider.privateKeyId

    companion object {
        fun providerForKeys(publicKey: Any?, privateKey: Any?): RSAKeyProvider {
            require(!(publicKey == null && privateKey == null)) { "Both provided Keys cannot be null." }
            return object : RSAKeyProvider {
                override fun getPublicKeyById(keyId: String?): Any? {
                    return publicKey
                }

                override val privateKey: Any?
                    get() = privateKey

                override val privateKeyId: String?
                    get() = null
            }
        }
    }
}