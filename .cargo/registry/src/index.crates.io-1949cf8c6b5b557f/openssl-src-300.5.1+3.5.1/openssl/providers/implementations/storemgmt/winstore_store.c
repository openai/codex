/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#include <openssl/store.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/core_object.h>
#include <openssl/bio.h>
#include <openssl/err.h>
#include <openssl/params.h>
#include <openssl/decoder.h>
#include <openssl/proverr.h>
#include <openssl/store.h>       /* The OSSL_STORE_INFO type numbers */
#include "internal/cryptlib.h"
#include "internal/o_dir.h"
#include "crypto/decoder.h"
#include "crypto/ctype.h"        /* ossl_isdigit() */
#include "prov/implementations.h"
#include "prov/providercommon.h"
#include "prov/bio.h"
#include "file_store_local.h"
#ifdef __CYGWIN__
# include <windows.h>
#endif
#include <wincrypt.h>

enum {
    STATE_IDLE,
    STATE_READ,
    STATE_EOF,
};

struct winstore_ctx_st {
    void                   *provctx;
    char                   *propq;
    unsigned char          *subject;
    size_t                  subject_len;

    HCERTSTORE              win_store;
    const CERT_CONTEXT     *win_ctx;
    int                     state;

    OSSL_DECODER_CTX       *dctx;
};

static void winstore_win_reset(struct winstore_ctx_st *ctx)
{
    if (ctx->win_ctx != NULL) {
        CertFreeCertificateContext(ctx->win_ctx);
        ctx->win_ctx = NULL;
    }

    ctx->state = STATE_IDLE;
}

static void winstore_win_advance(struct winstore_ctx_st *ctx)
{
    CERT_NAME_BLOB name = {0};

    if (ctx->state == STATE_EOF)
        return;

    name.cbData = ctx->subject_len;
    name.pbData = ctx->subject;

    ctx->win_ctx = (name.cbData == 0 ? NULL :
        CertFindCertificateInStore(ctx->win_store,
                                   X509_ASN_ENCODING | PKCS_7_ASN_ENCODING,
                                   0, CERT_FIND_SUBJECT_NAME,
                                   &name, ctx->win_ctx));

    ctx->state = (ctx->win_ctx == NULL) ? STATE_EOF : STATE_READ;
}

static void *winstore_open(void *provctx, const char *uri)
{
    struct winstore_ctx_st *ctx = NULL;

    if (!HAS_CASE_PREFIX(uri, "org.openssl.winstore:"))
        return NULL;

    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx == NULL)
        return NULL;

    ctx->provctx    = provctx;
    ctx->win_store  = CertOpenSystemStoreW(0, L"ROOT");
    if (ctx->win_store == NULL) {
        OPENSSL_free(ctx);
        return NULL;
    }

    winstore_win_reset(ctx);
    return ctx;
}

static void *winstore_attach(void *provctx, OSSL_CORE_BIO *cin)
{
    return NULL; /* not supported */
}

static const OSSL_PARAM *winstore_settable_ctx_params(void *loaderctx, const OSSL_PARAM params[])
{
    static const OSSL_PARAM known_settable_ctx_params[] = {
        OSSL_PARAM_octet_string(OSSL_STORE_PARAM_SUBJECT, NULL, 0),
        OSSL_PARAM_utf8_string(OSSL_STORE_PARAM_PROPERTIES, NULL, 0),
        OSSL_PARAM_END
    };
    return known_settable_ctx_params;
}

static int winstore_set_ctx_params(void *loaderctx, const OSSL_PARAM params[])
{
    struct winstore_ctx_st *ctx = loaderctx;
    const OSSL_PARAM *p;
    int do_reset = 0;

    if (ossl_param_is_empty(params))
        return 1;

    p = OSSL_PARAM_locate_const(params, OSSL_STORE_PARAM_PROPERTIES);
    if (p != NULL) {
        do_reset = 1;
        OPENSSL_free(ctx->propq);
        ctx->propq = NULL;
        if (!OSSL_PARAM_get_utf8_string(p, &ctx->propq, 0))
            return 0;
    }

    p = OSSL_PARAM_locate_const(params, OSSL_STORE_PARAM_SUBJECT);
    if (p != NULL) {
        const unsigned char *der = NULL;
        size_t der_len = 0;

        if (!OSSL_PARAM_get_octet_string_ptr(p, (const void **)&der, &der_len))
            return 0;

        do_reset = 1;

        OPENSSL_free(ctx->subject);

        ctx->subject = OPENSSL_malloc(der_len);
        if (ctx->subject == NULL) {
            ctx->subject_len = 0;
            return 0;
        }

        ctx->subject_len = der_len;
        memcpy(ctx->subject, der, der_len);
    }

    if (do_reset) {
        winstore_win_reset(ctx);
        winstore_win_advance(ctx);
    }

    return 1;
}

struct load_data_st {
    OSSL_CALLBACK  *object_cb;
    void           *object_cbarg;
};

static int load_construct(OSSL_DECODER_INSTANCE *decoder_inst,
                           const OSSL_PARAM *params, void *construct_data)
{
    struct load_data_st *data = construct_data;
    return data->object_cb(params, data->object_cbarg);
}

static void load_cleanup(void *construct_data)
{
    /* No-op. */
}

