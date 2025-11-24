/*
 * Copyright 2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/crypto.h>
#include "prov/digestcommon.h"
#include "prov/implementations.h"

typedef struct {
    unsigned char nothing;
} NULLMD_CTX;

static int null_init(NULLMD_CTX *ctx)
{
    return 1;
}

static int null_update(NULLMD_CTX *ctx, const void *data, size_t datalen)
{
    return 1;
}

static int null_final(unsigned char *md, NULLMD_CTX *ctx)
{
    return 1;
}

/*
 * We must override the PROV_FUNC_DIGEST_FINAL as dgstsize == 0
 * and that would cause compilation warnings with the default implementation.
 */
#undef PROV_FUNC_DIGEST_FINAL
#define PROV_FUNC_DIGEST_FINAL(name, dgstsize, fin)                            \
static OSSL_FUNC_digest_final_fn name##_internal_final;                        \
static int name##_internal_final(void *ctx, unsigned char *out, size_t *outl,  \
                                 size_t outsz)                                 \
{                                                                              \
    if (ossl_prov_is_running() && fin(out, ctx)) {                             \
        *outl = dgstsize;                                                      \
        return 1;                                                              \
    }                                                                          \
    return 0;                                                                  \
}

IMPLEMENT_digest_functions(nullmd, NULLMD_CTX,
                           0, 0, 0,
                           null_init, null_update, null_final)
