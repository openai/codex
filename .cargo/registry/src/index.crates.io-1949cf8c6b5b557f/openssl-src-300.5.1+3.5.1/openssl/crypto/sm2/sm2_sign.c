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

#include "internal/deprecated.h"

#include "crypto/sm2.h"
#include "crypto/sm2err.h"
#include "crypto/ec.h" /* ossl_ec_group_do_inverse_ord() */
#include "internal/numbers.h"
#include <openssl/err.h>
#include <openssl/evp.h>
#include <openssl/bn.h>
#include <string.h>

int ossl_sm2_compute_z_digest(uint8_t *out,
                              const EVP_MD *digest,
                              const uint8_t *id,
                              const size_t id_len,
                              const EC_KEY *key)
{
    int rc = 0;
    const EC_GROUP *group = EC_KEY_get0_group(key);
    const EC_POINT *pubkey = EC_KEY_get0_public_key(key);
    BN_CTX *ctx = NULL;
    EVP_MD_CTX *hash = NULL;
    BIGNUM *p = NULL;
    BIGNUM *a = NULL;
    BIGNUM *b = NULL;
    BIGNUM *xG = NULL;
    BIGNUM *yG = NULL;
    BIGNUM *xA = NULL;
    BIGNUM *yA = NULL;
    int p_bytes = 0;
    uint8_t *buf = NULL;
    uint16_t entl = 0;
    uint8_t e_byte = 0;

    /* SM2 Signatures require a public key, check for it */
    if (pubkey == NULL) {
        ERR_raise(ERR_LIB_SM2, ERR_R_PASSED_NULL_PARAMETER);
        goto done;
    }

    hash = EVP_MD_CTX_new();
    if (hash == NULL) {
        ERR_raise(ERR_LIB_SM2, ERR_R_EVP_LIB);
        goto done;
    }
    ctx = BN_CTX_new_ex(ossl_ec_key_get_libctx(key));
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_SM2, ERR_R_BN_LIB);
        goto done;
    }

    p = BN_CTX_get(ctx);
    a = BN_CTX_get(ctx);
    b = BN_CTX_get(ctx);
    xG = BN_CTX_get(ctx);
    yG = BN_CTX_get(ctx);
    xA = BN_CTX_get(ctx);
    yA = BN_CTX_get(ctx);

    if (yA == NULL) {
        ERR_raise(ERR_LIB_SM2, ERR_R_BN_LIB);
        goto done;
    }

    if (!EVP_DigestInit(hash, digest)) {
        ERR_raise(ERR_LIB_SM2, ERR_R_EVP_LIB);
        goto done;
    }

    /* Z = h(ENTL || ID || a || b || xG || yG || xA || yA) */

    if (id_len >= (UINT16_MAX / 8)) {
        /* too large */
        ERR_raise(ERR_LIB_SM2, SM2_R_ID_TOO_LARGE);
        goto done;
    }

    entl = (uint16_t)(8 * id_len);

    e_byte = entl >> 8;
    if (!EVP_DigestUpdate(hash, &e_byte, 1)) {
        ERR_raise(ERR_LIB_SM2, ERR_R_EVP_LIB);
        goto done;
    }
    e_byte = entl & 0xFF;
    if (!EVP_DigestUpdate(hash, &e_byte, 1)) {
        ERR_raise(ERR_LIB_SM2, ERR_R_EVP_LIB);
        goto done;
    }

    if (id_len > 0 && !EVP_DigestUpdate(hash, id, id_len)) {
        ERR_raise(ERR_LIB_SM2, ERR_R_EVP_LIB);
        goto done;
    }

    if (!EC_GROUP_get_curve(group, p, a, b, ctx)) {
        ERR_raise(ERR_LIB_SM2, ERR_R_EC_LIB);
        goto done;
    }

    p_bytes = BN_num_bytes(p);
    buf = OPENSSL_zalloc(p_bytes);
    if (buf == NULL)
        goto done;

    if (BN_bn2binpad(a, buf, p_bytes) < 0
            || !EVP_DigestUpdate(hash, buf, p_bytes)
            || BN_bn2binpad(b, buf, p_bytes) < 0
            || !EVP_DigestUpdate(hash, buf, p_bytes)
            || !EC_POINT_get_affine_coordinates(group,
                                                EC_GROUP_get0_generator(group),
                                                xG, yG, ctx)
            || BN_bn2binpad(xG, buf, p_bytes) < 0
            || !EVP_DigestUpdate(hash, buf, p_bytes)
            || BN_bn2binpad(yG, buf, p_bytes) < 0
            || !EVP_DigestUpdate(hash, buf, p_bytes)
            || !EC_POINT_get_affine_coordinates(group,
                                                pubkey,
                                                xA, yA, ctx)
            || BN_bn2binpad(xA, buf, p_bytes) < 0
            || !EVP_DigestUpdate(hash, buf, p_bytes)
            || BN_bn2binpad(yA, buf, p_bytes) < 0
            || !EVP_DigestUpdate(hash, buf, p_bytes)
            || !EVP_DigestFinal(hash, out, NULL)) {
        ERR_raise(ERR_LIB_SM2, ERR_R_INTERNAL_ERROR);
        goto done;
    }

    rc = 1;

 done:
    OPENSSL_free(buf);
    BN_CTX_free(ctx);
    EVP_MD_CTX_free(hash);
    return rc;
}

