/*
 * Copyright 2002-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <openssl/crypto.h>
#include "internal/cryptlib.h"
#include "internal/conf.h"
#include "conf_local.h"
#include <openssl/x509.h>
#include <openssl/asn1.h>
#include <openssl/engine.h>

#if defined(_WIN32) && !defined(__BORLANDC__)
# define strdup _strdup
#endif

/*
 * This is the automatic configuration loader: it is called automatically by
 * OpenSSL when any of a number of standard initialisation functions are
 * called, unless this is overridden by calling OPENSSL_no_config()
 */

static int openssl_configured = 0;

#ifndef OPENSSL_NO_DEPRECATED_1_1_0
void OPENSSL_config(const char *appname)
{
    OPENSSL_INIT_SETTINGS settings;

    memset(&settings, 0, sizeof(settings));
    if (appname != NULL)
        settings.appname = strdup(appname);
    settings.flags = DEFAULT_CONF_MFLAGS;
    OPENSSL_init_crypto(OPENSSL_INIT_LOAD_CONFIG, &settings);

    free(settings.appname);
}
#endif

int ossl_config_int(const OPENSSL_INIT_SETTINGS *settings)
{
    int ret = 0;
#if defined(OPENSSL_INIT_DEBUG) || !defined(OPENSSL_SYS_UEFI)
    const char *filename;
    const char *appname;
    unsigned long flags;
#endif

    if (openssl_configured)
        return 1;

#if defined(OPENSSL_INIT_DEBUG) || !defined(OPENSSL_SYS_UEFI)
    filename = settings ? settings->filename : NULL;
    appname = settings ? settings->appname : NULL;
    flags = settings ? settings->flags : DEFAULT_CONF_MFLAGS;
#endif

#ifdef OPENSSL_INIT_DEBUG
    fprintf(stderr, "OPENSSL_INIT: ossl_config_int(%s, %s, %lu)\n",
            filename, appname, flags);
#endif

#ifndef OPENSSL_SYS_UEFI
    ret = CONF_modules_load_file_ex(OSSL_LIB_CTX_get0_global_default(),
                                    filename, appname, flags);
#else
    ret = 1;
#endif
    openssl_configured = 1;
    return ret;
}

void ossl_no_config_int(void)
{
    openssl_configured = 1;
}
