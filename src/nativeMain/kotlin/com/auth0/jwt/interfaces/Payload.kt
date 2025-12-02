package com.auth0.jwt.interfaces

import kotlin.time.Instant

/**
 * The Payload class represents the 2nd part of the JWT, where the Payload value is held.
 */
interface Payload {
    /**
     * Get the value of the "iss" claim, or null if it's not available.
     *
     * @return the Issuer value or null.
     */
    val issuer: String?

    /**
     * Get the value of the "sub" claim, or null if it's not available.
     *
     * @return the Subject value or null.
     */
    val subject: String?

    /**
     * Get the value of the "aud" claim, or null if it's not available.
     *
     * @return the Audience value or null.
     */
    val audience: List<String>?

    /**
     * Get the value of the "exp" claim, or null if it's not available.
     *
     * @return the Expiration Time value or null.
     */
    val expiresAt: Instant?

    /**
     * Get the value of the "nbf" claim, or null if it's not available.
     *
     * @return the Not Before value or null.
     */
    val notBefore: Instant?

    /**
     * Get the value of the "iat" claim, or null if it's not available.
     *
     * @return the Issued At value or null.
     */
    val issuedAt: Instant?

    /**
     * Get the value of the "jti" claim, or null if it's not available.
     *
     * @return the JWT ID value or null.
     */
    val id: String?

    /**
     * Get a Claim given its name. If the Claim wasn't specified in the Payload, a 'null claim'
     * will be returned. All the methods of that claim will return `null`.
     *
     * @param name the name of the Claim to retrieve.
     * @return a non-null Claim.
     */
    fun getClaim(name: String): Claim

    /**
     * Get the Claims defined in the Token.
     *
     * @return a non-null Map containing the Claims defined in the Token.
     */
    val claims: Map<String, Claim>
}