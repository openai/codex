/*
 * Copyright 2020-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_INTERNAL_PASSPHRASE_H
# define OSSL_INTERNAL_PASSPHRASE_H
# pragma once

/*
 * This is a passphrase reader bridge with bells and whistles.
 *
 * On one hand, an API may wish to offer all sorts of passphrase callback
 * possibilities to users, or may have to do so for historical reasons.
 * On the other hand, that same API may have demands from other interfaces,
 * notably from the libcrypto <-> provider interface, which uses
 * OSSL_PASSPHRASE_CALLBACK consistently.
 *
 * The structure and functions below are the fundaments for bridging one
 * passphrase callback form to another.
 *
 * In addition, extra features are included (this may be a growing list):
 *
 * -   password caching.  This is to be used by APIs where it's likely
 *     that the same passphrase may be asked for more than once, but the
 *     user shouldn't get prompted more than once.  For example, this is
 *     useful for OSSL_DECODER, which may have to use a passphrase while
 *     trying to find out what input it has.
 */

/*
 * Structure to hold whatever the calling user may specify.  This structure
 * is intended to be integrated into API specific structures or to be used
 * as a local on-stack variable type.  Therefore, no functions to allocate
 * or freed it on the heap is offered.
 */
struct ossl_passphrase_data_st {
    enum {
        is_expl_passphrase = 1, /* Explicit passphrase given by user */
        is_pem_password,        /* pem_password_cb given by user */
        is_ossl_passphrase,     /* OSSL_PASSPHRASE_CALLBACK given by user */
        is_ui_method            /* UI_METHOD given by user */
    } type;
    union {
        struct {
            char *passphrase_copy;
            size_t passphrase_len;
        } expl_passphrase;

        struct {
            pem_password_cb *password_cb;
            void *password_cbarg;
        } pem_password;

        struct {
            OSSL_PASSPHRASE_CALLBACK *passphrase_cb;
            void *passphrase_cbarg;
        } ossl_passphrase;

        struct {
            const UI_METHOD *ui_method;
            void *ui_method_data;
        } ui_method;
    } _;

    /*-
     * Flags section
     */

    /* Set to indicate that caching should be done */
    unsigned int flag_cache_passphrase:1;

    /*-
     * Misc section: caches and other
     */

    char *cached_passphrase;
    size_t cached_passphrase_len;
};

/* Structure manipulation */

void ossl_pw_clear_passphrase_data(struct ossl_passphrase_data_st *data);
void ossl_pw_clear_passphrase_cache(struct ossl_passphrase_data_st *data);

int ossl_pw_set_passphrase(struct ossl_passphrase_data_st *data,
                           const unsigned char *passphrase,
                           size_t passphrase_len);
int ossl_pw_set_pem_password_cb(struct ossl_passphrase_data_st *data,
                                pem_password_cb *cb, void *cbarg);
int ossl_pw_set_ossl_passphrase_cb(struct ossl_passphrase_data_st *data,
                                   OSSL_PASSPHRASE_CALLBACK *cb, void *cbarg);
int ossl_pw_set_ui_method(struct ossl_passphrase_data_st *data,
                          const UI_METHOD *ui_method, void *ui_data);

int ossl_pw_enable_passphrase_caching(struct ossl_passphrase_data_st *data);
int ossl_pw_disable_passphrase_caching(struct ossl_passphrase_data_st *data);

/* Central function for direct calls */

int ossl_pw_get_passphrase(char *pass, size_t pass_size, size_t *pass_len,
                           const OSSL_PARAM params[], int verify,
                           struct ossl_passphrase_data_st *data);

/* Callback functions */

/*
 * All of these callback expect that the callback argument is a
 * struct ossl_passphrase_data_st
 */

pem_password_cb ossl_pw_pem_password;
pem_password_cb ossl_pw_pvk_password;
/* One callback for encoding (verification prompt) and one for decoding */
OSSL_PASSPHRASE_CALLBACK ossl_pw_passphrase_callback_enc;
OSSL_PASSPHRASE_CALLBACK ossl_pw_passphrase_callback_dec;

#endif
