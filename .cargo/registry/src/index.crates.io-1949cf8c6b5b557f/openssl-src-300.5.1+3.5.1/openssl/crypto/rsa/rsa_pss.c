/*
 * Copyright 2005-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * RSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/bn.h>
#include <openssl/rsa.h>
#include <openssl/evp.h>
#include <openssl/rand.h>
#include <openssl/sha.h>
#include "rsa_local.h"

static const unsigned char zeroes[] = { 0, 0, 0, 0, 0, 0, 0, 0 };

#if defined(_MSC_VER) && defined(_ARM_)
# pragma optimize("g", off)
#endif

int RSA_verify_PKCS1_PSS(RSA *rsa, const unsigned char *mHash,
                         const EVP_MD *Hash, const unsigned char *EM,
                         int sLen)
{
    return RSA_verify_PKCS1_PSS_mgf1(rsa, mHash, Hash, NULL, EM, sLen);
}

int RSA_verify_PKCS1_PSS_mgf1(RSA *rsa, const unsigned char *mHash,
                              const EVP_MD *Hash, const EVP_MD *mgf1Hash,
                              const unsigned char *EM, int sLen)
{
    return ossl_rsa_verify_PKCS1_PSS_mgf1(rsa, mHash, Hash, mgf1Hash, EM, &sLen);
}

int ossl_rsa_verify_PKCS1_PSS_mgf1(RSA *rsa, const unsigned char *mHash,
                                   const EVP_MD *Hash, const EVP_MD *mgf1Hash,
                                   const unsigned char *EM, int *sLenOut)
{
    int i;
    int ret = 0;
    int sLen = *sLenOut;
    int hLen, maskedDBLen, MSBits, emLen;
    const unsigned char *H;
    unsigned char *DB = NULL;
    EVP_MD_CTX *ctx = EVP_MD_CTX_new();
    unsigned char H_[EVP_MAX_MD_SIZE];

    if (ctx == NULL)
        goto err;

    if (mgf1Hash == NULL)
        mgf1Hash = Hash;

    hLen = EVP_MD_get_size(Hash);
    if (hLen <= 0)
        goto err;
    /*-
     * Negative sLen has special meanings:
     *      -1      sLen == hLen
     *      -2      salt length is autorecovered from signature
     *      -3      salt length is maximized
     *      -4      salt length is autorecovered from signature
     *      -N      reserved
     */
    if (sLen == RSA_PSS_SALTLEN_DIGEST) {
        sLen = hLen;
    } else if (sLen < RSA_PSS_SALTLEN_AUTO_DIGEST_MAX) {
        ERR_raise(ERR_LIB_RSA, RSA_R_SLEN_CHECK_FAILED);
        goto err;
    }

    MSBits = (BN_num_bits(rsa->n) - 1) & 0x7;
    emLen = RSA_size(rsa);
    if (EM[0] & (0xFF << MSBits)) {
        ERR_raise(ERR_LIB_RSA, RSA_R_FIRST_OCTET_INVALID);
        goto err;
    }
    if (MSBits == 0) {
        EM++;
        emLen--;
    }
    if (emLen < hLen + 2) {
        ERR_raise(ERR_LIB_RSA, RSA_R_DATA_TOO_LARGE);
        goto err;
    }
    if (sLen == RSA_PSS_SALTLEN_MAX) {
        sLen = emLen - hLen - 2;
    } else if (sLen > emLen - hLen - 2) { /* sLen can be small negative */
        ERR_raise(ERR_LIB_RSA, RSA_R_DATA_TOO_LARGE);
        goto err;
    }
    if (EM[emLen - 1] != 0xbc) {
        ERR_raise(ERR_LIB_RSA, RSA_R_LAST_OCTET_INVALID);
        goto err;
    }
    maskedDBLen = emLen - hLen - 1;
    H = EM + maskedDBLen;
    DB = OPENSSL_malloc(maskedDBLen);
    if (DB == NULL)
        goto err;
    if (PKCS1_MGF1(DB, maskedDBLen, H, hLen, mgf1Hash) < 0)
        goto err;
    for (i = 0; i < maskedDBLen; i++)
        DB[i] ^= EM[i];
    if (MSBits)
        DB[0] &= 0xFF >> (8 - MSBits);
    for (i = 0; DB[i] == 0 && i < (maskedDBLen - 1); i++) ;
    if (DB[i++] != 0x1) {
        ERR_raise(ERR_LIB_RSA, RSA_R_SLEN_RECOVERY_FAILED);
        goto err;
    }
    if (sLen != RSA_PSS_SALTLEN_AUTO
            && sLen != RSA_PSS_SALTLEN_AUTO_DIGEST_MAX
            && (maskedDBLen - i) != sLen) {
        ERR_raise_data(ERR_LIB_RSA, RSA_R_SLEN_CHECK_FAILED,
                       "expected: %d retrieved: %d", sLen,
                       maskedDBLen - i);
        goto err;
    } else {
        sLen = maskedDBLen - i;
    }
    if (!EVP_DigestInit_ex(ctx, Hash, NULL)
        || !EVP_DigestUpdate(ctx, zeroes, sizeof(zeroes))
        || !EVP_DigestUpdate(ctx, mHash, hLen))
        goto err;
    if (sLen != 0) {
        if (!EVP_DigestUpdate(ctx, DB + i, sLen))
            goto err;
    }
    if (!EVP_DigestFinal_ex(ctx, H_, NULL))
        goto err;
    if (memcmp(H_, H, hLen)) {
        ERR_raise(ERR_LIB_RSA, RSA_R_BAD_SIGNATURE);
        ret = 0;
    } else {
        ret = 1;
    }

    *sLenOut = sLen;
 err:
    OPENSSL_free(DB);
    EVP_MD_CTX_free(ctx);

    return ret;

}

