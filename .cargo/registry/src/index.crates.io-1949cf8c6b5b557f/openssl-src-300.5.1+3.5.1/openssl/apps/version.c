/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "apps.h"
#include "progs.h"
#include <openssl/evp.h>
#include <openssl/crypto.h>
#include <openssl/bn.h>

typedef enum OPTION_choice {
    OPT_COMMON,
    OPT_B, OPT_D, OPT_E, OPT_M, OPT_F, OPT_O, OPT_P, OPT_V, OPT_A, OPT_R, OPT_C
#if defined(_WIN32)
    ,OPT_W
#endif
} OPTION_CHOICE;

const OPTIONS version_options[] = {
    OPT_SECTION("General"),
    {"help", OPT_HELP, '-', "Display this summary"},

    OPT_SECTION("Output"),
    {"a", OPT_A, '-', "Show all data"},
    {"b", OPT_B, '-', "Show build date"},
    {"d", OPT_D, '-', "Show configuration directory"},
    {"e", OPT_E, '-', "Show engines directory"},
    {"m", OPT_M, '-', "Show modules directory"},
    {"f", OPT_F, '-', "Show compiler flags used"},
    {"o", OPT_O, '-', "Show some internal datatype options"},
    {"p", OPT_P, '-', "Show target build platform"},
    {"r", OPT_R, '-', "Show random seeding options"},
    {"v", OPT_V, '-', "Show library version"},
    {"c", OPT_C, '-', "Show CPU settings info"},
#if defined(_WIN32)
    {"w", OPT_W, '-', "Show Windows install context"},
#endif
    {NULL}
};

int version_main(int argc, char **argv)
{
    int ret = 1, dirty = 0, seed = 0;
    int cflags = 0, version = 0, date = 0, options = 0, platform = 0, dir = 0;
    int engdir = 0, moddir = 0, cpuinfo = 0;
#if defined(_WIN32)
    int windows = 0;
#endif
    char *prog;
    OPTION_CHOICE o;

    prog = opt_init(argc, argv, version_options);
    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_EOF:
        case OPT_ERR:
opthelp:
            BIO_printf(bio_err, "%s: Use -help for summary.\n", prog);
            goto end;
        case OPT_HELP:
            opt_help(version_options);
            ret = 0;
            goto end;
        case OPT_B:
            dirty = date = 1;
            break;
        case OPT_D:
            dirty = dir = 1;
            break;
        case OPT_E:
            dirty = engdir = 1;
            break;
        case OPT_M:
            dirty = moddir = 1;
            break;
        case OPT_F:
            dirty = cflags = 1;
            break;
        case OPT_O:
            dirty = options = 1;
            break;
        case OPT_P:
            dirty = platform = 1;
            break;
        case OPT_R:
            dirty = seed = 1;
            break;
        case OPT_V:
            dirty = version = 1;
            break;
        case OPT_C:
            dirty = cpuinfo = 1;
            break;
#if defined(_WIN32)
        case OPT_W:
            dirty = windows = 1;
            break;
#endif
        case OPT_A:
            seed = options = cflags = version = date = platform
                = dir = engdir = moddir = cpuinfo
                = 1;
            break;
        }
    }

    /* No extra arguments. */
    if (!opt_check_rest_arg(NULL))
        goto opthelp;

    if (!dirty)
        version = 1;

    if (version)
        printf("%s (Library: %s)\n",
               OPENSSL_VERSION_TEXT, OpenSSL_version(OPENSSL_VERSION));
    if (date)
        printf("%s\n", OpenSSL_version(OPENSSL_BUILT_ON));
    if (platform)
        printf("%s\n", OpenSSL_version(OPENSSL_PLATFORM));
    if (options) {
        printf("options: ");
        printf(" %s", BN_options());
        printf("\n");
    }
    if (cflags)
        printf("%s\n", OpenSSL_version(OPENSSL_CFLAGS));
    if (dir)
        printf("%s\n", OpenSSL_version(OPENSSL_DIR));
    if (engdir)
        printf("%s\n", OpenSSL_version(OPENSSL_ENGINES_DIR));
    if (moddir)
        printf("%s\n", OpenSSL_version(OPENSSL_MODULES_DIR));
    if (seed) {
        const char *src = OPENSSL_info(OPENSSL_INFO_SEED_SOURCE);
        printf("Seeding source: %s\n", src ? src : "N/A");
    }
    if (cpuinfo)
        printf("%s\n", OpenSSL_version(OPENSSL_CPU_INFO));
#if defined(_WIN32)
    if (windows)
        printf("%s\n", OpenSSL_version(OPENSSL_WINCTX));
#endif
    ret = 0;
 end:
    return ret;
}


#if defined(__TANDEM) && defined(OPENSSL_VPROC)
/*
 * Define a VPROC function for the openssl program.
 * This is used by platform version identification tools.
 * Do not inline this procedure or make it static.
 */
# define OPENSSL_VPROC_STRING_(x)    x##_OPENSSL
# define OPENSSL_VPROC_STRING(x)     OPENSSL_VPROC_STRING_(x)
# define OPENSSL_VPROC_FUNC          OPENSSL_VPROC_STRING(OPENSSL_VPROC)
void OPENSSL_VPROC_FUNC(void) {}
#endif
