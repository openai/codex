/*
 * Copyright 2020-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core_dispatch.h>
#include "prov/seeding.h"
#include "prov/providercommon.h"

static OSSL_FUNC_get_entropy_fn *c_get_entropy = NULL;
static OSSL_FUNC_get_user_entropy_fn *c_get_user_entropy = NULL;
static OSSL_FUNC_cleanup_entropy_fn *c_cleanup_entropy = NULL;
static OSSL_FUNC_cleanup_user_entropy_fn *c_cleanup_user_entropy = NULL;
static OSSL_FUNC_get_nonce_fn *c_get_nonce = NULL;
static OSSL_FUNC_get_user_nonce_fn *c_get_user_nonce = NULL;
static OSSL_FUNC_cleanup_nonce_fn *c_cleanup_nonce = NULL;
static OSSL_FUNC_cleanup_user_nonce_fn *c_cleanup_user_nonce = NULL;

#ifdef FIPS_MODULE
/*
 * The FIPS provider uses an internal library context which is what the
 * passed provider context references.  Since the seed source is external
 * to the FIPS provider, this is the wrong one.  We need to convert this
 * to the correct core handle before up-calling libcrypto.
 */
# define CORE_HANDLE(provctx) \
    FIPS_get_core_handle(ossl_prov_ctx_get0_libctx(provctx))
#else
/*
 * The non-FIPS path *should* be unused because the full DRBG chain including
 * seed source is instantiated.  However, that might not apply for third
 * party providers, so this is retained for compatibility.
 */
# define CORE_HANDLE(provctx) ossl_prov_ctx_get0_handle(provctx)
#endif

int ossl_prov_seeding_from_dispatch(const OSSL_DISPATCH *fns)
{
    for (; fns->function_id != 0; fns++) {
        /*
         * We do not support the scenario of an application linked against
         * multiple versions of libcrypto (e.g. one static and one dynamic), but
         * sharing a single fips.so. We do a simple sanity check here.
         */
#define set_func(c, f) \
    do { if (c == NULL) c = f; else if (c != f) return 0; } while (0)
        switch (fns->function_id) {
        case OSSL_FUNC_GET_ENTROPY:
            set_func(c_get_entropy, OSSL_FUNC_get_entropy(fns));
            break;
        case OSSL_FUNC_GET_USER_ENTROPY:
            set_func(c_get_user_entropy, OSSL_FUNC_get_user_entropy(fns));
            break;
        case OSSL_FUNC_CLEANUP_ENTROPY:
            set_func(c_cleanup_entropy, OSSL_FUNC_cleanup_entropy(fns));
            break;
        case OSSL_FUNC_CLEANUP_USER_ENTROPY:
            set_func(c_cleanup_user_entropy, OSSL_FUNC_cleanup_user_entropy(fns));
            break;
        case OSSL_FUNC_GET_NONCE:
            set_func(c_get_nonce, OSSL_FUNC_get_nonce(fns));
            break;
        case OSSL_FUNC_GET_USER_NONCE:
            set_func(c_get_user_nonce, OSSL_FUNC_get_user_nonce(fns));
            break;
        case OSSL_FUNC_CLEANUP_NONCE:
            set_func(c_cleanup_nonce, OSSL_FUNC_cleanup_nonce(fns));
            break;
        case OSSL_FUNC_CLEANUP_USER_NONCE:
            set_func(c_cleanup_user_nonce, OSSL_FUNC_cleanup_user_nonce(fns));
            break;
        }
#undef set_func
    }
    return 1;
}

size_t ossl_prov_get_entropy(PROV_CTX *prov_ctx, unsigned char **pout,
                             int entropy, size_t min_len, size_t max_len)
{
    const OSSL_CORE_HANDLE *handle = CORE_HANDLE(prov_ctx);

    if (c_get_user_entropy != NULL)
        return c_get_user_entropy(handle, pout, entropy, min_len, max_len);
    if (c_get_entropy != NULL)
        return c_get_entropy(handle, pout, entropy, min_len, max_len);
    return 0;
}

void ossl_prov_cleanup_entropy(PROV_CTX *prov_ctx, unsigned char *buf,
                               size_t len)
{
    const OSSL_CORE_HANDLE *handle = CORE_HANDLE(prov_ctx);

    if (c_cleanup_user_entropy != NULL)
        c_cleanup_user_entropy(handle, buf, len);
    else if (c_cleanup_entropy != NULL)
        c_cleanup_entropy(handle, buf, len);
}

size_t ossl_prov_get_nonce(PROV_CTX *prov_ctx, unsigned char **pout,
                           size_t min_len, size_t max_len,
                           const void *salt, size_t salt_len)
{
    const OSSL_CORE_HANDLE *handle = CORE_HANDLE(prov_ctx);

    if (c_get_user_nonce != NULL)
        return c_get_user_nonce(handle, pout, min_len, max_len, salt, salt_len);
    if (c_get_nonce != NULL)
        return c_get_nonce(handle, pout, min_len, max_len, salt, salt_len);
    return 0;
}

void ossl_prov_cleanup_nonce(PROV_CTX *prov_ctx, unsigned char *buf, size_t len)
{
    const OSSL_CORE_HANDLE *handle = CORE_HANDLE(prov_ctx);

    if (c_cleanup_user_nonce != NULL)
        c_cleanup_user_nonce(handle, buf, len);
    else if (c_cleanup_nonce != NULL)
        c_cleanup_nonce(handle, buf, len);
}
