/*
 * Copyright 2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/crypto.h>
#include <openssl/comp.h>
#include <openssl/obj_mac.h>

#include "internal/cryptlib.h"
#include "internal/comp.h"

#define SSL_COMP_NULL_IDX       0
#define SSL_COMP_ZLIB_IDX       1
#define SSL_COMP_NUM_IDX        2

#ifndef OPENSSL_NO_COMP
static int sk_comp_cmp(const SSL_COMP *const *a, const SSL_COMP *const *b)
{
    return ((*a)->id - (*b)->id);
}
#endif

STACK_OF(SSL_COMP) *ossl_load_builtin_compressions(void)
{
    STACK_OF(SSL_COMP) *comp_methods = NULL;
#ifndef OPENSSL_NO_COMP
    SSL_COMP *comp = NULL;
    COMP_METHOD *method = COMP_zlib();

    comp_methods = sk_SSL_COMP_new(sk_comp_cmp);

    if (COMP_get_type(method) != NID_undef && comp_methods != NULL) {
        comp = OPENSSL_malloc(sizeof(*comp));
        if (comp != NULL) {
            comp->method = method;
            comp->id = SSL_COMP_ZLIB_IDX;
            comp->name = COMP_get_name(method);
            if (!sk_SSL_COMP_push(comp_methods, comp))
                OPENSSL_free(comp);
        }
    }
#endif
    return comp_methods;
}

static void cmeth_free(SSL_COMP *cm)
{
    OPENSSL_free(cm);
}

void ossl_free_compression_methods_int(STACK_OF(SSL_COMP) *methods)
{
    sk_SSL_COMP_pop_free(methods, cmeth_free);
}
