/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * RSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <string.h>

#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/core_object.h>
#include <openssl/crypto.h>
#include <openssl/err.h>
#include <openssl/params.h>
#include <openssl/pem.h>
#include <openssl/proverr.h>
#include "internal/nelem.h"
#include "internal/sizes.h"
#include "prov/bio.h"
#include "prov/decoders.h"
#include "prov/implementations.h"
#include "endecoder_local.h"

static int read_pem(PROV_CTX *provctx, OSSL_CORE_BIO *cin,
                    char **pem_name, char **pem_header,
                    unsigned char **data, long *len)
{
    BIO *in = ossl_bio_new_from_core_bio(provctx, cin);
    int ok;

    if (in == NULL)
        return 0;
    ok = (PEM_read_bio(in, pem_name, pem_header, data, len) > 0);

    BIO_free(in);
    return ok;
}

static OSSL_FUNC_decoder_newctx_fn pem2der_newctx;
static OSSL_FUNC_decoder_freectx_fn pem2der_freectx;
static OSSL_FUNC_decoder_decode_fn pem2der_decode;

/*
 * Context used for PEM to DER decoding.
 */
struct pem2der_ctx_st {
    PROV_CTX *provctx;
    char data_structure[OSSL_MAX_CODEC_STRUCT_SIZE];
    char propq[OSSL_MAX_PROPQUERY_SIZE];
};

static void *pem2der_newctx(void *provctx)
{
    struct pem2der_ctx_st *ctx = OPENSSL_zalloc(sizeof(*ctx));

    if (ctx != NULL)
        ctx->provctx = provctx;
    return ctx;
}

static void pem2der_freectx(void *vctx)
{
    struct pem2der_ctx_st *ctx = vctx;

    OPENSSL_free(ctx);
}

static const OSSL_PARAM *pem2der_settable_ctx_params(ossl_unused void *provctx)
{
    static const OSSL_PARAM settables[] = {
        OSSL_PARAM_utf8_string(OSSL_DECODER_PARAM_PROPERTIES, NULL, 0),
        OSSL_PARAM_utf8_string(OSSL_OBJECT_PARAM_DATA_STRUCTURE, NULL, 0),
        OSSL_PARAM_END
    };
    return settables;
}

static int pem2der_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    struct pem2der_ctx_st *ctx = vctx;
    const OSSL_PARAM *p;
    char *str;

    p = OSSL_PARAM_locate_const(params, OSSL_DECODER_PARAM_PROPERTIES);
    str = ctx->propq;
    if (p != NULL
        && !OSSL_PARAM_get_utf8_string(p, &str, sizeof(ctx->propq)))
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_OBJECT_PARAM_DATA_STRUCTURE);
    str = ctx->data_structure;
    if (p != NULL
        && !OSSL_PARAM_get_utf8_string(p, &str, sizeof(ctx->data_structure)))
        return 0;

    return 1;
}

/* pem_password_cb compatible function */
struct pem2der_pass_data_st {
    OSSL_PASSPHRASE_CALLBACK *cb;
    void *cbarg;
};

static int pem2der_pass_helper(char *buf, int num, int w, void *data)
{
    struct pem2der_pass_data_st *pass_data = data;
    size_t plen;

    if (pass_data == NULL
        || pass_data->cb == NULL
        || !pass_data->cb(buf, num, &plen, NULL, pass_data->cbarg))
        return -1;
    return (int)plen;
}

