/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <stdio.h>
#include <openssl/core.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/err.h>
#include <openssl/params.h>
#include "prov/provider_ctx.h"
#include "prov/implementations.h"
#include "prov/names.h"
#include "prov/providercommon.h"

/*
 * Forward declarations to ensure that interface functions are correctly
 * defined.
 */
static OSSL_FUNC_provider_gettable_params_fn legacy_gettable_params;
static OSSL_FUNC_provider_get_params_fn legacy_get_params;
static OSSL_FUNC_provider_query_operation_fn legacy_query;

#define ALG(NAMES, FUNC) { NAMES, "provider=legacy", FUNC }

#ifdef STATIC_LEGACY
OSSL_provider_init_fn ossl_legacy_provider_init;
# define OSSL_provider_init ossl_legacy_provider_init
#endif

#ifndef STATIC_LEGACY
/*
 * Should these function pointers be stored in the provider side provctx?
 * Could they ever be different from one init to the next? We assume not for
 * now.
 */

/* Functions provided by the core */
static OSSL_FUNC_core_new_error_fn *c_new_error;
static OSSL_FUNC_core_set_error_debug_fn *c_set_error_debug;
static OSSL_FUNC_core_vset_error_fn *c_vset_error;
static OSSL_FUNC_core_set_error_mark_fn *c_set_error_mark;
static OSSL_FUNC_core_clear_last_error_mark_fn *c_clear_last_error_mark;
static OSSL_FUNC_core_pop_error_to_mark_fn *c_pop_error_to_mark;
#endif

/* Parameters we provide to the core */
static const OSSL_PARAM legacy_param_types[] = {
    OSSL_PARAM_DEFN(OSSL_PROV_PARAM_NAME, OSSL_PARAM_UTF8_PTR, NULL, 0),
    OSSL_PARAM_DEFN(OSSL_PROV_PARAM_VERSION, OSSL_PARAM_UTF8_PTR, NULL, 0),
    OSSL_PARAM_DEFN(OSSL_PROV_PARAM_BUILDINFO, OSSL_PARAM_UTF8_PTR, NULL, 0),
    OSSL_PARAM_DEFN(OSSL_PROV_PARAM_STATUS, OSSL_PARAM_INTEGER, NULL, 0),
    OSSL_PARAM_END
};

static const OSSL_PARAM *legacy_gettable_params(void *provctx)
{
    return legacy_param_types;
}

static int legacy_get_params(void *provctx, OSSL_PARAM params[])
{
    OSSL_PARAM *p;

    p = OSSL_PARAM_locate(params, OSSL_PROV_PARAM_NAME);
    if (p != NULL && !OSSL_PARAM_set_utf8_ptr(p, "OpenSSL Legacy Provider"))
        return 0;
    p = OSSL_PARAM_locate(params, OSSL_PROV_PARAM_VERSION);
    if (p != NULL && !OSSL_PARAM_set_utf8_ptr(p, OPENSSL_VERSION_STR))
        return 0;
    p = OSSL_PARAM_locate(params, OSSL_PROV_PARAM_BUILDINFO);
    if (p != NULL && !OSSL_PARAM_set_utf8_ptr(p, OPENSSL_FULL_VERSION_STR))
        return 0;
    p = OSSL_PARAM_locate(params, OSSL_PROV_PARAM_STATUS);
    if (p != NULL && !OSSL_PARAM_set_int(p, ossl_prov_is_running()))
        return 0;
    return 1;
}

static const OSSL_ALGORITHM legacy_digests[] = {
#ifndef OPENSSL_NO_MD2
    ALG(PROV_NAMES_MD2, ossl_md2_functions),
#endif
#ifndef OPENSSL_NO_MD4
    ALG(PROV_NAMES_MD4, ossl_md4_functions),
#endif
#ifndef OPENSSL_NO_MDC2
    ALG(PROV_NAMES_MDC2, ossl_mdc2_functions),
#endif /* OPENSSL_NO_MDC2 */
#ifndef OPENSSL_NO_WHIRLPOOL
    ALG(PROV_NAMES_WHIRLPOOL, ossl_wp_functions),
#endif /* OPENSSL_NO_WHIRLPOOL */
#ifndef OPENSSL_NO_RMD160
    ALG(PROV_NAMES_RIPEMD_160, ossl_ripemd160_functions),
#endif /* OPENSSL_NO_RMD160 */
    { NULL, NULL, NULL }
};

