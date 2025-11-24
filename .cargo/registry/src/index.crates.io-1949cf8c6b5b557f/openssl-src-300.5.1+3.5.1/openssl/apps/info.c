/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/crypto.h>
#include "apps.h"
#include "progs.h"

typedef enum OPTION_choice {
    OPT_COMMON,
    OPT_CONFIGDIR, OPT_ENGINESDIR, OPT_MODULESDIR, OPT_DSOEXT, OPT_DIRNAMESEP,
    OPT_LISTSEP, OPT_SEEDS, OPT_CPUSETTINGS, OPT_WINDOWSCONTEXT
} OPTION_CHOICE;

const OPTIONS info_options[] = {

    OPT_SECTION("General"),
    {"help", OPT_HELP, '-', "Display this summary"},

    OPT_SECTION("Output"),
    {"configdir", OPT_CONFIGDIR, '-', "Default configuration file directory"},
    {"enginesdir", OPT_ENGINESDIR, '-', "Default engine module directory"},
    {"modulesdir", OPT_MODULESDIR, '-',
     "Default module directory (other than engine modules)"},
    {"dsoext", OPT_DSOEXT, '-', "Configured extension for modules"},
    {"dirnamesep", OPT_DIRNAMESEP, '-', "Directory-filename separator"},
    {"listsep", OPT_LISTSEP, '-', "List separator character"},
    {"seeds", OPT_SEEDS, '-', "Seed sources"},
    {"cpusettings", OPT_CPUSETTINGS, '-', "CPU settings info"},
    {"windowscontext", OPT_WINDOWSCONTEXT, '-', "Windows install context"},
    {NULL}
};

int info_main(int argc, char **argv)
{
    int ret = 1, dirty = 0, type = 0;
    char *prog;
    OPTION_CHOICE o;
    const char *typedata;

    prog = opt_init(argc, argv, info_options);
    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        default:
opthelp:
            BIO_printf(bio_err, "%s: Use -help for summary.\n", prog);
            goto end;
        case OPT_HELP:
            opt_help(info_options);
            ret = 0;
            goto end;
        case OPT_CONFIGDIR:
            type = OPENSSL_INFO_CONFIG_DIR;
            dirty++;
            break;
        case OPT_ENGINESDIR:
            type = OPENSSL_INFO_ENGINES_DIR;
            dirty++;
            break;
        case OPT_MODULESDIR:
            type = OPENSSL_INFO_MODULES_DIR;
            dirty++;
            break;
        case OPT_DSOEXT:
            type = OPENSSL_INFO_DSO_EXTENSION;
            dirty++;
            break;
        case OPT_DIRNAMESEP:
            type = OPENSSL_INFO_DIR_FILENAME_SEPARATOR;
            dirty++;
            break;
        case OPT_LISTSEP:
            type = OPENSSL_INFO_LIST_SEPARATOR;
            dirty++;
            break;
        case OPT_SEEDS:
            type = OPENSSL_INFO_SEED_SOURCE;
            dirty++;
            break;
        case OPT_CPUSETTINGS:
            type = OPENSSL_INFO_CPU_SETTINGS;
            dirty++;
            break;
        case OPT_WINDOWSCONTEXT:
            type = OPENSSL_INFO_WINDOWS_CONTEXT;
            dirty++;
            break;
        }
    }
    if (!opt_check_rest_arg(NULL))
        goto opthelp;
    if (dirty > 1) {
        BIO_printf(bio_err, "%s: Only one item allowed\n", prog);
        goto opthelp;
    }
    if (dirty == 0) {
        BIO_printf(bio_err, "%s: No items chosen\n", prog);
        goto opthelp;
    }

    typedata = OPENSSL_info(type);
    BIO_printf(bio_out, "%s\n", typedata == NULL ? "Undefined" : typedata);
    ret = 0;
 end:
    return ret;
}
