/*
 * Copyright 2002-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* This file contains deprecated functions as wrappers to the new ones */

/*
 * DH low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <openssl/opensslconf.h>

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/bn.h>
#include <openssl/dh.h>

DH *DH_generate_parameters(int prime_len, int generator,
                           void (*callback) (int, int, void *), void *cb_arg)
{
    BN_GENCB *cb;
    DH *ret = NULL;

    if ((ret = DH_new()) == NULL)
        return NULL;
    cb = BN_GENCB_new();
    if (cb == NULL) {
        DH_free(ret);
        return NULL;
    }

    BN_GENCB_set_old(cb, callback, cb_arg);

    if (DH_generate_parameters_ex(ret, prime_len, generator, cb)) {
        BN_GENCB_free(cb);
        return ret;
    }
    BN_GENCB_free(cb);
    DH_free(ret);
    return NULL;
}