static const OSSL_ALGORITHM legacy_ciphers[] = {
#ifndef OPENSSL_NO_CAST
    ALG(PROV_NAMES_CAST5_ECB, ossl_cast5128ecb_functions),
    ALG(PROV_NAMES_CAST5_CBC, ossl_cast5128cbc_functions),
    ALG(PROV_NAMES_CAST5_OFB, ossl_cast5128ofb64_functions),
    ALG(PROV_NAMES_CAST5_CFB, ossl_cast5128cfb64_functions),
#endif /* OPENSSL_NO_CAST */
#ifndef OPENSSL_NO_BF
    ALG(PROV_NAMES_BF_ECB, ossl_blowfish128ecb_functions),
    ALG(PROV_NAMES_BF_CBC, ossl_blowfish128cbc_functions),
    ALG(PROV_NAMES_BF_OFB, ossl_blowfish128ofb64_functions),
    ALG(PROV_NAMES_BF_CFB, ossl_blowfish128cfb64_functions),
#endif /* OPENSSL_NO_BF */
#ifndef OPENSSL_NO_IDEA
    ALG(PROV_NAMES_IDEA_ECB, ossl_idea128ecb_functions),
    ALG(PROV_NAMES_IDEA_CBC, ossl_idea128cbc_functions),
    ALG(PROV_NAMES_IDEA_OFB, ossl_idea128ofb64_functions),
    ALG(PROV_NAMES_IDEA_CFB, ossl_idea128cfb64_functions),
#endif /* OPENSSL_NO_IDEA */
#ifndef OPENSSL_NO_SEED
    ALG(PROV_NAMES_SEED_ECB, ossl_seed128ecb_functions),
    ALG(PROV_NAMES_SEED_CBC, ossl_seed128cbc_functions),
    ALG(PROV_NAMES_SEED_OFB, ossl_seed128ofb128_functions),
    ALG(PROV_NAMES_SEED_CFB, ossl_seed128cfb128_functions),
#endif /* OPENSSL_NO_SEED */
#ifndef OPENSSL_NO_RC2
    ALG(PROV_NAMES_RC2_ECB, ossl_rc2128ecb_functions),
    ALG(PROV_NAMES_RC2_CBC, ossl_rc2128cbc_functions),
    ALG(PROV_NAMES_RC2_40_CBC, ossl_rc240cbc_functions),
    ALG(PROV_NAMES_RC2_64_CBC, ossl_rc264cbc_functions),
    ALG(PROV_NAMES_RC2_CFB, ossl_rc2128cfb128_functions),
    ALG(PROV_NAMES_RC2_OFB, ossl_rc2128ofb128_functions),
#endif /* OPENSSL_NO_RC2 */
#ifndef OPENSSL_NO_RC4
    ALG(PROV_NAMES_RC4, ossl_rc4128_functions),
    ALG(PROV_NAMES_RC4_40, ossl_rc440_functions),
# ifndef OPENSSL_NO_MD5
    ALG(PROV_NAMES_RC4_HMAC_MD5, ossl_rc4_hmac_ossl_md5_functions),
# endif /* OPENSSL_NO_MD5 */
#endif /* OPENSSL_NO_RC4 */
#ifndef OPENSSL_NO_RC5
    ALG(PROV_NAMES_RC5_ECB, ossl_rc5128ecb_functions),
    ALG(PROV_NAMES_RC5_CBC, ossl_rc5128cbc_functions),
    ALG(PROV_NAMES_RC5_OFB, ossl_rc5128ofb64_functions),
    ALG(PROV_NAMES_RC5_CFB, ossl_rc5128cfb64_functions),
#endif /* OPENSSL_NO_RC5 */
#ifndef OPENSSL_NO_DES
    ALG(PROV_NAMES_DESX_CBC, ossl_tdes_desx_cbc_functions),
    ALG(PROV_NAMES_DES_ECB, ossl_des_ecb_functions),
    ALG(PROV_NAMES_DES_CBC, ossl_des_cbc_functions),
    ALG(PROV_NAMES_DES_OFB, ossl_des_ofb64_functions),
    ALG(PROV_NAMES_DES_CFB, ossl_des_cfb64_functions),
    ALG(PROV_NAMES_DES_CFB1, ossl_des_cfb1_functions),
    ALG(PROV_NAMES_DES_CFB8, ossl_des_cfb8_functions),
#endif /* OPENSSL_NO_DES */
    { NULL, NULL, NULL }
};

static const OSSL_ALGORITHM legacy_kdfs[] = {
    ALG(PROV_NAMES_PBKDF1, ossl_kdf_pbkdf1_functions),
    ALG(PROV_NAMES_PVKKDF, ossl_kdf_pvk_functions),
    { NULL, NULL, NULL }
};

