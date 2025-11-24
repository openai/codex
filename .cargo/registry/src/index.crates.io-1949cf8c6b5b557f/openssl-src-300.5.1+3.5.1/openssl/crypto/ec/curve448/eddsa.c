/*
 * Copyright 2017-2024 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright 2015-2016 Cryptography Research, Inc.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 *
 * Originally written by Mike Hamburg
 */
#include <string.h>
#include <openssl/crypto.h>
#include <openssl/evp.h>
#include "crypto/ecx.h"
#include "curve448_local.h"
#include "word.h"
#include "ed448.h"
#include "internal/numbers.h"

#define COFACTOR 4

static c448_error_t oneshot_hash(OSSL_LIB_CTX *ctx, uint8_t *out, size_t outlen,
                                 const uint8_t *in, size_t inlen,
                                 const char *propq)
{
    EVP_MD_CTX *hashctx = EVP_MD_CTX_new();
    EVP_MD *shake256 = NULL;
    c448_error_t ret = C448_FAILURE;

    if (hashctx == NULL)
        return C448_FAILURE;

    shake256 = EVP_MD_fetch(ctx, "SHAKE256", propq);
    if (shake256 == NULL)
        goto err;

    if (!EVP_DigestInit_ex(hashctx, shake256, NULL)
            || !EVP_DigestUpdate(hashctx, in, inlen)
            || !EVP_DigestFinalXOF(hashctx, out, outlen))
        goto err;

    ret = C448_SUCCESS;
 err:
    EVP_MD_CTX_free(hashctx);
    EVP_MD_free(shake256);
    return ret;
}

static void clamp(uint8_t secret_scalar_ser[EDDSA_448_PRIVATE_BYTES])
{
    secret_scalar_ser[0] &= -COFACTOR;
    secret_scalar_ser[EDDSA_448_PRIVATE_BYTES - 1] = 0;
    secret_scalar_ser[EDDSA_448_PRIVATE_BYTES - 2] |= 0x80;
}

static c448_error_t hash_init_with_dom(OSSL_LIB_CTX *ctx, EVP_MD_CTX *hashctx,
                                       uint8_t prehashed,
                                       uint8_t for_prehash,
                                       const uint8_t *context,
                                       size_t context_len,
                                       const char *propq)
{
    /* ASCII: "SigEd448", in hex for EBCDIC compatibility */
    const char dom_s[] = "\x53\x69\x67\x45\x64\x34\x34\x38";
    uint8_t dom[2];
    EVP_MD *shake256 = NULL;

    if (context_len > UINT8_MAX)
        return C448_FAILURE;

    dom[0] = (uint8_t)(2 - (prehashed == 0 ? 1 : 0)
                       - (for_prehash == 0 ? 1 : 0));
    dom[1] = (uint8_t)context_len;

    shake256 = EVP_MD_fetch(ctx, "SHAKE256", propq);
    if (shake256 == NULL)
        return C448_FAILURE;

    if (!EVP_DigestInit_ex(hashctx, shake256, NULL)
            || !EVP_DigestUpdate(hashctx, dom_s, sizeof(dom_s)-1)
            || !EVP_DigestUpdate(hashctx, dom, sizeof(dom))
            || !EVP_DigestUpdate(hashctx, context, context_len)) {
        EVP_MD_free(shake256);
        return C448_FAILURE;
    }

    EVP_MD_free(shake256);
    return C448_SUCCESS;
}

/* In this file because it uses the hash */
c448_error_t
ossl_c448_ed448_convert_private_key_to_x448(
                            OSSL_LIB_CTX *ctx,
                            uint8_t x[X448_PRIVATE_BYTES],
                            const uint8_t ed [EDDSA_448_PRIVATE_BYTES],
                            const char *propq)
{
    /* pass the private key through oneshot_hash function */
    /* and keep the first X448_PRIVATE_BYTES bytes */
    return oneshot_hash(ctx, x, X448_PRIVATE_BYTES, ed,
                        EDDSA_448_PRIVATE_BYTES, propq);
}

