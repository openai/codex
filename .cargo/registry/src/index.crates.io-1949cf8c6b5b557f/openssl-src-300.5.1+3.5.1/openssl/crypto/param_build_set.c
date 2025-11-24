/*
 * Copyright 2020-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Key Management utility functions to share functionality between the export()
 * and get_params() methods.
 * export() uses OSSL_PARAM_BLD, and get_params() used the OSSL_PARAM[] to
 * fill in parameter data for the same key and data fields.
 */

#include <openssl/core_names.h>
#include "internal/param_build_set.h"

DEFINE_SPECIAL_STACK_OF_CONST(BIGNUM_const, BIGNUM)

int ossl_param_build_set_int(OSSL_PARAM_BLD *bld, OSSL_PARAM *p,
                             const char *key, int num)
{
    if (bld != NULL)
        return OSSL_PARAM_BLD_push_int(bld, key, num);
    p = OSSL_PARAM_locate(p, key);
    if (p != NULL)
        return OSSL_PARAM_set_int(p, num);
    return 1;
}

int ossl_param_build_set_long(OSSL_PARAM_BLD *bld, OSSL_PARAM *p,
                              const char *key, long num)
{
    if (bld != NULL)
        return OSSL_PARAM_BLD_push_long(bld, key, num);
    p = OSSL_PARAM_locate(p, key);
    if (p != NULL)
        return OSSL_PARAM_set_long(p, num);
    return 1;
}

int ossl_param_build_set_utf8_string(OSSL_PARAM_BLD *bld, OSSL_PARAM *p,
                                     const char *key, const char *buf)
{
    if (bld != NULL)
        return OSSL_PARAM_BLD_push_utf8_string(bld, key, buf, 0);
    p = OSSL_PARAM_locate(p, key);
    if (p != NULL)
        return OSSL_PARAM_set_utf8_string(p, buf);
    return 1;
}

int ossl_param_build_set_octet_string(OSSL_PARAM_BLD *bld, OSSL_PARAM *p,
                                      const char *key,
                                      const unsigned char *data,
                                      size_t data_len)
{
    if (bld != NULL)
        return OSSL_PARAM_BLD_push_octet_string(bld, key, data, data_len);

    p = OSSL_PARAM_locate(p, key);
    if (p != NULL)
        return OSSL_PARAM_set_octet_string(p, data, data_len);
    return 1;
}

int ossl_param_build_set_bn_pad(OSSL_PARAM_BLD *bld, OSSL_PARAM *p,
                                const char *key, const BIGNUM *bn,  size_t sz)
{
    if (bld != NULL)
        return OSSL_PARAM_BLD_push_BN_pad(bld, key, bn, sz);
    p = OSSL_PARAM_locate(p, key);
    if (p != NULL) {
        if (sz > p->data_size) {
            ERR_raise(ERR_LIB_CRYPTO, CRYPTO_R_TOO_SMALL_BUFFER);
            return 0;
        }
        p->data_size = sz;
        return OSSL_PARAM_set_BN(p, bn);
    }
    return 1;
}

int ossl_param_build_set_bn(OSSL_PARAM_BLD *bld, OSSL_PARAM *p,
                            const char *key, const BIGNUM *bn)
{
    if (bld != NULL)
        return OSSL_PARAM_BLD_push_BN(bld, key, bn);

    p = OSSL_PARAM_locate(p, key);
    if (p != NULL)
        return OSSL_PARAM_set_BN(p, bn) > 0;
    return 1;
}

int ossl_param_build_set_multi_key_bn(OSSL_PARAM_BLD *bld, OSSL_PARAM *params,
                                      const char *names[],
                                      STACK_OF(BIGNUM_const) *stk)
{
    int i, sz = sk_BIGNUM_const_num(stk);
    OSSL_PARAM *p;
    const BIGNUM *bn;

    if (bld != NULL) {
        for (i = 0; i < sz && names[i] != NULL; ++i) {
            bn = sk_BIGNUM_const_value(stk, i);
            if (bn != NULL && !OSSL_PARAM_BLD_push_BN(bld, names[i], bn))
                return 0;
        }
        return 1;
    }

    for (i = 0; i < sz && names[i] != NULL; ++i) {
        bn = sk_BIGNUM_const_value(stk, i);
        p = OSSL_PARAM_locate(params, names[i]);
        if (p != NULL && bn != NULL) {
            if (!OSSL_PARAM_set_BN(p, bn))
                return 0;
        }
    }
    return 1;
}
