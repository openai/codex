/*
 * Copyright 2020-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_INTERNAL_PARAM_BUILD_SET_H
# define OSSL_INTERNAL_PARAM_BUILD_SET_H
# pragma once

# include <openssl/safestack.h>
# include <openssl/param_build.h>
# include "internal/cryptlib.h"

typedef union {
    OSSL_UNION_ALIGN;
} OSSL_PARAM_ALIGNED_BLOCK;

# define OSSL_PARAM_ALIGN_SIZE  sizeof(OSSL_PARAM_ALIGNED_BLOCK)

size_t ossl_param_bytes_to_blocks(size_t bytes);
void ossl_param_set_secure_block(OSSL_PARAM *last, void *secure_buffer,
                                 size_t secure_buffer_sz);

int ossl_param_build_set_int(OSSL_PARAM_BLD *bld, OSSL_PARAM *p,
                             const char *key, int num);
int ossl_param_build_set_long(OSSL_PARAM_BLD *bld, OSSL_PARAM *p,
                              const char *key, long num);
int ossl_param_build_set_utf8_string(OSSL_PARAM_BLD *bld, OSSL_PARAM *p,
                                     const char *key, const char *buf);
int ossl_param_build_set_octet_string(OSSL_PARAM_BLD *bld, OSSL_PARAM *p,
                                      const char *key,
                                      const unsigned char *data,
                                      size_t data_len);
int ossl_param_build_set_bn(OSSL_PARAM_BLD *bld, OSSL_PARAM *p,
                            const char *key, const BIGNUM *bn);
int ossl_param_build_set_bn_pad(OSSL_PARAM_BLD *bld, OSSL_PARAM *p,
                                const char *key, const BIGNUM *bn,  size_t sz);
int ossl_param_build_set_signed_bn(OSSL_PARAM_BLD *bld, OSSL_PARAM *p,
                                   const char *key, const BIGNUM *bn);
int ossl_param_build_set_signed_bn_pad(OSSL_PARAM_BLD *bld, OSSL_PARAM *p,
                                       const char *key, const BIGNUM *bn,
                                       size_t sz);
int ossl_param_build_set_multi_key_bn(OSSL_PARAM_BLD *bld, OSSL_PARAM *p,
                                      const char *names[],
                                      STACK_OF(BIGNUM_const) *stk);

#endif  /* OSSL_INTERNAL_PARAM_BUILD_SET_H */