c448_error_t
ossl_c448_ed448_derive_public_key(
                        OSSL_LIB_CTX *ctx,
                        uint8_t pubkey[EDDSA_448_PUBLIC_BYTES],
                        const uint8_t privkey[EDDSA_448_PRIVATE_BYTES],
                        const char *propq)
{
    /* only this much used for keygen */
    uint8_t secret_scalar_ser[EDDSA_448_PRIVATE_BYTES];
    curve448_scalar_t secret_scalar;
    unsigned int c;
    curve448_point_t p;

    if (!oneshot_hash(ctx, secret_scalar_ser, sizeof(secret_scalar_ser),
                      privkey,
                      EDDSA_448_PRIVATE_BYTES,
                      propq))
        return C448_FAILURE;

    clamp(secret_scalar_ser);

    ossl_curve448_scalar_decode_long(secret_scalar, secret_scalar_ser,
                                     sizeof(secret_scalar_ser));

    /*
     * Since we are going to mul_by_cofactor during encoding, divide by it
     * here. However, the EdDSA base point is not the same as the decaf base
     * point if the sigma isogeny is in use: the EdDSA base point is on
     * Etwist_d/(1-d) and the decaf base point is on Etwist_d, and when
     * converted it effectively picks up a factor of 2 from the isogenies.  So
     * we might start at 2 instead of 1.
     */
    for (c = 1; c < C448_EDDSA_ENCODE_RATIO; c <<= 1)
        ossl_curve448_scalar_halve(secret_scalar, secret_scalar);

    ossl_curve448_precomputed_scalarmul(p, ossl_curve448_precomputed_base,
                                        secret_scalar);

    ossl_curve448_point_mul_by_ratio_and_encode_like_eddsa(pubkey, p);

    /* Cleanup */
    ossl_curve448_scalar_destroy(secret_scalar);
    ossl_curve448_point_destroy(p);
    OPENSSL_cleanse(secret_scalar_ser, sizeof(secret_scalar_ser));

    return C448_SUCCESS;
}

c448_error_t
ossl_c448_ed448_sign(OSSL_LIB_CTX *ctx,
                     uint8_t signature[EDDSA_448_SIGNATURE_BYTES],
                     const uint8_t privkey[EDDSA_448_PRIVATE_BYTES],
                     const uint8_t pubkey[EDDSA_448_PUBLIC_BYTES],
                     const uint8_t *message, size_t message_len,
                     uint8_t prehashed, const uint8_t *context,
                     size_t context_len, const char *propq)
{
    curve448_scalar_t secret_scalar;
    EVP_MD_CTX *hashctx = EVP_MD_CTX_new();
    c448_error_t ret = C448_FAILURE;
    curve448_scalar_t nonce_scalar;
    uint8_t nonce_point[EDDSA_448_PUBLIC_BYTES] = { 0 };
    unsigned int c;
    curve448_scalar_t challenge_scalar;

    if (hashctx == NULL)
        return C448_FAILURE;

    {
        /*
         * Schedule the secret key, First EDDSA_448_PRIVATE_BYTES is serialized
         * secret scalar,next EDDSA_448_PRIVATE_BYTES bytes is the seed.
         */
        uint8_t expanded[EDDSA_448_PRIVATE_BYTES * 2];

        if (!oneshot_hash(ctx, expanded, sizeof(expanded), privkey,
                          EDDSA_448_PRIVATE_BYTES, propq))
            goto err;
        clamp(expanded);
        ossl_curve448_scalar_decode_long(secret_scalar, expanded,
                                         EDDSA_448_PRIVATE_BYTES);

        /* Hash to create the nonce */
        if (!hash_init_with_dom(ctx, hashctx, prehashed, 0, context,
                                context_len, propq)
                || !EVP_DigestUpdate(hashctx,
                                     expanded + EDDSA_448_PRIVATE_BYTES,
                                     EDDSA_448_PRIVATE_BYTES)
                || !EVP_DigestUpdate(hashctx, message, message_len)) {
            OPENSSL_cleanse(expanded, sizeof(expanded));
            goto err;
        }
        OPENSSL_cleanse(expanded, sizeof(expanded));
    }

    /* Decode the nonce */
    {
        uint8_t nonce[2 * EDDSA_448_PRIVATE_BYTES];

        if (!EVP_DigestFinalXOF(hashctx, nonce, sizeof(nonce)))
            goto err;
        ossl_curve448_scalar_decode_long(nonce_scalar, nonce, sizeof(nonce));
        OPENSSL_cleanse(nonce, sizeof(nonce));
    }

    {
        /* Scalarmul to create the nonce-point */
        curve448_scalar_t nonce_scalar_2;
        curve448_point_t p;

        ossl_curve448_scalar_halve(nonce_scalar_2, nonce_scalar);
        for (c = 2; c < C448_EDDSA_ENCODE_RATIO; c <<= 1)
            ossl_curve448_scalar_halve(nonce_scalar_2, nonce_scalar_2);

        ossl_curve448_precomputed_scalarmul(p, ossl_curve448_precomputed_base,
                                            nonce_scalar_2);
        ossl_curve448_point_mul_by_ratio_and_encode_like_eddsa(nonce_point, p);
        ossl_curve448_point_destroy(p);
        ossl_curve448_scalar_destroy(nonce_scalar_2);
    }

    {
        uint8_t challenge[2 * EDDSA_448_PRIVATE_BYTES];

        /* Compute the challenge */
        if (!hash_init_with_dom(ctx, hashctx, prehashed, 0, context, context_len,
                                propq)
                || !EVP_DigestUpdate(hashctx, nonce_point, sizeof(nonce_point))
                || !EVP_DigestUpdate(hashctx, pubkey, EDDSA_448_PUBLIC_BYTES)
                || !EVP_DigestUpdate(hashctx, message, message_len)
                || !EVP_DigestFinalXOF(hashctx, challenge, sizeof(challenge)))
            goto err;

        ossl_curve448_scalar_decode_long(challenge_scalar, challenge,
                                         sizeof(challenge));
        OPENSSL_cleanse(challenge, sizeof(challenge));
    }

    ossl_curve448_scalar_mul(challenge_scalar, challenge_scalar, secret_scalar);
    ossl_curve448_scalar_add(challenge_scalar, challenge_scalar, nonce_scalar);

    OPENSSL_cleanse(signature, EDDSA_448_SIGNATURE_BYTES);
    memcpy(signature, nonce_point, sizeof(nonce_point));
    ossl_curve448_scalar_encode(&signature[EDDSA_448_PUBLIC_BYTES],
                                challenge_scalar);

    ossl_curve448_scalar_destroy(secret_scalar);
    ossl_curve448_scalar_destroy(nonce_scalar);
    ossl_curve448_scalar_destroy(challenge_scalar);

    ret = C448_SUCCESS;
 err:
    EVP_MD_CTX_free(hashctx);
    return ret;
}

