/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * This is a very simple provider that does absolutely nothing except respond
 * to provider global parameter requests.  It does this by simply echoing back
 * a parameter request it makes to the loading library.
 */

#include <string.h>
#include <stdio.h>

#include <stdarg.h>

/*
 * When built as an object file to link the application with, we get the
 * init function name through the macro PROVIDER_INIT_FUNCTION_NAME.  If
 * not defined, we use the standard init function name for the shared
 * object form.
 */
#ifdef PROVIDER_INIT_FUNCTION_NAME
# define OSSL_provider_init PROVIDER_INIT_FUNCTION_NAME
#endif

#include "internal/e_os.h"
#include <openssl/core.h>
#include <openssl/core_dispatch.h>
#include <openssl/err.h>
#include <openssl/evp.h>
#include <openssl/crypto.h>
#include <openssl/provider.h>

typedef struct p_test_ctx {
    char *thisfile;
    char *thisfunc;
    const OSSL_CORE_HANDLE *handle;
    OSSL_LIB_CTX *libctx;
} P_TEST_CTX;

static OSSL_FUNC_core_gettable_params_fn *c_gettable_params = NULL;
static OSSL_FUNC_core_get_params_fn *c_get_params = NULL;
static OSSL_FUNC_core_new_error_fn *c_new_error;
static OSSL_FUNC_core_set_error_debug_fn *c_set_error_debug;
static OSSL_FUNC_core_vset_error_fn *c_vset_error;
static OSSL_FUNC_BIO_vsnprintf_fn *c_BIO_vsnprintf;

/* Tell the core what params we provide and what type they are */
static const OSSL_PARAM p_param_types[] = {
    { "greeting", OSSL_PARAM_UTF8_STRING, NULL, 0, 0 },
    { "digest-check", OSSL_PARAM_UNSIGNED_INTEGER, NULL, 0, 0},
    { NULL, 0, NULL, 0, 0 }
};

/* This is a trick to ensure we define the provider functions correctly */
static OSSL_FUNC_provider_gettable_params_fn p_gettable_params;
static OSSL_FUNC_provider_get_params_fn p_get_params;
static OSSL_FUNC_provider_get_reason_strings_fn p_get_reason_strings;
static OSSL_FUNC_provider_teardown_fn p_teardown;

static int local_snprintf(char *buf, size_t n, const char *format, ...)
{
    va_list args;
    int ret;

    va_start(args, format);
    ret = (*c_BIO_vsnprintf)(buf, n, format, args);
    va_end(args);
    return ret;
}

static void p_set_error(int lib, int reason, const char *file, int line,
                        const char *func, const char *fmt, ...)
{
    va_list ap;

    va_start(ap, fmt);
    c_new_error(NULL);
    c_set_error_debug(NULL, file, line, func);
    c_vset_error(NULL, ERR_PACK(lib, 0, reason), fmt, ap);
    va_end(ap);
}

static const OSSL_PARAM *p_gettable_params(void *_)
{
    return p_param_types;
}

