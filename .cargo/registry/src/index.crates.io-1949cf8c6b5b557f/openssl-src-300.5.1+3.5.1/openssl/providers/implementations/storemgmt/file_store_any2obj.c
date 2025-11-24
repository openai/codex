/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * This is a decoder that's completely internal to the 'file:' store
 * implementation.  Only code in file_store.c know about this one.  Because
 * of this close relationship, we can cut certain corners, such as making
 * assumptions about the "provider context", which is currently simply the
 * provider context that the file_store.c code operates within.
 *
 * All this does is to read known binary encodings (currently: DER, MSBLOB,
 * PVK) from the input if it can, and passes it on to the data callback as
 * an object abstraction, leaving it to the callback to figure out what it
 * actually is.
 *
 * This MUST be made the last decoder in a chain, leaving it to other more
 * specialized decoders to recognise and process their stuff first.
 */

#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/core_object.h>
#include <openssl/bio.h>
#include <openssl/buffer.h>
#include <openssl/err.h>
#include <openssl/asn1err.h>
#include <openssl/params.h>
#include "internal/asn1.h"
#include "internal/sizes.h"
#include "crypto/pem.h"          /* For internal PVK and "blob" headers */
#include "prov/bio.h"
#include "file_store_local.h"

/*
 * newctx and freectx are not strictly necessary.  However, the method creator,
 * ossl_decoder_from_algorithm(), demands that they exist, so we make sure to
 * oblige.
 */

static OSSL_FUNC_decoder_newctx_fn any2obj_newctx;
static OSSL_FUNC_decoder_freectx_fn any2obj_freectx;

struct any2obj_ctx_st {
    PROV_CTX *provctx;
    char data_structure[OSSL_MAX_CODEC_STRUCT_SIZE];
};

static void *any2obj_newctx(void *provctx)
{
    struct any2obj_ctx_st *ctx = OPENSSL_zalloc(sizeof(*ctx));

    if (ctx != NULL)
        ctx->provctx = provctx;
    return ctx;
}

static void any2obj_freectx(void *ctx)
{
    OPENSSL_free(ctx);
}

static int any2obj_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    struct any2obj_ctx_st *ctx = vctx;
    const OSSL_PARAM *p;
    char *str;

    p = OSSL_PARAM_locate_const(params, OSSL_OBJECT_PARAM_DATA_STRUCTURE);
    str = ctx->data_structure;
    if (p != NULL
        && !OSSL_PARAM_get_utf8_string(p, &str, sizeof(ctx->data_structure)))
        return 0;

    return 1;
}

static const OSSL_PARAM *any2obj_settable_ctx_params(ossl_unused void *provctx)
{
    static const OSSL_PARAM settables[] = {
        OSSL_PARAM_utf8_string(OSSL_OBJECT_PARAM_DATA_STRUCTURE, NULL, 0),
        OSSL_PARAM_END
    };
    return settables;
}

static int any2obj_decode_final(void *vctx, int objtype, const char *input_type,
                                const char *data_type, BUF_MEM *mem,
                                OSSL_CALLBACK *data_cb, void *data_cbarg)
{
    struct any2obj_ctx_st *ctx = vctx;
    /*
     * 1 indicates that we successfully decoded something, or not at all.
     * Ending up "empty handed" is not an error.
     */
    int ok = 1;

    if (mem != NULL) {
        OSSL_PARAM params[6], *p = params;

        if (data_type != NULL)
            *p++ = OSSL_PARAM_construct_utf8_string(OSSL_OBJECT_PARAM_DATA_TYPE,
                                                    (char *)data_type, 0);
        if (input_type != NULL)
            *p++ = OSSL_PARAM_construct_utf8_string(OSSL_OBJECT_PARAM_INPUT_TYPE,
                                                    (char *)input_type, 0);
        if (*ctx->data_structure != '\0')
            *p++ = OSSL_PARAM_construct_utf8_string(OSSL_OBJECT_PARAM_DATA_STRUCTURE,
                                                    (char *)ctx->data_structure, 0);
        *p++ = OSSL_PARAM_construct_int(OSSL_OBJECT_PARAM_TYPE, &objtype);
        *p++ = OSSL_PARAM_construct_octet_string(OSSL_OBJECT_PARAM_DATA,
                                                 mem->data, mem->length);
        *p = OSSL_PARAM_construct_end();

        ok = data_cb(params, data_cbarg);
        BUF_MEM_free(mem);
    }
    return ok;
}