c448_error_t
ossl_c448_ed448_sign_prehash(
                        OSSL_LIB_CTX *ctx,
                        uint8_t signature[EDDSA_448_SIGNATURE_BYTES],
                        const uint8_t privkey[EDDSA_448_PRIVATE_BYTES],
                        const uint8_t pubkey[EDDSA_448_PUBLIC_BYTES],
                        const uint8_t hash[64], const uint8_t *context,
                        size_t context_len, const char *propq)
{
    return ossl_c448_ed448_sign(ctx, signature, privkey, pubkey, hash, 64, 1,
                                context, context_len, propq);
}

static c448_error_t
c448_ed448_pubkey_verify(const uint8_t *pub, size_t pub_len)
{
    curve448_point_t pk_point;

    if (pub_len != EDDSA_448_PUBLIC_BYTES)
        return C448_FAILURE;

    return ossl_curve448_point_decode_like_eddsa_and_mul_by_ratio(pk_point, pub);
}

c448_error_t
ossl_c448_ed448_verify(
                    OSSL_LIB_CTX *ctx,
                    const uint8_t signature[EDDSA_448_SIGNATURE_BYTES],
                    const uint8_t pubkey[EDDSA_448_PUBLIC_BYTES],
                    const uint8_t *message, size_t message_len,
                    uint8_t prehashed, const uint8_t *context,
                    uint8_t context_len, const char *propq)
{
    curve448_point_t pk_point, r_point;
    c448_error_t error;
    curve448_scalar_t challenge_scalar;
    curve448_scalar_t response_scalar;
    /* Order in little endian format */
    static const uint8_t order[] = {
        0xF3, 0x44, 0x58, 0xAB, 0x92, 0xC2, 0x78, 0x23, 0x55, 0x8F, 0xC5, 0x8D,
        0x72, 0xC2, 0x6C, 0x21, 0x90, 0x36, 0xD6, 0xAE, 0x49, 0xDB, 0x4E, 0xC4,
        0xE9, 0x23, 0xCA, 0x7C, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x3F, 0x00
    };
    int i;

    /*
     * Check that s (second 57 bytes of the sig) is less than the order. Both
     * s and the order are in little-endian format. This can be done in
     * variable time, since if this is not the case the signature if publicly
     * invalid.
     */
    for (i = EDDSA_448_PUBLIC_BYTES - 1; i >= 0; i--) {
        if (signature[i + EDDSA_448_PUBLIC_BYTES] > order[i])
            return C448_FAILURE;
        if (signature[i + EDDSA_448_PUBLIC_BYTES] < order[i])
            break;
    }
    if (i < 0)
        return C448_FAILURE;

    error =
        ossl_curve448_point_decode_like_eddsa_and_mul_by_ratio(pk_point, pubkey);

    if (C448_SUCCESS != error)
        return error;

    error =
        ossl_curve448_point_decode_like_eddsa_and_mul_by_ratio(r_point, signature);
    if (C448_SUCCESS != error)
        return error;

    {
        /* Compute the challenge */
        EVP_MD_CTX *hashctx = EVP_MD_CTX_new();
        uint8_t challenge[2 * EDDSA_448_PRIVATE_BYTES];

        if (hashctx == NULL
                || !hash_init_with_dom(ctx, hashctx, prehashed, 0, context,
                                       context_len, propq)
                || !EVP_DigestUpdate(hashctx, signature, EDDSA_448_PUBLIC_BYTES)
                || !EVP_DigestUpdate(hashctx, pubkey, EDDSA_448_PUBLIC_BYTES)
                || !EVP_DigestUpdate(hashctx, message, message_len)
                || !EVP_DigestFinalXOF(hashctx, challenge, sizeof(challenge))) {
            EVP_MD_CTX_free(hashctx);
            return C448_FAILURE;
        }

        EVP_MD_CTX_free(hashctx);
        ossl_curve448_scalar_decode_long(challenge_scalar, challenge,
                                         sizeof(challenge));
        OPENSSL_cleanse(challenge, sizeof(challenge));
    }
    ossl_curve448_scalar_sub(challenge_scalar, ossl_curve448_scalar_zero,
                             challenge_scalar);

    ossl_curve448_scalar_decode_long(response_scalar,
                                     &signature[EDDSA_448_PUBLIC_BYTES],
                                     EDDSA_448_PRIVATE_BYTES);

    /* pk_point = -c(x(P)) + (cx + k)G = kG */
    ossl_curve448_base_double_scalarmul_non_secret(pk_point,
                                                   response_scalar,
                                                   pk_point, challenge_scalar);
    return c448_succeed_if(ossl_curve448_point_eq(pk_point, r_point));
}

