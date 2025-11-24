/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core.h>

typedef struct {
    char *name;
    char *value;
} INFOPAIR;
DEFINE_STACK_OF(INFOPAIR)

typedef struct {
    char *name;
    char *path;
    OSSL_provider_init_fn *init;
    STACK_OF(INFOPAIR) *parameters;
    unsigned int is_fallback:1;
} OSSL_PROVIDER_INFO;

extern const OSSL_PROVIDER_INFO ossl_predefined_providers[];

void ossl_provider_info_clear(OSSL_PROVIDER_INFO *info);
int ossl_provider_info_add_to_store(OSSL_LIB_CTX *libctx,
                                    OSSL_PROVIDER_INFO *entry);
int ossl_provider_info_add_parameter(OSSL_PROVIDER_INFO *provinfo,
                                     const char *name,
                                     const char *value);
