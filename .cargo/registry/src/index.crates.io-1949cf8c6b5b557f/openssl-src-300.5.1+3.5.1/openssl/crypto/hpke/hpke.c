/*
 * Copyright 2022-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* An OpenSSL-based HPKE implementation of RFC9180 */

#include <string.h>
#include <openssl/rand.h>
#include <openssl/kdf.h>
#include <openssl/core_names.h>
#include <openssl/hpke.h>
#include <openssl/sha.h>
#include <openssl/evp.h>
#include <openssl/err.h>
#include "internal/hpke_util.h"
#include "internal/nelem.h"
#include "internal/common.h"

/* default buffer size for keys and internal buffers we use */
#define OSSL_HPKE_MAXSIZE 512

/* Define HPKE labels from RFC9180 in hex for EBCDIC compatibility */
/* "HPKE" - "suite_id" label for section 5.1 */
static const char OSSL_HPKE_SEC51LABEL[] = "\x48\x50\x4b\x45";
/* "psk_id_hash" - in key_schedule_context */
static const char OSSL_HPKE_PSKIDHASH_LABEL[] = "\x70\x73\x6b\x5f\x69\x64\x5f\x68\x61\x73\x68";
/*  "info_hash" - in key_schedule_context */
static const char OSSL_HPKE_INFOHASH_LABEL[] = "\x69\x6e\x66\x6f\x5f\x68\x61\x73\x68";
/*  "base_nonce" - base nonce calc label */
static const char OSSL_HPKE_NONCE_LABEL[] = "\x62\x61\x73\x65\x5f\x6e\x6f\x6e\x63\x65";
/*  "exp" - internal exporter secret generation label */
static const char OSSL_HPKE_EXP_LABEL[] = "\x65\x78\x70";
/*  "sec" - external label for exporting secret */
static const char OSSL_HPKE_EXP_SEC_LABEL[] = "\x73\x65\x63";
/*  "key" - label for use when generating key from shared secret */
static const char OSSL_HPKE_KEY_LABEL[] = "\x6b\x65\x79";
/*  "secret" - for generating shared secret */
static const char OSSL_HPKE_SECRET_LABEL[] = "\x73\x65\x63\x72\x65\x74";

/**
 * @brief sender or receiver context
 */
struct ossl_hpke_ctx_st {
    OSSL_LIB_CTX *libctx; /* library context */
    char *propq; /* properties */
    int mode; /* HPKE mode */
    OSSL_HPKE_SUITE suite; /* suite */
    const OSSL_HPKE_KEM_INFO *kem_info;
    const OSSL_HPKE_KDF_INFO *kdf_info;
    const OSSL_HPKE_AEAD_INFO *aead_info;
    EVP_CIPHER *aead_ciph;
    int role; /* sender(0) or receiver(1) */
    uint64_t seq; /* aead sequence number */
    unsigned char *shared_secret; /* KEM output, zz */
    size_t shared_secretlen;
    unsigned char *key; /* final aead key */
    size_t keylen;
    unsigned char *nonce; /* aead base nonce */
    size_t noncelen;
    unsigned char *exportersec; /* exporter secret */
    size_t exporterseclen;
    char *pskid; /* PSK stuff */
    unsigned char *psk;
    size_t psklen;
    EVP_PKEY *authpriv; /* sender's authentication private key */
    unsigned char *authpub; /* auth public key */
    size_t authpublen;
    unsigned char *ikme; /* IKM for sender deterministic key gen */
    size_t ikmelen;
};

/**
 * @brief check if KEM uses NIST curve or not
 * @param kem_id is the externally supplied kem_id
 * @return 1 for NIST curves, 0 for other
 */
static int hpke_kem_id_nist_curve(uint16_t kem_id)
{
    const OSSL_HPKE_KEM_INFO *kem_info;

    kem_info = ossl_HPKE_KEM_INFO_find_id(kem_id);
    return kem_info != NULL && kem_info->groupname != NULL;
}

/**
 * @brief wrapper to import NIST curve public key as easily as x25519/x448
 * @param libctx is the context to use
 * @param propq is a properties string
 * @param gname is the curve groupname
 * @param buf is the binary buffer with the (uncompressed) public value
 * @param buflen is the length of the private key buffer
 * @return a working EVP_PKEY * or NULL
 *
 * Note that this could be a useful function to make public in
 * future, but would likely require a name change.
 */
static EVP_PKEY *evp_pkey_new_raw_nist_public_key(OSSL_LIB_CTX *libctx,
                                                  const char *propq,
                                                  const char *gname,
                                                  const unsigned char *buf,
                                                  size_t buflen)
{
    OSSL_PARAM params[2];
    EVP_PKEY *ret = NULL;
    EVP_PKEY_CTX *cctx = EVP_PKEY_CTX_new_from_name(libctx, "EC", propq);

    params[0] = OSSL_PARAM_construct_utf8_string(OSSL_PKEY_PARAM_GROUP_NAME,
                                                 (char *)gname, 0);
    params[1] = OSSL_PARAM_construct_end();
    if (cctx == NULL
        || EVP_PKEY_paramgen_init(cctx) <= 0
        || EVP_PKEY_CTX_set_params(cctx, params) <= 0
        || EVP_PKEY_paramgen(cctx, &ret) <= 0
        || EVP_PKEY_set1_encoded_public_key(ret, buf, buflen) != 1) {
        EVP_PKEY_CTX_free(cctx);
        EVP_PKEY_free(ret);
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        return NULL;
    }
    EVP_PKEY_CTX_free(cctx);
    return ret;
}

/**
 * @brief do the AEAD decryption
 * @param hctx is the context to use
 * @param iv is the initialisation vector
 * @param aad is the additional authenticated data
 * @param aadlen is the length of the aad
 * @param ct is the ciphertext buffer
 * @param ctlen is the ciphertext length (including tag).
 * @param pt is the output buffer
 * @param ptlen input/output, better be big enough on input, exact on output
 * @return 1 on success, 0 otherwise
 */
