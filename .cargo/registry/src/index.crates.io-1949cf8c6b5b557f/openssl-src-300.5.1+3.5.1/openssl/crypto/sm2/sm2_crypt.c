/*
 * Copyright 2017-2024 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright 2017 Ribose Inc. All Rights Reserved.
 * Ported from Ribose contributions from Botan.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * ECDSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include "crypto/sm2.h"
#include "crypto/sm2err.h"
#include "crypto/ec.h" /* ossl_ecdh_kdf_X9_63() */
#include <openssl/err.h>
#include <openssl/evp.h>
#include <openssl/bn.h>
#include <openssl/asn1.h>
#include <openssl/asn1t.h>
#include <string.h>

typedef struct SM2_Ciphertext_st SM2_Ciphertext;
DECLARE_ASN1_FUNCTIONS(SM2_Ciphertext)

struct SM2_Ciphertext_st {
    BIGNUM *C1x;
    BIGNUM *C1y;
    ASN1_OCTET_STRING *C3;
    ASN1_OCTET_STRING *C2;
};

ASN1_SEQUENCE(SM2_Ciphertext) = {
    ASN1_SIMPLE(SM2_Ciphertext, C1x, BIGNUM),
    ASN1_SIMPLE(SM2_Ciphertext, C1y, BIGNUM),
    ASN1_SIMPLE(SM2_Ciphertext, C3, ASN1_OCTET_STRING),
    ASN1_SIMPLE(SM2_Ciphertext, C2, ASN1_OCTET_STRING),
} ASN1_SEQUENCE_END(SM2_Ciphertext)

IMPLEMENT_ASN1_FUNCTIONS(SM2_Ciphertext)

static size_t ec_field_size(const EC_GROUP *group)
{
    const BIGNUM *p = EC_GROUP_get0_field(group);

    if (p == NULL)
        return 0;

    return BN_num_bytes(p);
}

static int is_all_zeros(const unsigned char *msg, size_t msglen)
{
    unsigned char re = 0;
    size_t i;

    for (i = 0; i < msglen; i++) {
        re |= msg[i];
    }

    return re == 0 ? 1 : 0;
}

int ossl_sm2_plaintext_size(const unsigned char *ct, size_t ct_size,
                            size_t *pt_size)
{
    struct SM2_Ciphertext_st *sm2_ctext = NULL;

    sm2_ctext = d2i_SM2_Ciphertext(NULL, &ct, ct_size);

    if (sm2_ctext == NULL) {
        ERR_raise(ERR_LIB_SM2, SM2_R_INVALID_ENCODING);
        return 0;
    }

    *pt_size = sm2_ctext->C2->length;
    SM2_Ciphertext_free(sm2_ctext);

    return 1;
}

int ossl_sm2_ciphertext_size(const EC_KEY *key, const EVP_MD *digest,
                             size_t msg_len, size_t *ct_size)
{
    const size_t field_size = ec_field_size(EC_KEY_get0_group(key));
    const int md_size = EVP_MD_get_size(digest);
    size_t sz;

    if (field_size == 0 || md_size <= 0)
        return 0;

    /* Integer and string are simple type; set constructed = 0, means primitive and definite length encoding. */
    sz = 2 * ASN1_object_size(0, field_size + 1, V_ASN1_INTEGER)
         + ASN1_object_size(0, md_size, V_ASN1_OCTET_STRING)
         + ASN1_object_size(0, msg_len, V_ASN1_OCTET_STRING);
    /* Sequence is structured type; set constructed = 1, means constructed and definite length encoding. */
    *ct_size = ASN1_object_size(1, sz, V_ASN1_SEQUENCE);

    return 1;
}

