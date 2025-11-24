/*
 * Copyright 2009-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdlib.h>
#include <string.h>

#include <openssl/opensslconf.h>
#include "crypto/chacha.h"
#include "crypto/ppc_arch.h"

void ChaCha20_ctr32_int(unsigned char *out, const unsigned char *inp,
                        size_t len, const unsigned int key[8],
                        const unsigned int counter[4]);
void ChaCha20_ctr32_vmx(unsigned char *out, const unsigned char *inp,
                        size_t len, const unsigned int key[8],
                        const unsigned int counter[4]);
void ChaCha20_ctr32_vsx(unsigned char *out, const unsigned char *inp,
                        size_t len, const unsigned int key[8],
                        const unsigned int counter[4]);
void ChaCha20_ctr32_vsx_p10(unsigned char *out, const unsigned char *inp,
                        size_t len, const unsigned int key[8],
                        const unsigned int counter[4]);
void ChaCha20_ctr32(unsigned char *out, const unsigned char *inp,
                    size_t len, const unsigned int key[8],
                    const unsigned int counter[4])
{
#if !defined(OPENSSL_SYS_AIX) && !defined(OPENSSL_SYS_MACOSX)
    OPENSSL_ppccap_P & PPC_BRD31
        ? ChaCha20_ctr32_vsx_p10(out, inp, len, key, counter) :
#endif
          OPENSSL_ppccap_P & PPC_CRYPTO207
            ? ChaCha20_ctr32_vsx(out, inp, len, key, counter)
            : OPENSSL_ppccap_P & PPC_ALTIVEC
                 ? ChaCha20_ctr32_vmx(out, inp, len, key, counter)
                 : ChaCha20_ctr32_int(out, inp, len, key, counter);
}
