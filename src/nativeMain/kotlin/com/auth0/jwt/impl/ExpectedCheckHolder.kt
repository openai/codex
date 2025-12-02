package com.auth0.jwt.impl

import com.auth0.jwt.interfaces.Claim
import com.auth0.jwt.interfaces.DecodedJWT

/**
 * This holds the checks that are run to verify a JWT.
 */
interface ExpectedCheckHolder {
    /**
     * The claim name that will be checked.
     *
     * @return the claim name
     */
    val claimName: String

    /**
     * The verification that will be run.
     *
     * @param claim the claim for which verification is done
     * @param decodedJWT the JWT on which verification is done
     * @return whether the verification passed or not
     */
    fun verify(claim: Claim, decodedJWT: DecodedJWT): Boolean
}