int ossl_sm2_encrypt(const EC_KEY *key,
                     const EVP_MD *digest,
                     const uint8_t *msg, size_t msg_len,
                     uint8_t *ciphertext_buf, size_t *ciphertext_len)
{
    int rc = 0, ciphertext_leni;
    size_t i;
    BN_CTX *ctx = NULL;
    BIGNUM *k = NULL;
    BIGNUM *x1 = NULL;
    BIGNUM *y1 = NULL;
    BIGNUM *x2 = NULL;
    BIGNUM *y2 = NULL;
    EVP_MD_CTX *hash = EVP_MD_CTX_new();
    struct SM2_Ciphertext_st ctext_struct;
    const EC_GROUP *group = EC_KEY_get0_group(key);
    const BIGNUM *order = EC_GROUP_get0_order(group);
    const EC_POINT *P = EC_KEY_get0_public_key(key);
    EC_POINT *kG = NULL;
    EC_POINT *kP = NULL;
    uint8_t *msg_mask = NULL;
    uint8_t *x2y2 = NULL;
    uint8_t *C3 = NULL;
    size_t field_size;
    const int C3_size = EVP_MD_get_size(digest);
    EVP_MD *fetched_digest = NULL;
    OSSL_LIB_CTX *libctx = ossl_ec_key_get_libctx(key);
    const char *propq = ossl_ec_key_get0_propq(key);

    /* NULL these before any "goto done" */
    ctext_struct.C2 = NULL;
    ctext_struct.C3 = NULL;

    if (hash == NULL || C3_size <= 0) {
        ERR_raise(ERR_LIB_SM2, ERR_R_INTERNAL_ERROR);
        goto done;
    }

    field_size = ec_field_size(group);
    if (field_size == 0) {
        ERR_raise(ERR_LIB_SM2, ERR_R_INTERNAL_ERROR);
        goto done;
    }

    kG = EC_POINT_new(group);
    kP = EC_POINT_new(group);
    if (kG == NULL || kP == NULL) {
        ERR_raise(ERR_LIB_SM2, ERR_R_EC_LIB);
        goto done;
    }
    ctx = BN_CTX_new_ex(libctx);
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_SM2, ERR_R_BN_LIB);
        goto done;
    }

    BN_CTX_start(ctx);
    k = BN_CTX_get(ctx);
    x1 = BN_CTX_get(ctx);
    x2 = BN_CTX_get(ctx);
    y1 = BN_CTX_get(ctx);
    y2 = BN_CTX_get(ctx);

    if (y2 == NULL) {
        ERR_raise(ERR_LIB_SM2, ERR_R_BN_LIB);
        goto done;
    }

    x2y2 = OPENSSL_zalloc(2 * field_size);
    C3 = OPENSSL_zalloc(C3_size);

    if (x2y2 == NULL || C3 == NULL)
        goto done;

    memset(ciphertext_buf, 0, *ciphertext_len);

    msg_mask = OPENSSL_zalloc(msg_len);
    if (msg_mask == NULL)
       goto done;

