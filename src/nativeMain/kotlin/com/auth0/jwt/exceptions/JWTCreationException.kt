package com.auth0.jwt.exceptions

/**
 * The exception that is thrown when a JWT cannot be created.
 */
open class JWTCreationException(message: String?, cause: Throwable?) : RuntimeException(message, cause)