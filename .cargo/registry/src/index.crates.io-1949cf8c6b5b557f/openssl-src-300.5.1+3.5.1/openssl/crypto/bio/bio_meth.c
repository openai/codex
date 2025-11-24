/*
 * Copyright 2016-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "bio_local.h"
#include "internal/thread_once.h"

CRYPTO_REF_COUNT bio_type_count;
static CRYPTO_ONCE bio_type_init = CRYPTO_ONCE_STATIC_INIT;

DEFINE_RUN_ONCE_STATIC(do_bio_type_init)
{
    return CRYPTO_NEW_REF(&bio_type_count, BIO_TYPE_START);
}

int BIO_get_new_index(void)
{
    int newval;

    if (!RUN_ONCE(&bio_type_init, do_bio_type_init)) {
        /* Perhaps the error should be raised in do_bio_type_init()? */
        ERR_raise(ERR_LIB_BIO, ERR_R_CRYPTO_LIB);
        return -1;
    }
    if (!CRYPTO_UP_REF(&bio_type_count, &newval))
        return -1;
    if (newval > BIO_TYPE_MASK)
        return -1;
    return newval;
}

BIO_METHOD *BIO_meth_new(int type, const char *name)
{
    BIO_METHOD *biom = OPENSSL_zalloc(sizeof(BIO_METHOD));

    if (biom == NULL
            || (biom->name = OPENSSL_strdup(name)) == NULL) {
        OPENSSL_free(biom);
        return NULL;
    }
    biom->type = type;
    return biom;
}

void BIO_meth_free(BIO_METHOD *biom)
{
    if (biom != NULL) {
        OPENSSL_free(biom->name);
        OPENSSL_free(biom);
    }
}

#ifndef OPENSSL_NO_DEPRECATED_3_5
int (*BIO_meth_get_write(const BIO_METHOD *biom)) (BIO *, const char *, int)
{
    return biom->bwrite_old;
}

int (*BIO_meth_get_write_ex(const BIO_METHOD *biom)) (BIO *, const char *, size_t,
                                                size_t *)
{
    return biom->bwrite;
}
#endif

/* Conversion for old style bwrite to new style */
int bwrite_conv(BIO *bio, const char *data, size_t datal, size_t *written)
{
    int ret;

    if (datal > INT_MAX)
        datal = INT_MAX;

    ret = bio->method->bwrite_old(bio, data, (int)datal);

    if (ret <= 0) {
        *written = 0;
        return ret;
    }

    *written = (size_t)ret;

    return 1;
}

int BIO_meth_set_write(BIO_METHOD *biom,
                       int (*bwrite) (BIO *, const char *, int))
{
    biom->bwrite_old = bwrite;
    biom->bwrite = bwrite_conv;
    return 1;
}

int BIO_meth_set_write_ex(BIO_METHOD *biom,
                       int (*bwrite) (BIO *, const char *, size_t, size_t *))
{
    biom->bwrite_old = NULL;
    biom->bwrite = bwrite;
    return 1;
}

#ifndef OPENSSL_NO_DEPRECATED_3_5
int (*BIO_meth_get_read(const BIO_METHOD *biom)) (BIO *, char *, int)
{
    return biom->bread_old;
}

int (*BIO_meth_get_read_ex(const BIO_METHOD *biom)) (BIO *, char *, size_t, size_t *)
{
    return biom->bread;
}
#endif

/* Conversion for old style bread to new style */
int bread_conv(BIO *bio, char *data, size_t datal, size_t *readbytes)
{
    int ret;

    if (datal > INT_MAX)
        datal = INT_MAX;

    ret = bio->method->bread_old(bio, data, (int)datal);

    if (ret <= 0) {
        *readbytes = 0;
        return ret;
    }

    *readbytes = (size_t)ret;

    return 1;
}

