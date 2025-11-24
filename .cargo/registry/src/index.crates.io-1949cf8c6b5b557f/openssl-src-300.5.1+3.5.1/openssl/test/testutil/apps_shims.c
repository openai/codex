/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdlib.h>
#include "apps.h"
#include "../testutil.h"

/* shim that avoids sucking in too much from apps/apps.c */

void *app_malloc(size_t sz, const char *what)
{
    void *vp;

    /*
     * This isn't ideal but it is what the app's app_malloc() does on failure.
     * Instead of exiting with a failure, abort() is called which makes sure
     * that there will be a good stack trace for debugging purposes.
     */
    if (!TEST_ptr(vp = OPENSSL_malloc(sz))) {
        TEST_info("Could not allocate %zu bytes for %s\n", sz, what);
        abort();
    }
    return vp;
}

/* shim to prevent sucking in too much from apps */

int opt_legacy_okay(void)
{
    return 1;
}

/*
 * These three functions are defined here so that they don't need to come from
 * the apps source code and pull in a lot of additional things.
 */
int opt_provider_option_given(void)
{
    return 0;
}

const char *app_get0_propq(void)
{
    return NULL;
}

OSSL_LIB_CTX *app_get0_libctx(void)
{
    return NULL;
}