int RSA_padding_add_PKCS1_PSS(RSA *rsa, unsigned char *EM,
                              const unsigned char *mHash,
                              const EVP_MD *Hash, int sLen)
{
    return RSA_padding_add_PKCS1_PSS_mgf1(rsa, EM, mHash, Hash, NULL, sLen);
}

int RSA_padding_add_PKCS1_PSS_mgf1(RSA *rsa, unsigned char *EM,
                                   const unsigned char *mHash,
                                   const EVP_MD *Hash, const EVP_MD *mgf1Hash,
                                   int sLen)
{
    return ossl_rsa_padding_add_PKCS1_PSS_mgf1(rsa, EM, mHash, Hash, mgf1Hash, &sLen);
}

int ossl_rsa_padding_add_PKCS1_PSS_mgf1(RSA *rsa, unsigned char *EM,
                                        const unsigned char *mHash,
                                        const EVP_MD *Hash, const EVP_MD *mgf1Hash,
                                        int *sLenOut)
{
    int i;
    int ret = 0;
    int sLen = *sLenOut;
    int hLen, maskedDBLen, MSBits, emLen;
    unsigned char *H, *salt = NULL, *p;
    EVP_MD_CTX *ctx = NULL;
    int sLenMax = -1;

    if (mgf1Hash == NULL)
        mgf1Hash = Hash;

    hLen = EVP_MD_get_size(Hash);
    if (hLen <= 0)
        goto err;
    /*-
     * Negative sLen has special meanings:
     *      -1      sLen == hLen
     *      -2      salt length is maximized
     *      -3      same as above (on signing)
     *      -4      salt length is min(hLen, maximum salt length)
     *      -N      reserved
     */
    /* FIPS 186-4 section 5 "The RSA Digital Signature Algorithm", subsection
     * 5.5 "PKCS #1" says: "For RSASSA-PSS […] the length (in bytes) of the
     * salt (sLen) shall satisfy 0 <= sLen <= hLen, where hLen is the length of
     * the hash function output block (in bytes)."
     *
     * Provide a way to use at most the digest length, so that the default does
     * not violate FIPS 186-4. */
    if (sLen == RSA_PSS_SALTLEN_DIGEST) {
        sLen = hLen;
    } else if (sLen == RSA_PSS_SALTLEN_MAX_SIGN
               || sLen == RSA_PSS_SALTLEN_AUTO) {
        sLen = RSA_PSS_SALTLEN_MAX;
    } else if (sLen == RSA_PSS_SALTLEN_AUTO_DIGEST_MAX) {
        sLen = RSA_PSS_SALTLEN_MAX;
        sLenMax = hLen;
    } else if (sLen < RSA_PSS_SALTLEN_AUTO_DIGEST_MAX) {
        ERR_raise(ERR_LIB_RSA, RSA_R_SLEN_CHECK_FAILED);
        goto err;
    }

    MSBits = (BN_num_bits(rsa->n) - 1) & 0x7;
    emLen = RSA_size(rsa);
    if (MSBits == 0) {
        *EM++ = 0;
        emLen--;
    }
    if (emLen < hLen + 2) {
        ERR_raise(ERR_LIB_RSA, RSA_R_DATA_TOO_LARGE_FOR_KEY_SIZE);
        goto err;
    }
    if (sLen == RSA_PSS_SALTLEN_MAX) {
        sLen = emLen - hLen - 2;
        if (sLenMax >= 0 && sLen > sLenMax)
            sLen = sLenMax;
    } else if (sLen > emLen - hLen - 2) {
        ERR_raise(ERR_LIB_RSA, RSA_R_DATA_TOO_LARGE_FOR_KEY_SIZE);
        goto err;
    }
    if (sLen > 0) {
        salt = OPENSSL_malloc(sLen);
        if (salt == NULL)
            goto err;
        if (RAND_bytes_ex(rsa->libctx, salt, sLen, 0) <= 0)
            goto err;
    }
    maskedDBLen = emLen - hLen - 1;
    H = EM + maskedDBLen;
    ctx = EVP_MD_CTX_new();
    if (ctx == NULL)
        goto err;
    if (!EVP_DigestInit_ex(ctx, Hash, NULL)
        || !EVP_DigestUpdate(ctx, zeroes, sizeof(zeroes))
        || !EVP_DigestUpdate(ctx, mHash, hLen))
        goto err;
    if (sLen && !EVP_DigestUpdate(ctx, salt, sLen))
        goto err;
    if (!EVP_DigestFinal_ex(ctx, H, NULL))
        goto err;

    /* Generate dbMask in place then perform XOR on it */
    if (PKCS1_MGF1(EM, maskedDBLen, H, hLen, mgf1Hash))
        goto err;

    p = EM;

    /*
     * Initial PS XORs with all zeroes which is a NOP so just update pointer.
     * Note from a test above this value is guaranteed to be non-negative.
     */
    p += emLen - sLen - hLen - 2;
    *p++ ^= 0x1;
    if (sLen > 0) {
        for (i = 0; i < sLen; i++)
            *p++ ^= salt[i];
    }
    if (MSBits)
        EM[0] &= 0xFF >> (8 - MSBits);

    /* H is already in place so just set final 0xbc */

    EM[emLen - 1] = 0xbc;

    ret = 1;

    *sLenOut = sLen;
 err:
    EVP_MD_CTX_free(ctx);
    OPENSSL_clear_free(salt, (size_t)sLen); /* salt != NULL implies sLen > 0 */

    return ret;

}

