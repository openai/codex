package com.auth0.jwt.algorithms

import com.auth0.jwt.exceptions.SignatureGenerationException
import com.auth0.jwt.interfaces.DecodedJWT

internal class NoneAlgorithm : Algorithm("none", "none") {
    override fun verify(jwt: DecodedJWT?) {
        // No signature to verify
    }

    @Throws(SignatureGenerationException::class)
    override fun sign(headerBytes: ByteArray?, payloadBytes: ByteArray?): ByteArray {
        return ByteArray(0)
    }

    @Throws(SignatureGenerationException::class)
    override fun sign(contentBytes: ByteArray?): ByteArray {
        return ByteArray(0)
    }
}