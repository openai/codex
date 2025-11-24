/*
 * Copyright 2013-2025 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2020, Intel Corporation. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 *
 * Originally written by Shay Gueron (1, 2), and Vlad Krasnov (1)
 * (1) Intel Corporation, Israel Development Center, Haifa, Israel
 * (2) University of Haifa, Israel
 */

#ifndef OSSL_CRYPTO_BN_RSAZ_EXP_H
# define OSSL_CRYPTO_BN_RSAZ_EXP_H

# undef RSAZ_ENABLED
# if defined(OPENSSL_BN_ASM_MONT) && \
        (defined(__x86_64) || defined(__x86_64__) || \
         defined(_M_AMD64) || defined(_M_X64))
#  define RSAZ_ENABLED

#  include <openssl/bn.h>
#  include "internal/constant_time.h"
#  include "bn_local.h"

void RSAZ_1024_mod_exp_avx2(BN_ULONG result[16],
                            const BN_ULONG base_norm[16],
                            const BN_ULONG exponent[16],
                            const BN_ULONG m_norm[16], const BN_ULONG RR[16],
                            BN_ULONG k0);
int rsaz_avx2_eligible(void);

void RSAZ_512_mod_exp(BN_ULONG result[8],
                      const BN_ULONG base_norm[8], const BN_ULONG exponent[8],
                      const BN_ULONG m_norm[8], BN_ULONG k0,
                      const BN_ULONG RR[8]);


int ossl_rsaz_avx512ifma_eligible(void);

int ossl_rsaz_avxifma_eligible(void);

int ossl_rsaz_mod_exp_avx512_x2(BN_ULONG *res1,
                                const BN_ULONG *base1,
                                const BN_ULONG *exponent1,
                                const BN_ULONG *m1,
                                const BN_ULONG *RR1,
                                BN_ULONG k0_1,
                                BN_ULONG *res2,
                                const BN_ULONG *base2,
                                const BN_ULONG *exponent2,
                                const BN_ULONG *m2,
                                const BN_ULONG *RR2,
                                BN_ULONG k0_2,
                                int factor_size);

static ossl_inline void bn_select_words(BN_ULONG *r, BN_ULONG mask,
                                        const BN_ULONG *a,
                                        const BN_ULONG *b, size_t num)
{
    size_t i;

    for (i = 0; i < num; i++) {
        r[i] = constant_time_select_64(mask, a[i], b[i]);
    }
}

static ossl_inline BN_ULONG bn_reduce_once_in_place(BN_ULONG *r,
                                                    BN_ULONG carry,
                                                    const BN_ULONG *m,
                                                    BN_ULONG *tmp, size_t num)
{
    carry -= bn_sub_words(tmp, r, m, num);
    bn_select_words(r, carry, r /* tmp < 0 */, tmp /* tmp >= 0 */, num);
    return carry;
}

# endif

#endif
