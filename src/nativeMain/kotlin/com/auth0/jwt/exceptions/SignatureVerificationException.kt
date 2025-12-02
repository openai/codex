package com.auth0.jwt.exceptions

import com.auth0.jwt.algorithms.Algorithm

/**
 * The exception that is thrown if the Signature verification fails.
 */
class SignatureVerificationException(algorithm: Algorithm?, cause: Throwable?) : JWTVerificationException(
    "The Token's Signature resulted invalid when verified using the Algorithm: " + algorithm,
    cause
) {
    constructor(algorithm: Algorithm?) : this(algorithm, null)
}