static int hpke_aead_dec(OSSL_HPKE_CTX *hctx, const unsigned char *iv,
                         const unsigned char *aad, size_t aadlen,
                         const unsigned char *ct, size_t ctlen,
                         unsigned char *pt, size_t *ptlen)
{
    int erv = 0;
    EVP_CIPHER_CTX *ctx = NULL;
    int len = 0;
    size_t taglen;

    taglen = hctx->aead_info->taglen;
    if (ctlen <= taglen || *ptlen < ctlen - taglen) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    /* Create and initialise the context */
    if ((ctx = EVP_CIPHER_CTX_new()) == NULL)
        return 0;
    /* Initialise the decryption operation. */
    if (EVP_DecryptInit_ex(ctx, hctx->aead_ciph, NULL, NULL, NULL) != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    if (EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_SET_IVLEN,
                            hctx->noncelen, NULL) != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    /* Initialise key and IV */
    if (EVP_DecryptInit_ex(ctx, NULL, NULL, hctx->key, iv) != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    /* Provide AAD. */
    if (aadlen != 0 && aad != NULL) {
        if (EVP_DecryptUpdate(ctx, NULL, &len, aad, aadlen) != 1) {
            ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
            goto err;
        }
    }
    if (EVP_DecryptUpdate(ctx, pt, &len, ct, ctlen - taglen) != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    *ptlen = len;
    if (!EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_SET_TAG,
                             taglen, (void *)(ct + ctlen - taglen))) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    /* Finalise decryption.  */
    if (EVP_DecryptFinal_ex(ctx, pt + len, &len) <= 0) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    erv = 1;

err:
    if (erv != 1)
        OPENSSL_cleanse(pt, *ptlen);
    EVP_CIPHER_CTX_free(ctx);
    return erv;
}

/**
 * @brief do AEAD encryption as per the RFC
 * @param hctx is the context to use
 * @param iv is the initialisation vector
 * @param aad is the additional authenticated data
 * @param aadlen is the length of the aad
 * @param pt is the plaintext buffer
 * @param ptlen is the length of pt
 * @param ct is the output buffer
 * @param ctlen input/output, needs space for tag on input, exact on output
 * @return 1 for success, 0 otherwise
 */
static int hpke_aead_enc(OSSL_HPKE_CTX *hctx, const unsigned char *iv,
                         const unsigned char *aad, size_t aadlen,
                         const unsigned char *pt, size_t ptlen,
                         unsigned char *ct, size_t *ctlen)
{
    int erv = 0;
    EVP_CIPHER_CTX *ctx = NULL;
    int len;
    size_t taglen = 0;
    unsigned char tag[EVP_MAX_AEAD_TAG_LENGTH];

    taglen = hctx->aead_info->taglen;
    if (*ctlen <= taglen || ptlen > *ctlen - taglen) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (!ossl_assert(taglen <= sizeof(tag))) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    /* Create and initialise the context */
    if ((ctx = EVP_CIPHER_CTX_new()) == NULL)
        return 0;
    /* Initialise the encryption operation. */
    if (EVP_EncryptInit_ex(ctx, hctx->aead_ciph, NULL, NULL, NULL) != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    if (EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_SET_IVLEN,
                            hctx->noncelen, NULL) != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    /* Initialise key and IV */
    if (EVP_EncryptInit_ex(ctx, NULL, NULL, hctx->key, iv) != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    /* Provide any AAD data. */
    if (aadlen != 0 && aad != NULL) {
        if (EVP_EncryptUpdate(ctx, NULL, &len, aad, aadlen) != 1) {
            ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
            goto err;
        }
    }
    if (EVP_EncryptUpdate(ctx, ct, &len, pt, ptlen) != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    *ctlen = len;
    /* Finalise the encryption. */
    if (EVP_EncryptFinal_ex(ctx, ct + len, &len) != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    *ctlen += len;
    /* Get tag. Not a duplicate so needs to be added to the ciphertext */
    if (EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_GET_TAG, taglen, tag) != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    memcpy(ct + *ctlen, tag, taglen);
    *ctlen += taglen;
    erv = 1;

err:
    if (erv != 1)
        OPENSSL_cleanse(ct, *ctlen);
    EVP_CIPHER_CTX_free(ctx);
    return erv;
}

/**
 * @brief check mode is in-range and supported
 * @param mode is the caller's chosen mode
 * @return 1 for good mode, 0 otherwise
 */
static int hpke_mode_check(unsigned int mode)
{
    switch (mode) {
    case OSSL_HPKE_MODE_BASE:
    case OSSL_HPKE_MODE_PSK:
    case OSSL_HPKE_MODE_AUTH:
    case OSSL_HPKE_MODE_PSKAUTH:
        break;
    default:
        return 0;
    }
    return 1;
}

/**
 * @brief check if a suite is supported locally
 * @param suite is the suite to check
 * @return 1 for good, 0 otherwise
 */
static int hpke_suite_check(OSSL_HPKE_SUITE suite,
                            const OSSL_HPKE_KEM_INFO **kem_info,
                            const OSSL_HPKE_KDF_INFO **kdf_info,
                            const OSSL_HPKE_AEAD_INFO **aead_info)
{
    const OSSL_HPKE_KEM_INFO *kem_info_;
    const OSSL_HPKE_KDF_INFO *kdf_info_;
    const OSSL_HPKE_AEAD_INFO *aead_info_;

    /* check KEM, KDF and AEAD are supported here */
    if ((kem_info_ = ossl_HPKE_KEM_INFO_find_id(suite.kem_id)) == NULL)
        return 0;
    if ((kdf_info_ = ossl_HPKE_KDF_INFO_find_id(suite.kdf_id)) == NULL)
        return 0;
    if ((aead_info_ = ossl_HPKE_AEAD_INFO_find_id(suite.aead_id)) == NULL)
        return 0;

    if (kem_info != NULL)
        *kem_info = kem_info_;
    if (kdf_info != NULL)
        *kdf_info = kdf_info_;
    if (aead_info != NULL)
        *aead_info = aead_info_;

    return 1;
}

/*
 * @brief randomly pick a suite
 * @param libctx is the context to use
 * @param propq is a properties string
 * @param suite is the result
 * @return 1 for success, 0 otherwise
 */
static int hpke_random_suite(OSSL_LIB_CTX *libctx,
                             const char *propq,
                             OSSL_HPKE_SUITE *suite)
{
    const OSSL_HPKE_KEM_INFO *kem_info = NULL;
    const OSSL_HPKE_KDF_INFO *kdf_info = NULL;
    const OSSL_HPKE_AEAD_INFO *aead_info = NULL;

    /* random kem, kdf and aead */
    kem_info = ossl_HPKE_KEM_INFO_find_random(libctx);
    if (kem_info == NULL)
        return 0;
    suite->kem_id = kem_info->kem_id;
    kdf_info = ossl_HPKE_KDF_INFO_find_random(libctx);
    if (kdf_info == NULL)
        return 0;
    suite->kdf_id = kdf_info->kdf_id;
    aead_info = ossl_HPKE_AEAD_INFO_find_random(libctx);
    if (aead_info == NULL)
        return 0;
    suite->aead_id = aead_info->aead_id;
    return 1;
}

/*
 * @brief tell the caller how big the ciphertext will be
 *
 * AEAD algorithms add a tag for data authentication.
 * Those are almost always, but not always, 16 octets
 * long, and who knows what will be true in the future.
 * So this function allows a caller to find out how
 * much data expansion they will see with a given suite.
 *
 * "enc" is the name used in RFC9180 for the encapsulated
 * public value of the sender, who calls OSSL_HPKE_seal(),
 * that is sent to the recipient, who calls OSSL_HPKE_open().
 *
 * @param suite is the suite to be used
 * @param enclen points to what will be enc length
 * @param clearlen is the length of plaintext
 * @param cipherlen points to what will be ciphertext length (including tag)
 * @return 1 for success, 0 otherwise
 */
static int hpke_expansion(OSSL_HPKE_SUITE suite,
                          size_t *enclen,
                          size_t clearlen,
                          size_t *cipherlen)
{
    const OSSL_HPKE_AEAD_INFO *aead_info = NULL;
    const OSSL_HPKE_KEM_INFO *kem_info = NULL;

    if (cipherlen == NULL || enclen == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (hpke_suite_check(suite, &kem_info, NULL, &aead_info) != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    *cipherlen = clearlen + aead_info->taglen;
    *enclen = kem_info->Nenc;
    return 1;
}

/*
 * @brief expand and XOR the 64-bit unsigned seq with (nonce) buffer
 * @param ctx is the HPKE context
 * @param buf is the buffer for the XOR'd seq and nonce
 * @param blen is the size of buf
 * @return 0 for error, otherwise blen
 */
static size_t hpke_seqnonce2buf(OSSL_HPKE_CTX *ctx,
                                unsigned char *buf, size_t blen)
{
    size_t i;
    uint64_t seq_copy;

    if (ctx == NULL || blen < sizeof(seq_copy) || blen != ctx->noncelen)
        return 0;
    seq_copy = ctx->seq;
    memset(buf, 0, blen);
    for (i = 0; i < sizeof(seq_copy); i++) {
        buf[blen - i - 1] = seq_copy & 0xff;
        seq_copy >>= 8;
    }
    for (i = 0; i < blen; i++)
        buf[i] ^= ctx->nonce[i];
    return blen;
}

/*
 * @brief call the underlying KEM to encap
 * @param ctx is the OSSL_HPKE_CTX
 * @param enc is a buffer for the sender's ephemeral public value
 * @param enclen is the size of enc on input, number of octets used on output
 * @param pub is the recipient's public value
 * @param publen is the length of pub
 * @return 1 for success, 0 for error
 */
static int hpke_encap(OSSL_HPKE_CTX *ctx, unsigned char *enc, size_t *enclen,
                      const unsigned char *pub, size_t publen)
{
    int erv = 0;
    OSSL_PARAM params[3], *p = params;
    size_t lsslen = 0, lenclen = 0;
    EVP_PKEY_CTX *pctx = NULL;
    EVP_PKEY *pkR = NULL;
    const OSSL_HPKE_KEM_INFO *kem_info = NULL;

    if (ctx == NULL || enc == NULL || enclen == NULL || *enclen == 0
        || pub == NULL || publen == 0) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (ctx->shared_secret != NULL) {
        /* only run the KEM once per OSSL_HPKE_CTX */
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    kem_info = ossl_HPKE_KEM_INFO_find_id(ctx->suite.kem_id);
    if (kem_info == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        return 0;
    }
    if (hpke_kem_id_nist_curve(ctx->suite.kem_id) == 1) {
        pkR = evp_pkey_new_raw_nist_public_key(ctx->libctx, ctx->propq,
                                               kem_info->groupname,
                                               pub, publen);
    } else {
        pkR = EVP_PKEY_new_raw_public_key_ex(ctx->libctx,
                                             kem_info->keytype,
                                             ctx->propq, pub, publen);
    }
    if (pkR == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    pctx = EVP_PKEY_CTX_new_from_pkey(ctx->libctx, pkR, ctx->propq);
    if (pctx == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KEM_PARAM_OPERATION,
                                            OSSL_KEM_PARAM_OPERATION_DHKEM,
                                            0);
    if (ctx->ikme != NULL) {
        *p++ = OSSL_PARAM_construct_octet_string(OSSL_KEM_PARAM_IKME,
                                                 ctx->ikme, ctx->ikmelen);
    }
    *p = OSSL_PARAM_construct_end();
    if (ctx->mode == OSSL_HPKE_MODE_AUTH
        || ctx->mode == OSSL_HPKE_MODE_PSKAUTH) {
        if (EVP_PKEY_auth_encapsulate_init(pctx, ctx->authpriv,
                                           params) != 1) {
            ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
            goto err;
        }
    } else {
        if (EVP_PKEY_encapsulate_init(pctx, params) != 1) {
            ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
            goto err;
        }
    }
    lenclen = *enclen;
    if (EVP_PKEY_encapsulate(pctx, NULL, &lenclen, NULL, &lsslen) != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    if (lenclen > *enclen) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        goto err;
    }
    ctx->shared_secret = OPENSSL_malloc(lsslen);
    if (ctx->shared_secret == NULL)
        goto err;
    ctx->shared_secretlen = lsslen;
    if (EVP_PKEY_encapsulate(pctx, enc, enclen, ctx->shared_secret,
                             &ctx->shared_secretlen) != 1) {
        ctx->shared_secretlen = 0;
        OPENSSL_free(ctx->shared_secret);
        ctx->shared_secret = NULL;
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    erv = 1;

err:
    EVP_PKEY_CTX_free(pctx);
    EVP_PKEY_free(pkR);
    return erv;
}

/*
 * @brief call the underlying KEM to decap
 * @param ctx is the OSSL_HPKE_CTX
 * @param enc is a buffer for the sender's ephemeral public value
 * @param enclen is the length of enc
 * @param priv is the recipient's private value
 * @return 1 for success, 0 for error
 */
static int hpke_decap(OSSL_HPKE_CTX *ctx,
                      const unsigned char *enc, size_t enclen,
                      EVP_PKEY *priv)
{
    int erv = 0;
    EVP_PKEY_CTX *pctx = NULL;
    EVP_PKEY *spub = NULL;
    OSSL_PARAM params[2], *p = params;
    size_t lsslen = 0;

    if (ctx == NULL || enc == NULL || enclen == 0 || priv == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (ctx->shared_secret != NULL) {
        /* only run the KEM once per OSSL_HPKE_CTX */
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    pctx = EVP_PKEY_CTX_new_from_pkey(ctx->libctx, priv, ctx->propq);
    if (pctx == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KEM_PARAM_OPERATION,
                                            OSSL_KEM_PARAM_OPERATION_DHKEM,
                                            0);
    *p = OSSL_PARAM_construct_end();
    if (ctx->mode == OSSL_HPKE_MODE_AUTH
        || ctx->mode == OSSL_HPKE_MODE_PSKAUTH) {
        const OSSL_HPKE_KEM_INFO *kem_info = NULL;

        kem_info = ossl_HPKE_KEM_INFO_find_id(ctx->suite.kem_id);
        if (kem_info == NULL) {
            ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
            goto err;
        }
        if (hpke_kem_id_nist_curve(ctx->suite.kem_id) == 1) {
            spub = evp_pkey_new_raw_nist_public_key(ctx->libctx, ctx->propq,
                                                    kem_info->groupname,
                                                    ctx->authpub,
                                                    ctx->authpublen);
        } else {
            spub = EVP_PKEY_new_raw_public_key_ex(ctx->libctx,
                                                  kem_info->keytype,
                                                  ctx->propq,
                                                  ctx->authpub,
                                                  ctx->authpublen);
        }
        if (spub == NULL) {
            ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
            goto err;
        }
        if (EVP_PKEY_auth_decapsulate_init(pctx, spub, params) != 1) {
            ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
            goto err;
        }
    } else {
        if (EVP_PKEY_decapsulate_init(pctx, params) != 1) {
            ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
            goto err;
        }
    }
    if (EVP_PKEY_decapsulate(pctx, NULL, &lsslen, enc, enclen) != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    ctx->shared_secret = OPENSSL_malloc(lsslen);
    if (ctx->shared_secret == NULL)
        goto err;
    if (EVP_PKEY_decapsulate(pctx, ctx->shared_secret, &lsslen,
                             enc, enclen) != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    ctx->shared_secretlen = lsslen;
    erv = 1;

err:
    EVP_PKEY_CTX_free(pctx);
    EVP_PKEY_free(spub);
    if (erv == 0) {
        OPENSSL_free(ctx->shared_secret);
        ctx->shared_secret = NULL;
        ctx->shared_secretlen = 0;
    }
    return erv;
}

/*
 * @brief do "middle" of HPKE, between KEM and AEAD
 * @param ctx is the OSSL_HPKE_CTX
 * @param info is a buffer for the added binding information
 * @param infolen is the length of info
 * @return 0 for error, 1 for success
 *
 * This does all the HPKE extracts and expands as defined in RFC9180
 * section 5.1, (badly termed there as a "key schedule") and sets the
 * ctx fields for the shared_secret, nonce, key and exporter_secret
 */
static int hpke_do_middle(OSSL_HPKE_CTX *ctx,
                          const unsigned char *info, size_t infolen)
{
    int erv = 0;
    size_t ks_contextlen = OSSL_HPKE_MAXSIZE;
    unsigned char ks_context[OSSL_HPKE_MAXSIZE];
    size_t halflen = 0;
    size_t pskidlen = 0;
    const OSSL_HPKE_AEAD_INFO *aead_info = NULL;
    const OSSL_HPKE_KDF_INFO *kdf_info = NULL;
    size_t secretlen = OSSL_HPKE_MAXSIZE;
    unsigned char secret[OSSL_HPKE_MAXSIZE];
    EVP_KDF_CTX *kctx = NULL;
    unsigned char suitebuf[6];
    const char *mdname = NULL;

    /* only let this be done once */
    if (ctx->exportersec != NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    if (ossl_HPKE_KEM_INFO_find_id(ctx->suite.kem_id) == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        return 0;
    }
    aead_info = ossl_HPKE_AEAD_INFO_find_id(ctx->suite.aead_id);
    if (aead_info == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        return 0;
    }
    kdf_info = ossl_HPKE_KDF_INFO_find_id(ctx->suite.kdf_id);
    if (kdf_info == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        return 0;
    }
    mdname = kdf_info->mdname;
    /* create key schedule context */
    memset(ks_context, 0, sizeof(ks_context));
    ks_context[0] = (unsigned char)(ctx->mode % 256);
    ks_contextlen--; /* remaining space */
    halflen = kdf_info->Nh;
    if ((2 * halflen) > ks_contextlen) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        return 0;
    }
    /* check a psk was set if in that mode */
    if (ctx->mode == OSSL_HPKE_MODE_PSK
        || ctx->mode == OSSL_HPKE_MODE_PSKAUTH) {
        if (ctx->psk == NULL || ctx->psklen == 0 || ctx->pskid == NULL) {
            ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
            return 0;
        }
    }
    kctx = ossl_kdf_ctx_create("HKDF", mdname, ctx->libctx, ctx->propq);
    if (kctx == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        return 0;
    }
    pskidlen = (ctx->psk == NULL ? 0 : strlen(ctx->pskid));
    /* full suite details as per RFC9180 sec 5.1 */
    suitebuf[0] = ctx->suite.kem_id / 256;
    suitebuf[1] = ctx->suite.kem_id % 256;
    suitebuf[2] = ctx->suite.kdf_id / 256;
    suitebuf[3] = ctx->suite.kdf_id % 256;
    suitebuf[4] = ctx->suite.aead_id / 256;
    suitebuf[5] = ctx->suite.aead_id % 256;
    /* Extract and Expand variously... */
    if (ossl_hpke_labeled_extract(kctx, ks_context + 1, halflen,
                                  NULL, 0, OSSL_HPKE_SEC51LABEL,
                                  suitebuf, sizeof(suitebuf),
                                  OSSL_HPKE_PSKIDHASH_LABEL,
                                  (unsigned char *)ctx->pskid, pskidlen) != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    if (ossl_hpke_labeled_extract(kctx, ks_context + 1 + halflen, halflen,
                                  NULL, 0, OSSL_HPKE_SEC51LABEL,
                                  suitebuf, sizeof(suitebuf),
                                  OSSL_HPKE_INFOHASH_LABEL,
                                  (unsigned char *)info, infolen) != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    ks_contextlen = 1 + 2 * halflen;
    secretlen = kdf_info->Nh;
    if (secretlen > OSSL_HPKE_MAXSIZE) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    if (ossl_hpke_labeled_extract(kctx, secret, secretlen,
                                  ctx->shared_secret, ctx->shared_secretlen,
                                  OSSL_HPKE_SEC51LABEL,
                                  suitebuf, sizeof(suitebuf),
                                  OSSL_HPKE_SECRET_LABEL,
                                  ctx->psk, ctx->psklen) != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    if (ctx->suite.aead_id != OSSL_HPKE_AEAD_ID_EXPORTONLY) {
        /* we only need nonce/key for non export AEADs */
        ctx->noncelen = aead_info->Nn;
        ctx->nonce = OPENSSL_malloc(ctx->noncelen);
        if (ctx->nonce == NULL)
            goto err;
        if (ossl_hpke_labeled_expand(kctx, ctx->nonce, ctx->noncelen,
                                     secret, secretlen, OSSL_HPKE_SEC51LABEL,
                                     suitebuf, sizeof(suitebuf),
                                     OSSL_HPKE_NONCE_LABEL,
                                     ks_context, ks_contextlen) != 1) {
            ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
            goto err;
        }
        ctx->keylen = aead_info->Nk;
        ctx->key = OPENSSL_malloc(ctx->keylen);
        if (ctx->key == NULL)
            goto err;
        if (ossl_hpke_labeled_expand(kctx, ctx->key, ctx->keylen,
                                     secret, secretlen, OSSL_HPKE_SEC51LABEL,
                                     suitebuf, sizeof(suitebuf),
                                     OSSL_HPKE_KEY_LABEL,
                                     ks_context, ks_contextlen) != 1) {
            ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
            goto err;
        }
    }
    ctx->exporterseclen = kdf_info->Nh;
    ctx->exportersec = OPENSSL_malloc(ctx->exporterseclen);
    if (ctx->exportersec == NULL)
        goto err;
    if (ossl_hpke_labeled_expand(kctx, ctx->exportersec, ctx->exporterseclen,
                                 secret, secretlen, OSSL_HPKE_SEC51LABEL,
                                 suitebuf, sizeof(suitebuf),
                                 OSSL_HPKE_EXP_LABEL,
                                 ks_context, ks_contextlen) != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    erv = 1;

err:
    OPENSSL_cleanse(ks_context, OSSL_HPKE_MAXSIZE);
    OPENSSL_cleanse(secret, OSSL_HPKE_MAXSIZE);
    EVP_KDF_CTX_free(kctx);
    return erv;
}

/*
 * externally visible functions from below here, API documentation is
 * in doc/man3/OSSL_HPKE_CTX_new.pod to avoid duplication
 */

OSSL_HPKE_CTX *OSSL_HPKE_CTX_new(int mode, OSSL_HPKE_SUITE suite, int role,
                                 OSSL_LIB_CTX *libctx, const char *propq)
{
    OSSL_HPKE_CTX *ctx = NULL;
    const OSSL_HPKE_KEM_INFO *kem_info;
    const OSSL_HPKE_KDF_INFO *kdf_info;
    const OSSL_HPKE_AEAD_INFO *aead_info;

    if (hpke_mode_check(mode) != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return NULL;
    }
    if (hpke_suite_check(suite, &kem_info, &kdf_info, &aead_info) != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return NULL;
    }
    if (role != OSSL_HPKE_ROLE_SENDER && role != OSSL_HPKE_ROLE_RECEIVER) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx == NULL)
        return NULL;
    ctx->libctx = libctx;
    if (propq != NULL) {
        ctx->propq = OPENSSL_strdup(propq);
        if (ctx->propq == NULL)
            goto err;
    }
    if (suite.aead_id != OSSL_HPKE_AEAD_ID_EXPORTONLY) {
        ctx->aead_ciph = EVP_CIPHER_fetch(libctx, aead_info->name, propq);
        if (ctx->aead_ciph == NULL) {
            ERR_raise(ERR_LIB_CRYPTO, ERR_R_FETCH_FAILED);
            goto err;
        }
    }
    ctx->role = role;
    ctx->mode = mode;
    ctx->suite = suite;
    ctx->kem_info = kem_info;
    ctx->kdf_info = kdf_info;
    ctx->aead_info = aead_info;
    return ctx;

 err:
    EVP_CIPHER_free(ctx->aead_ciph);
    OPENSSL_free(ctx->propq);
    OPENSSL_free(ctx);
    return NULL;
}

void OSSL_HPKE_CTX_free(OSSL_HPKE_CTX *ctx)
{
    if (ctx == NULL)
        return;
    EVP_CIPHER_free(ctx->aead_ciph);
    OPENSSL_free(ctx->propq);
    OPENSSL_clear_free(ctx->exportersec, ctx->exporterseclen);
    OPENSSL_free(ctx->pskid);
    OPENSSL_clear_free(ctx->psk, ctx->psklen);
    OPENSSL_clear_free(ctx->key, ctx->keylen);
    OPENSSL_clear_free(ctx->nonce, ctx->noncelen);
    OPENSSL_clear_free(ctx->shared_secret, ctx->shared_secretlen);
    OPENSSL_clear_free(ctx->ikme, ctx->ikmelen);
    EVP_PKEY_free(ctx->authpriv);
    OPENSSL_free(ctx->authpub);

    OPENSSL_free(ctx);
    return;
}

int OSSL_HPKE_CTX_set1_psk(OSSL_HPKE_CTX *ctx,
                           const char *pskid,
                           const unsigned char *psk, size_t psklen)
{
    if (ctx == NULL || pskid == NULL || psk == NULL || psklen == 0) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (psklen > OSSL_HPKE_MAX_PARMLEN) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (psklen < OSSL_HPKE_MIN_PSKLEN) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (strlen(pskid) > OSSL_HPKE_MAX_PARMLEN) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (strlen(pskid) == 0) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (ctx->mode != OSSL_HPKE_MODE_PSK
        && ctx->mode != OSSL_HPKE_MODE_PSKAUTH) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    /* free previous values if any */
    OPENSSL_clear_free(ctx->psk, ctx->psklen);
    ctx->psk = OPENSSL_memdup(psk, psklen);
    if (ctx->psk == NULL)
        return 0;
    ctx->psklen = psklen;
    OPENSSL_free(ctx->pskid);
    ctx->pskid = OPENSSL_strdup(pskid);
    if (ctx->pskid == NULL) {
        OPENSSL_clear_free(ctx->psk, ctx->psklen);
        ctx->psk = NULL;
        ctx->psklen = 0;
        return 0;
    }
    return 1;
}

int OSSL_HPKE_CTX_set1_ikme(OSSL_HPKE_CTX *ctx,
                            const unsigned char *ikme, size_t ikmelen)
{
    if (ctx == NULL || ikme == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (ikmelen == 0 || ikmelen > OSSL_HPKE_MAX_PARMLEN) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (ctx->role != OSSL_HPKE_ROLE_SENDER) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    OPENSSL_clear_free(ctx->ikme, ctx->ikmelen);
    ctx->ikme = OPENSSL_memdup(ikme, ikmelen);
    if (ctx->ikme == NULL)
        return 0;
    ctx->ikmelen = ikmelen;
    return 1;
}

int OSSL_HPKE_CTX_set1_authpriv(OSSL_HPKE_CTX *ctx, EVP_PKEY *priv)
{
    if (ctx == NULL || priv == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (ctx->mode != OSSL_HPKE_MODE_AUTH
        && ctx->mode != OSSL_HPKE_MODE_PSKAUTH) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (ctx->role != OSSL_HPKE_ROLE_SENDER) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    EVP_PKEY_free(ctx->authpriv);
    ctx->authpriv = EVP_PKEY_dup(priv);
    if (ctx->authpriv == NULL)
        return 0;
    return 1;
}

int OSSL_HPKE_CTX_set1_authpub(OSSL_HPKE_CTX *ctx,
                               const unsigned char *pub, size_t publen)
{
    int erv = 0;
    EVP_PKEY *pubp = NULL;
    unsigned char *lpub = NULL;
    size_t lpublen = 0;
    const OSSL_HPKE_KEM_INFO *kem_info = NULL;

    if (ctx == NULL || pub == NULL || publen == 0) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (ctx->mode != OSSL_HPKE_MODE_AUTH
        && ctx->mode != OSSL_HPKE_MODE_PSKAUTH) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (ctx->role != OSSL_HPKE_ROLE_RECEIVER) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    /* check the value seems like a good public key for this kem */
    kem_info = ossl_HPKE_KEM_INFO_find_id(ctx->suite.kem_id);
    if (kem_info == NULL)
        return 0;
    if (hpke_kem_id_nist_curve(ctx->suite.kem_id) == 1) {
        pubp = evp_pkey_new_raw_nist_public_key(ctx->libctx, ctx->propq,
                                                kem_info->groupname,
                                                pub, publen);
    } else {
        pubp = EVP_PKEY_new_raw_public_key_ex(ctx->libctx,
                                              kem_info->keytype,
                                              ctx->propq,
                                              pub, publen);
    }
    if (pubp == NULL) {
        /* can happen based on external input - buffer value may be garbage */
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        goto err;
    }
    /*
     * extract out the public key in encoded form so we
     * should be fine even if given compressed form
     */
    lpub = OPENSSL_malloc(OSSL_HPKE_MAXSIZE);
    if (lpub == NULL)
        goto err;
    if (EVP_PKEY_get_octet_string_param(pubp,
                                        OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY,
                                        lpub, OSSL_HPKE_MAXSIZE, &lpublen)
        != 1) {
        OPENSSL_free(lpub);
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    /* free up old value */
    OPENSSL_free(ctx->authpub);
    ctx->authpub = lpub;
    ctx->authpublen = lpublen;
    erv = 1;

err:
    EVP_PKEY_free(pubp);
    return erv;
}

int OSSL_HPKE_CTX_get_seq(OSSL_HPKE_CTX *ctx, uint64_t *seq)
{
    if (ctx == NULL || seq == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    *seq = ctx->seq;
    return 1;
}

int OSSL_HPKE_CTX_set_seq(OSSL_HPKE_CTX *ctx, uint64_t seq)
{
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    /*
     * We disallow senders from doing this as it's dangerous
     * Receivers are ok to use this, as no harm should ensue
     * if they go wrong.
     */
    if (ctx->role == OSSL_HPKE_ROLE_SENDER) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    ctx->seq = seq;
    return 1;
}

int OSSL_HPKE_encap(OSSL_HPKE_CTX *ctx,
                    unsigned char *enc, size_t *enclen,
                    const unsigned char *pub, size_t publen,
                    const unsigned char *info, size_t infolen)
{
    int erv = 1;
    size_t minenc = 0;

    if (ctx == NULL || enc == NULL || enclen == NULL || *enclen == 0
        || pub == NULL || publen == 0) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (ctx->role != OSSL_HPKE_ROLE_SENDER) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (infolen > OSSL_HPKE_MAX_INFOLEN) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (infolen > 0 && info == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    minenc = OSSL_HPKE_get_public_encap_size(ctx->suite);
    if (minenc == 0 || minenc > *enclen) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (ctx->shared_secret != NULL) {
        /* only allow one encap per OSSL_HPKE_CTX */
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    if (hpke_encap(ctx, enc, enclen, pub, publen) != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        return 0;
    }
    /*
     * note that the info is not part of the context as it
     * only needs to be used once here so doesn't need to
     * be stored
     */
    erv = hpke_do_middle(ctx, info, infolen);
    return erv;
}

int OSSL_HPKE_decap(OSSL_HPKE_CTX *ctx,
                    const unsigned char *enc, size_t enclen,
                    EVP_PKEY *recippriv,
                    const unsigned char *info, size_t infolen)
{
    int erv = 1;
    size_t minenc = 0;

    if (ctx == NULL || enc == NULL || enclen == 0 || recippriv == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (ctx->role != OSSL_HPKE_ROLE_RECEIVER) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (infolen > OSSL_HPKE_MAX_INFOLEN) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (infolen > 0 && info == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    minenc = OSSL_HPKE_get_public_encap_size(ctx->suite);
    if (minenc == 0 || minenc > enclen) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (ctx->shared_secret != NULL) {
        /* only allow one encap per OSSL_HPKE_CTX */
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    erv = hpke_decap(ctx, enc, enclen, recippriv);
    if (erv != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        return 0;
    }
    /*
     * note that the info is not part of the context as it
     * only needs to be used once here so doesn't need to
     * be stored
     */
    erv = hpke_do_middle(ctx, info, infolen);
    return erv;
}

int OSSL_HPKE_seal(OSSL_HPKE_CTX *ctx,
                   unsigned char *ct, size_t *ctlen,
                   const unsigned char *aad, size_t aadlen,
                   const unsigned char *pt, size_t ptlen)
{
    unsigned char seqbuf[OSSL_HPKE_MAX_NONCELEN];
    size_t seqlen = 0;

    if (ctx == NULL || ct == NULL || ctlen == NULL || *ctlen == 0
        || pt == NULL || ptlen == 0) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (ctx->role != OSSL_HPKE_ROLE_SENDER) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if ((ctx->seq + 1) == 0) { /* wrap around imminent !!! */
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    if (ctx->key == NULL || ctx->nonce == NULL) {
        /* need to have done an encap first, info can be NULL */
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    seqlen = hpke_seqnonce2buf(ctx, seqbuf, sizeof(seqbuf));
    if (seqlen == 0) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        return 0;
    }
    if (hpke_aead_enc(ctx, seqbuf, aad, aadlen, pt, ptlen, ct, ctlen) != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        OPENSSL_cleanse(seqbuf, sizeof(seqbuf));
        return 0;
    } else {
        ctx->seq++;
    }
    OPENSSL_cleanse(seqbuf, sizeof(seqbuf));
    return 1;
}

int OSSL_HPKE_open(OSSL_HPKE_CTX *ctx,
                   unsigned char *pt, size_t *ptlen,
                   const unsigned char *aad, size_t aadlen,
                   const unsigned char *ct, size_t ctlen)
{
    unsigned char seqbuf[OSSL_HPKE_MAX_NONCELEN];
    size_t seqlen = 0;

    if (ctx == NULL || pt == NULL || ptlen == NULL || *ptlen == 0
        || ct == NULL || ctlen == 0) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (ctx->role != OSSL_HPKE_ROLE_RECEIVER) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if ((ctx->seq + 1) == 0) { /* wrap around imminent !!! */
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    if (ctx->key == NULL || ctx->nonce == NULL) {
        /* need to have done an encap first, info can be NULL */
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    seqlen = hpke_seqnonce2buf(ctx, seqbuf, sizeof(seqbuf));
    if (seqlen == 0) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        return 0;
    }
    if (hpke_aead_dec(ctx, seqbuf, aad, aadlen, ct, ctlen, pt, ptlen) != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        OPENSSL_cleanse(seqbuf, sizeof(seqbuf));
        return 0;
    }
    ctx->seq++;
    OPENSSL_cleanse(seqbuf, sizeof(seqbuf));
    return 1;
}

int OSSL_HPKE_export(OSSL_HPKE_CTX *ctx,
                     unsigned char *secret, size_t secretlen,
                     const unsigned char *label, size_t labellen)
{
    int erv = 0;
    EVP_KDF_CTX *kctx = NULL;
    unsigned char suitebuf[6];
    const char *mdname = NULL;
    const OSSL_HPKE_KDF_INFO *kdf_info = NULL;

    if (ctx == NULL || secret == NULL || secretlen == 0) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (labellen > OSSL_HPKE_MAX_PARMLEN) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (labellen > 0 && label == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (ctx->exportersec == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    kdf_info = ossl_HPKE_KDF_INFO_find_id(ctx->suite.kdf_id);
    if (kdf_info == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        return 0;
    }
    mdname = kdf_info->mdname;
    kctx = ossl_kdf_ctx_create("HKDF", mdname, ctx->libctx, ctx->propq);
    if (kctx == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        return 0;
    }
    /* full suiteid as per RFC9180 sec 5.3 */
    suitebuf[0] = ctx->suite.kem_id / 256;
    suitebuf[1] = ctx->suite.kem_id % 256;
    suitebuf[2] = ctx->suite.kdf_id / 256;
    suitebuf[3] = ctx->suite.kdf_id % 256;
    suitebuf[4] = ctx->suite.aead_id / 256;
    suitebuf[5] = ctx->suite.aead_id % 256;
    erv = ossl_hpke_labeled_expand(kctx, secret, secretlen,
                                   ctx->exportersec, ctx->exporterseclen,
                                   OSSL_HPKE_SEC51LABEL,
                                   suitebuf, sizeof(suitebuf),
                                   OSSL_HPKE_EXP_SEC_LABEL,
                                   label, labellen);
    EVP_KDF_CTX_free(kctx);
    if (erv != 1)
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
    return erv;
}

int OSSL_HPKE_keygen(OSSL_HPKE_SUITE suite,
                     unsigned char *pub, size_t *publen, EVP_PKEY **priv,
                     const unsigned char *ikm, size_t ikmlen,
                     OSSL_LIB_CTX *libctx, const char *propq)
{
    int erv = 0; /* Our error return value - 1 is success */
    EVP_PKEY_CTX *pctx = NULL;
    EVP_PKEY *skR = NULL;
    const OSSL_HPKE_KEM_INFO *kem_info = NULL;
    OSSL_PARAM params[3], *p = params;

    if (pub == NULL || publen == NULL || *publen == 0 || priv == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (hpke_suite_check(suite, &kem_info, NULL, NULL) != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if ((ikmlen > 0 && ikm == NULL)
        || (ikmlen == 0 && ikm != NULL)
        || ikmlen > OSSL_HPKE_MAX_PARMLEN) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }

    if (hpke_kem_id_nist_curve(suite.kem_id) == 1) {
        *p++ = OSSL_PARAM_construct_utf8_string(OSSL_PKEY_PARAM_GROUP_NAME,
                                                (char *)kem_info->groupname, 0);
        pctx = EVP_PKEY_CTX_new_from_name(libctx, "EC", propq);
    } else {
        pctx = EVP_PKEY_CTX_new_from_name(libctx, kem_info->keytype, propq);
    }
    if (pctx == NULL
        || EVP_PKEY_keygen_init(pctx) <= 0) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    if (ikm != NULL)
        *p++ = OSSL_PARAM_construct_octet_string(OSSL_PKEY_PARAM_DHKEM_IKM,
                                                 (char *)ikm, ikmlen);
    *p = OSSL_PARAM_construct_end();
    if (EVP_PKEY_CTX_set_params(pctx, params) <= 0) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    if (EVP_PKEY_generate(pctx, &skR) <= 0) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    EVP_PKEY_CTX_free(pctx);
    pctx = NULL;
    if (EVP_PKEY_get_octet_string_param(skR, OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY,
                                        pub, *publen, publen) != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    *priv = skR;
    erv = 1;

err:
    if (erv != 1)
        EVP_PKEY_free(skR);
    EVP_PKEY_CTX_free(pctx);
    return erv;
}

int OSSL_HPKE_suite_check(OSSL_HPKE_SUITE suite)
{
    return hpke_suite_check(suite, NULL, NULL, NULL);
}

int OSSL_HPKE_get_grease_value(const OSSL_HPKE_SUITE *suite_in,
                               OSSL_HPKE_SUITE *suite,
                               unsigned char *enc, size_t *enclen,
                               unsigned char *ct, size_t ctlen,
                               OSSL_LIB_CTX *libctx, const char *propq)
{
    OSSL_HPKE_SUITE chosen;
    size_t plen = 0;
    const OSSL_HPKE_KEM_INFO *kem_info = NULL;
    const OSSL_HPKE_AEAD_INFO *aead_info = NULL;
    EVP_PKEY *fakepriv = NULL;

    if (enc == NULL || enclen == 0
        || ct == NULL || ctlen == 0 || suite == NULL) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (suite_in == NULL) {
        /* choose a random suite */
        if (hpke_random_suite(libctx, propq, &chosen) != 1) {
            ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
            goto err;
        }
    } else {
        chosen = *suite_in;
    }
    if (hpke_suite_check(chosen, &kem_info, NULL, &aead_info) != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    *suite = chosen;
    /* make sure room for tag and one plaintext octet */
    if (aead_info->taglen >= ctlen) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    /* publen */
    plen = kem_info->Npk;
    if (plen > *enclen) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    /*
     * In order for our enc to look good for sure, we generate and then
     * delete a real key for that curve - bit OTT but it ensures we do
     * get the encoding right (e.g. 0x04 as 1st octet for NIST curves in
     * uncompressed form) and that the value really does map to a point on
     * the relevant curve.
     */
    if (OSSL_HPKE_keygen(chosen, enc, enclen, &fakepriv, NULL, 0,
                         libctx, propq) != 1) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    EVP_PKEY_free(fakepriv);
    if (RAND_bytes_ex(libctx, ct, ctlen, 0) <= 0) {
        ERR_raise(ERR_LIB_CRYPTO, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    return 1;
err:
    return 0;
}

int OSSL_HPKE_str2suite(const char *str, OSSL_HPKE_SUITE *suite)
{
    return ossl_hpke_str2suite(str, suite);
}

size_t OSSL_HPKE_get_ciphertext_size(OSSL_HPKE_SUITE suite, size_t clearlen)
{
    size_t enclen = 0;
    size_t cipherlen = 0;

    if (hpke_expansion(suite, &enclen, clearlen, &cipherlen) != 1)
        return 0;
    return cipherlen;
}

size_t OSSL_HPKE_get_public_encap_size(OSSL_HPKE_SUITE suite)
{
    size_t enclen = 0;
    size_t cipherlen = 0;
    size_t clearlen = 16;

    if (hpke_expansion(suite, &enclen, clearlen, &cipherlen) != 1)
        return 0;
    return enclen;
}

size_t OSSL_HPKE_get_recommended_ikmelen(OSSL_HPKE_SUITE suite)
{
    const OSSL_HPKE_KEM_INFO *kem_info = NULL;

    if (hpke_suite_check(suite, &kem_info, NULL, NULL) != 1)
        return 0;
    if (kem_info == NULL)
        return 0;

    return kem_info->Nsk;
}