static int p_get_params(void *provctx, OSSL_PARAM params[])
{
    P_TEST_CTX *ctx = (P_TEST_CTX *)provctx;
    const OSSL_CORE_HANDLE *hand = ctx->handle;
    OSSL_PARAM *p = params;
    int ok = 1;

    for (; ok && p->key != NULL; p++) {
        if (strcmp(p->key, "greeting") == 0) {
            static char *opensslv;
            static char *provname;
            static char *greeting;
            static OSSL_PARAM counter_request[] = {
                /* Known libcrypto provided parameters */
                { "openssl-version", OSSL_PARAM_UTF8_PTR,
                  &opensslv, sizeof(&opensslv), 0 },
                { "provider-name", OSSL_PARAM_UTF8_PTR,
                  &provname, sizeof(&provname), 0},

                /* This might be present, if there's such a configuration */
                { "greeting", OSSL_PARAM_UTF8_PTR,
                  &greeting, sizeof(&greeting), 0 },

                { NULL, 0, NULL, 0, 0 }
            };
            char buf[256];
            size_t buf_l;

            opensslv = provname = greeting = NULL;

            if (c_get_params(hand, counter_request)) {
                if (greeting) {
                    strcpy(buf, greeting);
                } else {
                    const char *versionp = *(void **)counter_request[0].data;
                    const char *namep = *(void **)counter_request[1].data;

                    local_snprintf(buf, sizeof(buf), "Hello OpenSSL %.20s, greetings from %s!",
                                   versionp, namep);
                }
            } else {
                local_snprintf(buf, sizeof(buf), "Howdy stranger...");
            }

            p->return_size = buf_l = strlen(buf) + 1;
            if (p->data_size >= buf_l)
                strcpy(p->data, buf);
            else
                ok = 0;
        } else if (strcmp(p->key, "digest-check") == 0) {
            unsigned int digestsuccess = 0;

            /*
             * Test we can use an algorithm from another provider. We're using
             * legacy to check that legacy is actually available and we haven't
             * just fallen back to default.
             */
#ifdef PROVIDER_INIT_FUNCTION_NAME
            EVP_MD *md4 = EVP_MD_fetch(ctx->libctx, "MD4", NULL);
            EVP_MD_CTX *mdctx = EVP_MD_CTX_new();
            const char *msg = "Hello world";
            unsigned char out[16];
            OSSL_PROVIDER *deflt;

            /*
            * "default" has not been loaded into the parent libctx. We should be able
            * to explicitly load it as a non-child provider.
            */
            deflt = OSSL_PROVIDER_load(ctx->libctx, "default");
            if (deflt == NULL
                    || !OSSL_PROVIDER_available(ctx->libctx, "default")) {
                /* We set error "3" for a failure to load the default provider */
                p_set_error(ERR_LIB_PROV, 3, ctx->thisfile, OPENSSL_LINE,
                            ctx->thisfunc, NULL);
                ok = 0;
            }

            /*
             * We should have the default provider available that we loaded
             * ourselves, and the base and legacy providers which we inherit
             * from the parent libctx. We should also have "this" provider
             * available.
             */
            if (ok
                    && OSSL_PROVIDER_available(ctx->libctx, "default")
                    && OSSL_PROVIDER_available(ctx->libctx, "base")
                    && OSSL_PROVIDER_available(ctx->libctx, "legacy")
                    && OSSL_PROVIDER_available(ctx->libctx, "p_test")
                    && md4 != NULL
                    && mdctx != NULL) {
                if (EVP_DigestInit_ex(mdctx, md4, NULL)
                        && EVP_DigestUpdate(mdctx, (const unsigned char *)msg,
                                            strlen(msg))
                        && EVP_DigestFinal(mdctx, out, NULL))
                    digestsuccess = 1;
            }
            EVP_MD_CTX_free(mdctx);
            EVP_MD_free(md4);
            OSSL_PROVIDER_unload(deflt);
#endif
            if (p->data_size >= sizeof(digestsuccess)) {
                *(unsigned int *)p->data = digestsuccess;
                p->return_size = sizeof(digestsuccess);
            } else {
                ok = 0;
            }
        } else if (strcmp(p->key, "stop-property-mirror") == 0) {
            /*
             * Setting the default properties explicitly should stop mirroring
             * of properties from the parent libctx.
             */
            unsigned int stopsuccess = 0;

#ifdef PROVIDER_INIT_FUNCTION_NAME
            stopsuccess = EVP_set_default_properties(ctx->libctx, NULL);
#endif
            if (p->data_size >= sizeof(stopsuccess)) {
                *(unsigned int *)p->data = stopsuccess;
                p->return_size = sizeof(stopsuccess);
            } else {
                ok = 0;
            }
        }
    }
    return ok;
}

static const OSSL_ITEM *p_get_reason_strings(void *_)
{
    static const OSSL_ITEM reason_strings[] = {
        {1, "dummy reason string"},
        {2, "Can't create child library context"},
        {3, "Can't load default provider"},
        {0, NULL}
    };

    return reason_strings;
}

static const OSSL_ALGORITHM *p_query(OSSL_PROVIDER *prov,
                                     int operation_id,
                                     int *no_cache)
{
    *no_cache = 1;
    return NULL;
}

