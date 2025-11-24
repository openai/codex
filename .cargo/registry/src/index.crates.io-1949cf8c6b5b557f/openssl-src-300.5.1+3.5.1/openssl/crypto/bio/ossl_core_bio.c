/*
 * Copyright 2021-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core.h>
#include "bio_local.h"

/*-
 * Core BIO structure
 * This is distinct from a BIO to prevent casting between the two which could
 * lead to versioning problems.
 */
struct ossl_core_bio_st {
    CRYPTO_REF_COUNT ref_cnt;
    BIO *bio;
};

static OSSL_CORE_BIO *core_bio_new(void)
{
    OSSL_CORE_BIO *cb = OPENSSL_malloc(sizeof(*cb));

    if (cb == NULL || !CRYPTO_NEW_REF(&cb->ref_cnt, 1)) {
        OPENSSL_free(cb);
        return NULL;
    }
    return cb;
}

int ossl_core_bio_up_ref(OSSL_CORE_BIO *cb)
{
    int ref = 0;

    return CRYPTO_UP_REF(&cb->ref_cnt, &ref);
}

int ossl_core_bio_free(OSSL_CORE_BIO *cb)
{
    int ref = 0, res = 1;

    if (cb != NULL) {
        CRYPTO_DOWN_REF(&cb->ref_cnt, &ref);
        if (ref <= 0) {
            res = BIO_free(cb->bio);
            CRYPTO_FREE_REF(&cb->ref_cnt);
            OPENSSL_free(cb);
        }
    }
    return res;
}

OSSL_CORE_BIO *ossl_core_bio_new_from_bio(BIO *bio)
{
    OSSL_CORE_BIO *cb = core_bio_new();

    if (cb == NULL || !BIO_up_ref(bio)) {
        ossl_core_bio_free(cb);
        return NULL;
    }
    cb->bio = bio;
    return cb;
}

static OSSL_CORE_BIO *core_bio_new_from_new_bio(BIO *bio)
{
    OSSL_CORE_BIO *cb = NULL;

    if (bio == NULL)
        return NULL;
    if ((cb = core_bio_new()) == NULL) {
        BIO_free(bio);
        return NULL;
    }
    cb->bio = bio;
    return cb;
}

OSSL_CORE_BIO *ossl_core_bio_new_file(const char *filename, const char *mode)
{
    return core_bio_new_from_new_bio(BIO_new_file(filename, mode));
}

OSSL_CORE_BIO *ossl_core_bio_new_mem_buf(const void *buf, int len)
{
    return core_bio_new_from_new_bio(BIO_new_mem_buf(buf, len));
}

int ossl_core_bio_read_ex(OSSL_CORE_BIO *cb, void *data, size_t dlen,
                          size_t *readbytes)
{
    return BIO_read_ex(cb->bio, data, dlen, readbytes);
}

int ossl_core_bio_write_ex(OSSL_CORE_BIO *cb, const void *data, size_t dlen,
                           size_t *written)
{
    return BIO_write_ex(cb->bio, data, dlen, written);
}

int ossl_core_bio_gets(OSSL_CORE_BIO *cb, char *buf, int size)
{
    return BIO_gets(cb->bio, buf, size);
}

int ossl_core_bio_puts(OSSL_CORE_BIO *cb, const char *buf)
{
    return BIO_puts(cb->bio, buf);
}

long ossl_core_bio_ctrl(OSSL_CORE_BIO *cb, int cmd, long larg, void *parg)
{
    return BIO_ctrl(cb->bio, cmd, larg, parg);
}

int ossl_core_bio_vprintf(OSSL_CORE_BIO *cb, const char *format, va_list args)
{
    return BIO_vprintf(cb->bio, format, args);
}
