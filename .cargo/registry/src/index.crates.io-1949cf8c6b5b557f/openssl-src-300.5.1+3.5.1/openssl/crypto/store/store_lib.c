/*
 * Copyright 2016-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdlib.h>
#include <string.h>
#include <assert.h>

/* We need to use some STORE deprecated APIs */
#define OPENSSL_SUPPRESS_DEPRECATED

#include "internal/e_os.h"

#include <openssl/crypto.h>
#include <openssl/err.h>
#include <openssl/trace.h>
#include <openssl/core_names.h>
#include <openssl/provider.h>
#include <openssl/param_build.h>
#include <openssl/store.h>
#include "internal/thread_once.h"
#include "internal/cryptlib.h"
#include "internal/provider.h"
#include "internal/bio.h"
#include "crypto/store.h"
#include "store_local.h"

static int ossl_store_close_it(OSSL_STORE_CTX *ctx);

static int loader_set_params(OSSL_STORE_LOADER *loader,
                             OSSL_STORE_LOADER_CTX *loader_ctx,
                             const OSSL_PARAM params[], const char *propq)
{
   if (params != NULL) {
       if (!loader->p_set_ctx_params(loader_ctx, params))
           return 0;
   }

   if (propq != NULL) {
       OSSL_PARAM propp[2];

       if (OSSL_PARAM_locate_const(params,
                                   OSSL_STORE_PARAM_PROPERTIES) != NULL)
           /* use the propq from params */
           return 1;

       propp[0] = OSSL_PARAM_construct_utf8_string(OSSL_STORE_PARAM_PROPERTIES,
                                                   (char *)propq, 0);
       propp[1] = OSSL_PARAM_construct_end();

       if (!loader->p_set_ctx_params(loader_ctx, propp))
           return 0;
    }
    return 1;
}

