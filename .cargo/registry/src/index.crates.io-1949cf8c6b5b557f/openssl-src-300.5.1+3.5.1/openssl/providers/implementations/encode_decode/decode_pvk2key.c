/*
 * Copyright 2020-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <string.h>

#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/core_object.h>
#include <openssl/crypto.h>
#include <openssl/params.h>
#include <openssl/err.h>
#include <openssl/pem.h>         /* For public PVK functions */
#include <openssl/x509.h>
#include "internal/passphrase.h"
#include "internal/sizes.h"
#include "crypto/pem.h"          /* For internal PVK and "blob" headers */
#include "crypto/rsa.h"
#include "prov/bio.h"
#include "prov/implementations.h"
#include "endecoder_local.h"

struct pvk2key_ctx_st;            /* Forward declaration */
typedef int check_key_fn(void *, struct pvk2key_ctx_st *ctx);
typedef void adjust_key_fn(void *, struct pvk2key_ctx_st *ctx);
typedef void *b2i_PVK_of_bio_pw_fn(BIO *in, pem_password_cb *cb, void *u,
                                   OSSL_LIB_CTX *libctx, const char *propq);
typedef void free_key_fn(void *);
struct keytype_desc_st {
    int type;                 /* EVP key type */
    const char *name;         /* Keytype */
    const OSSL_DISPATCH *fns; /* Keymgmt (to pilfer functions from) */

    b2i_PVK_of_bio_pw_fn *read_private_key;
    adjust_key_fn *adjust_key;
    free_key_fn *free_key;
};

static OSSL_FUNC_decoder_freectx_fn pvk2key_freectx;
static OSSL_FUNC_decoder_decode_fn pvk2key_decode;
static OSSL_FUNC_decoder_export_object_fn pvk2key_export_object;
static OSSL_FUNC_decoder_settable_ctx_params_fn pvk2key_settable_ctx_params;
static OSSL_FUNC_decoder_set_ctx_params_fn pvk2key_set_ctx_params;

/*
 * Context used for DER to key decoding.
 */
struct pvk2key_ctx_st {
    PROV_CTX *provctx;
    char propq[OSSL_MAX_PROPQUERY_SIZE];
    const struct keytype_desc_st *desc;
    /* The selection that is passed to der2key_decode() */
    int selection;
};

static struct pvk2key_ctx_st *
pvk2key_newctx(void *provctx, const struct keytype_desc_st *desc)
{
    struct pvk2key_ctx_st *ctx = OPENSSL_zalloc(sizeof(*ctx));

    if (ctx != NULL) {
        ctx->provctx = provctx;
        ctx->desc = desc;
    }
    return ctx;
}

static void pvk2key_freectx(void *vctx)
{
    struct pvk2key_ctx_st *ctx = vctx;

    OPENSSL_free(ctx);
}

static const OSSL_PARAM *pvk2key_settable_ctx_params(ossl_unused void *provctx)
{
    static const OSSL_PARAM settables[] = {
        OSSL_PARAM_utf8_string(OSSL_DECODER_PARAM_PROPERTIES, NULL, 0),
        OSSL_PARAM_END,
    };
    return settables;
}

static int pvk2key_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    struct pvk2key_ctx_st *ctx = vctx;
    const OSSL_PARAM *p;
    char *str = ctx->propq;

    p = OSSL_PARAM_locate_const(params, OSSL_DECODER_PARAM_PROPERTIES);
    if (p != NULL && !OSSL_PARAM_get_utf8_string(p, &str, sizeof(ctx->propq)))
        return 0;

    return 1;
}

static int pvk2key_does_selection(void *provctx, int selection)
{
    if (selection == 0)
        return 1;

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY)  != 0)
        return 1;

    return 0;
}

