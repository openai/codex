/*
 * Copyright 2015-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdlib.h>
#include <openssl/crypto.h>
#include "platform.h"            /* for copy_argv() */

char **newargv = NULL;

static void cleanup_argv(void)
{
    OPENSSL_free(newargv);
    newargv = NULL;
}

char **copy_argv(int *argc, char *argv[])
{
    /*-
     * The note below is for historical purpose.  On VMS now we always
     * copy argv "safely."
     *
     * 2011-03-22 SMS.
     * If we have 32-bit pointers everywhere, then we're safe, and
     * we bypass this mess, as on non-VMS systems.
     * Problem 1: Compaq/HP C before V7.3 always used 32-bit
     * pointers for argv[].
     * Fix 1: For a 32-bit argv[], when we're using 64-bit pointers
     * everywhere else, we always allocate and use a 64-bit
     * duplicate of argv[].
     * Problem 2: Compaq/HP C V7.3 (Alpha, IA64) before ECO1 failed
     * to NULL-terminate a 64-bit argv[].  (As this was written, the
     * compiler ECO was available only on IA64.)
     * Fix 2: Unless advised not to (VMS_TRUST_ARGV), we test a
     * 64-bit argv[argc] for NULL, and, if necessary, use a
     * (properly) NULL-terminated (64-bit) duplicate of argv[].
     * The same code is used in either case to duplicate argv[].
     * Some of these decisions could be handled in preprocessing,
     * but the code tends to get even uglier, and the penalty for
     * deciding at compile- or run-time is tiny.
     */

    int i, count = *argc;
    char **p = newargv;

    cleanup_argv();

    /*
     * We purposefully use OPENSSL_malloc() rather than app_malloc() here,
     * to avoid symbol name clashes in test programs that would otherwise
     * get them when linking with all of libapps.a.
     * See comment in test/build.info.
     */
    newargv = OPENSSL_malloc(sizeof(*newargv) * (count + 1));
    if (newargv == NULL)
        return NULL;

    /* Register automatic cleanup on first use */
    if (p == NULL)
        OPENSSL_atexit(cleanup_argv);

    for (i = 0; i < count; i++)
        newargv[i] = argv[i];
    newargv[i] = NULL;
    *argc = i;
    return newargv;
}