/*
 * The defaults for PSS restrictions are defined in RFC 8017, A.2.3 RSASSA-PSS
 * (https://tools.ietf.org/html/rfc8017#appendix-A.2.3):
 *
 * If the default values of the hashAlgorithm, maskGenAlgorithm, and
 * trailerField fields of RSASSA-PSS-params are used, then the algorithm
 * identifier will have the following value:
 *
 *     rSASSA-PSS-Default-Identifier    RSASSA-AlgorithmIdentifier ::= {
 *         algorithm   id-RSASSA-PSS,
 *         parameters  RSASSA-PSS-params : {
 *             hashAlgorithm       sha1,
 *             maskGenAlgorithm    mgf1SHA1,
 *             saltLength          20,
 *             trailerField        trailerFieldBC
 *         }
 *     }
 *
 *     RSASSA-AlgorithmIdentifier ::= AlgorithmIdentifier {
 *         {PKCS1Algorithms}
 *     }
 */
static const RSA_PSS_PARAMS_30 default_RSASSA_PSS_params = {
    NID_sha1,                    /* default hashAlgorithm */
    {
        NID_mgf1,                /* default maskGenAlgorithm */
        NID_sha1                 /* default MGF1 hash */
    },
    20,                          /* default saltLength */
    1                            /* default trailerField (0xBC) */
};

