package com.auth0.jwt.exceptions

/**
 * The exception that will be thrown if the exception doesn't match the one mentioned in the JWT Header.
 */
class AlgorithmMismatchException(message: String?) : JWTVerificationException(message)