int BIO_meth_set_read(BIO_METHOD *biom,
                      int (*bread) (BIO *, char *, int))
{
    biom->bread_old = bread;
    biom->bread = bread_conv;
    return 1;
}

int BIO_meth_set_read_ex(BIO_METHOD *biom,
                         int (*bread) (BIO *, char *, size_t, size_t *))
{
    biom->bread_old = NULL;
    biom->bread = bread;
    return 1;
}

#ifndef OPENSSL_NO_DEPRECATED_3_5
int (*BIO_meth_get_puts(const BIO_METHOD *biom)) (BIO *, const char *)
{
    return biom->bputs;
}
#endif

int BIO_meth_set_puts(BIO_METHOD *biom,
                      int (*bputs) (BIO *, const char *))
{
    biom->bputs = bputs;
    return 1;
}

#ifndef OPENSSL_NO_DEPRECATED_3_5
int (*BIO_meth_get_gets(const BIO_METHOD *biom)) (BIO *, char *, int)
{
    return biom->bgets;
}
#endif

int BIO_meth_set_gets(BIO_METHOD *biom,
                      int (*bgets) (BIO *, char *, int))
{
    biom->bgets = bgets;
    return 1;
}

#ifndef OPENSSL_NO_DEPRECATED_3_5
long (*BIO_meth_get_ctrl(const BIO_METHOD *biom)) (BIO *, int, long, void *)
{
    return biom->ctrl;
}
#endif

int BIO_meth_set_ctrl(BIO_METHOD *biom,
                      long (*ctrl) (BIO *, int, long, void *))
{
    biom->ctrl = ctrl;
    return 1;
}

#ifndef OPENSSL_NO_DEPRECATED_3_5
int (*BIO_meth_get_create(const BIO_METHOD *biom)) (BIO *)
{
    return biom->create;
}
#endif

int BIO_meth_set_create(BIO_METHOD *biom, int (*create) (BIO *))
{
    biom->create = create;
    return 1;
}

#ifndef OPENSSL_NO_DEPRECATED_3_5
int (*BIO_meth_get_destroy(const BIO_METHOD *biom)) (BIO *)
{
    return biom->destroy;
}
#endif

int BIO_meth_set_destroy(BIO_METHOD *biom, int (*destroy) (BIO *))
{
    biom->destroy = destroy;
    return 1;
}

#ifndef OPENSSL_NO_DEPRECATED_3_5
long (*BIO_meth_get_callback_ctrl(const BIO_METHOD *biom)) (BIO *, int, BIO_info_cb *)
{
    return biom->callback_ctrl;
}
#endif

int BIO_meth_set_callback_ctrl(BIO_METHOD *biom,
                               long (*callback_ctrl) (BIO *, int,
                                                      BIO_info_cb *))
{
    biom->callback_ctrl = callback_ctrl;
    return 1;
}

int BIO_meth_set_sendmmsg(BIO_METHOD *biom,
                          int (*bsendmmsg) (BIO *, BIO_MSG *, size_t, size_t, uint64_t, size_t *))
{
    biom->bsendmmsg = bsendmmsg;
    return 1;
}

#ifndef OPENSSL_NO_DEPRECATED_3_5
int (*BIO_meth_get_sendmmsg(const BIO_METHOD *biom))(BIO *, BIO_MSG *, size_t, size_t, uint64_t, size_t *) {
    return biom->bsendmmsg;
}
#endif

int BIO_meth_set_recvmmsg(BIO_METHOD *biom,
                          int (*brecvmmsg) (BIO *, BIO_MSG *, size_t, size_t, uint64_t, size_t *))
{
    biom->brecvmmsg = brecvmmsg;
    return 1;
}

#ifndef OPENSSL_NO_DEPRECATED_3_5
int (*BIO_meth_get_recvmmsg(const BIO_METHOD *biom))(BIO *, BIO_MSG *, size_t, size_t, uint64_t, size_t *) {
    return biom->brecvmmsg;
}
#endif
