@file:OptIn(kotlin.time.ExperimentalTime::class)

package com.auth0.jwt.interfaces

import com.auth0.jwt.JWTVerifier
import kotlin.time.Instant


/**
 * Constructs and holds the checks required for a JWT to be considered valid. Note that implementations are
 * **not** thread-safe. Once built by calling [.build], the resulting
 * [com.auth0.jwt.interfaces.JWTVerifier] is thread-safe.
 */
interface Verification {
    /**
     * Verifies whether the JWT contains an Issuer ("iss") claim that equals to the value provided.
     * This check is case-sensitive.
     *
     * @param issuer the required Issuer value.
     * @return this same Verification instance.
     */
    fun withIssuer(issuer: String?): Verification {
        return withIssuer(*arrayOf<String?>(issuer))
    }

    /**
     * Verifies whether the JWT contains an Issuer ("iss") claim that contains all the values provided.
     * This check is case-sensitive. An empty array is considered as a `null`.
     *
     * @param issuer the required Issuer value. If multiple values are given, the claim must at least match one of them
     * @return this same Verification instance.
     */
    fun withIssuer(vararg issuer: String?): Verification

    /**
     * Verifies whether the JWT contains a Subject ("sub") claim that equals to the value provided.
     * This check is case-sensitive.
     *
     * @param subject the required Subject value
     * @return this same Verification instance.
     */
    fun withSubject(subject: String?): Verification

    /**
     * Verifies whether the JWT contains an Audience ("aud") claim that contains all the values provided.
     * This check is case-sensitive. An empty array is considered as a `null`.
     *
     * @param audience the required Audience value
     * @return this same Verification instance.
     */
    fun withAudience(vararg audience: String?): Verification

    /**
     * Verifies whether the JWT contains an Audience ("aud") claim contain at least one of the specified audiences.
     * This check is case-sensitive. An empty array is considered as a `null`.
     *
     * @param audience the required Audience value for which the "aud" claim must contain at least one value.
     * @return this same Verification instance.
     */
    fun withAnyOfAudience(vararg audience: String?): Verification

    /**
     * Define the default window in seconds in which the Not Before, Issued At and Expires At Claims
     * will still be valid. Setting a specific leeway value on a given Claim will override this value for that Claim.
     *
     * @param leeway the window in seconds in which the Not Before, Issued At and Expires At Claims will still be valid.
     * @return this same Verification instance.
     * @throws IllegalArgumentException if leeway is negative.
     */
    @Throws(IllegalArgumentException::class)
    fun acceptLeeway(leeway: Long): Verification

    /**
     * Set a specific leeway window in seconds in which the Expires At ("exp") Claim will still be valid.
     * Expiration Date is always verified when the value is present.
     * This method overrides the value set with acceptLeeway
     *
     * @param leeway the window in seconds in which the Expires At Claim will still be valid.
     * @return this same Verification instance.
     * @throws IllegalArgumentException if leeway is negative.
     */
    @Throws(IllegalArgumentException::class)
    fun acceptExpiresAt(leeway: Long): Verification

    /**
     * Set a specific leeway window in seconds in which the Not Before ("nbf") Claim will still be valid.
     * Not Before Date is always verified when the value is present.
     * This method overrides the value set with acceptLeeway
     *
     * @param leeway the window in seconds in which the Not Before Claim will still be valid.
     * @return this same Verification instance.
     * @throws IllegalArgumentException if leeway is negative.
     */
    @Throws(IllegalArgumentException::class)
    fun acceptNotBefore(leeway: Long): Verification

    /**
     * Set a specific leeway window in seconds in which the Issued At ("iat") Claim will still be valid.
     * This method overrides the value set with [.acceptLeeway].
     * By default, the Issued At claim is always verified when the value is present,
     * unless disabled with [.ignoreIssuedAt].
     * If Issued At verification has been disabled, no verification of the Issued At claim will be performed,
     * and this method has no effect.
     *
     * @param leeway the window in seconds in which the Issued At Claim will still be valid.
     * @return this same Verification instance.
     * @throws IllegalArgumentException if leeway is negative.
     */
    @Throws(IllegalArgumentException::class)
    fun acceptIssuedAt(leeway: Long): Verification

    /**
     * Verifies whether the JWT contains a JWT ID ("jti") claim that equals to the value provided.
     * This check is case-sensitive.
     *
     * @param jwtId the required ID value
     * @return this same Verification instance.
     */
    fun withJWTId(jwtId: String?): Verification

