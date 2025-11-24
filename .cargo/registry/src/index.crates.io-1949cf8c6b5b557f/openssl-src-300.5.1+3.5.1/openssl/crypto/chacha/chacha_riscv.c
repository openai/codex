/*
 * This file is dual-licensed, meaning that you can use it under your
 * choice of either of the following two licenses:
 *
 * Copyright 2023-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 *
 * or
 *
 * Copyright (c) 2023, Jerry Shih <jerry.shih@sifive.com>
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
 * "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
 * LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
 * A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
 * OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
 * SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
 * LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
 * DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
 * THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
 * (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
 * OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */

#include <openssl/opensslconf.h>
#include "crypto/chacha.h"
#include "crypto/riscv_arch.h"

void ChaCha20_ctr32_v_zbb_zvkb(unsigned char *out, const unsigned char *inp,
                               size_t len, const unsigned int key[8],
                               const unsigned int counter[4]);

void ChaCha20_ctr32_v_zbb(unsigned char *out, const unsigned char *inp,
                          size_t len, const unsigned int key[8],
                          const unsigned int counter[4]);

void ChaCha20_ctr32(unsigned char *out, const unsigned char *inp, size_t len,
                    const unsigned int key[8], const unsigned int counter[4])
{
    if (len > CHACHA_BLK_SIZE && RISCV_HAS_ZBB() && riscv_vlen() >= 128) {
        if (RISCV_HAS_ZVKB()) {
            ChaCha20_ctr32_v_zbb_zvkb(out, inp, len, key, counter);
        } else {
            ChaCha20_ctr32_v_zbb(out, inp, len, key, counter);
        }
    } else {
        ChaCha20_ctr32_c(out, inp, len, key, counter);
    }
}