static OSSL_FUNC_decoder_decode_fn der2obj_decode;
static int der2obj_decode(void *vctx, OSSL_CORE_BIO *cin, int selection,
                          OSSL_CALLBACK *data_cb, void *data_cbarg,
                          OSSL_PASSPHRASE_CALLBACK *pw_cb, void *pw_cbarg)
{
    struct any2obj_ctx_st *ctx = vctx;
    BIO *in = ossl_bio_new_from_core_bio(ctx->provctx, cin);
    BUF_MEM *mem = NULL;
    int ok;

    if (in == NULL)
        return 0;

    ERR_set_mark();
    ok = (asn1_d2i_read_bio(in, &mem) >= 0);
    ERR_pop_to_mark();
    if (!ok && mem != NULL) {
        BUF_MEM_free(mem);
        mem = NULL;
    }
    BIO_free(in);

    /* any2obj_decode_final() frees |mem| for us */
    return any2obj_decode_final(ctx, OSSL_OBJECT_UNKNOWN, NULL, NULL, mem,
                                data_cb, data_cbarg);
}

static OSSL_FUNC_decoder_decode_fn msblob2obj_decode;
static int msblob2obj_decode(void *vctx, OSSL_CORE_BIO *cin, int selection,
                             OSSL_CALLBACK *data_cb, void *data_cbarg,
                             OSSL_PASSPHRASE_CALLBACK *pw_cb, void *pw_cbarg)
{
    struct any2obj_ctx_st *ctx = vctx;
    BIO *in = ossl_bio_new_from_core_bio(ctx->provctx, cin);
    BUF_MEM *mem = NULL;
    size_t mem_len = 0, mem_want;
    const unsigned char *p;
    unsigned int bitlen, magic;
    int isdss = -1;
    int ispub = -1;
    int ok = 0;

    if (in == NULL)
        goto err;

    mem_want = 16;               /* The size of the MSBLOB header */
    if ((mem = BUF_MEM_new()) == NULL
        || !BUF_MEM_grow(mem, mem_want)) {
        ERR_raise(ERR_LIB_PEM, ERR_R_BUF_LIB);
        goto err;
    }

    ERR_set_mark();
    ok = BIO_read(in, &mem->data[0], mem_want) == (int)mem_want;
    mem_len += mem_want;
    ERR_pop_to_mark();
    if (!ok)
        goto next;


    ERR_set_mark();
    p = (unsigned char *)&mem->data[0];
    ok = ossl_do_blob_header(&p, 16, &magic, &bitlen, &isdss, &ispub) > 0;
    ERR_pop_to_mark();
    if (!ok)
        goto next;

    ok = 0;
    mem_want = ossl_blob_length(bitlen, isdss, ispub);
    if (!BUF_MEM_grow(mem, mem_len + mem_want)) {
        ERR_raise(ERR_LIB_PEM, ERR_R_BUF_LIB);
        goto err;
    }

    ERR_set_mark();
    ok = BIO_read(in, &mem->data[mem_len], mem_want) == (int)mem_want;
    mem_len += mem_want;
    ERR_pop_to_mark();

 next:
    /* Free resources we no longer need. */
    BIO_free(in);
    if (!ok && mem != NULL) {
        BUF_MEM_free(mem);
        mem = NULL;
    }

    /* any2obj_decode_final() frees |mem| for us */
    return any2obj_decode_final(ctx, OSSL_OBJECT_PKEY, "msblob",
                                isdss ? "DSA" : "RSA", mem,
                                data_cb, data_cbarg);

 err:
    BIO_free(in);
    BUF_MEM_free(mem);
    return 0;
}

