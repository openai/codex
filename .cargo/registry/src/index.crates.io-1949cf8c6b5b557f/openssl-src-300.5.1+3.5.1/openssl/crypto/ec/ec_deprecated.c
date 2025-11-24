/*
 * Copyright 2002-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Suppress deprecation warnings for EC low level implementations that are
 * kept until removal.
 */
#define OPENSSL_SUPPRESS_DEPRECATED

#include <openssl/crypto.h>
#include <openssl/err.h>
#include <openssl/ec.h>

#ifndef OPENSSL_NO_DEPRECATED_3_0
BIGNUM *EC_POINT_point2bn(const EC_GROUP *group,
                          const EC_POINT *point,
                          point_conversion_form_t form,
                          BIGNUM *ret, BN_CTX *ctx)
{
    size_t buf_len = 0;
    unsigned char *buf;

    buf_len = EC_POINT_point2buf(group, point, form, &buf, ctx);

    if (buf_len == 0)
        return NULL;

    ret = BN_bin2bn(buf, buf_len, ret);

    OPENSSL_free(buf);

    return ret;
}

EC_POINT *EC_POINT_bn2point(const EC_GROUP *group,
                            const BIGNUM *bn, EC_POINT *point, BN_CTX *ctx)
{
    size_t buf_len = 0;
    unsigned char *buf;
    EC_POINT *ret;

    if ((buf_len = BN_num_bytes(bn)) == 0)
        buf_len = 1;
    if ((buf = OPENSSL_malloc(buf_len)) == NULL)
        return NULL;

    if (BN_bn2binpad(bn, buf, buf_len) < 0) {
        OPENSSL_free(buf);
        return NULL;
    }

    if (point == NULL) {
        if ((ret = EC_POINT_new(group)) == NULL) {
            OPENSSL_free(buf);
            return NULL;
        }
    } else
        ret = point;

    if (!EC_POINT_oct2point(group, ret, buf, buf_len, ctx)) {
        if (ret != point)
            EC_POINT_clear_free(ret);
        OPENSSL_free(buf);
        return NULL;
    }

    OPENSSL_free(buf);
    return ret;
}
#endif /* OPENSSL_NO_DEPRECATED_3_0 */
