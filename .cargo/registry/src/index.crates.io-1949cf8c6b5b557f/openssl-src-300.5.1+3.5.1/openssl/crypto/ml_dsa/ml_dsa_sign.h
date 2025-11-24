/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

struct ml_dsa_sig_st {
    VECTOR z;
    VECTOR hint;
    uint8_t *c_tilde;
    size_t c_tilde_len;
};