again:
    if (!BN_priv_rand_range_ex(k, order, 0, ctx)) {
        ERR_raise(ERR_LIB_SM2, ERR_R_INTERNAL_ERROR);
        goto done;
    }

    if (!EC_POINT_mul(group, kG, k, NULL, NULL, ctx)
            || !EC_POINT_get_affine_coordinates(group, kG, x1, y1, ctx)
            || !EC_POINT_mul(group, kP, NULL, P, k, ctx)
            || !EC_POINT_get_affine_coordinates(group, kP, x2, y2, ctx)) {
        ERR_raise(ERR_LIB_SM2, ERR_R_EC_LIB);
        goto done;
    }

    if (BN_bn2binpad(x2, x2y2, field_size) < 0
            || BN_bn2binpad(y2, x2y2 + field_size, field_size) < 0) {
        ERR_raise(ERR_LIB_SM2, ERR_R_INTERNAL_ERROR);
        goto done;
    }

    /* X9.63 with no salt happens to match the KDF used in SM2 */
    if (!ossl_ecdh_kdf_X9_63(msg_mask, msg_len, x2y2, 2 * field_size, NULL, 0,
                             digest, libctx, propq)) {
        ERR_raise(ERR_LIB_SM2, ERR_R_EVP_LIB);
        goto done;
    }

    if (is_all_zeros(msg_mask, msg_len)) {
        memset(x2y2, 0, 2 * field_size);
        goto again;
    }

    for (i = 0; i != msg_len; ++i)
        msg_mask[i] ^= msg[i];

    fetched_digest = EVP_MD_fetch(libctx, EVP_MD_get0_name(digest), propq);
    if (fetched_digest == NULL) {
        ERR_raise(ERR_LIB_SM2, ERR_R_INTERNAL_ERROR);
        goto done;
    }
    if (EVP_DigestInit(hash, fetched_digest) == 0
            || EVP_DigestUpdate(hash, x2y2, field_size) == 0
            || EVP_DigestUpdate(hash, msg, msg_len) == 0
            || EVP_DigestUpdate(hash, x2y2 + field_size, field_size) == 0
            || EVP_DigestFinal(hash, C3, NULL) == 0) {
        ERR_raise(ERR_LIB_SM2, ERR_R_EVP_LIB);
        goto done;
    }

    ctext_struct.C1x = x1;
    ctext_struct.C1y = y1;
    ctext_struct.C3 = ASN1_OCTET_STRING_new();
    ctext_struct.C2 = ASN1_OCTET_STRING_new();

    if (ctext_struct.C3 == NULL || ctext_struct.C2 == NULL) {
       ERR_raise(ERR_LIB_SM2, ERR_R_ASN1_LIB);
       goto done;
    }
    if (!ASN1_OCTET_STRING_set(ctext_struct.C3, C3, C3_size)
            || !ASN1_OCTET_STRING_set(ctext_struct.C2, msg_mask, msg_len)) {
        ERR_raise(ERR_LIB_SM2, ERR_R_INTERNAL_ERROR);
        goto done;
    }

    ciphertext_leni = i2d_SM2_Ciphertext(&ctext_struct, &ciphertext_buf);
    /* Ensure cast to size_t is safe */
    if (ciphertext_leni < 0) {
        ERR_raise(ERR_LIB_SM2, ERR_R_INTERNAL_ERROR);
        goto done;
    }
    *ciphertext_len = (size_t)ciphertext_leni;

    rc = 1;

 done:
    EVP_MD_free(fetched_digest);
    ASN1_OCTET_STRING_free(ctext_struct.C2);
    ASN1_OCTET_STRING_free(ctext_struct.C3);
    OPENSSL_free(msg_mask);
    OPENSSL_free(x2y2);
    OPENSSL_free(C3);
    EVP_MD_CTX_free(hash);
    BN_CTX_free(ctx);
    EC_POINT_free(kG);
    EC_POINT_free(kP);
    return rc;
}

