/*
 * Copyright 1995-2017 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <openssl/bio.h>
#include "bn_local.h"

static const char Hex[] = "0123456789ABCDEF";

#ifndef OPENSSL_NO_STDIO
int BN_print_fp(FILE *fp, const BIGNUM *a)
{
    BIO *b;
    int ret;

    if ((b = BIO_new(BIO_s_file())) == NULL)
        return 0;
    BIO_set_fp(b, fp, BIO_NOCLOSE);
    ret = BN_print(b, a);
    BIO_free(b);
    return ret;
}
#endif

int BN_print(BIO *bp, const BIGNUM *a)
{
    int i, j, v, z = 0;
    int ret = 0;

    if ((a->neg) && BIO_write(bp, "-", 1) != 1)
        goto end;
    if (BN_is_zero(a) && BIO_write(bp, "0", 1) != 1)
        goto end;
    for (i = a->top - 1; i >= 0; i--) {
        for (j = BN_BITS2 - 4; j >= 0; j -= 4) {
            /* strip leading zeros */
            v = (int)((a->d[i] >> j) & 0x0f);
            if (z || v != 0) {
                if (BIO_write(bp, &Hex[v], 1) != 1)
                    goto end;
                z = 1;
            }
        }
    }
    ret = 1;
 end:
    return ret;
}

char *BN_options(void)
{
    static int init = 0;
    static char data[16];

    if (!init) {
        init++;
#ifdef BN_LLONG
        BIO_snprintf(data, sizeof(data), "bn(%zu,%zu)",
                     sizeof(BN_ULLONG) * 8, sizeof(BN_ULONG) * 8);
#else
        BIO_snprintf(data, sizeof(data), "bn(%zu,%zu)",
                     sizeof(BN_ULONG) * 8, sizeof(BN_ULONG) * 8);
#endif
    }
    return data;
}