static int pem2der_decode(void *vctx, OSSL_CORE_BIO *cin, int selection,
                          OSSL_CALLBACK *data_cb, void *data_cbarg,
                          OSSL_PASSPHRASE_CALLBACK *pw_cb, void *pw_cbarg)
{
    /*
     * PEM names we recognise.  Other PEM names should be recognised by
     * other decoder implementations.
     */
    static struct pem_name_map_st {
        const char *pem_name;
        int object_type;
        const char *data_type;
        const char *data_structure;
    } pem_name_map[] = {
        /* PKCS#8 and SubjectPublicKeyInfo */
        { PEM_STRING_PKCS8, OSSL_OBJECT_PKEY, NULL, "EncryptedPrivateKeyInfo" },
        { PEM_STRING_PKCS8INF, OSSL_OBJECT_PKEY, NULL, "PrivateKeyInfo" },
#define PKCS8_LAST_IDX 1
        { PEM_STRING_PUBLIC, OSSL_OBJECT_PKEY, NULL, "SubjectPublicKeyInfo" },
#define SPKI_LAST_IDX 2
        /* Our set of type specific PEM types */
        { PEM_STRING_DHPARAMS, OSSL_OBJECT_PKEY, "DH", "type-specific" },
        { PEM_STRING_DHXPARAMS, OSSL_OBJECT_PKEY, "X9.42 DH", "type-specific" },
        { PEM_STRING_DSA, OSSL_OBJECT_PKEY, "DSA", "type-specific" },
        { PEM_STRING_DSA_PUBLIC, OSSL_OBJECT_PKEY, "DSA", "type-specific" },
        { PEM_STRING_DSAPARAMS, OSSL_OBJECT_PKEY, "DSA", "type-specific" },
        { PEM_STRING_ECPRIVATEKEY, OSSL_OBJECT_PKEY, "EC", "type-specific" },
        { PEM_STRING_ECPARAMETERS, OSSL_OBJECT_PKEY, "EC", "type-specific" },
        { PEM_STRING_SM2PARAMETERS, OSSL_OBJECT_PKEY, "SM2", "type-specific" },
        { PEM_STRING_RSA, OSSL_OBJECT_PKEY, "RSA", "type-specific" },
        { PEM_STRING_RSA_PUBLIC, OSSL_OBJECT_PKEY, "RSA", "type-specific" },

        /*
         * A few others that there is at least have an object type for, even
         * though there is no provider interface to handle such objects, yet.
         * However, this is beneficial for the OSSL_STORE result handler.
         */
        { PEM_STRING_X509, OSSL_OBJECT_CERT, NULL, "Certificate" },
        { PEM_STRING_X509_TRUSTED, OSSL_OBJECT_CERT, NULL, "Certificate" },
        { PEM_STRING_X509_OLD, OSSL_OBJECT_CERT, NULL, "Certificate" },
        { PEM_STRING_X509_CRL, OSSL_OBJECT_CRL, NULL, "CertificateList" }
    };
    struct pem2der_ctx_st *ctx = vctx;
    char *pem_name = NULL, *pem_header = NULL;
    size_t i;
    unsigned char *der = NULL;
    long der_len = 0;
    int ok = 0;
    int objtype = OSSL_OBJECT_UNKNOWN;

    ok = read_pem(ctx->provctx, cin, &pem_name, &pem_header,
                  &der, &der_len) > 0;
    /* We return "empty handed".  This is not an error. */
    if (!ok)
        return 1;

    /*
     * 10 is the number of characters in "Proc-Type:", which
     * PEM_get_EVP_CIPHER_INFO() requires to be present.
     * If the PEM header has less characters than that, it's
     * not worth spending cycles on it.
     */
    if (strlen(pem_header) > 10) {
        EVP_CIPHER_INFO cipher;
        struct pem2der_pass_data_st pass_data;

        ok = 0;                  /* Assume that we fail */
        pass_data.cb = pw_cb;
        pass_data.cbarg = pw_cbarg;
        if (!PEM_get_EVP_CIPHER_INFO(pem_header, &cipher)
            || !PEM_do_header(&cipher, der, &der_len,
                              pem2der_pass_helper, &pass_data))
            goto end;
    }

    /*
     * Indicated that we successfully decoded something, or not at all.
     * Ending up "empty handed" is not an error.
     */
    ok = 1;

    /* Have a look to see if we recognise anything */
    for (i = 0; i < OSSL_NELEM(pem_name_map); i++)
        if (strcmp(pem_name, pem_name_map[i].pem_name) == 0)
            break;

    if (i < OSSL_NELEM(pem_name_map)) {
        OSSL_PARAM params[5], *p = params;
        /* We expect these to be read only so casting away the const is ok */
        char *data_type = (char *)pem_name_map[i].data_type;
        char *data_structure = (char *)pem_name_map[i].data_structure;

        /*
         * Since this may perform decryption, we need to check the selection to
         * avoid password prompts for objects of no interest.
         */
        if (i <= PKCS8_LAST_IDX
            && ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY)
                || OPENSSL_strcasecmp(ctx->data_structure, "EncryptedPrivateKeyInfo") == 0
                || OPENSSL_strcasecmp(ctx->data_structure, "PrivateKeyInfo") == 0)) {
            ok = ossl_epki2pki_der_decode(der, der_len, selection, data_cb,
                                          data_cbarg, pw_cb, pw_cbarg,
                                          PROV_LIBCTX_OF(ctx->provctx),
                                          ctx->propq);
            goto end;
        }

        if (i <= SPKI_LAST_IDX
            && ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY)
                || OPENSSL_strcasecmp(ctx->data_structure, "SubjectPublicKeyInfo") == 0)) {
            ok = ossl_spki2typespki_der_decode(der, der_len, selection, data_cb,
                                               data_cbarg, pw_cb, pw_cbarg,
                                               PROV_LIBCTX_OF(ctx->provctx),
                                               ctx->propq);
            goto end;
        }

        objtype = pem_name_map[i].object_type;
        if (data_type != NULL)
            *p++ =
                OSSL_PARAM_construct_utf8_string(OSSL_OBJECT_PARAM_DATA_TYPE,
                                                 data_type, 0);

        /* We expect this to be read only so casting away the const is ok */
        if (data_structure != NULL)
            *p++ =
                OSSL_PARAM_construct_utf8_string(OSSL_OBJECT_PARAM_DATA_STRUCTURE,
                                                 data_structure, 0);
        *p++ =
            OSSL_PARAM_construct_octet_string(OSSL_OBJECT_PARAM_DATA,
                                              der, der_len);
        *p++ =
            OSSL_PARAM_construct_int(OSSL_OBJECT_PARAM_TYPE, &objtype);

        *p = OSSL_PARAM_construct_end();

        ok = data_cb(params, data_cbarg);
    }

 end:
    OPENSSL_free(pem_name);
    OPENSSL_free(pem_header);
    OPENSSL_free(der);
    return ok;
}

const OSSL_DISPATCH ossl_pem_to_der_decoder_functions[] = {
    { OSSL_FUNC_DECODER_NEWCTX, (void (*)(void))pem2der_newctx },
    { OSSL_FUNC_DECODER_FREECTX, (void (*)(void))pem2der_freectx },
    { OSSL_FUNC_DECODER_DECODE, (void (*)(void))pem2der_decode },
    { OSSL_FUNC_DECODER_SETTABLE_CTX_PARAMS,
      (void (*)(void))pem2der_settable_ctx_params },
    { OSSL_FUNC_DECODER_SET_CTX_PARAMS,
      (void (*)(void))pem2der_set_ctx_params },
    OSSL_DISPATCH_END
};
