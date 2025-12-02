package com.auth0.jwt

import com.auth0.jwt.exceptions.JWTDecodeException

internal object TokenUtils {
    /**
     * Splits the given token on the "." chars into a String array with 3 parts.
     *
     * @param token the string to split.
     * @return the array representing the 3 parts of the token.
     * @throws JWTDecodeException if the Token doesn't have 3 parts.
     */
    @Throws(JWTDecodeException::class)
    fun splitToken(token: String?): Array<String> {
        if (token == null) {
            throw JWTDecodeException("The token is null.")
        }

        val parts = token.split(".")
        
        if (parts.size != 3) {
             throw JWTDecodeException("The token was expected to have 3 parts, but got ${parts.size}.")
        }

        return parts.toTypedArray()
    }
}