package com.auth0.jwt.interfaces

/**
 * The Header class represents the 1st part of the JWT, where the Header value is held.
 */
interface Header {
    /**
     * Getter for the Algorithm "alg" claim defined in the JWT's Header. If the claim is missing it will return null.
     *
     * @return the Algorithm defined or null.
     */
    val algorithm: String?

    /**
     * Getter for the Type "typ" claim defined in the JWT's Header. If the claim is missing it will return null.
     *
     * @return the Type defined or null.
     */
    val type: String?

    /**
     * Getter for the Content Type "cty" claim defined in the JWT's Header. If the claim is missing it will return null.
     *
     * @return the Content Type defined or null.
     */
    val contentType: String?

    /**
     * Get the value of the "kid" claim, or null if it's not available.
     *
     * @return the Key ID value or null.
     */
    val keyId: String?

    /**
     * Get a Private Claim given it's name. If the Claim wasn't specified in the Header, a 'null claim' will be
     * returned. All the methods of that claim will return `null`.
     *
     * @param name the name of the Claim to retrieve.
     * @return a non-null Claim.
     */
    fun getHeaderClaim(name: String): Claim
}