package com.auth0.jwt

/**
 * Contains constants representing the name of the Registered Claim Names as defined in Section 4.1 of
 * [RFC 7529](https://datatracker.ietf.org/doc/html/rfc7519#section-4.1)
 */
object RegisteredClaims {
    /**
     * The "iss" (issuer) claim identifies the principal that issued the JWT.
     * Refer RFC 7529 [Section 4.1.1](https://datatracker.ietf.org/doc/html/rfc7519#section-4.1.1)
     */
    val ISSUER: String = "iss"

    /**
     * The "sub" (subject) claim identifies the principal that is the subject of the JWT.
     * Refer RFC 7529 [Section 4.1.2](https://datatracker.ietf.org/doc/html/rfc7519#section-4.1.2)
     */
    val SUBJECT: String = "sub"

    /**
     * The "aud" (audience) claim identifies the recipients that the JWT is intended for.
     * Refer RFC 7529 [Section 4.1.3](https://datatracker.ietf.org/doc/html/rfc7519#section-4.1.3)
     */
    val AUDIENCE: String = "aud"

    /**
     * The "exp" (expiration time) claim identifies the expiration time on or after which the JWT MUST NOT be
     * accepted for processing.
     * Refer RFC 7529 [Section 4.1.4](https://datatracker.ietf.org/doc/html/rfc7519#section-4.1.4)
     */
    val EXPIRES_AT: String = "exp"

    /**
     * The "nbf" (not before) claim identifies the time before which the JWT MUST NOT be accepted for processing.
     * Refer RFC 7529 [Section 4.1.5](https://datatracker.ietf.org/doc/html/rfc7519#section-4.1.5)
     */
    val NOT_BEFORE: String = "nbf"

    /**
     * The "iat" (issued at) claim identifies the time at which the JWT was issued.
     * Refer RFC 7529 [Section 4.1.6](https://datatracker.ietf.org/doc/html/rfc7519#section-4.1.6)
     */
    val ISSUED_AT: String = "iat"

    /**
     * The "jti" (JWT ID) claim provides a unique identifier for the JWT.
     * Refer RFC 7529 [Section 4.1.7](https://datatracker.ietf.org/doc/html/rfc7519#section-4.1.7)
     */
    val JWT_ID: String = "jti"
}