static int pvk2key_decode(void *vctx, OSSL_CORE_BIO *cin, int selection,
                         OSSL_CALLBACK *data_cb, void *data_cbarg,
                         OSSL_PASSPHRASE_CALLBACK *pw_cb, void *pw_cbarg)
{
    struct pvk2key_ctx_st *ctx = vctx;
    BIO *in = ossl_bio_new_from_core_bio(ctx->provctx, cin);
    void *key = NULL;
    int ok = 0;

    if (in == NULL)
        return 0;

    ctx->selection = selection;

    if ((selection == 0
         || (selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0)
        && ctx->desc->read_private_key != NULL) {
        struct ossl_passphrase_data_st pwdata;
        int err, lib, reason;

        memset(&pwdata, 0, sizeof(pwdata));
        if (!ossl_pw_set_ossl_passphrase_cb(&pwdata, pw_cb, pw_cbarg))
            goto end;

        key = ctx->desc->read_private_key(in, ossl_pw_pvk_password, &pwdata,
                                          PROV_LIBCTX_OF(ctx->provctx),
                                          ctx->propq);

        /*
         * Because the PVK API doesn't have a separate decrypt call, we need
         * to check the error queue for certain well known errors that are
         * considered fatal and which we pass through, while the rest gets
         * thrown away.
         */
        err = ERR_peek_last_error();
        lib = ERR_GET_LIB(err);
        reason = ERR_GET_REASON(err);
        if (lib == ERR_LIB_PEM
            && (reason == PEM_R_BAD_PASSWORD_READ
                || reason == PEM_R_BAD_DECRYPT)) {
            ERR_clear_last_mark();
            goto end;
        }

        if (selection != 0 && key == NULL)
            goto next;
    }

    if (key != NULL && ctx->desc->adjust_key != NULL)
        ctx->desc->adjust_key(key, ctx);

 next:
    /*
     * Indicated that we successfully decoded something, or not at all.
     * Ending up "empty handed" is not an error.
     */
    ok = 1;

    /*
     * We free resources here so it's not held up during the callback, because
     * we know the process is recursive and the allocated chunks of memory
     * add up.
     */
    BIO_free(in);
    in = NULL;

    if (key != NULL) {
        OSSL_PARAM params[4];
        int object_type = OSSL_OBJECT_PKEY;

        params[0] =
            OSSL_PARAM_construct_int(OSSL_OBJECT_PARAM_TYPE, &object_type);
        params[1] =
            OSSL_PARAM_construct_utf8_string(OSSL_OBJECT_PARAM_DATA_TYPE,
                                             (char *)ctx->desc->name, 0);
        /* The address of the key becomes the octet string */
        params[2] =
            OSSL_PARAM_construct_octet_string(OSSL_OBJECT_PARAM_REFERENCE,
                                              &key, sizeof(key));
        params[3] = OSSL_PARAM_construct_end();

        ok = data_cb(params, data_cbarg);
    }

 end:
    BIO_free(in);
    ctx->desc->free_key(key);

    return ok;
}

static int pvk2key_export_object(void *vctx,
                                const void *reference, size_t reference_sz,
                                OSSL_CALLBACK *export_cb, void *export_cbarg)
{
    struct pvk2key_ctx_st *ctx = vctx;
    OSSL_FUNC_keymgmt_export_fn *export =
        ossl_prov_get_keymgmt_export(ctx->desc->fns);
    void *keydata;

    if (reference_sz == sizeof(keydata) && export != NULL) {
        int selection = ctx->selection;

        if (selection == 0)
            selection = OSSL_KEYMGMT_SELECT_ALL;
        /* The contents of the reference is the address to our object */
        keydata = *(void **)reference;

        return export(keydata, selection, export_cb, export_cbarg);
    }
    return 0;
}

/* ---------------------------------------------------------------------- */

#define dsa_private_key_bio     (b2i_PVK_of_bio_pw_fn *)b2i_DSA_PVK_bio_ex
#define dsa_adjust              NULL
#define dsa_free                (void (*)(void *))DSA_free

/* ---------------------------------------------------------------------- */

#define rsa_private_key_bio     (b2i_PVK_of_bio_pw_fn *)b2i_RSA_PVK_bio_ex

static void rsa_adjust(void *key, struct pvk2key_ctx_st *ctx)
{
    ossl_rsa_set0_libctx(key, PROV_LIBCTX_OF(ctx->provctx));
}

#define rsa_free                (void (*)(void *))RSA_free

/* ---------------------------------------------------------------------- */

#define IMPLEMENT_MS(KEYTYPE, keytype)                                  \
    static const struct keytype_desc_st                                 \
    pvk2##keytype##_desc = {                                            \
        EVP_PKEY_##KEYTYPE, #KEYTYPE,                                   \
        ossl_##keytype##_keymgmt_functions,                             \
        keytype##_private_key_bio,                                      \
        keytype##_adjust,                                               \
        keytype##_free                                                  \
    };                                                                  \
    static OSSL_FUNC_decoder_newctx_fn pvk2##keytype##_newctx;          \
    static void *pvk2##keytype##_newctx(void *provctx)                  \
    {                                                                   \
        return pvk2key_newctx(provctx, &pvk2##keytype##_desc);          \
    }                                                                   \
    const OSSL_DISPATCH                                                 \
    ossl_##pvk_to_##keytype##_decoder_functions[] = {                   \
        { OSSL_FUNC_DECODER_NEWCTX,                                     \
          (void (*)(void))pvk2##keytype##_newctx },                     \
        { OSSL_FUNC_DECODER_FREECTX,                                    \
          (void (*)(void))pvk2key_freectx },                            \
        { OSSL_FUNC_DECODER_DOES_SELECTION,                             \
          (void (*)(void))pvk2key_does_selection },                     \
        { OSSL_FUNC_DECODER_DECODE,                                     \
          (void (*)(void))pvk2key_decode },                             \
        { OSSL_FUNC_DECODER_EXPORT_OBJECT,                              \
          (void (*)(void))pvk2key_export_object },                      \
        { OSSL_FUNC_DECODER_SETTABLE_CTX_PARAMS,                        \
          (void (*)(void))pvk2key_settable_ctx_params },                \
        { OSSL_FUNC_DECODER_SET_CTX_PARAMS,                             \
          (void (*)(void))pvk2key_set_ctx_params },                     \
        OSSL_DISPATCH_END                                               \
    }

#ifndef OPENSSL_NO_DSA
IMPLEMENT_MS(DSA, dsa);
#endif
IMPLEMENT_MS(RSA, rsa);
