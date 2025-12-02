package com.auth0.jwt.algorithms

import com.auth0.jwt.exceptions.SignatureGenerationException
import com.auth0.jwt.exceptions.SignatureVerificationException
import com.auth0.jwt.interfaces.DecodedJWT
import com.auth0.jwt.interfaces.ECDSAKeyProvider

internal class ECDSAAlgorithm(crypto: CryptoHelper, id: String?, algorithm: String?, keyProvider: ECDSAKeyProvider) :
    Algorithm(id ?: "ES256", algorithm ?: "ES256") {
    private val keyProvider: ECDSAKeyProvider
    private val crypto: CryptoHelper

    init {
        this.keyProvider = keyProvider
        this.crypto = crypto
    }

    constructor(id: String?, algorithm: String?, keyProvider: ECDSAKeyProvider) : this(
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
        fun providerForKeys(publicKey: Any?, privateKey: Any?): ECDSAKeyProvider {
            require(!(publicKey == null && privateKey == null)) { "Both provided Keys cannot be null." }
            return object : ECDSAKeyProvider {
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