static OSSL_FUNC_decoder_decode_fn pvk2obj_decode;
static int pvk2obj_decode(void *vctx, OSSL_CORE_BIO *cin, int selection,
                             OSSL_CALLBACK *data_cb, void *data_cbarg,
                             OSSL_PASSPHRASE_CALLBACK *pw_cb, void *pw_cbarg)
{
    struct any2obj_ctx_st *ctx = vctx;
    BIO *in = ossl_bio_new_from_core_bio(ctx->provctx, cin);
    BUF_MEM *mem = NULL;
    size_t mem_len = 0, mem_want;
    const unsigned char *p;
    unsigned int saltlen, keylen;
    int ok = 0, isdss = -1;

    if (in == NULL)
        goto err;

    mem_want = 24;               /* The size of the PVK header */
    if ((mem = BUF_MEM_new()) == NULL
        || !BUF_MEM_grow(mem, mem_want)) {
        ERR_raise(ERR_LIB_PEM, ERR_R_BUF_LIB);
        goto err;
    }

    ERR_set_mark();
    ok = BIO_read(in, &mem->data[0], mem_want) == (int)mem_want;
    mem_len += mem_want;
    ERR_pop_to_mark();
    if (!ok)
        goto next;


    ERR_set_mark();
    p = (unsigned char *)&mem->data[0];
    ok = ossl_do_PVK_header(&p, 24, 0, &isdss, &saltlen, &keylen) > 0;
    ERR_pop_to_mark();
    if (!ok)
        goto next;

    ok = 0;
    mem_want = saltlen + keylen;
    if (!BUF_MEM_grow(mem, mem_len + mem_want)) {
        ERR_raise(ERR_LIB_PEM, ERR_R_BUF_LIB);
        goto err;
    }

    ERR_set_mark();
    ok = BIO_read(in, &mem->data[mem_len], mem_want) == (int)mem_want;
    mem_len += mem_want;
    ERR_pop_to_mark();

 next:
    /* Free resources we no longer need. */
    BIO_free(in);
    if (!ok && mem != NULL) {
        BUF_MEM_free(mem);
        mem = NULL;
    }

    /* any2obj_decode_final() frees |mem| for us */
    return any2obj_decode_final(ctx, OSSL_OBJECT_PKEY, "pvk",
                                ok ? (isdss ? "DSA" : "RSA") : NULL, mem,
                                data_cb, data_cbarg);

 err:
    BIO_free(in);
    BUF_MEM_free(mem);
    return 0;
}

#define MAKE_DECODER(fromtype, objtype)                                      \
    static const OSSL_DISPATCH fromtype##_to_obj_decoder_functions[] = {     \
        { OSSL_FUNC_DECODER_NEWCTX, (void (*)(void))any2obj_newctx },        \
        { OSSL_FUNC_DECODER_FREECTX, (void (*)(void))any2obj_freectx },      \
        { OSSL_FUNC_DECODER_DECODE, (void (*)(void))fromtype##2obj_decode }, \
        { OSSL_FUNC_DECODER_SETTABLE_CTX_PARAMS,                             \
          (void (*)(void))any2obj_settable_ctx_params },                     \
        { OSSL_FUNC_DECODER_SET_CTX_PARAMS,                                  \
          (void (*)(void))any2obj_set_ctx_params },                          \
        OSSL_DISPATCH_END                                                    \
    }

MAKE_DECODER(der, OSSL_OBJECT_UNKNOWN);
MAKE_DECODER(msblob, OSSL_OBJECT_PKEY);
MAKE_DECODER(pvk, OSSL_OBJECT_PKEY);

const OSSL_ALGORITHM ossl_any_to_obj_algorithm[] = {
    { "obj", "input=DER", der_to_obj_decoder_functions },
    { "obj", "input=MSBLOB", msblob_to_obj_decoder_functions },
    { "obj", "input=PVK", pvk_to_obj_decoder_functions },
    { NULL, }
};