int ossl_sm2_decrypt(const EC_KEY *key,
                     const EVP_MD *digest,
                     const uint8_t *ciphertext, size_t ciphertext_len,
                     uint8_t *ptext_buf, size_t *ptext_len)
{
    int rc = 0;
    int i;
    BN_CTX *ctx = NULL;
    const EC_GROUP *group = EC_KEY_get0_group(key);
    EC_POINT *C1 = NULL;
    struct SM2_Ciphertext_st *sm2_ctext = NULL;
    BIGNUM *x2 = NULL;
    BIGNUM *y2 = NULL;
    uint8_t *x2y2 = NULL;
    uint8_t *computed_C3 = NULL;
    const size_t field_size = ec_field_size(group);
    const int hash_size = EVP_MD_get_size(digest);
    uint8_t *msg_mask = NULL;
    const uint8_t *C2 = NULL;
    const uint8_t *C3 = NULL;
    int msg_len = 0;
    EVP_MD_CTX *hash = NULL;
    OSSL_LIB_CTX *libctx = ossl_ec_key_get_libctx(key);
    const char *propq = ossl_ec_key_get0_propq(key);

    if (field_size == 0 || hash_size <= 0)
       goto done;

    memset(ptext_buf, 0xFF, *ptext_len);

    sm2_ctext = d2i_SM2_Ciphertext(NULL, &ciphertext, ciphertext_len);

    if (sm2_ctext == NULL) {
        ERR_raise(ERR_LIB_SM2, SM2_R_ASN1_ERROR);
        goto done;
    }

    if (sm2_ctext->C3->length != hash_size) {
        ERR_raise(ERR_LIB_SM2, SM2_R_INVALID_ENCODING);
        goto done;
    }

    C2 = sm2_ctext->C2->data;
    C3 = sm2_ctext->C3->data;
    msg_len = sm2_ctext->C2->length;
    if (*ptext_len < (size_t)msg_len) {
        ERR_raise(ERR_LIB_SM2, SM2_R_BUFFER_TOO_SMALL);
        goto done;
    }

    ctx = BN_CTX_new_ex(libctx);
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_SM2, ERR_R_BN_LIB);
        goto done;
    }

    BN_CTX_start(ctx);
    x2 = BN_CTX_get(ctx);
    y2 = BN_CTX_get(ctx);

    if (y2 == NULL) {
        ERR_raise(ERR_LIB_SM2, ERR_R_BN_LIB);
        goto done;
    }

    msg_mask = OPENSSL_zalloc(msg_len);
    x2y2 = OPENSSL_zalloc(2 * field_size);
    computed_C3 = OPENSSL_zalloc(hash_size);

    if (msg_mask == NULL || x2y2 == NULL || computed_C3 == NULL)
        goto done;

    C1 = EC_POINT_new(group);
    if (C1 == NULL) {
        ERR_raise(ERR_LIB_SM2, ERR_R_EC_LIB);
        goto done;
    }

    if (!EC_POINT_set_affine_coordinates(group, C1, sm2_ctext->C1x,
                                         sm2_ctext->C1y, ctx)
            || !EC_POINT_mul(group, C1, NULL, C1, EC_KEY_get0_private_key(key),
                             ctx)
            || !EC_POINT_get_affine_coordinates(group, C1, x2, y2, ctx)) {
        ERR_raise(ERR_LIB_SM2, ERR_R_EC_LIB);
        goto done;
    }

    if (BN_bn2binpad(x2, x2y2, field_size) < 0
            || BN_bn2binpad(y2, x2y2 + field_size, field_size) < 0
            || !ossl_ecdh_kdf_X9_63(msg_mask, msg_len, x2y2, 2 * field_size,
                                    NULL, 0, digest, libctx, propq)) {
        ERR_raise(ERR_LIB_SM2, ERR_R_INTERNAL_ERROR);
        goto done;
    }

    if (is_all_zeros(msg_mask, msg_len)) {
        ERR_raise(ERR_LIB_SM2, SM2_R_INVALID_ENCODING);
        goto done;
    }

    for (i = 0; i != msg_len; ++i)
        ptext_buf[i] = C2[i] ^ msg_mask[i];

    hash = EVP_MD_CTX_new();
    if (hash == NULL) {
        ERR_raise(ERR_LIB_SM2, ERR_R_EVP_LIB);
        goto done;
    }

    if (!EVP_DigestInit(hash, digest)
            || !EVP_DigestUpdate(hash, x2y2, field_size)
            || !EVP_DigestUpdate(hash, ptext_buf, msg_len)
            || !EVP_DigestUpdate(hash, x2y2 + field_size, field_size)
            || !EVP_DigestFinal(hash, computed_C3, NULL)) {
        ERR_raise(ERR_LIB_SM2, ERR_R_EVP_LIB);
        goto done;
    }

    if (CRYPTO_memcmp(computed_C3, C3, hash_size) != 0) {
        ERR_raise(ERR_LIB_SM2, SM2_R_INVALID_DIGEST);
        goto done;
    }

    rc = 1;
    *ptext_len = msg_len;

 done:
    if (rc == 0)
        memset(ptext_buf, 0, *ptext_len);

    OPENSSL_free(msg_mask);
    OPENSSL_free(x2y2);
    OPENSSL_free(computed_C3);
    EC_POINT_free(C1);
    BN_CTX_free(ctx);
    SM2_Ciphertext_free(sm2_ctext);
    EVP_MD_CTX_free(hash);

    return rc;
}
