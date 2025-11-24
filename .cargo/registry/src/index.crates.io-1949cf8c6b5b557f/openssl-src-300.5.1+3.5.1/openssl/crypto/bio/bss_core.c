/*
 * Copyright 2019-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core_dispatch.h>
#include "bio_local.h"
#include "internal/cryptlib.h"
#include "crypto/context.h"

typedef struct {
    OSSL_FUNC_BIO_read_ex_fn *c_bio_read_ex;
    OSSL_FUNC_BIO_write_ex_fn *c_bio_write_ex;
    OSSL_FUNC_BIO_gets_fn *c_bio_gets;
    OSSL_FUNC_BIO_puts_fn *c_bio_puts;
    OSSL_FUNC_BIO_ctrl_fn *c_bio_ctrl;
    OSSL_FUNC_BIO_up_ref_fn *c_bio_up_ref;
    OSSL_FUNC_BIO_free_fn *c_bio_free;
} BIO_CORE_GLOBALS;

void ossl_bio_core_globals_free(void *vbcg)
{
    OPENSSL_free(vbcg);
}

void *ossl_bio_core_globals_new(OSSL_LIB_CTX *ctx)
{
    return OPENSSL_zalloc(sizeof(BIO_CORE_GLOBALS));
}

static ossl_inline BIO_CORE_GLOBALS *get_globals(OSSL_LIB_CTX *libctx)
{
    return ossl_lib_ctx_get_data(libctx, OSSL_LIB_CTX_BIO_CORE_INDEX);
}

static int bio_core_read_ex(BIO *bio, char *data, size_t data_len,
                            size_t *bytes_read)
{
    BIO_CORE_GLOBALS *bcgbl = get_globals(bio->libctx);

    if (bcgbl == NULL || bcgbl->c_bio_read_ex == NULL)
        return 0;
    return bcgbl->c_bio_read_ex(BIO_get_data(bio), data, data_len, bytes_read);
}

static int bio_core_write_ex(BIO *bio, const char *data, size_t data_len,
                             size_t *written)
{
    BIO_CORE_GLOBALS *bcgbl = get_globals(bio->libctx);

    if (bcgbl == NULL || bcgbl->c_bio_write_ex == NULL)
        return 0;
    return bcgbl->c_bio_write_ex(BIO_get_data(bio), data, data_len, written);
}

static long bio_core_ctrl(BIO *bio, int cmd, long num, void *ptr)
{
    BIO_CORE_GLOBALS *bcgbl = get_globals(bio->libctx);

    if (bcgbl == NULL || bcgbl->c_bio_ctrl == NULL)
        return -1;
    return bcgbl->c_bio_ctrl(BIO_get_data(bio), cmd, num, ptr);
}

static int bio_core_gets(BIO *bio, char *buf, int size)
{
    BIO_CORE_GLOBALS *bcgbl = get_globals(bio->libctx);

    if (bcgbl == NULL || bcgbl->c_bio_gets == NULL)
        return -1;
    return bcgbl->c_bio_gets(BIO_get_data(bio), buf, size);
}

static int bio_core_puts(BIO *bio, const char *str)
{
    BIO_CORE_GLOBALS *bcgbl = get_globals(bio->libctx);

    if (bcgbl == NULL || bcgbl->c_bio_puts == NULL)
        return -1;
    return bcgbl->c_bio_puts(BIO_get_data(bio), str);
}

static int bio_core_new(BIO *bio)
{
    BIO_set_init(bio, 1);

    return 1;
}

static int bio_core_free(BIO *bio)
{
    BIO_CORE_GLOBALS *bcgbl = get_globals(bio->libctx);

    if (bcgbl == NULL)
        return 0;

    BIO_set_init(bio, 0);
    bcgbl->c_bio_free(BIO_get_data(bio));

    return 1;
}

static const BIO_METHOD corebiometh = {
    BIO_TYPE_CORE_TO_PROV,
    "BIO to Core filter",
    bio_core_write_ex,
    NULL,
    bio_core_read_ex,
    NULL,
    bio_core_puts,
    bio_core_gets,
    bio_core_ctrl,
    bio_core_new,
    bio_core_free,
    NULL,
};

const BIO_METHOD *BIO_s_core(void)
{
    return &corebiometh;
}

BIO *BIO_new_from_core_bio(OSSL_LIB_CTX *libctx, OSSL_CORE_BIO *corebio)
{
    BIO *outbio;
    BIO_CORE_GLOBALS *bcgbl = get_globals(libctx);

    /* Check the library context has been initialised with the callbacks */
    if (bcgbl == NULL || (bcgbl->c_bio_write_ex == NULL && bcgbl->c_bio_read_ex == NULL))
        return NULL;

    if ((outbio = BIO_new_ex(libctx, BIO_s_core())) == NULL)
        return NULL;

    if (!bcgbl->c_bio_up_ref(corebio)) {
        BIO_free(outbio);
        return NULL;
    }
    BIO_set_data(outbio, corebio);
    return outbio;
}

int ossl_bio_init_core(OSSL_LIB_CTX *libctx, const OSSL_DISPATCH *fns)
{
    BIO_CORE_GLOBALS *bcgbl = get_globals(libctx);

    if (bcgbl == NULL)
	    return 0;

    for (; fns->function_id != 0; fns++) {
        switch (fns->function_id) {
        case OSSL_FUNC_BIO_READ_EX:
            if (bcgbl->c_bio_read_ex == NULL)
                bcgbl->c_bio_read_ex = OSSL_FUNC_BIO_read_ex(fns);
            break;
        case OSSL_FUNC_BIO_WRITE_EX:
            if (bcgbl->c_bio_write_ex == NULL)
                bcgbl->c_bio_write_ex = OSSL_FUNC_BIO_write_ex(fns);
            break;
        case OSSL_FUNC_BIO_GETS:
            if (bcgbl->c_bio_gets == NULL)
                bcgbl->c_bio_gets = OSSL_FUNC_BIO_gets(fns);
            break;
        case OSSL_FUNC_BIO_PUTS:
            if (bcgbl->c_bio_puts == NULL)
                bcgbl->c_bio_puts = OSSL_FUNC_BIO_puts(fns);
            break;
        case OSSL_FUNC_BIO_CTRL:
            if (bcgbl->c_bio_ctrl == NULL)
                bcgbl->c_bio_ctrl = OSSL_FUNC_BIO_ctrl(fns);
            break;
        case OSSL_FUNC_BIO_UP_REF:
            if (bcgbl->c_bio_up_ref == NULL)
                bcgbl->c_bio_up_ref = OSSL_FUNC_BIO_up_ref(fns);
            break;
        case OSSL_FUNC_BIO_FREE:
            if (bcgbl->c_bio_free == NULL)
                bcgbl->c_bio_free = OSSL_FUNC_BIO_free(fns);
            break;
        }
    }

    return 1;
}
