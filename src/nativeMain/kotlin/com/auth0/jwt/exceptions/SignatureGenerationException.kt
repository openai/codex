package com.auth0.jwt.exceptions

import com.auth0.jwt.algorithms.Algorithm

/**
 * The exception that is thrown when signature is not able to be generated.
 */
class SignatureGenerationException(algorithm: Algorithm?, cause: Throwable?) : JWTCreationException(
    "The Token's Signature couldn't be generated when signing using the Algorithm: " + algorithm,
    cause
)