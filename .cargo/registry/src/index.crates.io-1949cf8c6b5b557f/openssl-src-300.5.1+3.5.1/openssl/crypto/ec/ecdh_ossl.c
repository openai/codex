/*
 * Copyright 2002-2021 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2002, Oracle and/or its affiliates. All rights reserved
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * ECDH low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <string.h>
#include <limits.h>

#include "internal/cryptlib.h"

#include <openssl/err.h>
#include <openssl/bn.h>
#include <openssl/objects.h>
#include <openssl/ec.h>
#include "ec_local.h"

int ossl_ecdh_compute_key(unsigned char **psec, size_t *pseclen,
                          const EC_POINT *pub_key, const EC_KEY *ecdh)
{
    if (ecdh->group->meth->ecdh_compute_key == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_CURVE_DOES_NOT_SUPPORT_ECDH);
        return 0;
    }

    return ecdh->group->meth->ecdh_compute_key(psec, pseclen, pub_key, ecdh);
}

/*-
 * This implementation is based on the following primitives in the
 * IEEE 1363 standard:
 *  - ECKAS-DH1
 *  - ECSVDP-DH
 *
 * It also conforms to SP800-56A r3
 * See Section 5.7.1.2 "Elliptic Curve Cryptography Cofactor Diffie-Hellman
 * (ECC CDH) Primitive:". The steps listed below refer to SP800-56A.
 */
int ossl_ecdh_simple_compute_key(unsigned char **pout, size_t *poutlen,
                                 const EC_POINT *pub_key, const EC_KEY *ecdh)
{
    BN_CTX *ctx;
    EC_POINT *tmp = NULL;
    BIGNUM *x = NULL;
    const BIGNUM *priv_key;
    const EC_GROUP *group;
    int ret = 0;
    size_t buflen, len;
    unsigned char *buf = NULL;

    if ((ctx = BN_CTX_new_ex(ecdh->libctx)) == NULL)
        goto err;
    BN_CTX_start(ctx);
    x = BN_CTX_get(ctx);
    if (x == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
        goto err;
    }

    priv_key = EC_KEY_get0_private_key(ecdh);
    if (priv_key == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_MISSING_PRIVATE_KEY);
        goto err;
    }

    group = EC_KEY_get0_group(ecdh);

    /*
     * Step(1) - Compute the point tmp = cofactor * owners_private_key
     *                                   * peer_public_key.
     */
    if (EC_KEY_get_flags(ecdh) & EC_FLAG_COFACTOR_ECDH) {
        if (!EC_GROUP_get_cofactor(group, x, NULL)) {
            ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
            goto err;
        }
        if (!BN_mul(x, x, priv_key, ctx)) {
            ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
            goto err;
        }
        priv_key = x;
    }

    if ((tmp = EC_POINT_new(group)) == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
        goto err;
    }

    if (!EC_POINT_mul(group, tmp, NULL, pub_key, priv_key, ctx)) {
        ERR_raise(ERR_LIB_EC, EC_R_POINT_ARITHMETIC_FAILURE);
        goto err;
    }

    /*
     * Step(2) : If point tmp is at infinity then clear intermediate values and
     * exit. Note: getting affine coordinates returns 0 if point is at infinity.
     * Step(3a) : Get x-coordinate of point x = tmp.x
     */
    if (!EC_POINT_get_affine_coordinates(group, tmp, x, NULL, ctx)) {
        ERR_raise(ERR_LIB_EC, EC_R_POINT_ARITHMETIC_FAILURE);
        goto err;
    }

    /*
     * Step(3b) : convert x to a byte string, using the field-element-to-byte
     * string conversion routine defined in Appendix C.2
     */
    buflen = (EC_GROUP_get_degree(group) + 7) / 8;
    len = BN_num_bytes(x);
    if (len > buflen) {
        ERR_raise(ERR_LIB_EC, ERR_R_INTERNAL_ERROR);
        goto err;
    }
    if ((buf = OPENSSL_malloc(buflen)) == NULL)
        goto err;

    memset(buf, 0, buflen - len);
    if (len != (size_t)BN_bn2bin(x, buf + buflen - len)) {
        ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
        goto err;
    }

    *pout = buf;
    *poutlen = buflen;
    buf = NULL;

    ret = 1;

 err:
    /* Step(4) : Destroy all intermediate calculations */
    BN_clear(x);
    EC_POINT_clear_free(tmp);
    BN_CTX_end(ctx);
    BN_CTX_free(ctx);
    OPENSSL_free(buf);
    return ret;
}