OSSL_STORE_CTX *
OSSL_STORE_open_ex(const char *uri, OSSL_LIB_CTX *libctx, const char *propq,
                   const UI_METHOD *ui_method, void *ui_data,
                   const OSSL_PARAM params[],
                   OSSL_STORE_post_process_info_fn post_process,
                   void *post_process_data)
{
    struct ossl_passphrase_data_st pwdata = { 0 };
    const OSSL_STORE_LOADER *loader = NULL;
    OSSL_STORE_LOADER *fetched_loader = NULL;
    OSSL_STORE_LOADER_CTX *loader_ctx = NULL;
    OSSL_STORE_CTX *ctx = NULL;
    char *propq_copy = NULL;
    int no_loader_found = 1;
    char scheme_copy[256], *p, *schemes[2], *scheme = NULL;
    size_t schemes_n = 0;
    size_t i;

    /*
     * Put the file scheme first.  If the uri does represent an existing file,
     * possible device name and all, then it should be loaded.  Only a failed
     * attempt at loading a local file should have us try something else.
     */
    schemes[schemes_n++] = "file";

    /*
     * Now, check if we have something that looks like a scheme, and add it
     * as a second scheme.  However, also check if there's an authority start
     * (://), because that will invalidate the previous file scheme.  Also,
     * check that this isn't actually the file scheme, as there's no point
     * going through that one twice!
     */
    OPENSSL_strlcpy(scheme_copy, uri, sizeof(scheme_copy));
    if ((p = strchr(scheme_copy, ':')) != NULL) {
        *p++ = '\0';
        if (OPENSSL_strcasecmp(scheme_copy, "file") != 0) {
            if (HAS_PREFIX(p, "//"))
                schemes_n--;         /* Invalidate the file scheme */
            schemes[schemes_n++] = scheme_copy;
        }
    }

    ERR_set_mark();

    if (ui_method != NULL
        && (!ossl_pw_set_ui_method(&pwdata, ui_method, ui_data)
            || !ossl_pw_enable_passphrase_caching(&pwdata))) {
        ERR_raise(ERR_LIB_OSSL_STORE, ERR_R_CRYPTO_LIB);
        goto err;
    }

    /*
     * Try each scheme until we find one that could open the URI.
     *
     * For each scheme, we look for the engine implementation first, and
     * failing that, we then try to fetch a provided implementation.
     * This is consistent with how we handle legacy / engine implementations
     * elsewhere.
     */
    for (i = 0; loader_ctx == NULL && i < schemes_n; i++) {
        scheme = schemes[i];
        OSSL_TRACE1(STORE, "Looking up scheme %s\n", scheme);
#ifndef OPENSSL_NO_DEPRECATED_3_0
        ERR_set_mark();
        if ((loader = ossl_store_get0_loader_int(scheme)) != NULL) {
            ERR_clear_last_mark();
            no_loader_found = 0;
            if (loader->open_ex != NULL)
                loader_ctx = loader->open_ex(loader, uri, libctx, propq,
                                             ui_method, ui_data);
            else
                loader_ctx = loader->open(loader, uri, ui_method, ui_data);
        } else {
            ERR_pop_to_mark();
        }
#endif
        if (loader == NULL
            && (fetched_loader =
                OSSL_STORE_LOADER_fetch(libctx, scheme, propq)) != NULL) {
            const OSSL_PROVIDER *provider =
                OSSL_STORE_LOADER_get0_provider(fetched_loader);
            void *provctx = OSSL_PROVIDER_get0_provider_ctx(provider);

            no_loader_found = 0;
            if (fetched_loader->p_open_ex != NULL) {
                loader_ctx =
                    fetched_loader->p_open_ex(provctx, uri, params,
                                              ossl_pw_passphrase_callback_dec,
                                              &pwdata);
            } else {
                if (fetched_loader->p_open != NULL &&
                    (loader_ctx = fetched_loader->p_open(provctx, uri)) != NULL &&
                    !loader_set_params(fetched_loader, loader_ctx,
                                       params, propq)) {
                    (void)fetched_loader->p_close(loader_ctx);
                    loader_ctx = NULL;
                }
            }
            if (loader_ctx == NULL) {
                OSSL_STORE_LOADER_free(fetched_loader);
                fetched_loader = NULL;
            }
            loader = fetched_loader;

            /* Clear any internally cached passphrase */
            (void)ossl_pw_clear_passphrase_cache(&pwdata);
        }
    }

    if (no_loader_found)
        /*
         * It's assumed that ossl_store_get0_loader_int() and
         * OSSL_STORE_LOADER_fetch() report their own errors
         */
        goto err;

    OSSL_TRACE1(STORE, "Found loader for scheme %s\n", scheme);

    if (loader_ctx == NULL)
        /*
         * It's assumed that the loader's open() method reports its own
         * errors
         */
        goto err;

    OSSL_TRACE2(STORE, "Opened %s => %p\n", uri, (void *)loader_ctx);

    if ((propq != NULL && (propq_copy = OPENSSL_strdup(propq)) == NULL)
        || (ctx = OPENSSL_zalloc(sizeof(*ctx))) == NULL)
        goto err;

    ctx->properties = propq_copy;
    ctx->fetched_loader = fetched_loader;
    ctx->loader = loader;
    ctx->loader_ctx = loader_ctx;
    ctx->post_process = post_process;
    ctx->post_process_data = post_process_data;
    ctx->pwdata = pwdata;

    /*
     * If the attempt to open with the 'file' scheme loader failed and the
     * other scheme loader succeeded, the failure to open with the 'file'
     * scheme loader leaves an error on the error stack.  Let's remove it.
     */
    ERR_pop_to_mark();

    return ctx;

 err:
    ERR_clear_last_mark();
    if (loader_ctx != NULL) {
        /*
         * Temporary structure so OSSL_STORE_close() can work even when
         * |ctx| couldn't be allocated properly
         */
        OSSL_STORE_CTX tmpctx = { NULL, };

        tmpctx.fetched_loader = fetched_loader;
        tmpctx.loader = loader;
        tmpctx.loader_ctx = loader_ctx;

        /*
         * We ignore a returned error because we will return NULL anyway in
         * this case, so if something goes wrong when closing, that'll simply
         * just add another entry on the error stack.
         */
        (void)ossl_store_close_it(&tmpctx);
    }
    /* Coverity false positive, the reference counting is confusing it */
    /* coverity[pass_freed_arg] */
    OSSL_STORE_LOADER_free(fetched_loader);
    OPENSSL_free(propq_copy);
    OPENSSL_free(ctx);
    return NULL;
}

OSSL_STORE_CTX *OSSL_STORE_open(const char *uri,
                                const UI_METHOD *ui_method, void *ui_data,
                                OSSL_STORE_post_process_info_fn post_process,
                                void *post_process_data)
{
    return OSSL_STORE_open_ex(uri, NULL, NULL, ui_method, ui_data, NULL,
                              post_process, post_process_data);
}

#ifndef OPENSSL_NO_DEPRECATED_3_0
int OSSL_STORE_ctrl(OSSL_STORE_CTX *ctx, int cmd, ...)
{
    va_list args;
    int ret;

    va_start(args, cmd);
    ret = OSSL_STORE_vctrl(ctx, cmd, args);
    va_end(args);

    return ret;
}

int OSSL_STORE_vctrl(OSSL_STORE_CTX *ctx, int cmd, va_list args)
{
    if (ctx->fetched_loader != NULL) {
        if (ctx->fetched_loader->p_set_ctx_params != NULL) {
            OSSL_PARAM params[2] = { OSSL_PARAM_END, OSSL_PARAM_END };

            switch (cmd) {
            case OSSL_STORE_C_USE_SECMEM:
                {
                    int on = *(va_arg(args, int *));

                    params[0] = OSSL_PARAM_construct_int("use_secmem", &on);
                }
                break;
            default:
                break;
            }

            return ctx->fetched_loader->p_set_ctx_params(ctx->loader_ctx,
                                                         params);
        }
    } else if (ctx->loader->ctrl != NULL) {
        return ctx->loader->ctrl(ctx->loader_ctx, cmd, args);
    }

    /*
     * If the fetched loader doesn't have a set_ctx_params or a ctrl, it's as
     * if there was one that ignored our params, which usually returns 1.
     */
    return 1;
}
#endif

