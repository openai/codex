@file:OptIn(kotlin.time.ExperimentalTime::class)

package com.auth0.jwt.exceptions

import kotlin.time.Instant

/**
 * The exception that is thrown if the token is expired.
 */
class TokenExpiredException(message: String?, expiredOn: Instant?) : JWTVerificationException(message) {
    private val expiredOn: Instant?

    init {
        this.expiredOn = expiredOn
    }

    fun getExpiredOn(): Instant? {
        return expiredOn
    }

    companion object {
        private val serialVersionUID = -7076928975713577708L
    }
}