/*
 * Copyright 2016-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CRYPTO_STORE_H
# define OSSL_CRYPTO_STORE_H
# pragma once

# include <openssl/bio.h>
# include <openssl/store.h>
# include <openssl/ui.h>

void ossl_store_cleanup_int(void);
int ossl_store_loader_get_number(const OSSL_STORE_LOADER *loader);
int ossl_store_loader_store_cache_flush(OSSL_LIB_CTX *libctx);
int ossl_store_loader_store_remove_all_provided(const OSSL_PROVIDER *prov);

#endif