int OSSL_STORE_expect(OSSL_STORE_CTX *ctx, int expected_type)
{
    int ret = 1;

    if (ctx == NULL
            || expected_type < 0 || expected_type > OSSL_STORE_INFO_CRL) {
        ERR_raise(ERR_LIB_OSSL_STORE, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    if (ctx->loading) {
        ERR_raise(ERR_LIB_OSSL_STORE, OSSL_STORE_R_LOADING_STARTED);
        return 0;
    }

    ctx->expected_type = expected_type;
    if (ctx->fetched_loader != NULL
        && ctx->fetched_loader->p_set_ctx_params != NULL) {
        OSSL_PARAM params[2] = { OSSL_PARAM_END, OSSL_PARAM_END };

        params[0] =
            OSSL_PARAM_construct_int(OSSL_STORE_PARAM_EXPECT, &expected_type);
        ret = ctx->fetched_loader->p_set_ctx_params(ctx->loader_ctx, params);
    }
#ifndef OPENSSL_NO_DEPRECATED_3_0
    if (ctx->fetched_loader == NULL
        && ctx->loader->expect != NULL) {
        ret = ctx->loader->expect(ctx->loader_ctx, expected_type);
    }
#endif
    return ret;
}

int OSSL_STORE_find(OSSL_STORE_CTX *ctx, const OSSL_STORE_SEARCH *search)
{
    int ret = 1;

    if (ctx->loading) {
        ERR_raise(ERR_LIB_OSSL_STORE, OSSL_STORE_R_LOADING_STARTED);
        return 0;
    }
    if (search == NULL) {
        ERR_raise(ERR_LIB_OSSL_STORE, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if (ctx->fetched_loader != NULL) {
        OSSL_PARAM_BLD *bld;
        OSSL_PARAM *params;
        /* OSSL_STORE_SEARCH_BY_NAME, OSSL_STORE_SEARCH_BY_ISSUER_SERIAL*/
        void *name_der = NULL;
        int name_der_sz;
        /* OSSL_STORE_SEARCH_BY_ISSUER_SERIAL */
        BIGNUM *number = NULL;

        if (ctx->fetched_loader->p_set_ctx_params == NULL) {
            ERR_raise(ERR_LIB_OSSL_STORE, OSSL_STORE_R_UNSUPPORTED_OPERATION);
            return 0;
        }

        if ((bld = OSSL_PARAM_BLD_new()) == NULL) {
            ERR_raise(ERR_LIB_OSSL_STORE, ERR_R_CRYPTO_LIB);
            return 0;
        }

        ret = 0;                 /* Assume the worst */

        switch (search->search_type) {
        case OSSL_STORE_SEARCH_BY_NAME:
            if ((name_der_sz = i2d_X509_NAME(search->name,
                                             (unsigned char **)&name_der)) > 0
                && OSSL_PARAM_BLD_push_octet_string(bld,
                                                    OSSL_STORE_PARAM_SUBJECT,
                                                    name_der, name_der_sz))
                ret = 1;
            break;
        case OSSL_STORE_SEARCH_BY_ISSUER_SERIAL:
            if ((name_der_sz = i2d_X509_NAME(search->name,
                                             (unsigned char **)&name_der)) > 0
                && (number = ASN1_INTEGER_to_BN(search->serial, NULL)) != NULL
                && OSSL_PARAM_BLD_push_octet_string(bld,
                                                    OSSL_STORE_PARAM_ISSUER,
                                                    name_der, name_der_sz)
                && OSSL_PARAM_BLD_push_BN(bld, OSSL_STORE_PARAM_SERIAL,
                                          number))
                ret = 1;
            break;
        case OSSL_STORE_SEARCH_BY_KEY_FINGERPRINT:
            if (OSSL_PARAM_BLD_push_utf8_string(bld, OSSL_STORE_PARAM_DIGEST,
                                                EVP_MD_get0_name(search->digest),
                                                0)
                && OSSL_PARAM_BLD_push_octet_string(bld,
                                                    OSSL_STORE_PARAM_FINGERPRINT,
                                                    search->string,
                                                    search->stringlength))
                ret = 1;
            break;
        case OSSL_STORE_SEARCH_BY_ALIAS:
            if (OSSL_PARAM_BLD_push_utf8_string(bld, OSSL_STORE_PARAM_ALIAS,
                                                (char *)search->string,
                                                search->stringlength))
                ret = 1;
            break;
        }
        if (ret) {
            params = OSSL_PARAM_BLD_to_param(bld);
            ret = ctx->fetched_loader->p_set_ctx_params(ctx->loader_ctx,
                                                        params);
            OSSL_PARAM_free(params);
        }
        OSSL_PARAM_BLD_free(bld);
        OPENSSL_free(name_der);
        BN_free(number);
    } else {
#ifndef OPENSSL_NO_DEPRECATED_3_0
        /* legacy loader section */
        if (ctx->loader->find == NULL) {
            ERR_raise(ERR_LIB_OSSL_STORE, OSSL_STORE_R_UNSUPPORTED_OPERATION);
            return 0;
        }
        ret = ctx->loader->find(ctx->loader_ctx, search);
#endif
    }

    return ret;
}

OSSL_STORE_INFO *OSSL_STORE_load(OSSL_STORE_CTX *ctx)
{
    OSSL_STORE_INFO *v = NULL;

    ctx->loading = 1;
 again:
    if (OSSL_STORE_eof(ctx))
        return NULL;

    if (ctx->loader != NULL)
        OSSL_TRACE(STORE, "Loading next object\n");

    if (ctx->cached_info != NULL
        && sk_OSSL_STORE_INFO_num(ctx->cached_info) == 0) {
        sk_OSSL_STORE_INFO_free(ctx->cached_info);
        ctx->cached_info = NULL;
    }

    if (ctx->cached_info != NULL) {
        v = sk_OSSL_STORE_INFO_shift(ctx->cached_info);
    } else {
        if (ctx->fetched_loader != NULL) {
            struct ossl_load_result_data_st load_data;

            load_data.v = NULL;
            load_data.ctx = ctx;
            ctx->error_flag = 0;

            if (!ctx->fetched_loader->p_load(ctx->loader_ctx,
                                             ossl_store_handle_load_result,
                                             &load_data,
                                             ossl_pw_passphrase_callback_dec,
                                             &ctx->pwdata)) {
                ctx->error_flag = 1;
                return NULL;
            }
            v = load_data.v;
        }
#ifndef OPENSSL_NO_DEPRECATED_3_0
        if (ctx->fetched_loader == NULL)
            v = ctx->loader->load(ctx->loader_ctx,
                                  ctx->pwdata._.ui_method.ui_method,
                                  ctx->pwdata._.ui_method.ui_method_data);
#endif
    }

    if (ctx->post_process != NULL && v != NULL) {
        v = ctx->post_process(v, ctx->post_process_data);

        /*
         * By returning NULL, the callback decides that this object should
         * be ignored.
         */
        if (v == NULL)
            goto again;
    }

    /* Clear any internally cached passphrase */
    (void)ossl_pw_clear_passphrase_cache(&ctx->pwdata);

    if (v != NULL && ctx->expected_type != 0) {
        int returned_type = OSSL_STORE_INFO_get_type(v);

        if (returned_type != OSSL_STORE_INFO_NAME && returned_type != 0) {
            if (ctx->expected_type != returned_type) {
                OSSL_STORE_INFO_free(v);
                goto again;
            }
        }
    }

    if (v != NULL)
        OSSL_TRACE1(STORE, "Got a %s\n",
                    OSSL_STORE_INFO_type_string(OSSL_STORE_INFO_get_type(v)));

    return v;
}

int OSSL_STORE_delete(const char *uri, OSSL_LIB_CTX *libctx, const char *propq,
                      const UI_METHOD *ui_method, void *ui_data,
                      const OSSL_PARAM params[])
{
    OSSL_STORE_LOADER *fetched_loader = NULL;
    char scheme[256], *p;
    int res = 0;
    struct ossl_passphrase_data_st pwdata = {0};

    OPENSSL_strlcpy(scheme, uri, sizeof(scheme));
    if ((p = strchr(scheme, ':')) != NULL)
        *p++ = '\0';
    else /* We don't work without explicit scheme */
        return 0;

    if (ui_method != NULL
        && (!ossl_pw_set_ui_method(&pwdata, ui_method, ui_data)
            || !ossl_pw_enable_passphrase_caching(&pwdata))) {
        ERR_raise(ERR_LIB_OSSL_STORE, ERR_R_CRYPTO_LIB);
        return 0;
    }

    OSSL_TRACE1(STORE, "Looking up scheme %s\n", scheme);
    fetched_loader = OSSL_STORE_LOADER_fetch(libctx, scheme, propq);

    if (fetched_loader != NULL && fetched_loader->p_delete != NULL) {
        const OSSL_PROVIDER *provider =
            OSSL_STORE_LOADER_get0_provider(fetched_loader);
        void *provctx = OSSL_PROVIDER_get0_provider_ctx(provider);

        /*
         * It's assumed that the loader's delete() method reports its own
         * errors
         */
        OSSL_TRACE1(STORE, "Performing URI delete %s\n", uri);
        res = fetched_loader->p_delete(provctx, uri, params,
                                       ossl_pw_passphrase_callback_dec,
                                       &pwdata);
    }
    /* Clear any internally cached passphrase */
    (void)ossl_pw_clear_passphrase_cache(&pwdata);

    OSSL_STORE_LOADER_free(fetched_loader);

    return res;
}

int OSSL_STORE_error(OSSL_STORE_CTX *ctx)
{
    int ret = 1;

    if (ctx->fetched_loader != NULL)
        ret = ctx->error_flag;
#ifndef OPENSSL_NO_DEPRECATED_3_0
    if (ctx->fetched_loader == NULL)
        ret = ctx->loader->error(ctx->loader_ctx);
#endif
    return ret;
}

int OSSL_STORE_eof(OSSL_STORE_CTX *ctx)
{
    int ret = 1;

    if (ctx->fetched_loader != NULL)
        ret = ctx->loader->p_eof(ctx->loader_ctx);
#ifndef OPENSSL_NO_DEPRECATED_3_0
    if (ctx->fetched_loader == NULL)
        ret = ctx->loader->eof(ctx->loader_ctx);
#endif
    return ret != 0;
}

static int ossl_store_close_it(OSSL_STORE_CTX *ctx)
{
    int ret = 0;

    if (ctx == NULL)
        return 1;
    OSSL_TRACE1(STORE, "Closing %p\n", (void *)ctx->loader_ctx);

    if (ctx->fetched_loader != NULL)
        ret = ctx->loader->p_close(ctx->loader_ctx);
#ifndef OPENSSL_NO_DEPRECATED_3_0
    if (ctx->fetched_loader == NULL)
        ret = ctx->loader->closefn(ctx->loader_ctx);
#endif

    sk_OSSL_STORE_INFO_pop_free(ctx->cached_info, OSSL_STORE_INFO_free);
    OSSL_STORE_LOADER_free(ctx->fetched_loader);
    OPENSSL_free(ctx->properties);
    ossl_pw_clear_passphrase_data(&ctx->pwdata);
    return ret;
}

int OSSL_STORE_close(OSSL_STORE_CTX *ctx)
{
    int ret = ossl_store_close_it(ctx);

    OPENSSL_free(ctx);
    return ret;
}

/*
 * Functions to generate OSSL_STORE_INFOs, one function for each type we
 * support having in them as well as a generic constructor.
 *
 * In all cases, ownership of the object is transferred to the OSSL_STORE_INFO
 * and will therefore be freed when the OSSL_STORE_INFO is freed.
 */
OSSL_STORE_INFO *OSSL_STORE_INFO_new(int type, void *data)
{
    OSSL_STORE_INFO *info = OPENSSL_zalloc(sizeof(*info));

    if (info == NULL)
        return NULL;

    info->type = type;
    info->_.data = data;
    return info;
}

OSSL_STORE_INFO *OSSL_STORE_INFO_new_NAME(char *name)
{
    OSSL_STORE_INFO *info = OSSL_STORE_INFO_new(OSSL_STORE_INFO_NAME, NULL);

    if (info == NULL) {
        ERR_raise(ERR_LIB_OSSL_STORE, ERR_R_OSSL_STORE_LIB);
        return NULL;
    }

    info->_.name.name = name;
    info->_.name.desc = NULL;

    return info;
}

int OSSL_STORE_INFO_set0_NAME_description(OSSL_STORE_INFO *info, char *desc)
{
    if (info->type != OSSL_STORE_INFO_NAME) {
        ERR_raise(ERR_LIB_OSSL_STORE, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }

    info->_.name.desc = desc;

    return 1;
}
OSSL_STORE_INFO *OSSL_STORE_INFO_new_PARAMS(EVP_PKEY *params)
{
    OSSL_STORE_INFO *info = OSSL_STORE_INFO_new(OSSL_STORE_INFO_PARAMS, params);

    if (info == NULL)
        ERR_raise(ERR_LIB_OSSL_STORE, ERR_R_OSSL_STORE_LIB);
    return info;
}

OSSL_STORE_INFO *OSSL_STORE_INFO_new_PUBKEY(EVP_PKEY *pkey)
{
    OSSL_STORE_INFO *info = OSSL_STORE_INFO_new(OSSL_STORE_INFO_PUBKEY, pkey);

    if (info == NULL)
        ERR_raise(ERR_LIB_OSSL_STORE, ERR_R_OSSL_STORE_LIB);
    return info;
}

OSSL_STORE_INFO *OSSL_STORE_INFO_new_PKEY(EVP_PKEY *pkey)
{
    OSSL_STORE_INFO *info = OSSL_STORE_INFO_new(OSSL_STORE_INFO_PKEY, pkey);

    if (info == NULL)
        ERR_raise(ERR_LIB_OSSL_STORE, ERR_R_OSSL_STORE_LIB);
    return info;
}

OSSL_STORE_INFO *OSSL_STORE_INFO_new_CERT(X509 *x509)
{
    OSSL_STORE_INFO *info = OSSL_STORE_INFO_new(OSSL_STORE_INFO_CERT, x509);

    if (info == NULL)
        ERR_raise(ERR_LIB_OSSL_STORE, ERR_R_OSSL_STORE_LIB);
    return info;
}

OSSL_STORE_INFO *OSSL_STORE_INFO_new_CRL(X509_CRL *crl)
{
    OSSL_STORE_INFO *info = OSSL_STORE_INFO_new(OSSL_STORE_INFO_CRL, crl);

    if (info == NULL)
        ERR_raise(ERR_LIB_OSSL_STORE, ERR_R_OSSL_STORE_LIB);
    return info;
}

/*
 * Functions to try to extract data from an OSSL_STORE_INFO.
 */
int OSSL_STORE_INFO_get_type(const OSSL_STORE_INFO *info)
{
    return info->type;
}

void *OSSL_STORE_INFO_get0_data(int type, const OSSL_STORE_INFO *info)
{
    if (info->type == type)
        return info->_.data;
    return NULL;
}

const char *OSSL_STORE_INFO_get0_NAME(const OSSL_STORE_INFO *info)
{
    if (info->type == OSSL_STORE_INFO_NAME)
        return info->_.name.name;
    return NULL;
}

char *OSSL_STORE_INFO_get1_NAME(const OSSL_STORE_INFO *info)
{
    if (info->type == OSSL_STORE_INFO_NAME)
        return OPENSSL_strdup(info->_.name.name);
    ERR_raise(ERR_LIB_OSSL_STORE, OSSL_STORE_R_NOT_A_NAME);
    return NULL;
}

const char *OSSL_STORE_INFO_get0_NAME_description(const OSSL_STORE_INFO *info)
{
    if (info->type == OSSL_STORE_INFO_NAME)
        return info->_.name.desc;
    return NULL;
}

char *OSSL_STORE_INFO_get1_NAME_description(const OSSL_STORE_INFO *info)
{
    if (info->type == OSSL_STORE_INFO_NAME)
        return OPENSSL_strdup(info->_.name.desc ? info->_.name.desc : "");
    ERR_raise(ERR_LIB_OSSL_STORE, OSSL_STORE_R_NOT_A_NAME);
    return NULL;
}

EVP_PKEY *OSSL_STORE_INFO_get0_PARAMS(const OSSL_STORE_INFO *info)
{
    if (info->type == OSSL_STORE_INFO_PARAMS)
        return info->_.params;
    return NULL;
}

EVP_PKEY *OSSL_STORE_INFO_get1_PARAMS(const OSSL_STORE_INFO *info)
{
    if (info->type == OSSL_STORE_INFO_PARAMS) {
        if (!EVP_PKEY_up_ref(info->_.params))
            return NULL;
        return info->_.params;
    }
    ERR_raise(ERR_LIB_OSSL_STORE, OSSL_STORE_R_NOT_PARAMETERS);
    return NULL;
}

EVP_PKEY *OSSL_STORE_INFO_get0_PUBKEY(const OSSL_STORE_INFO *info)
{
    if (info->type == OSSL_STORE_INFO_PUBKEY)
        return info->_.pubkey;
    return NULL;
}

EVP_PKEY *OSSL_STORE_INFO_get1_PUBKEY(const OSSL_STORE_INFO *info)
{
    if (info->type == OSSL_STORE_INFO_PUBKEY) {
        if (!EVP_PKEY_up_ref(info->_.pubkey))
            return NULL;
        return info->_.pubkey;
    }
    ERR_raise(ERR_LIB_OSSL_STORE, OSSL_STORE_R_NOT_A_PUBLIC_KEY);
    return NULL;
}

EVP_PKEY *OSSL_STORE_INFO_get0_PKEY(const OSSL_STORE_INFO *info)
{
    if (info->type == OSSL_STORE_INFO_PKEY)
        return info->_.pkey;
    return NULL;
}

EVP_PKEY *OSSL_STORE_INFO_get1_PKEY(const OSSL_STORE_INFO *info)
{
    if (info->type == OSSL_STORE_INFO_PKEY) {
        if (!EVP_PKEY_up_ref(info->_.pkey))
            return NULL;
        return info->_.pkey;
    }
    ERR_raise(ERR_LIB_OSSL_STORE, OSSL_STORE_R_NOT_A_PRIVATE_KEY);
    return NULL;
}

X509 *OSSL_STORE_INFO_get0_CERT(const OSSL_STORE_INFO *info)
{
    if (info->type == OSSL_STORE_INFO_CERT)
        return info->_.x509;
    return NULL;
}

X509 *OSSL_STORE_INFO_get1_CERT(const OSSL_STORE_INFO *info)
{
    if (info->type == OSSL_STORE_INFO_CERT) {
        if (!X509_up_ref(info->_.x509))
            return NULL;
        return info->_.x509;
    }
    ERR_raise(ERR_LIB_OSSL_STORE, OSSL_STORE_R_NOT_A_CERTIFICATE);
    return NULL;
}

X509_CRL *OSSL_STORE_INFO_get0_CRL(const OSSL_STORE_INFO *info)
{
    if (info->type == OSSL_STORE_INFO_CRL)
        return info->_.crl;
    return NULL;
}

X509_CRL *OSSL_STORE_INFO_get1_CRL(const OSSL_STORE_INFO *info)
{
    if (info->type == OSSL_STORE_INFO_CRL) {
        if (!X509_CRL_up_ref(info->_.crl))
            return NULL;
        return info->_.crl;
    }
    ERR_raise(ERR_LIB_OSSL_STORE, OSSL_STORE_R_NOT_A_CRL);
    return NULL;
}

/*
 * Free the OSSL_STORE_INFO
 */
void OSSL_STORE_INFO_free(OSSL_STORE_INFO *info)
{
    if (info != NULL) {
        switch (info->type) {
        case OSSL_STORE_INFO_NAME:
            OPENSSL_free(info->_.name.name);
            OPENSSL_free(info->_.name.desc);
            break;
        case OSSL_STORE_INFO_PARAMS:
            EVP_PKEY_free(info->_.params);
            break;
        case OSSL_STORE_INFO_PUBKEY:
            EVP_PKEY_free(info->_.pubkey);
            break;
        case OSSL_STORE_INFO_PKEY:
            EVP_PKEY_free(info->_.pkey);
            break;
        case OSSL_STORE_INFO_CERT:
            X509_free(info->_.x509);
            break;
        case OSSL_STORE_INFO_CRL:
            X509_CRL_free(info->_.crl);
            break;
        }
        OPENSSL_free(info);
    }
}

int OSSL_STORE_supports_search(OSSL_STORE_CTX *ctx, int search_type)
{
    int ret = 0;

    if (ctx->fetched_loader != NULL) {
        void *provctx =
            ossl_provider_ctx(OSSL_STORE_LOADER_get0_provider(ctx->fetched_loader));
        const OSSL_PARAM *params;
        const OSSL_PARAM *p_subject = NULL;
        const OSSL_PARAM *p_issuer = NULL;
        const OSSL_PARAM *p_serial = NULL;
        const OSSL_PARAM *p_fingerprint = NULL;
        const OSSL_PARAM *p_alias = NULL;

        if (ctx->fetched_loader->p_settable_ctx_params == NULL)
            return 0;

        params = ctx->fetched_loader->p_settable_ctx_params(provctx);
        p_subject = OSSL_PARAM_locate_const(params, OSSL_STORE_PARAM_SUBJECT);
        p_issuer = OSSL_PARAM_locate_const(params, OSSL_STORE_PARAM_ISSUER);
        p_serial = OSSL_PARAM_locate_const(params, OSSL_STORE_PARAM_SERIAL);
        p_fingerprint =
            OSSL_PARAM_locate_const(params, OSSL_STORE_PARAM_FINGERPRINT);
        p_alias = OSSL_PARAM_locate_const(params, OSSL_STORE_PARAM_ALIAS);

        switch (search_type) {
        case OSSL_STORE_SEARCH_BY_NAME:
            ret = (p_subject != NULL);
            break;
        case OSSL_STORE_SEARCH_BY_ISSUER_SERIAL:
            ret = (p_issuer != NULL && p_serial != NULL);
            break;
        case OSSL_STORE_SEARCH_BY_KEY_FINGERPRINT:
            ret = (p_fingerprint != NULL);
            break;
        case OSSL_STORE_SEARCH_BY_ALIAS:
            ret = (p_alias != NULL);
            break;
        }
    }
#ifndef OPENSSL_NO_DEPRECATED_3_0
    if (ctx->fetched_loader == NULL) {
        OSSL_STORE_SEARCH tmp_search;

        if (ctx->loader->find == NULL)
            return 0;
        tmp_search.search_type = search_type;
        ret = ctx->loader->find(NULL, &tmp_search);
    }
#endif
    return ret;
}

/* Search term constructors */
OSSL_STORE_SEARCH *OSSL_STORE_SEARCH_by_name(X509_NAME *name)
{
    OSSL_STORE_SEARCH *search = OPENSSL_zalloc(sizeof(*search));

    if (search == NULL)
        return NULL;

    search->search_type = OSSL_STORE_SEARCH_BY_NAME;
    search->name = name;
    return search;
}

OSSL_STORE_SEARCH *OSSL_STORE_SEARCH_by_issuer_serial(X509_NAME *name,
                                                      const ASN1_INTEGER *serial)
{
    OSSL_STORE_SEARCH *search = OPENSSL_zalloc(sizeof(*search));

    if (search == NULL)
        return NULL;

    search->search_type = OSSL_STORE_SEARCH_BY_ISSUER_SERIAL;
    search->name = name;
    search->serial = serial;
    return search;
}

OSSL_STORE_SEARCH *OSSL_STORE_SEARCH_by_key_fingerprint(const EVP_MD *digest,
                                                        const unsigned char
                                                        *bytes, size_t len)
{
    OSSL_STORE_SEARCH *search = OPENSSL_zalloc(sizeof(*search));
    int md_size;

    if (search == NULL)
        return NULL;

    md_size = EVP_MD_get_size(digest);
    if (md_size <= 0) {
        OPENSSL_free(search);
        return NULL;
    }

    if (digest != NULL && len != (size_t)md_size) {
        ERR_raise_data(ERR_LIB_OSSL_STORE,
                       OSSL_STORE_R_FINGERPRINT_SIZE_DOES_NOT_MATCH_DIGEST,
                       "%s size is %d, fingerprint size is %zu",
                       EVP_MD_get0_name(digest), md_size, len);
        OPENSSL_free(search);
        return NULL;
    }

    search->search_type = OSSL_STORE_SEARCH_BY_KEY_FINGERPRINT;
    search->digest = digest;
    search->string = bytes;
    search->stringlength = len;
    return search;
}

OSSL_STORE_SEARCH *OSSL_STORE_SEARCH_by_alias(const char *alias)
{
    OSSL_STORE_SEARCH *search = OPENSSL_zalloc(sizeof(*search));

    if (search == NULL)
        return NULL;

    search->search_type = OSSL_STORE_SEARCH_BY_ALIAS;
    search->string = (const unsigned char *)alias;
    search->stringlength = strlen(alias);
    return search;
}

/* Search term destructor */
void OSSL_STORE_SEARCH_free(OSSL_STORE_SEARCH *search)
{
    OPENSSL_free(search);
}

/* Search term accessors */
int OSSL_STORE_SEARCH_get_type(const OSSL_STORE_SEARCH *criterion)
{
    return criterion->search_type;
}

X509_NAME *OSSL_STORE_SEARCH_get0_name(const OSSL_STORE_SEARCH *criterion)
{
    return criterion->name;
}

const ASN1_INTEGER *OSSL_STORE_SEARCH_get0_serial(const OSSL_STORE_SEARCH
                                                  *criterion)
{
    return criterion->serial;
}

const unsigned char *OSSL_STORE_SEARCH_get0_bytes(const OSSL_STORE_SEARCH
                                                  *criterion, size_t *length)
{
    *length = criterion->stringlength;
    return criterion->string;
}

const char *OSSL_STORE_SEARCH_get0_string(const OSSL_STORE_SEARCH *criterion)
{
    return (const char *)criterion->string;
}

const EVP_MD *OSSL_STORE_SEARCH_get0_digest(const OSSL_STORE_SEARCH *criterion)
{
    return criterion->digest;
}

OSSL_STORE_CTX *OSSL_STORE_attach(BIO *bp, const char *scheme,
                                  OSSL_LIB_CTX *libctx, const char *propq,
                                  const UI_METHOD *ui_method, void *ui_data,
                                  const OSSL_PARAM params[],
                                  OSSL_STORE_post_process_info_fn post_process,
                                  void *post_process_data)
{
    const OSSL_STORE_LOADER *loader = NULL;
    OSSL_STORE_LOADER *fetched_loader = NULL;
    OSSL_STORE_LOADER_CTX *loader_ctx = NULL;
    OSSL_STORE_CTX *ctx = NULL;

    if (scheme == NULL)
        scheme = "file";

    OSSL_TRACE1(STORE, "Looking up scheme %s\n", scheme);
    ERR_set_mark();
#ifndef OPENSSL_NO_DEPRECATED_3_0
    if ((loader = ossl_store_get0_loader_int(scheme)) != NULL)
        loader_ctx = loader->attach(loader, bp, libctx, propq,
                                    ui_method, ui_data);
#endif
    if (loader == NULL
        && (fetched_loader =
            OSSL_STORE_LOADER_fetch(libctx, scheme, propq)) != NULL) {
        const OSSL_PROVIDER *provider =
            OSSL_STORE_LOADER_get0_provider(fetched_loader);
        void *provctx = OSSL_PROVIDER_get0_provider_ctx(provider);
        OSSL_CORE_BIO *cbio = ossl_core_bio_new_from_bio(bp);

        if (cbio == NULL
            || fetched_loader->p_attach == NULL
            || (loader_ctx = fetched_loader->p_attach(provctx, cbio)) == NULL) {
            OSSL_STORE_LOADER_free(fetched_loader);
            fetched_loader = NULL;
        } else if (!loader_set_params(fetched_loader, loader_ctx,
                                      params, propq)) {
            (void)fetched_loader->p_close(loader_ctx);
            OSSL_STORE_LOADER_free(fetched_loader);
            fetched_loader = NULL;
        }
        loader = fetched_loader;
        ossl_core_bio_free(cbio);
    }

    if (loader_ctx == NULL) {
        ERR_clear_last_mark();
        return NULL;
    }

    if ((ctx = OPENSSL_zalloc(sizeof(*ctx))) == NULL) {
        ERR_clear_last_mark();
        return NULL;
    }

    if (ui_method != NULL
        && !ossl_pw_set_ui_method(&ctx->pwdata, ui_method, ui_data)) {
        ERR_clear_last_mark();
        OPENSSL_free(ctx);
        return NULL;
    }

    ctx->fetched_loader = fetched_loader;
    ctx->loader = loader;
    ctx->loader_ctx = loader_ctx;
    ctx->post_process = post_process;
    ctx->post_process_data = post_process_data;

    /*
     * ossl_store_get0_loader_int will raise an error if the loader for
     * the scheme cannot be retrieved. But if a loader was successfully
     * fetched then we remove this error from the error stack.
     */
    ERR_pop_to_mark();

    return ctx;
}