static int setup_decoder(struct winstore_ctx_st *ctx)
{
    OSSL_LIB_CTX *libctx = ossl_prov_ctx_get0_libctx(ctx->provctx);
    const OSSL_ALGORITHM *to_algo = NULL;
    const char *input_structure = NULL;

    if (ctx->dctx != NULL)
        return 1;

    ctx->dctx = OSSL_DECODER_CTX_new();
    if (ctx->dctx == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_OSSL_DECODER_LIB);
        return 0;
    }

    if (!OSSL_DECODER_CTX_set_input_type(ctx->dctx, "DER")) {
        ERR_raise(ERR_LIB_PROV, ERR_R_OSSL_DECODER_LIB);
        goto err;
    }

    input_structure = "Certificate";
    if (!OSSL_DECODER_CTX_set_input_structure(ctx->dctx, input_structure)) {
        ERR_raise(ERR_LIB_PROV, ERR_R_OSSL_DECODER_LIB);
        goto err;
    }

    for (to_algo = ossl_any_to_obj_algorithm;
         to_algo->algorithm_names != NULL;
         to_algo++) {
        OSSL_DECODER *to_obj = NULL;
        OSSL_DECODER_INSTANCE *to_obj_inst = NULL;
        const char *input_type;

        /*
         * Create the internal last resort decoder implementation
         * together with a "decoder instance".
         * The decoder doesn't need any identification or to be
         * attached to any provider, since it's only used locally.
         */
        to_obj = ossl_decoder_from_algorithm(0, to_algo, NULL);
        if (to_obj != NULL)
            to_obj_inst = ossl_decoder_instance_new_forprov(to_obj, ctx->provctx,
                                                            input_structure);

        OSSL_DECODER_free(to_obj);
        if (to_obj_inst == NULL)
            goto err;

        /*
         * The input type has to be DER
         */
        input_type = OSSL_DECODER_INSTANCE_get_input_type(to_obj_inst);
        if (OPENSSL_strcasecmp(input_type, "DER") != 0) {
            ossl_decoder_instance_free(to_obj_inst);
            continue;
        }

        if (!ossl_decoder_ctx_add_decoder_inst(ctx->dctx,
                                               to_obj_inst)) {
            ossl_decoder_instance_free(to_obj_inst);
            ERR_raise(ERR_LIB_PROV, ERR_R_OSSL_DECODER_LIB);
            goto err;
        }
    }

    if (!OSSL_DECODER_CTX_add_extra(ctx->dctx, libctx, ctx->propq)) {
        ERR_raise(ERR_LIB_PROV, ERR_R_OSSL_DECODER_LIB);
        goto err;
    }

    if (!OSSL_DECODER_CTX_set_construct(ctx->dctx, load_construct)) {
        ERR_raise(ERR_LIB_PROV, ERR_R_OSSL_DECODER_LIB);
        goto err;
    }

    if (!OSSL_DECODER_CTX_set_cleanup(ctx->dctx, load_cleanup)) {
        ERR_raise(ERR_LIB_PROV, ERR_R_OSSL_DECODER_LIB);
        goto err;
    }

    return 1;

err:
    OSSL_DECODER_CTX_free(ctx->dctx);
    ctx->dctx = NULL;
    return 0;
}

static int winstore_load_using(struct winstore_ctx_st *ctx,
                               OSSL_CALLBACK *object_cb, void *object_cbarg,
                               OSSL_PASSPHRASE_CALLBACK *pw_cb, void *pw_cbarg,
                               const void *der, size_t der_len)
{
    struct load_data_st data;
    const unsigned char *der_ = der;
    size_t der_len_ = der_len;

    if (setup_decoder(ctx) == 0)
        return 0;

    data.object_cb      = object_cb;
    data.object_cbarg   = object_cbarg;

    OSSL_DECODER_CTX_set_construct_data(ctx->dctx, &data);
    OSSL_DECODER_CTX_set_passphrase_cb(ctx->dctx, pw_cb, pw_cbarg);

    if (OSSL_DECODER_from_data(ctx->dctx, &der_, &der_len_) == 0)
        return 0;

    return 1;
}

static int winstore_load(void *loaderctx,
                         OSSL_CALLBACK *object_cb, void *object_cbarg,
                         OSSL_PASSPHRASE_CALLBACK *pw_cb, void *pw_cbarg)
{
    int ret = 0;
    struct winstore_ctx_st *ctx = loaderctx;

    if (ctx->state != STATE_READ)
        return 0;

    ret = winstore_load_using(ctx, object_cb, object_cbarg, pw_cb, pw_cbarg,
                              ctx->win_ctx->pbCertEncoded,
                              ctx->win_ctx->cbCertEncoded);

    if (ret == 1)
        winstore_win_advance(ctx);

    return ret;
}

static int winstore_eof(void *loaderctx)
{
    struct winstore_ctx_st *ctx = loaderctx;

    return ctx->state != STATE_READ;
}

static int winstore_close(void *loaderctx)
{
    struct winstore_ctx_st *ctx = loaderctx;

    winstore_win_reset(ctx);
    CertCloseStore(ctx->win_store, 0);
    OSSL_DECODER_CTX_free(ctx->dctx);
    OPENSSL_free(ctx->propq);
    OPENSSL_free(ctx->subject);
    OPENSSL_free(ctx);
    return 1;
}

const OSSL_DISPATCH ossl_winstore_store_functions[] = {
    { OSSL_FUNC_STORE_OPEN, (void (*)(void))winstore_open },
    { OSSL_FUNC_STORE_ATTACH, (void (*)(void))winstore_attach },
    { OSSL_FUNC_STORE_SETTABLE_CTX_PARAMS, (void (*)(void))winstore_settable_ctx_params },
    { OSSL_FUNC_STORE_SET_CTX_PARAMS, (void (*)(void))winstore_set_ctx_params },
    { OSSL_FUNC_STORE_LOAD, (void (*)(void))winstore_load },
    { OSSL_FUNC_STORE_EOF, (void (*)(void))winstore_eof },
    { OSSL_FUNC_STORE_CLOSE, (void (*)(void))winstore_close },
    OSSL_DISPATCH_END,
};