int ossl_rsa_pss_params_30_set_defaults(RSA_PSS_PARAMS_30 *rsa_pss_params)
{
    if (rsa_pss_params == NULL)
        return 0;
    *rsa_pss_params = default_RSASSA_PSS_params;
    return 1;
}

int ossl_rsa_pss_params_30_is_unrestricted(const RSA_PSS_PARAMS_30 *rsa_pss_params)
{
    static RSA_PSS_PARAMS_30 pss_params_cmp = { 0, };

    return rsa_pss_params == NULL
        || memcmp(rsa_pss_params, &pss_params_cmp,
                  sizeof(*rsa_pss_params)) == 0;
}

int ossl_rsa_pss_params_30_copy(RSA_PSS_PARAMS_30 *to,
                                const RSA_PSS_PARAMS_30 *from)
{
    memcpy(to, from, sizeof(*to));
    return 1;
}

int ossl_rsa_pss_params_30_set_hashalg(RSA_PSS_PARAMS_30 *rsa_pss_params,
                                       int hashalg_nid)
{
    if (rsa_pss_params == NULL)
        return 0;
    rsa_pss_params->hash_algorithm_nid = hashalg_nid;
    return 1;
}

int ossl_rsa_pss_params_30_set_maskgenhashalg(RSA_PSS_PARAMS_30 *rsa_pss_params,
                                              int maskgenhashalg_nid)
{
    if (rsa_pss_params == NULL)
        return 0;
    rsa_pss_params->mask_gen.hash_algorithm_nid = maskgenhashalg_nid;
    return 1;
}

int ossl_rsa_pss_params_30_set_saltlen(RSA_PSS_PARAMS_30 *rsa_pss_params,
                                       int saltlen)
{
    if (rsa_pss_params == NULL)
        return 0;
    rsa_pss_params->salt_len = saltlen;
    return 1;
}

int ossl_rsa_pss_params_30_set_trailerfield(RSA_PSS_PARAMS_30 *rsa_pss_params,
                                            int trailerfield)
{
    if (rsa_pss_params == NULL)
        return 0;
    rsa_pss_params->trailer_field = trailerfield;
    return 1;
}

int ossl_rsa_pss_params_30_hashalg(const RSA_PSS_PARAMS_30 *rsa_pss_params)
{
    if (rsa_pss_params == NULL)
        return default_RSASSA_PSS_params.hash_algorithm_nid;
    return rsa_pss_params->hash_algorithm_nid;
}

int ossl_rsa_pss_params_30_maskgenalg(const RSA_PSS_PARAMS_30 *rsa_pss_params)
{
    if (rsa_pss_params == NULL)
        return default_RSASSA_PSS_params.mask_gen.algorithm_nid;
    return rsa_pss_params->mask_gen.algorithm_nid;
}

int ossl_rsa_pss_params_30_maskgenhashalg(const RSA_PSS_PARAMS_30 *rsa_pss_params)
{
    if (rsa_pss_params == NULL)
        return default_RSASSA_PSS_params.hash_algorithm_nid;
    return rsa_pss_params->mask_gen.hash_algorithm_nid;
}

int ossl_rsa_pss_params_30_saltlen(const RSA_PSS_PARAMS_30 *rsa_pss_params)
{
    if (rsa_pss_params == NULL)
        return default_RSASSA_PSS_params.salt_len;
    return rsa_pss_params->salt_len;
}

int ossl_rsa_pss_params_30_trailerfield(const RSA_PSS_PARAMS_30 *rsa_pss_params)
{
    if (rsa_pss_params == NULL)
        return default_RSASSA_PSS_params.trailer_field;
    return rsa_pss_params->trailer_field;
}

#if defined(_MSC_VER)
# pragma optimize("",on)
#endif
