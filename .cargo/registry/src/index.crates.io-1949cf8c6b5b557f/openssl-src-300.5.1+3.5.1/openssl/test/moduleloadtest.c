/*
 * Copyright 2020-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Extremely simple dynamic loader, must never be linked with anything other
 * than the standard C library.  Its purpose is to try to load a dynamic module
 * and verify the presence of one symbol, if that's given.
 */

#include <stdio.h>
#include <stdlib.h>
#include <openssl/core.h>
#include "simpledynamic.h"

static int test_load(const char *path, const char *symbol)
{
#ifdef SD_INIT
    SD sd = SD_INIT;
    SD_SYM sym;
    int ret;

    if (!sd_load(path, &sd, SD_MODULE))
        return 0;
    ret = symbol == NULL || sd_sym(sd, symbol, &sym);
    if (!sd_close(sd))
        ret = 0;
    return ret;
#else
    fprintf(stderr, "No dynamic loader\n");
    return 0;
#endif
}

int main(int argc, char *argv[])
{
    const char *m, *s;

    if (argc != 2 && argc != 3) {
        fprintf(stderr, "Usage: %s sharedobject [ entrypoint ]\n", argv[0]);
        return 1;
    }

    m = argv[1];
    s = argc == 3 ? argv[2] : NULL;

    return test_load(m, s) ? 0 : 1;
}