static const OSSL_ALGORITHM *legacy_query(void *provctx, int operation_id,
                                          int *no_cache)
{
    *no_cache = 0;
    switch (operation_id) {
    case OSSL_OP_DIGEST:
        return legacy_digests;
    case OSSL_OP_CIPHER:
        return legacy_ciphers;
    case OSSL_OP_KDF:
        return legacy_kdfs;
    }
    return NULL;
}

static void legacy_teardown(void *provctx)
{
    OSSL_LIB_CTX_free(PROV_LIBCTX_OF(provctx));
    ossl_prov_ctx_free(provctx);
}

/* Functions we provide to the core */
static const OSSL_DISPATCH legacy_dispatch_table[] = {
    { OSSL_FUNC_PROVIDER_TEARDOWN, (void (*)(void))legacy_teardown },
    { OSSL_FUNC_PROVIDER_GETTABLE_PARAMS, (void (*)(void))legacy_gettable_params },
    { OSSL_FUNC_PROVIDER_GET_PARAMS, (void (*)(void))legacy_get_params },
    { OSSL_FUNC_PROVIDER_QUERY_OPERATION, (void (*)(void))legacy_query },
    OSSL_DISPATCH_END
};

int OSSL_provider_init(const OSSL_CORE_HANDLE *handle,
                       const OSSL_DISPATCH *in,
                       const OSSL_DISPATCH **out,
                       void **provctx)
{
    OSSL_LIB_CTX *libctx = NULL;
#ifndef STATIC_LEGACY
    const OSSL_DISPATCH *tmp;
#endif

#ifndef STATIC_LEGACY
    for (tmp = in; tmp->function_id != 0; tmp++) {
        /*
         * We do not support the scenario of an application linked against
         * multiple versions of libcrypto (e.g. one static and one dynamic),
         * but sharing a single legacy.so. We do a simple sanity check here.
         */
#define set_func(c, f) if (c == NULL) c = f; else if (c != f) return 0;
        switch (tmp->function_id) {
        case OSSL_FUNC_CORE_NEW_ERROR:
            set_func(c_new_error, OSSL_FUNC_core_new_error(tmp));
            break;
        case OSSL_FUNC_CORE_SET_ERROR_DEBUG:
            set_func(c_set_error_debug, OSSL_FUNC_core_set_error_debug(tmp));
            break;
        case OSSL_FUNC_CORE_VSET_ERROR:
            set_func(c_vset_error, OSSL_FUNC_core_vset_error(tmp));
            break;
        case OSSL_FUNC_CORE_SET_ERROR_MARK:
            set_func(c_set_error_mark, OSSL_FUNC_core_set_error_mark(tmp));
            break;
        case OSSL_FUNC_CORE_CLEAR_LAST_ERROR_MARK:
            set_func(c_clear_last_error_mark,
                     OSSL_FUNC_core_clear_last_error_mark(tmp));
            break;
        case OSSL_FUNC_CORE_POP_ERROR_TO_MARK:
            set_func(c_pop_error_to_mark, OSSL_FUNC_core_pop_error_to_mark(tmp));
            break;
        }
    }
#endif

    if ((*provctx = ossl_prov_ctx_new()) == NULL
        || (libctx = OSSL_LIB_CTX_new_child(handle, in)) == NULL) {
        OSSL_LIB_CTX_free(libctx);
        legacy_teardown(*provctx);
        *provctx = NULL;
        return 0;
    }
    ossl_prov_ctx_set0_libctx(*provctx, libctx);
    ossl_prov_ctx_set0_handle(*provctx, handle);

    *out = legacy_dispatch_table;

    return 1;
}

#ifndef STATIC_LEGACY
/*
 * Provider specific implementation of libcrypto functions in terms of
 * upcalls.
 */

/*
 * For ERR functions, we pass a NULL context.  This is valid to do as long
 * as only error codes that the calling libcrypto supports are used.
 */
void ERR_new(void)
{
    c_new_error(NULL);
}

void ERR_set_debug(const char *file, int line, const char *func)
{
    c_set_error_debug(NULL, file, line, func);
}

void ERR_set_error(int lib, int reason, const char *fmt, ...)
{
    va_list args;

    va_start(args, fmt);
    c_vset_error(NULL, ERR_PACK(lib, 0, reason), fmt, args);
    va_end(args);
}

void ERR_vset_error(int lib, int reason, const char *fmt, va_list args)
{
    c_vset_error(NULL, ERR_PACK(lib, 0, reason), fmt, args);
}

int ERR_set_mark(void)
{
    return c_set_error_mark(NULL);
}

int ERR_clear_last_mark(void)
{
    return c_clear_last_error_mark(NULL);
}

int ERR_pop_to_mark(void)
{
    return c_pop_error_to_mark(NULL);
}
#endif
