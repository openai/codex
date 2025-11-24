/*
 * Copyright 2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core.h>

int ossl_epki2pki_der_decode(unsigned char *der, long der_len, int selection,
                             OSSL_CALLBACK *data_cb, void *data_cbarg,
                             OSSL_PASSPHRASE_CALLBACK *pw_cb, void *pw_cbarg,
                             OSSL_LIB_CTX *libctx, const char *propq);

int ossl_spki2typespki_der_decode(unsigned char *der, long len, int selection,
                                  OSSL_CALLBACK *data_cb, void *data_cbarg,
                                  OSSL_PASSPHRASE_CALLBACK *pw_cb, void *pw_cbarg,
                                  OSSL_LIB_CTX *libctx, const char *propq);
