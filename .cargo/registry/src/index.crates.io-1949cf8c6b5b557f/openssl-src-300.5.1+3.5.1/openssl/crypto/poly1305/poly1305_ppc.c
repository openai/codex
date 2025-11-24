/*
 * Copyright 2009-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/opensslconf.h>
#include <openssl/types.h>
#include "crypto/poly1305.h"
#include "crypto/ppc_arch.h"

void poly1305_init_int(void *ctx, const unsigned char key[16]);
void poly1305_blocks(void *ctx, const unsigned char *inp, size_t len,
                         unsigned int padbit);
void poly1305_emit(void *ctx, unsigned char mac[16],
                       const unsigned int nonce[4]);
void poly1305_init_fpu(void *ctx, const unsigned char key[16]);
void poly1305_blocks_fpu(void *ctx, const unsigned char *inp, size_t len,
                         unsigned int padbit);
void poly1305_emit_fpu(void *ctx, unsigned char mac[16],
                       const unsigned int nonce[4]);
void poly1305_init_vsx(void *ctx, const unsigned char key[16]);
void poly1305_blocks_vsx(void *ctx, const unsigned char *inp, size_t len,
                         unsigned int padbit);
void poly1305_emit_vsx(void *ctx, unsigned char mac[16],
                       const unsigned int nonce[4]);
int poly1305_init(void *ctx, const unsigned char key[16], void *func[2]);
int poly1305_init(void *ctx, const unsigned char key[16], void *func[2])
{
    if (OPENSSL_ppccap_P & PPC_CRYPTO207) {
        poly1305_init_int(ctx, key);
        func[0] = (void*)(uintptr_t)poly1305_blocks_vsx;
        func[1] = (void*)(uintptr_t)poly1305_emit;
    } else if (sizeof(size_t) == 4 && (OPENSSL_ppccap_P & PPC_FPU)) {
        poly1305_init_fpu(ctx, key);
        func[0] = (void*)(uintptr_t)poly1305_blocks_fpu;
        func[1] = (void*)(uintptr_t)poly1305_emit_fpu;
    } else {
        poly1305_init_int(ctx, key);
        func[0] = (void*)(uintptr_t)poly1305_blocks;
        func[1] = (void*)(uintptr_t)poly1305_emit;
    }
    return 1;
}
