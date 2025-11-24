/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/self_test.h>
#include <openssl/core_names.h>
#include <openssl/params.h>
#include "internal/cryptlib.h"
#include "crypto/context.h"

typedef struct self_test_cb_st {
    OSSL_CALLBACK *cb;
    void *cbarg;
} SELF_TEST_CB;

struct ossl_self_test_st {
    /* local state variables */
    const char *phase;
    const char *type;
    const char *desc;
    OSSL_CALLBACK *cb;

    /* callback related variables used to pass the state back to the user */
    OSSL_PARAM params[4];
    void *cb_arg;
};

#ifndef FIPS_MODULE
void *ossl_self_test_set_callback_new(OSSL_LIB_CTX *ctx)
{
    SELF_TEST_CB *stcb;

    stcb = OPENSSL_zalloc(sizeof(*stcb));
    return stcb;
}

void ossl_self_test_set_callback_free(void *stcb)
{
    OPENSSL_free(stcb);
}

static SELF_TEST_CB *get_self_test_callback(OSSL_LIB_CTX *libctx)
{
    return ossl_lib_ctx_get_data(libctx, OSSL_LIB_CTX_SELF_TEST_CB_INDEX);
}

void OSSL_SELF_TEST_set_callback(OSSL_LIB_CTX *libctx, OSSL_CALLBACK *cb,
                                 void *cbarg)
{
    SELF_TEST_CB *stcb = get_self_test_callback(libctx);

    if (stcb != NULL) {
        stcb->cb = cb;
        stcb->cbarg = cbarg;
    }
}

void OSSL_SELF_TEST_get_callback(OSSL_LIB_CTX *libctx, OSSL_CALLBACK **cb,
                                 void **cbarg)
{
    SELF_TEST_CB *stcb = get_self_test_callback(libctx);

    if (cb != NULL)
        *cb = (stcb != NULL ? stcb->cb : NULL);
    if (cbarg != NULL)
        *cbarg = (stcb != NULL ? stcb->cbarg : NULL);
}
#endif /* FIPS_MODULE */

static void self_test_setparams(OSSL_SELF_TEST *st)
{
    size_t n = 0;

    if (st->cb != NULL) {
        st->params[n++] =
            OSSL_PARAM_construct_utf8_string(OSSL_PROV_PARAM_SELF_TEST_PHASE,
                                             (char *)st->phase, 0);
        st->params[n++] =
            OSSL_PARAM_construct_utf8_string(OSSL_PROV_PARAM_SELF_TEST_TYPE,
                                             (char *)st->type, 0);
        st->params[n++] =
            OSSL_PARAM_construct_utf8_string(OSSL_PROV_PARAM_SELF_TEST_DESC,
                                             (char *)st->desc, 0);
    }
    st->params[n++] = OSSL_PARAM_construct_end();
}

OSSL_SELF_TEST *OSSL_SELF_TEST_new(OSSL_CALLBACK *cb, void *cbarg)
{
    OSSL_SELF_TEST *ret = OPENSSL_zalloc(sizeof(*ret));

    if (ret == NULL)
        return NULL;

    ret->cb = cb;
    ret->cb_arg = cbarg;
    ret->phase = "";
    ret->type = "";
    ret->desc = "";
    self_test_setparams(ret);
    return ret;
}

void OSSL_SELF_TEST_free(OSSL_SELF_TEST *st)
{
    OPENSSL_free(st);
}

/* Can be used during application testing to log that a test has started. */
void OSSL_SELF_TEST_onbegin(OSSL_SELF_TEST *st, const char *type,
                            const char *desc)
{
    if (st != NULL && st->cb != NULL) {
        st->phase = OSSL_SELF_TEST_PHASE_START;
        st->type = type;
        st->desc = desc;
        self_test_setparams(st);
        (void)st->cb(st->params, st->cb_arg);
    }
}

/*
 * Can be used during application testing to log that a test has either
 * passed or failed.
 */
void OSSL_SELF_TEST_onend(OSSL_SELF_TEST *st, int ret)
{
    if (st != NULL && st->cb != NULL) {
        st->phase =
            (ret == 1 ? OSSL_SELF_TEST_PHASE_PASS : OSSL_SELF_TEST_PHASE_FAIL);
        self_test_setparams(st);
        (void)st->cb(st->params, st->cb_arg);

        st->phase = OSSL_SELF_TEST_PHASE_NONE;
        st->type = OSSL_SELF_TEST_TYPE_NONE;
        st->desc = OSSL_SELF_TEST_DESC_NONE;
    }
}

/*
 * Used for failure testing.
 *
 * Call the applications SELF_TEST_cb() if it exists.
 * If the application callback decides to return 0 then the first byte of 'bytes'
 * is modified (corrupted). This is used to modify output signatures or
 * ciphertext before they are verified or decrypted.
 */
int OSSL_SELF_TEST_oncorrupt_byte(OSSL_SELF_TEST *st, unsigned char *bytes)
{
    if (st != NULL && st->cb != NULL) {
        st->phase = OSSL_SELF_TEST_PHASE_CORRUPT;
        self_test_setparams(st);
        if (!st->cb(st->params, st->cb_arg)) {
            bytes[0] ^= 1;
            return 1;
        }
    }
    return 0;
}
