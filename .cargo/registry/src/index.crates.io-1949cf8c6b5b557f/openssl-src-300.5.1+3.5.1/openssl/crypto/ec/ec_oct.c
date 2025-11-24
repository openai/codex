/*
 * Copyright 2011-2021 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2002, Oracle and/or its affiliates. All rights reserved
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

#include <string.h>

#include <openssl/err.h>
#include <openssl/opensslv.h>

#include "ec_local.h"

int EC_POINT_set_compressed_coordinates(const EC_GROUP *group, EC_POINT *point,
                                        const BIGNUM *x, int y_bit, BN_CTX *ctx)
{
    if (group->meth->point_set_compressed_coordinates == NULL
        && !(group->meth->flags & EC_FLAGS_DEFAULT_OCT)) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    if (!ec_point_is_compat(point, group)) {
        ERR_raise(ERR_LIB_EC, EC_R_INCOMPATIBLE_OBJECTS);
        return 0;
    }
    if (group->meth->flags & EC_FLAGS_DEFAULT_OCT) {
        if (group->meth->field_type == NID_X9_62_prime_field)
            return ossl_ec_GFp_simple_set_compressed_coordinates(group, point, x,
                                                                 y_bit, ctx);
        else
#ifdef OPENSSL_NO_EC2M
        {
            ERR_raise(ERR_LIB_EC, EC_R_GF2M_NOT_SUPPORTED);
            return 0;
        }
#else
            return ossl_ec_GF2m_simple_set_compressed_coordinates(group, point,
                                                                  x, y_bit, ctx);
#endif
    }
    return group->meth->point_set_compressed_coordinates(group, point, x,
                                                         y_bit, ctx);
}

#ifndef OPENSSL_NO_DEPRECATED_3_0
int EC_POINT_set_compressed_coordinates_GFp(const EC_GROUP *group,
                                            EC_POINT *point, const BIGNUM *x,
                                            int y_bit, BN_CTX *ctx)
{
    return EC_POINT_set_compressed_coordinates(group, point, x, y_bit, ctx);
}

# ifndef OPENSSL_NO_EC2M
int EC_POINT_set_compressed_coordinates_GF2m(const EC_GROUP *group,
                                             EC_POINT *point, const BIGNUM *x,
                                             int y_bit, BN_CTX *ctx)
{
    return EC_POINT_set_compressed_coordinates(group, point, x, y_bit, ctx);
}
# endif
#endif

size_t EC_POINT_point2oct(const EC_GROUP *group, const EC_POINT *point,
                          point_conversion_form_t form, unsigned char *buf,
                          size_t len, BN_CTX *ctx)
{
    if (point == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (group->meth->point2oct == 0
        && !(group->meth->flags & EC_FLAGS_DEFAULT_OCT)) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    if (!ec_point_is_compat(point, group)) {
        ERR_raise(ERR_LIB_EC, EC_R_INCOMPATIBLE_OBJECTS);
        return 0;
    }
    if (group->meth->flags & EC_FLAGS_DEFAULT_OCT) {
        if (group->meth->field_type == NID_X9_62_prime_field)
            return ossl_ec_GFp_simple_point2oct(group, point, form, buf, len,
                                                ctx);
        else
#ifdef OPENSSL_NO_EC2M
        {
            ERR_raise(ERR_LIB_EC, EC_R_GF2M_NOT_SUPPORTED);
            return 0;
        }
#else
            return ossl_ec_GF2m_simple_point2oct(group, point,
                                                 form, buf, len, ctx);
#endif
    }

    return group->meth->point2oct(group, point, form, buf, len, ctx);
}

int EC_POINT_oct2point(const EC_GROUP *group, EC_POINT *point,
                       const unsigned char *buf, size_t len, BN_CTX *ctx)
{
    if (group->meth->oct2point == 0
        && !(group->meth->flags & EC_FLAGS_DEFAULT_OCT)) {
        ERR_raise(ERR_LIB_EC, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
        return 0;
    }
    if (!ec_point_is_compat(point, group)) {
        ERR_raise(ERR_LIB_EC, EC_R_INCOMPATIBLE_OBJECTS);
        return 0;
    }
    if (group->meth->flags & EC_FLAGS_DEFAULT_OCT) {
        if (group->meth->field_type == NID_X9_62_prime_field)
            return ossl_ec_GFp_simple_oct2point(group, point, buf, len, ctx);
        else
#ifdef OPENSSL_NO_EC2M
        {
            ERR_raise(ERR_LIB_EC, EC_R_GF2M_NOT_SUPPORTED);
            return 0;
        }
#else
            return ossl_ec_GF2m_simple_oct2point(group, point, buf, len, ctx);
#endif
    }
    return group->meth->oct2point(group, point, buf, len, ctx);
}

size_t EC_POINT_point2buf(const EC_GROUP *group, const EC_POINT *point,
                          point_conversion_form_t form,
                          unsigned char **pbuf, BN_CTX *ctx)
{
    size_t len;
    unsigned char *buf;

    len = EC_POINT_point2oct(group, point, form, NULL, 0, NULL);
    if (len == 0)
        return 0;
    if ((buf = OPENSSL_malloc(len)) == NULL)
        return 0;
    len = EC_POINT_point2oct(group, point, form, buf, len, ctx);
    if (len == 0) {
        OPENSSL_free(buf);
        return 0;
    }
    *pbuf = buf;
    return len;
}
