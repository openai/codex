@file:OptIn(kotlin.time.ExperimentalTime::class)

package com.auth0.jwt

import com.auth0.jwt.algorithms.Algorithm
import com.auth0.jwt.exceptions.*
import com.auth0.jwt.impl.ExpectedCheckHolder
import com.auth0.jwt.impl.JWTParser
import com.auth0.jwt.interfaces.Claim
import com.auth0.jwt.interfaces.DecodedJWT
import com.auth0.jwt.interfaces.Verification
import kotlinx.datetime.Clock
import kotlinx.datetime.Instant
import kotlin.time.Duration
import kotlin.time.Duration.Companion.seconds

/**
 * The JWTVerifier class holds the verify method to assert that a given Token has not only a proper JWT format,
 * but also its signature matches.
 *
 *
 * This class is thread-safe.
 *
 * @see com.auth0.jwt.interfaces.JWTVerifier
 */
class JWTVerifier internal constructor(
    private val algorithm: Algorithm,
    expectedChecks: List<ExpectedCheckHolder>?
) : com.auth0.jwt.interfaces.JWTVerifier {
    
    val expectedChecks: List<ExpectedCheckHolder> = expectedChecks ?: emptyList()
    private val parser: JWTParser = JWTParser()

    /**
     * [Verification] implementation that accepts all the expected Claim values for verification, and
     * builds a [com.auth0.jwt.interfaces.JWTVerifier] used to verify a JWT's signature and expected claims.
     *
     * Note that this class is **not** thread-safe. Calling [.build] returns an instance of
     * [com.auth0.jwt.interfaces.JWTVerifier] which can be reused.
     */
    class BaseVerification internal constructor(private val algorithm: Algorithm) : Verification {
        private val expectedChecks: MutableList<ExpectedCheckHolder> = mutableListOf()
        private var defaultLeeway: Long = 0
        private val customLeeways: MutableMap<String, Long> = mutableMapOf()
        private var ignoreIssuedAt = false
        private var clock: Clock = Clock.System

        init {
            // Algorithm check is implicit in constructor
        }

        override fun withIssuer(vararg issuer: String?): Verification {
            val value = issuer.filterNotNull()
            val check = { claim: Claim, _: DecodedJWT ->
                verifyNull(claim, value.ifEmpty { null }) || (value.isNotEmpty() && value.contains(claim.asString()))
            }
            addCheck(RegisteredClaims.ISSUER) { claim, jwt ->
                if (check(claim, jwt)) true else throw IncorrectClaimException(
                    "The Claim 'iss' value doesn't match the required issuer.",
                    RegisteredClaims.ISSUER,
                    claim
                )
            }
            return this
        }

        override fun withSubject(subject: String?): Verification {
            requireNotNull(subject)
            addCheck(RegisteredClaims.SUBJECT) { claim, _ ->
                verifyNull(claim, subject) || subject == claim.asString()
            }
            return this
        }

        override fun withAudience(vararg audience: String?): Verification {
            val value = audience.filterNotNull()
            addCheck(RegisteredClaims.AUDIENCE) { claim, jwt ->
                if (verifyNull(claim, value.ifEmpty { null })) return@addCheck true
                
                if (!assertValidAudienceClaim(jwt.audience, value, true)) {
                    throw IncorrectClaimException(
                        "The Claim 'aud' value doesn't contain the required audience.",
                        RegisteredClaims.AUDIENCE, claim
                    )
                }
                true
            }
            return this
        }

        override fun withAnyOfAudience(vararg audience: String?): Verification {
            val value = audience.filterNotNull()
            addCheck(RegisteredClaims.AUDIENCE) { claim, jwt ->
                if (verifyNull(claim, value.ifEmpty { null })) return@addCheck true
                
                if (!assertValidAudienceClaim(jwt.audience, value, false)) {
                    throw IncorrectClaimException(
                        "The Claim 'aud' value doesn't contain the required audience.",
                        RegisteredClaims.AUDIENCE, claim
                    )
                }
                true
            }
            return this
        }

        override fun acceptLeeway(leeway: Long): Verification {
            assertPositive(leeway)
            this.defaultLeeway = leeway
            return this
        }

        override fun acceptExpiresAt(leeway: Long): Verification {
            assertPositive(leeway)
            customLeeways[RegisteredClaims.EXPIRES_AT] = leeway
            return this
        }

        override fun acceptNotBefore(leeway: Long): Verification {
            assertPositive(leeway)
            customLeeways[RegisteredClaims.NOT_BEFORE] = leeway
            return this
        }

        override fun acceptIssuedAt(leeway: Long): Verification {
            assertPositive(leeway)
            customLeeways[RegisteredClaims.ISSUED_AT] = leeway
            return this
        }

        override fun ignoreIssuedAt(): Verification {
            this.ignoreIssuedAt = true
            return this
        }

        override fun withJWTId(jwtId: String?): Verification {
            requireNotNull(jwtId)
            addCheck(RegisteredClaims.JWT_ID) { claim, _ ->
                verifyNull(claim, jwtId) || jwtId == claim.asString()
            }
            return this
        }

        override fun withClaimPresence(name: String?): Verification {
            requireNotNull(name)
            withClaim(name) { _, _ -> true }
            return this
        }

        override fun withNullClaim(name: String?): Verification {
            requireNotNull(name)
            withClaim(name) { claim, _ -> claim.isNull() }
            return this
        }

        override fun withClaim(name: String?, value: Boolean?): Verification {
            requireNotNull(name)
            requireNotNull(value)
            addCheck(name) { claim, _ ->
                verifyNull(claim, value) || value == claim.asBoolean()
            }
            return this
        }

        override fun withClaim(name: String?, value: Int?): Verification {
            requireNotNull(name)
            requireNotNull(value)
            addCheck(name) { claim, _ ->
                verifyNull(claim, value) || value == claim.asInt()
            }
            return this
        }

        override fun withClaim(name: String?, value: Long?): Verification {
            requireNotNull(name)
            requireNotNull(value)
            addCheck(name) { claim, _ ->
                verifyNull(claim, value) || value == claim.asLong()
            }
            return this
        }

        override fun withClaim(name: String?, value: Double?): Verification {
            requireNotNull(name)
            requireNotNull(value)
            addCheck(name) { claim, _ ->
                verifyNull(claim, value) || value == claim.asDouble()
            }
            return this
        }

        override fun withClaim(name: String?, value: String?): Verification {
            requireNotNull(name)
            requireNotNull(value)
            addCheck(name) { claim, _ ->
                verifyNull(claim, value) || value == claim.asString()
            }
            return this
        }

        override fun withClaim(name: String?, value: Instant?): Verification {
            requireNotNull(name)
            requireNotNull(value)
            addCheck(name) { claim, _ ->
                verifyNull(claim, value) || value.epochSeconds == claim.asDate()?.epochSeconds
            }
            return this
        }

        // Helper for functional interface simulation
        fun withClaim(name: String?, predicate: (Claim, DecodedJWT) -> Boolean): Verification {
            requireNotNull(name)
            addCheck(name) { claim, jwt ->
                verifyNull(claim, predicate) || predicate(claim, jwt)
            }
            return this
        }

        override fun withArrayClaim(name: String?, vararg items: String?): Verification {
            requireNotNull(name)
            val nonNullItems = items.filterNotNull()
            addCheck(name) { claim, _ ->
                verifyNull(claim, nonNullItems.ifEmpty { null }) || assertValidCollectionClaim(claim, nonNullItems)
            }
            return this
        }

        override fun withArrayClaim(name: String?, vararg items: Int?): Verification {
            requireNotNull(name)
            val nonNullItems = items.filterNotNull()
            addCheck(name) { claim, _ ->
                verifyNull(claim, nonNullItems.ifEmpty { null }) || assertValidCollectionClaim(claim, nonNullItems)
            }
            return this
        }

        override fun withArrayClaim(name: String?, vararg items: Long?): Verification {
            requireNotNull(name)
            val nonNullItems = items.filterNotNull()
            addCheck(name) { claim, _ ->
                verifyNull(claim, nonNullItems.ifEmpty { null }) || assertValidCollectionClaim(claim, nonNullItems)
            }
            return this
        }

        override fun build(): JWTVerifier {
            return build(Clock.System)
        }

        fun build(clock: Clock): JWTVerifier {
            this.clock = clock
            addMandatoryClaimChecks()
            return JWTVerifier(algorithm, expectedChecks)
        }

        private fun getLeewayFor(name: String): Long {
            return customLeeways[name] ?: defaultLeeway
        }

        private fun addMandatoryClaimChecks() {
            val expiresAtLeeway = getLeewayFor(RegisteredClaims.EXPIRES_AT)
            val notBeforeLeeway = getLeewayFor(RegisteredClaims.NOT_BEFORE)
            val issuedAtLeeway = getLeewayFor(RegisteredClaims.ISSUED_AT)

            expectedChecks.add(
                constructExpectedCheck(RegisteredClaims.EXPIRES_AT) { claim, _ ->
                    assertValidInstantClaim(RegisteredClaims.EXPIRES_AT, claim, expiresAtLeeway, true)
                }
            )
            expectedChecks.add(
                constructExpectedCheck(RegisteredClaims.NOT_BEFORE) { claim, _ ->
                    assertValidInstantClaim(RegisteredClaims.NOT_BEFORE, claim, notBeforeLeeway, false)
                }
            )
            if (!ignoreIssuedAt) {
                expectedChecks.add(
                    constructExpectedCheck(RegisteredClaims.ISSUED_AT) { claim, _ ->
                        assertValidInstantClaim(RegisteredClaims.ISSUED_AT, claim, issuedAtLeeway, false)
                    }
                )
            }
        }

        private fun assertValidCollectionClaim(claim: Claim, expectedClaimValue: List<Any>): Boolean {
            val claimList = claim.asList(Any::class) ?: return false
            // Simple check, might need type conversion logic similar to original if types mismatch
            // Assuming strict type matching for now or basic conversion
            return claimList.containsAll(expectedClaimValue)
        }

        private fun assertValidInstantClaim(
            claimName: String,
            claim: Claim,
            leeway: Long,
            shouldBeFuture: Boolean
        ): Boolean {
            val claimVal = claim.asDate()
            val now = clock.now()
            
            if (shouldBeFuture) {
                if (!assertInstantIsFuture(claimVal, leeway, now)) {
                    throw TokenExpiredException("The Token has expired on $claimVal.", claimVal)
                }
            } else {
                if (!assertInstantIsLessThanOrEqualToNow(claimVal, leeway, now)) {
                    throw IncorrectClaimException(
                        "The Token can't be used before $claimVal.", claimName, claim
                    )
                }
            }
            return true
        }

        private fun assertInstantIsFuture(claimVal: Instant?, leeway: Long, now: Instant): Boolean {
            return claimVal == null || (now - leeway.seconds) < claimVal
        }

        private fun assertInstantIsLessThanOrEqualToNow(claimVal: Instant?, leeway: Long, now: Instant): Boolean {
            return !(claimVal != null && (now + leeway.seconds) < claimVal)
        }

        private fun assertValidAudienceClaim(
            actualAudience: List<String>?,
            expectedAudience: List<String>,
            shouldContainAll: Boolean
        ): Boolean {
            if (actualAudience == null) return false
            
            return if (shouldContainAll) {
                actualAudience.containsAll(expectedAudience)
            } else {
                actualAudience.any { expectedAudience.contains(it) }
            }
        }

        private fun assertPositive(leeway: Long) {
            require(leeway >= 0) { "Leeway value can't be negative." }
        }

        private fun addCheck(name: String, predicate: (Claim, DecodedJWT) -> Boolean) {
            expectedChecks.add(constructExpectedCheck(name) { claim, jwt ->
                if (claim.isNull()) {
                    throw MissingClaimException(name)
                }
                predicate(claim, jwt)
            })
        }

        private fun constructExpectedCheck(
            claimName: String,
            check: (Claim, DecodedJWT) -> Boolean
        ): ExpectedCheckHolder {
            return object : ExpectedCheckHolder {
                override val claimName: String = claimName
                override fun verify(claim: Claim, decodedJWT: DecodedJWT): Boolean {
                    return check(claim, decodedJWT)
                }
            }
        }

        private fun verifyNull(claim: Claim, value: Any?): Boolean {
            return value == null && claim.isNull()
        }
    }

    @Throws(JWTVerificationException::class)
    override fun verify(token: String?): DecodedJWT {
        requireNotNull(token) { "Token cannot be null" }
        val jwt = JWTDecoder(parser).decode(token)
        return verify(jwt)
    }

    @Throws(JWTVerificationException::class)
    override fun verify(jwt: DecodedJWT?): DecodedJWT {
        requireNotNull(jwt) { "DecodedJWT cannot be null" }
        verifyAlgorithm(jwt, algorithm)
        algorithm.verify(jwt)
        verifyClaims(jwt, expectedChecks)
        return jwt
    }

    @Throws(AlgorithmMismatchException::class)
    private fun verifyAlgorithm(jwt: DecodedJWT, expectedAlgorithm: Algorithm) {
        if (expectedAlgorithm.name != jwt.algorithm) {
            throw AlgorithmMismatchException(
                "The provided Algorithm doesn't match the one defined in the JWT's Header."
            )
        }
    }

    @Throws(TokenExpiredException::class, InvalidClaimException::class)
    private fun verifyClaims(jwt: DecodedJWT, expectedChecks: List<ExpectedCheckHolder>) {
        for (expectedCheck in expectedChecks) {
            val claimName = expectedCheck.claimName
            val claim = jwt.getClaim(claimName)

            val isValid = expectedCheck.verify(claim, jwt)

            if (!isValid) {
                throw IncorrectClaimException(
                    "The Claim '$claimName' value doesn't match the required one.",
                    claimName,
                    claim
                )
            }
        }
    }

    companion object {
        fun init(algorithm: Algorithm): Verification {
            return BaseVerification(algorithm)
        }
    }
}