    /**
     * Verifies whether the claim is present in the JWT, with any value including `null`.
     *
     * @param name the Claim's name.
     * @return this same Verification instance
     * @throws IllegalArgumentException if the name is `null`.
     */
    @Throws(IllegalArgumentException::class)
    fun withClaimPresence(name: String?): Verification

    /**
     * Verifies whether the claim is present with a `null` value.
     *
     * @param name the Claim's name.
     * @return this same Verification instance.
     * @throws IllegalArgumentException if the name is `null`.
     */
    @Throws(IllegalArgumentException::class)
    fun withNullClaim(name: String?): Verification

    /**
     * Verifies whether the claim is equal to the given Boolean value.
     *
     * @param name  the Claim's name.
     * @param value the Claim's value.
     * @return this same Verification instance.
     * @throws IllegalArgumentException if the name is `null`.
     */
    @Throws(IllegalArgumentException::class)
    fun withClaim(name: String?, value: Boolean?): Verification

    /**
     * Verifies whether the claim is equal to the given Integer value.
     *
     * @param name  the Claim's name.
     * @param value the Claim's value.
     * @return this same Verification instance.
     * @throws IllegalArgumentException if the name is `null`.
     */
    @Throws(IllegalArgumentException::class)
    fun withClaim(name: String?, value: Int?): Verification

    /**
     * Verifies whether the claim is equal to the given Long value.
     *
     * @param name  the Claim's name.
     * @param value the Claim's value.
     * @return this same Verification instance.
     * @throws IllegalArgumentException if the name is `null`.
     */
    @Throws(IllegalArgumentException::class)
    fun withClaim(name: String?, value: Long?): Verification

    /**
     * Verifies whether the claim is equal to the given Double value.
     *
     * @param name  the Claim's name.
     * @param value the Claim's value.
     * @return this same Verification instance.
     * @throws IllegalArgumentException if the name is `null`.
     */
    @Throws(IllegalArgumentException::class)
    fun withClaim(name: String?, value: Double?): Verification

    /**
     * Verifies whether the claim is equal to the given String value.
     * This check is case-sensitive.
     *
     * @param name  the Claim's name.
     * @param value the Claim's value.
     * @return this same Verification instance.
     * @throws IllegalArgumentException if the name is `null`.
     */
    @Throws(IllegalArgumentException::class)
    fun withClaim(name: String?, value: String?): Verification

    /**
     * Verifies whether the claim is equal to the given Instant value.
     * Note that date-time claims are serialized as seconds since the epoch;
     * when verifying a date-time claim value, any time units more granular than seconds will not be considered.
     *
     * @param name  the Claim's name.
     * @param value the Claim's value.
     * @return this same Verification instance.
     * @throws IllegalArgumentException if the name is `null`.
     */
    @Throws(IllegalArgumentException::class)
    fun withClaim(name: String?, value: Instant?): Verification

    /**
     * Verifies whether the claim contain at least the given String items.
     *
     * @param name  the Claim's name.
     * @param items the items the Claim must contain.
     * @return this same Verification instance.
     * @throws IllegalArgumentException if the name is `null`.
     */
    @Throws(IllegalArgumentException::class)
    fun withArrayClaim(name: String?, vararg items: String?): Verification

    /**
     * Verifies whether the claim contain at least the given Integer items.
     *
     * @param name  the Claim's name.
     * @param items the items the Claim must contain.
     * @return this same Verification instance.
     * @throws IllegalArgumentException if the name is `null`.
     */
    @Throws(IllegalArgumentException::class)
    fun withArrayClaim(name: String?, vararg items: Int?): Verification

    /**
     * Verifies whether the claim contain at least the given Long items.
     *
     * @param name  the Claim's name.
     * @param items the items the Claim must contain.
     * @return this same Verification instance.
     * @throws IllegalArgumentException if the name is `null`.
     */
    @Throws(IllegalArgumentException::class)
    fun withArrayClaim(name: String?, vararg items: Long?): Verification

    /**
     * Skip the Issued At ("iat") claim verification. By default, the verification is performed.
     *
     * @return this same Verification instance.
     */
    fun ignoreIssuedAt(): Verification

    /**
     * Creates a new and reusable instance of the JWTVerifier with the configuration already provided.
     *
     * @return a new [com.auth0.jwt.interfaces.JWTVerifier] instance.
     */
    fun build(): JWTVerifier
}