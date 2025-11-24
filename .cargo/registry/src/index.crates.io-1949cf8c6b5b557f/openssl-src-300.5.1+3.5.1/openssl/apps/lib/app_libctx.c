/*
 * Copyright 1995-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#include "app_libctx.h"
#include "apps.h"

static OSSL_LIB_CTX *app_libctx = NULL;
static const char *app_propq = NULL;

int app_set_propq(const char *arg)
{
    app_propq = arg;
    return 1;
}

const char *app_get0_propq(void)
{
    return app_propq;
}

OSSL_LIB_CTX *app_get0_libctx(void)
{
    return app_libctx;
}

OSSL_LIB_CTX *app_create_libctx(void)
{
    /*
     * Load the NULL provider into the default library context and create a
     * library context which will then be used for any OPT_PROV options.
     */
    if (app_libctx == NULL) {
        if (!app_provider_load(NULL, "null")) {
            opt_printf_stderr("Failed to create null provider\n");
            return NULL;
        }
        app_libctx = OSSL_LIB_CTX_new();
    }
    if (app_libctx == NULL)
        opt_printf_stderr("Failed to create library context\n");
    return app_libctx;
}

