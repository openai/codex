package com.auth0.jwt.exceptions

/**
 * The exception that is thrown when any part of the token contained an invalid JWT or JSON format.
 */
class JWTDecodeException(message: String?, cause: Throwable?) : JWTVerificationException(message, cause) {
    constructor(message: String?) : this(message, null)
}