static BIGNUM *sm2_compute_msg_hash(const EVP_MD *digest,
                                    const EC_KEY *key,
                                    const uint8_t *id,
                                    const size_t id_len,
                                    const uint8_t *msg, size_t msg_len)
{
    EVP_MD_CTX *hash = EVP_MD_CTX_new();
    const int md_size = EVP_MD_get_size(digest);
    uint8_t *z = NULL;
    BIGNUM *e = NULL;
    EVP_MD *fetched_digest = NULL;
    OSSL_LIB_CTX *libctx = ossl_ec_key_get_libctx(key);
    const char *propq = ossl_ec_key_get0_propq(key);

    if (md_size <= 0) {
        ERR_raise(ERR_LIB_SM2, SM2_R_INVALID_DIGEST);
        goto done;
    }
    if (hash == NULL) {
        ERR_raise(ERR_LIB_SM2, ERR_R_EVP_LIB);
        goto done;
    }

    z = OPENSSL_zalloc(md_size);
    if (z == NULL)
        goto done;

    fetched_digest = EVP_MD_fetch(libctx, EVP_MD_get0_name(digest), propq);
    if (fetched_digest == NULL) {
        ERR_raise(ERR_LIB_SM2, ERR_R_INTERNAL_ERROR);
        goto done;
    }

    if (!ossl_sm2_compute_z_digest(z, fetched_digest, id, id_len, key)) {
        /* SM2err already called */
        goto done;
    }

    if (!EVP_DigestInit(hash, fetched_digest)
            || !EVP_DigestUpdate(hash, z, md_size)
            || !EVP_DigestUpdate(hash, msg, msg_len)
               /* reuse z buffer to hold H(Z || M) */
            || !EVP_DigestFinal(hash, z, NULL)) {
        ERR_raise(ERR_LIB_SM2, ERR_R_EVP_LIB);
        goto done;
    }

    e = BN_bin2bn(z, md_size, NULL);
    if (e == NULL)
        ERR_raise(ERR_LIB_SM2, ERR_R_INTERNAL_ERROR);

 done:
    EVP_MD_free(fetched_digest);
    OPENSSL_free(z);
    EVP_MD_CTX_free(hash);
    return e;
}

