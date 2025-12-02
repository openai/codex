package com.auth0.jwt

import com.auth0.jwt.algorithms.Algorithm
import com.auth0.jwt.exceptions.JWTDecodeException
import com.auth0.jwt.impl.JWTParser
import com.auth0.jwt.interfaces.DecodedJWT
import com.auth0.jwt.interfaces.Verification

/**
 * The JWT class is the main entry point for the library.
 * It provides methods to create, decode and verify JWTs.
 */
object JWT {
    private val decoder =
        _root_ide_package_.com.auth0.jwt.JWTDecoder(_root_ide_package_.com.auth0.jwt.impl.JWTParser())

    /**
     * Decode a given Json Web Token.
     *
     *
     * Note that this method **doesn't verify the token's signature!**
     * Use it only if you trust the token or if you have already verified it.
     *
     * @param token with jwt format as string.
     * @return a decoded JWT.
     * @throws JWTDecodeException if any part of the token contained an invalid jwt
     * or JSON format of each of the jwt parts.
     */
    @Throws(JWTDecodeException::class)
    fun decode(token: String?): DecodedJWT {
        return JWTDecoder(JWTParser()).decode(token!!)
    }

    /**
     * Returns a [Verification] builder with the algorithm to be used to validate token signature.
     *
     * @param algorithm that will be used to verify the token's signature.
     * @return [Verification] builder
     * @throws IllegalArgumentException if the provided algorithm is null.
     */
    fun require(algorithm: Algorithm?): Verification {
        return com.auth0.jwt.JWTVerifier.init(algorithm!!)
    }

    /**
     * Returns a Json Web Token builder used to create and sign tokens.
     *
     * @return a token builder.
     */
    fun create(): JWTCreator.Builder {
        return JWTCreator.init()
    }
}