c448_error_t
ossl_c448_ed448_verify_prehash(
                    OSSL_LIB_CTX *ctx,
                    const uint8_t signature[EDDSA_448_SIGNATURE_BYTES],
                    const uint8_t pubkey[EDDSA_448_PUBLIC_BYTES],
                    const uint8_t hash[64], const uint8_t *context,
                    uint8_t context_len, const char *propq)
{
    return ossl_c448_ed448_verify(ctx, signature, pubkey, hash, 64, 1, context,
                                  context_len, propq);
}

int
ossl_ed448_sign(OSSL_LIB_CTX *ctx, uint8_t *out_sig,
                const uint8_t *message, size_t message_len,
                const uint8_t public_key[57], const uint8_t private_key[57],
                const uint8_t *context, size_t context_len,
                const uint8_t phflag, const char *propq)
{
    return ossl_c448_ed448_sign(ctx, out_sig, private_key, public_key, message,
                                message_len, phflag, context, context_len,
                                propq) == C448_SUCCESS;
}

/*
 * This function should not be necessary since ossl_ed448_verify() already
 * does this check internally.
 * For some reason the FIPS ACVP requires a EDDSA KeyVer test.
 */
int
ossl_ed448_pubkey_verify(const uint8_t *pub, size_t pub_len)
{
    return c448_ed448_pubkey_verify(pub, pub_len);
}

int
ossl_ed448_verify(OSSL_LIB_CTX *ctx,
                  const uint8_t *message, size_t message_len,
                  const uint8_t signature[114], const uint8_t public_key[57],
                  const uint8_t *context, size_t context_len,
                  const uint8_t phflag, const char *propq)
{
    return ossl_c448_ed448_verify(ctx, signature, public_key, message,
                                  message_len, phflag, context, (uint8_t)context_len,
                                  propq) == C448_SUCCESS;
}

int
ossl_ed448_public_from_private(OSSL_LIB_CTX *ctx, uint8_t out_public_key[57],
                               const uint8_t private_key[57], const char *propq)
{
    return ossl_c448_ed448_derive_public_key(ctx, out_public_key, private_key,
                                             propq) == C448_SUCCESS;
}
