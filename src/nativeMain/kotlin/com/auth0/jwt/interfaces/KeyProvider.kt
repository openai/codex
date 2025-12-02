package com.auth0.jwt.interfaces


/**
 * Generic Public/Private Key provider.
 * While implementing, ensure the Private Key and Private Key ID doesn't change in between signing a token.
 *
 * @param <U> the class that represents the Public Key
 * @param <R> the class that represents the Private Key
</R></U> */
interface KeyProvider<U : Any?, R : Any?> {
    /**
     * Getter for the Public Key instance with the given Id. Used to verify the signature on the JWT verification stage.
     *
     * @param keyId the Key Id specified in the Token's Header or null if none is available.
     * Provides a hint on which Public Key to use to verify the token's signature.
     * @return the Public Key instance
     */
    fun getPublicKeyById(keyId: String?): U?

    /**
     * Getter for the Private Key instance. Used to sign the content on the JWT signing stage.
     *
     * @return the Private Key instance
     */
    val privateKey: R?

    /**
     * Getter for the Id of the Private Key used to sign the tokens.
     * This represents the `kid` claim and will be placed in the Header.
     *
     * @return the Key Id that identifies the Private Key or null if it's not specified.
     */
    val privateKeyId: String?
}