static const OSSL_DISPATCH p_test_table[] = {
    { OSSL_FUNC_PROVIDER_GETTABLE_PARAMS, (void (*)(void))p_gettable_params },
    { OSSL_FUNC_PROVIDER_GET_PARAMS, (void (*)(void))p_get_params },
    { OSSL_FUNC_PROVIDER_GET_REASON_STRINGS,
        (void (*)(void))p_get_reason_strings},
    { OSSL_FUNC_PROVIDER_TEARDOWN, (void (*)(void))p_teardown },
    { OSSL_FUNC_PROVIDER_QUERY_OPERATION, (void (*)(void))p_query },
    OSSL_DISPATCH_END
};

int OSSL_provider_init(const OSSL_CORE_HANDLE *handle,
                       const OSSL_DISPATCH *oin,
                       const OSSL_DISPATCH **out,
                       void **provctx)
{
    P_TEST_CTX *ctx;
    const OSSL_DISPATCH *in = oin;

    for (; in->function_id != 0; in++) {
        switch (in->function_id) {
        case OSSL_FUNC_CORE_GETTABLE_PARAMS:
            c_gettable_params = OSSL_FUNC_core_gettable_params(in);
            break;
        case OSSL_FUNC_CORE_GET_PARAMS:
            c_get_params = OSSL_FUNC_core_get_params(in);
            break;
        case OSSL_FUNC_CORE_NEW_ERROR:
            c_new_error = OSSL_FUNC_core_new_error(in);
            break;
        case OSSL_FUNC_CORE_SET_ERROR_DEBUG:
            c_set_error_debug = OSSL_FUNC_core_set_error_debug(in);
            break;
        case OSSL_FUNC_CORE_VSET_ERROR:
            c_vset_error = OSSL_FUNC_core_vset_error(in);
            break;
        case OSSL_FUNC_BIO_VSNPRINTF:
            c_BIO_vsnprintf = OSSL_FUNC_BIO_vsnprintf(in);
            break;
        default:
            /* Just ignore anything we don't understand */
            break;
        }
    }

    /*
     * We want to test that libcrypto doesn't use the file and func pointers
     * that we provide to it via c_set_error_debug beyond the time that they
     * are valid for. Therefore we dynamically allocate these strings now and
     * free them again when the provider is torn down. If anything tries to
     * use those strings after that point there will be a use-after-free and
     * asan will complain (and hence the tests will fail).
     * This file isn't linked against libcrypto, so we use malloc and strdup
     * instead of OPENSSL_malloc and OPENSSL_strdup
     */
    ctx = malloc(sizeof(*ctx));
    if (ctx == NULL)
        return 0;
    ctx->thisfile = strdup(OPENSSL_FILE);
    ctx->thisfunc = strdup(OPENSSL_FUNC);
    ctx->handle = handle;
#ifdef PROVIDER_INIT_FUNCTION_NAME
    /* We only do this if we are linked with libcrypto */
    ctx->libctx = OSSL_LIB_CTX_new_child(handle, oin);
    if (ctx->libctx == NULL) {
        /* We set error "2" for a failure to create the child libctx*/
        p_set_error(ERR_LIB_PROV, 2, ctx->thisfile, OPENSSL_LINE, ctx->thisfunc,
                    NULL);
        p_teardown(ctx);
        return 0;
    }
    /*
     * The default provider is loaded - but the default properties should not
     * allow its use.
     */
    {
        EVP_MD *sha256 = EVP_MD_fetch(ctx->libctx, "SHA2-256", NULL);
        if (sha256 != NULL) {
            EVP_MD_free(sha256);
            p_teardown(ctx);
            return 0;
        }
    }
#endif

    /*
     * Set a spurious error to check error handling works correctly. This will
     * be ignored
     */
    p_set_error(ERR_LIB_PROV, 1, ctx->thisfile, OPENSSL_LINE, ctx->thisfunc, NULL);

    *provctx = (void *)ctx;
    *out = p_test_table;
    return 1;
}

static void p_teardown(void *provctx)
{
    P_TEST_CTX *ctx = (P_TEST_CTX *)provctx;

#ifdef PROVIDER_INIT_FUNCTION_NAME
    OSSL_LIB_CTX_free(ctx->libctx);
#endif
    free(ctx->thisfile);
    free(ctx->thisfunc);
    free(ctx);
}
