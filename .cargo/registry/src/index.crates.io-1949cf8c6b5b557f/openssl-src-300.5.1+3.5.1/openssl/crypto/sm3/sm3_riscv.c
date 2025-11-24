/*
 * Copyright 2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdlib.h>
#include <string.h>

#include <openssl/opensslconf.h>
#include "internal/sm3.h"
#include "crypto/riscv_arch.h"
#include <stdio.h>

void ossl_hwsm3_block_data_order_zvksh(SM3_CTX *c, const void *p, size_t num);
void ossl_sm3_block_data_order(SM3_CTX *c, const void *p, size_t num);
void ossl_hwsm3_block_data_order(SM3_CTX *c, const void *p, size_t num);

void ossl_hwsm3_block_data_order(SM3_CTX *c, const void *p, size_t num)
{
    if (RISCV_HAS_ZVKB_AND_ZVKSH() && riscv_vlen() >= 128) {
        ossl_hwsm3_block_data_order_zvksh(c, p, num);
    } else {
        ossl_sm3_block_data_order(c, p, num);
    }
}
