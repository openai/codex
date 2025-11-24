/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/asn1t.h>
#include <openssl/core_names.h>
#include <openssl/core_object.h>
#include <openssl/params.h>
#include <openssl/x509.h>
#include "internal/sizes.h"
#include "crypto/x509.h"
#include "crypto/ec.h"
#include "prov/bio.h"
#include "prov/decoders.h"
#include "prov/implementations.h"
#include "endecoder_local.h"

static OSSL_FUNC_decoder_newctx_fn spki2typespki_newctx;
static OSSL_FUNC_decoder_freectx_fn spki2typespki_freectx;
static OSSL_FUNC_decoder_decode_fn spki2typespki_decode;
static OSSL_FUNC_decoder_settable_ctx_params_fn spki2typespki_settable_ctx_params;
static OSSL_FUNC_decoder_set_ctx_params_fn spki2typespki_set_ctx_params;

/*
 * Context used for SubjectPublicKeyInfo to Type specific SubjectPublicKeyInfo
 * decoding.
 */
struct spki2typespki_ctx_st {
    PROV_CTX *provctx;
    char propq[OSSL_MAX_PROPQUERY_SIZE];
};

static void *spki2typespki_newctx(void *provctx)
{
    struct spki2typespki_ctx_st *ctx = OPENSSL_zalloc(sizeof(*ctx));

    if (ctx != NULL)
        ctx->provctx = provctx;
    return ctx;
}

static void spki2typespki_freectx(void *vctx)
{
    struct spki2typespki_ctx_st *ctx = vctx;

    OPENSSL_free(ctx);
}

static const OSSL_PARAM *spki2typespki_settable_ctx_params(ossl_unused void *provctx)
{
    static const OSSL_PARAM settables[] = {
        OSSL_PARAM_utf8_string(OSSL_DECODER_PARAM_PROPERTIES, NULL, 0),
        OSSL_PARAM_END
    };
    return settables;
}

static int spki2typespki_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    struct spki2typespki_ctx_st *ctx = vctx;
    const OSSL_PARAM *p;
    char *str = ctx->propq;

    p = OSSL_PARAM_locate_const(params, OSSL_DECODER_PARAM_PROPERTIES);
    if (p != NULL && !OSSL_PARAM_get_utf8_string(p, &str, sizeof(ctx->propq)))
        return 0;

    return 1;
}

static int spki2typespki_decode(void *vctx, OSSL_CORE_BIO *cin, int selection,
                                OSSL_CALLBACK *data_cb, void *data_cbarg,
                                OSSL_PASSPHRASE_CALLBACK *pw_cb, void *pw_cbarg)
{
    struct spki2typespki_ctx_st *ctx = vctx;
    unsigned char *der;
    long len;
    int ok = 0;

    if (!ossl_read_der(ctx->provctx, cin, &der, &len))
        return 1;

    ok = ossl_spki2typespki_der_decode(der, len, selection, data_cb, data_cbarg,
                                       pw_cb, pw_cbarg,
                                       PROV_LIBCTX_OF(ctx->provctx), ctx->propq);
    OPENSSL_free(der);
    return ok;
}

int ossl_spki2typespki_der_decode(unsigned char *der, long len, int selection,
                                  OSSL_CALLBACK *data_cb, void *data_cbarg,
                                  OSSL_PASSPHRASE_CALLBACK *pw_cb, void *pw_cbarg,
                                  OSSL_LIB_CTX *libctx, const char *propq)
{
    const unsigned char *derp = der;
    X509_PUBKEY *xpub = NULL;
    X509_ALGOR *algor = NULL;
    const ASN1_OBJECT *oid = NULL;
    char dataname[OSSL_MAX_NAME_SIZE];
    OSSL_PARAM params[6], *p = params;
    int objtype = OSSL_OBJECT_PKEY;
    int ok = 0;

    xpub = ossl_d2i_X509_PUBKEY_INTERNAL(&derp, len, libctx, propq);

    if (xpub == NULL) {
        /* We return "empty handed".  This is not an error. */
        ok = 1;
        goto end;
    }

    if (!X509_PUBKEY_get0_param(NULL, NULL, NULL, &algor, xpub))
        goto end;
    X509_ALGOR_get0(&oid, NULL, NULL, algor);

#ifndef OPENSSL_NO_EC
    /* SM2 abuses the EC oid, so this could actually be SM2 */
    if (OBJ_obj2nid(oid) == NID_X9_62_id_ecPublicKey
            && ossl_x509_algor_is_sm2(algor))
        strcpy(dataname, "SM2");
    else
#endif
    if (OBJ_obj2txt(dataname, sizeof(dataname), oid, 0) <= 0)
        goto end;

    ossl_X509_PUBKEY_INTERNAL_free(xpub);
    xpub = NULL;

    *p++ =
        OSSL_PARAM_construct_utf8_string(OSSL_OBJECT_PARAM_DATA_TYPE,
                                            dataname, 0);

    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_OBJECT_PARAM_INPUT_TYPE,
                                            "DER", 0);

    *p++ =
        OSSL_PARAM_construct_utf8_string(OSSL_OBJECT_PARAM_DATA_STRUCTURE,
                                            "SubjectPublicKeyInfo",
                                            0);
    *p++ =
        OSSL_PARAM_construct_octet_string(OSSL_OBJECT_PARAM_DATA, der, len);
    *p++ =
        OSSL_PARAM_construct_int(OSSL_OBJECT_PARAM_TYPE, &objtype);

    *p = OSSL_PARAM_construct_end();

    ok = data_cb(params, data_cbarg);

 end:
    ossl_X509_PUBKEY_INTERNAL_free(xpub);
    return ok;
}

const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_der_decoder_functions[] = {
    { OSSL_FUNC_DECODER_NEWCTX, (void (*)(void))spki2typespki_newctx },
    { OSSL_FUNC_DECODER_FREECTX, (void (*)(void))spki2typespki_freectx },
    { OSSL_FUNC_DECODER_DECODE, (void (*)(void))spki2typespki_decode },
    { OSSL_FUNC_DECODER_SETTABLE_CTX_PARAMS,
      (void (*)(void))spki2typespki_settable_ctx_params },
    { OSSL_FUNC_DECODER_SET_CTX_PARAMS,
      (void (*)(void))spki2typespki_set_ctx_params },
    OSSL_DISPATCH_END
};
