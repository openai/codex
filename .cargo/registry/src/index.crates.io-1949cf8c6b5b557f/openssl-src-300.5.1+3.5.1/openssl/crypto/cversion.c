/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <openssl/bio.h>
#include "internal/e_os.h"
#include "internal/cryptlib.h"
#include "internal/common.h"
#include "internal/thread_once.h"

#include "buildinf.h"

unsigned long OpenSSL_version_num(void)
{
    return OPENSSL_VERSION_NUMBER;
}

unsigned int OPENSSL_version_major(void)
{
    return OPENSSL_VERSION_MAJOR;
}

unsigned int OPENSSL_version_minor(void)
{
    return OPENSSL_VERSION_MINOR;
}

unsigned int OPENSSL_version_patch(void)
{
    return OPENSSL_VERSION_PATCH;
}

const char *OPENSSL_version_pre_release(void)
{
    return OPENSSL_VERSION_PRE_RELEASE;
}

const char *OPENSSL_version_build_metadata(void)
{
    return OPENSSL_VERSION_BUILD_METADATA;
}

extern char ossl_cpu_info_str[];

#if defined(_WIN32) && defined(OSSL_WINCTX)
/* size: MAX_PATH + sizeof("OPENSSLDIR: \"\"") */
static char openssldir[MAX_PATH + 15];

/* size: MAX_PATH + sizeof("ENGINESDIR: \"\"") */
static char enginesdir[MAX_PATH + 15];

/* size: MAX_PATH + sizeof("MODULESDIR: \"\"") */
static char modulesdir[MAX_PATH + 15];

static CRYPTO_ONCE version_strings_once = CRYPTO_ONCE_STATIC_INIT;

DEFINE_RUN_ONCE_STATIC(version_strings_setup)
{
    BIO_snprintf(openssldir, sizeof(openssldir), "OPENSSLDIR: \"%s\"",
                 ossl_get_openssldir());
    BIO_snprintf(enginesdir, sizeof(enginesdir), "ENGINESDIR: \"%s\"",
                 ossl_get_enginesdir());
    BIO_snprintf(modulesdir, sizeof(modulesdir), "MODULESDIR: \"%s\"",
                 ossl_get_modulesdir());
    return 1;
}

# define TOSTR(x) #x
# define OSSL_WINCTX_STRING "OSSL_WINCTX: \"" TOSTR(OSSL_WINCTX) "\""

#endif

const char *OpenSSL_version(int t)
{
#if defined(_WIN32) && defined(OSSL_WINCTX)
    /* Cannot really fail but we would return empty strings anyway */
    (void)RUN_ONCE(&version_strings_once, version_strings_setup);
#endif

    switch (t) {
    case OPENSSL_VERSION:
        return OPENSSL_VERSION_TEXT;
    case OPENSSL_VERSION_STRING:
        return OPENSSL_VERSION_STR;
    case OPENSSL_FULL_VERSION_STRING:
        return OPENSSL_FULL_VERSION_STR;
    case OPENSSL_BUILT_ON:
        return DATE;
    case OPENSSL_CFLAGS:
        return compiler_flags;
    case OPENSSL_PLATFORM:
        return PLATFORM;
#if defined(_WIN32) && defined(OSSL_WINCTX)
    case OPENSSL_DIR:
        return openssldir;
    case OPENSSL_ENGINES_DIR:
        return enginesdir;
    case OPENSSL_MODULES_DIR:
        return modulesdir;
#else
    case OPENSSL_DIR:
# ifdef OPENSSLDIR
        return "OPENSSLDIR: \"" OPENSSLDIR "\"";
# else
        return "OPENSSLDIR: N/A";
# endif
    case OPENSSL_ENGINES_DIR:
# ifdef ENGINESDIR
        return "ENGINESDIR: \"" ENGINESDIR "\"";
# else
        return "ENGINESDIR: N/A";
# endif
    case OPENSSL_MODULES_DIR:
# ifdef MODULESDIR
        return "MODULESDIR: \"" MODULESDIR "\"";
# else
        return "MODULESDIR: N/A";
# endif
#endif
    case OPENSSL_CPU_INFO:
        if (OPENSSL_info(OPENSSL_INFO_CPU_SETTINGS) != NULL)
            return ossl_cpu_info_str;
        else
            return "CPUINFO: N/A";
    case OPENSSL_WINCTX:
#if defined(_WIN32) && defined(OSSL_WINCTX)
        return OSSL_WINCTX_STRING;
#else
        return "OSSL_WINCTX: Undefined";
#endif
    }
    return "not available";
}
