package com.auth0.jwt.exceptions

/**
 * This exception is thrown when the claim to be verified is missing.
 */
class MissingClaimException(
    /**
     * This method can be used to fetch the name for which the Claim is missing during the verification.
     *
     * @return The name of the Claim that doesn't exist.
     */
    val claimName: String?
) : InvalidClaimException(
    "The Claim '$claimName' is not present in the JWT."
) 