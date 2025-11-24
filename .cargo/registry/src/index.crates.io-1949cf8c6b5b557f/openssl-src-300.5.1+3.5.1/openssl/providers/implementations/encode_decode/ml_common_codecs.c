/*
 * Copyright 2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/crypto.h>
#include <openssl/err.h>
#include <openssl/proverr.h>
#include "ml_common_codecs.h"

static int pref_cmp(const void *va, const void *vb)
{
    const ML_COMMON_PKCS8_FMT_PREF *a = va;
    const ML_COMMON_PKCS8_FMT_PREF *b = vb;

    /*
     * Zeros sort last, otherwise the sort is in increasing order.
     *
     * The preferences are small enough to ensure the comparison is transitive
     * as required by qsort(3).  When overflow or underflow is possible, the
     * correct transitive comparison would be: (b < a) - (a < b).
     */
    if (a->pref > 0 && b->pref > 0)
        return a->pref - b->pref;
    /* A preference of 0 is "larger" than (sorts after) any nonzero value. */
    return b->pref - a->pref;
}

ML_COMMON_PKCS8_FMT_PREF *
ossl_ml_common_pkcs8_fmt_order(const char *algorithm_name,
                               const ML_COMMON_PKCS8_FMT *p8fmt,
                               const char *direction, const char *formats)
{
    ML_COMMON_PKCS8_FMT_PREF *ret;
    int i, count = 0;
    const char *fmt = formats, *end;
    const char *sep = "\t ,";

    /* Reserve an extra terminal slot with fmt == NULL */
    if ((ret = OPENSSL_zalloc((NUM_PKCS8_FORMATS + 1) * sizeof(*ret))) == NULL)
        return NULL;

    /* Entries that match a format will get a non-zero preference. */
    for (i = 0; i < NUM_PKCS8_FORMATS; ++i) {
        ret[i].fmt = &p8fmt[i];
        ret[i].pref = 0;
    }

    /* Default to compile-time table order when none specified. */
    if (formats == NULL)
        return ret;

    /*
     * Formats are case-insensitive, separated by spaces, tabs or commas.
     * Duplicate formats are allowed, the first occurence determines the order.
     */
    do {
        if (*(fmt += strspn(fmt, sep)) == '\0')
            break;
        end = fmt + strcspn(fmt, sep);
        for (i = 0; i < NUM_PKCS8_FORMATS; ++i) {
            /* Skip slots already selected or with a different name. */
            if (ret[i].pref > 0
                || OPENSSL_strncasecmp(ret[i].fmt->p8_name,
                                       fmt, (end - fmt)) != 0)
                continue;
            /* First time match */
            ret[i].pref = ++count;
            break;
        }
        fmt = end;
    } while (count < NUM_PKCS8_FORMATS);

    /* No formats matched, raise an error */
    if (count == 0) {
        OPENSSL_free(ret);
        ERR_raise_data(ERR_LIB_PROV, PROV_R_ML_DSA_NO_FORMAT,
                       "no %s private key %s formats are enabled",
                       algorithm_name, direction);
        return NULL;
    }
    /* Sort by preference, with 0's last */
    qsort(ret, NUM_PKCS8_FORMATS, sizeof(*ret), pref_cmp);
    /* Terminate the list at first unselected entry, perhaps reserved slot. */
    ret[count].fmt = NULL;
    return ret;
}