static ECDSA_SIG *sm2_sig_gen(const EC_KEY *key, const BIGNUM *e)
{
    const BIGNUM *dA = EC_KEY_get0_private_key(key);
    const EC_GROUP *group = EC_KEY_get0_group(key);
    const BIGNUM *order = EC_GROUP_get0_order(group);
    ECDSA_SIG *sig = NULL;
    EC_POINT *kG = NULL;
    BN_CTX *ctx = NULL;
    BIGNUM *k = NULL;
    BIGNUM *rk = NULL;
    BIGNUM *r = NULL;
    BIGNUM *s = NULL;
    BIGNUM *x1 = NULL;
    BIGNUM *tmp = NULL;
    OSSL_LIB_CTX *libctx = ossl_ec_key_get_libctx(key);

    kG = EC_POINT_new(group);
    if (kG == NULL) {
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
    rk = BN_CTX_get(ctx);
    x1 = BN_CTX_get(ctx);
    tmp = BN_CTX_get(ctx);
    if (tmp == NULL) {
        ERR_raise(ERR_LIB_SM2, ERR_R_BN_LIB);
        goto done;
    }

    /*
     * These values are returned and so should not be allocated out of the
     * context
     */
    r = BN_new();
    s = BN_new();

    if (r == NULL || s == NULL) {
        ERR_raise(ERR_LIB_SM2, ERR_R_BN_LIB);
        goto done;
    }

    /*
     * A3: Generate a random number k in [1,n-1] using random number generators;
     * A4: Compute (x1,y1)=[k]G, and convert the type of data x1 to be integer
     *     as specified in clause 4.2.8 of GM/T 0003.1-2012;
     * A5: Compute r=(e+x1) mod n. If r=0 or r+k=n, then go to A3;
     * A6: Compute s=(1/(1+dA)*(k-r*dA)) mod n. If s=0, then go to A3;
     * A7: Convert the type of data (r,s) to be bit strings according to the details
     *     in clause 4.2.2 of GM/T 0003.1-2012. Then the signature of message M is (r,s).
     */
    for (;;) {
        if (!BN_priv_rand_range_ex(k, order, 0, ctx)) {
            ERR_raise(ERR_LIB_SM2, ERR_R_INTERNAL_ERROR);
            goto done;
        }

        if (!EC_POINT_mul(group, kG, k, NULL, NULL, ctx)
                || !EC_POINT_get_affine_coordinates(group, kG, x1, NULL,
                                                    ctx)
                || !BN_mod_add(r, e, x1, order, ctx)) {
            ERR_raise(ERR_LIB_SM2, ERR_R_INTERNAL_ERROR);
            goto done;
        }

        /* try again if r == 0 or r+k == n */
        if (BN_is_zero(r))
            continue;

        if (!BN_add(rk, r, k)) {
            ERR_raise(ERR_LIB_SM2, ERR_R_INTERNAL_ERROR);
            goto done;
        }

        if (BN_cmp(rk, order) == 0)
            continue;

        if (!BN_add(s, dA, BN_value_one())
                || !ossl_ec_group_do_inverse_ord(group, s, s, ctx)
                || !BN_mod_mul(tmp, dA, r, order, ctx)
                || !BN_sub(tmp, k, tmp)
                || !BN_mod_mul(s, s, tmp, order, ctx)) {
            ERR_raise(ERR_LIB_SM2, ERR_R_BN_LIB);
            goto done;
        }

        /* try again if s == 0 */
        if (BN_is_zero(s))
            continue;

        sig = ECDSA_SIG_new();
        if (sig == NULL) {
            ERR_raise(ERR_LIB_SM2, ERR_R_ECDSA_LIB);
            goto done;
        }

         /* takes ownership of r and s */
        ECDSA_SIG_set0(sig, r, s);
        break;
    }

 done:
    if (sig == NULL) {
        BN_free(r);
        BN_free(s);
    }

    BN_CTX_free(ctx);
    EC_POINT_free(kG);
    return sig;
}

static int sm2_sig_verify(const EC_KEY *key, const ECDSA_SIG *sig,
                          const BIGNUM *e)
{
    int ret = 0;
    const EC_GROUP *group = EC_KEY_get0_group(key);
    const BIGNUM *order = EC_GROUP_get0_order(group);
    BN_CTX *ctx = NULL;
    EC_POINT *pt = NULL;
    BIGNUM *t = NULL;
    BIGNUM *x1 = NULL;
    const BIGNUM *r = NULL;
    const BIGNUM *s = NULL;
    OSSL_LIB_CTX *libctx = ossl_ec_key_get_libctx(key);

    ctx = BN_CTX_new_ex(libctx);
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_SM2, ERR_R_BN_LIB);
        goto done;
    }
    BN_CTX_start(ctx);
    t = BN_CTX_get(ctx);
    x1 = BN_CTX_get(ctx);
    if (x1 == NULL) {
        ERR_raise(ERR_LIB_SM2, ERR_R_BN_LIB);
        goto done;
    }

    pt = EC_POINT_new(group);
    if (pt == NULL) {
        ERR_raise(ERR_LIB_SM2, ERR_R_EC_LIB);
        goto done;
    }

    /*
     * B1: verify whether r' in [1,n-1], verification failed if not
     * B2: verify whether s' in [1,n-1], verification failed if not
     * B3: set M'~=ZA || M'
     * B4: calculate e'=Hv(M'~)
     * B5: calculate t = (r' + s') modn, verification failed if t=0
     * B6: calculate the point (x1', y1')=[s']G + [t]PA
     * B7: calculate R=(e'+x1') modn, verification pass if yes, otherwise failed
     */

    ECDSA_SIG_get0(sig, &r, &s);

    if (BN_cmp(r, BN_value_one()) < 0
            || BN_cmp(s, BN_value_one()) < 0
            || BN_cmp(order, r) <= 0
            || BN_cmp(order, s) <= 0) {
        ERR_raise(ERR_LIB_SM2, SM2_R_BAD_SIGNATURE);
        goto done;
    }

    if (!BN_mod_add(t, r, s, order, ctx)) {
        ERR_raise(ERR_LIB_SM2, ERR_R_BN_LIB);
        goto done;
    }

    if (BN_is_zero(t)) {
        ERR_raise(ERR_LIB_SM2, SM2_R_BAD_SIGNATURE);
        goto done;
    }

    if (!EC_POINT_mul(group, pt, s, EC_KEY_get0_public_key(key), t, ctx)
            || !EC_POINT_get_affine_coordinates(group, pt, x1, NULL, ctx)) {
        ERR_raise(ERR_LIB_SM2, ERR_R_EC_LIB);
        goto done;
    }

    if (!BN_mod_add(t, e, x1, order, ctx)) {
        ERR_raise(ERR_LIB_SM2, ERR_R_BN_LIB);
        goto done;
    }

    if (BN_cmp(r, t) == 0)
        ret = 1;

 done:
    BN_CTX_end(ctx);
    EC_POINT_free(pt);
    BN_CTX_free(ctx);
    return ret;
}

