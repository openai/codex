/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/cryptlib.h"

typedef struct ossl_namemap_st OSSL_NAMEMAP;

OSSL_NAMEMAP *ossl_namemap_stored(OSSL_LIB_CTX *libctx);

OSSL_NAMEMAP *ossl_namemap_new(OSSL_LIB_CTX *libctx);
void ossl_namemap_free(OSSL_NAMEMAP *namemap);
int ossl_namemap_empty(OSSL_NAMEMAP *namemap);

int ossl_namemap_add_name(OSSL_NAMEMAP *namemap, int number, const char *name);

/*
 * The number<->name relationship is 1<->many
 * Therefore, the name->number mapping is a simple function, while the
 * number->name mapping is an iterator.
 */
int ossl_namemap_name2num(const OSSL_NAMEMAP *namemap, const char *name);
int ossl_namemap_name2num_n(const OSSL_NAMEMAP *namemap,
                            const char *name, size_t name_len);
const char *ossl_namemap_num2name(const OSSL_NAMEMAP *namemap, int number,
                                  size_t idx);
int ossl_namemap_doall_names(const OSSL_NAMEMAP *namemap, int number,
                             void (*fn)(const char *name, void *data),
                             void *data);

/*
 * A utility that handles several names in a string, divided by a given
 * separator.
 */
int ossl_namemap_add_names(OSSL_NAMEMAP *namemap, int number,
                           const char *names, const char separator);
