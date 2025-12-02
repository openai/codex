package com.auth0.jwt.exceptions

import com.auth0.jwt.interfaces.Claim

/**
 * This exception is thrown when the expected value is not found while verifying the Claims.
 */
class IncorrectClaimException(
    message: String?,
    /**
     * This method can be used to fetch the name for which the Claim verification failed.
     *
     * @return The claim name for which the verification failed.
     */
    val claimName: String?, claim: Claim?
) : InvalidClaimException(message) {
    private val claimValue: Claim?

    /**
     * Used internally to construct the IncorrectClaimException which is thrown when there is verification
     * failure for a Claim that exists.
     *
     * @param message The error message
     * @param claimName The Claim name for which verification failed
     * @param claim The Claim value for which verification failed
     */
    init {
        this.claimValue = claim
    }

    /**
     * This method can be used to fetch the value for which the Claim verification failed.
     *
     * @return The value for which the verification failed
     */
    fun getClaimValue(): Claim? {
        return claimValue
    }
}