ECDSA_SIG *ossl_sm2_do_sign(const EC_KEY *key,
                            const EVP_MD *digest,
                            const uint8_t *id,
                            const size_t id_len,
                            const uint8_t *msg, size_t msg_len)
{
    BIGNUM *e = NULL;
    ECDSA_SIG *sig = NULL;

    e = sm2_compute_msg_hash(digest, key, id, id_len, msg, msg_len);
    if (e == NULL) {
        /* SM2err already called */
        goto done;
    }

    sig = sm2_sig_gen(key, e);

 done:
    BN_free(e);
    return sig;
}

int ossl_sm2_do_verify(const EC_KEY *key,
                       const EVP_MD *digest,
                       const ECDSA_SIG *sig,
                       const uint8_t *id,
                       const size_t id_len,
                       const uint8_t *msg, size_t msg_len)
{
    BIGNUM *e = NULL;
    int ret = 0;

    e = sm2_compute_msg_hash(digest, key, id, id_len, msg, msg_len);
    if (e == NULL) {
        /* SM2err already called */
        goto done;
    }

    ret = sm2_sig_verify(key, sig, e);

 done:
    BN_free(e);
    return ret;
}

int ossl_sm2_internal_sign(const unsigned char *dgst, int dgstlen,
                           unsigned char *sig, unsigned int *siglen,
                           EC_KEY *eckey)
{
    BIGNUM *e = NULL;
    ECDSA_SIG *s = NULL;
    int sigleni;
    int ret = -1;

    if (sig == NULL) {
        ERR_raise(ERR_LIB_SM2, ERR_R_PASSED_NULL_PARAMETER);
        goto done;
    }

    e = BN_bin2bn(dgst, dgstlen, NULL);
    if (e == NULL) {
       ERR_raise(ERR_LIB_SM2, ERR_R_BN_LIB);
       goto done;
    }

    s = sm2_sig_gen(eckey, e);
    if (s == NULL) {
        ERR_raise(ERR_LIB_SM2, ERR_R_INTERNAL_ERROR);
        goto done;
    }

    sigleni = i2d_ECDSA_SIG(s, &sig);
    if (sigleni < 0) {
       ERR_raise(ERR_LIB_SM2, ERR_R_INTERNAL_ERROR);
       goto done;
    }
    *siglen = (unsigned int)sigleni;

    ret = 1;

 done:
    ECDSA_SIG_free(s);
    BN_free(e);
    return ret;
}

int ossl_sm2_internal_verify(const unsigned char *dgst, int dgstlen,
                             const unsigned char *sig, int sig_len,
                             EC_KEY *eckey)
{
    ECDSA_SIG *s = NULL;
    BIGNUM *e = NULL;
    const unsigned char *p = sig;
    unsigned char *der = NULL;
    int derlen = -1;
    int ret = -1;

    s = ECDSA_SIG_new();
    if (s == NULL) {
        ERR_raise(ERR_LIB_SM2, ERR_R_ECDSA_LIB);
        goto done;
    }
    if (d2i_ECDSA_SIG(&s, &p, sig_len) == NULL) {
        ERR_raise(ERR_LIB_SM2, SM2_R_INVALID_ENCODING);
        goto done;
    }
    /* Ensure signature uses DER and doesn't have trailing garbage */
    derlen = i2d_ECDSA_SIG(s, &der);
    if (derlen != sig_len || memcmp(sig, der, derlen) != 0) {
        ERR_raise(ERR_LIB_SM2, SM2_R_INVALID_ENCODING);
        goto done;
    }

    e = BN_bin2bn(dgst, dgstlen, NULL);
    if (e == NULL) {
        ERR_raise(ERR_LIB_SM2, ERR_R_BN_LIB);
        goto done;
    }

    ret = sm2_sig_verify(eckey, s, e);

 done:
    OPENSSL_free(der);
    BN_free(e);
    ECDSA_SIG_free(s);
    return ret;
}
