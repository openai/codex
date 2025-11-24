/*
 * Copyright 2016-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 * https://www.openssl.org/source/license.html
 * or in the file LICENSE in the source distribution.
 */

/*
 * Given a list of files, run each of them through the fuzzer.  Note that
 * failure will be indicated by some kind of crash. Switching on things like
 * asan improves the test.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <openssl/crypto.h>
#include "fuzzer.h"
#include "internal/o_dir.h"

#if defined(_WIN32) && defined(_MAX_PATH) && !defined(PATH_MAX)
# define PATH_MAX _MAX_PATH
#endif

#ifndef PATH_MAX
# define PATH_MAX 4096
#endif

# if !defined(S_ISREG)
#   define S_ISREG(m) ((m) & S_IFREG)
# endif

static void testfile(const char *pathname)
{
    struct stat st;
    FILE *f;
    unsigned char *buf;
    size_t s;

    if (stat(pathname, &st) < 0 || !S_ISREG(st.st_mode))
        return;
    printf("# %s\n", pathname);
    fflush(stdout);
    f = fopen(pathname, "rb");
    if (f == NULL)
        return;
    buf = malloc(st.st_size);
    if (buf != NULL) {
        s = fread(buf, 1, st.st_size, f);
        OPENSSL_assert(s == (size_t)st.st_size);
        FuzzerTestOneInput(buf, s);
        free(buf);
    }
    fclose(f);
}

int main(int argc, char **argv) {
    int n;

    FuzzerInitialize(&argc, &argv);

    for (n = 1; n < argc; ++n) {
        size_t dirname_len = strlen(argv[n]);
        const char *filename = NULL;
        char *pathname = NULL;
        OPENSSL_DIR_CTX *ctx = NULL;
        int wasdir = 0;

        /*
         * We start with trying to read the given path as a directory.
         */
        while ((filename = OPENSSL_DIR_read(&ctx, argv[n])) != NULL) {
            wasdir = 1;
            if (pathname == NULL) {
                pathname = malloc(PATH_MAX);
                if (pathname == NULL)
                    break;
                strcpy(pathname, argv[n]);
#ifdef __VMS
                if (strchr(":<]", pathname[dirname_len - 1]) == NULL)
#endif
                    pathname[dirname_len++] = '/';
                pathname[dirname_len] = '\0';
            }
            strcpy(pathname + dirname_len, filename);
            testfile(pathname);
        }
        OPENSSL_DIR_end(&ctx);

        /* If it wasn't a directory, treat it as a file instead */
        if (!wasdir)
            testfile(argv[n]);

        free(pathname);
    }

    FuzzerCleanup();

    return 0;
}
