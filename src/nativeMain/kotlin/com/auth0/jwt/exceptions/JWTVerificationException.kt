package com.auth0.jwt.exceptions

/**
 * Parent to all the exception thrown while verifying a JWT.
 */
open class JWTVerificationException(message: String?, cause: Throwable?) : RuntimeException(message, cause) {
    constructor(message: String?) : this(message, null)
}