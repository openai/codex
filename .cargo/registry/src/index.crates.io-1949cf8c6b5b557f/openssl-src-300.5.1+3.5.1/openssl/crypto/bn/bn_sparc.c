/*
 * Copyright 2005-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdlib.h>
#include <openssl/bn.h>
#include "internal/cryptlib.h"
#include "crypto/sparc_arch.h"
#include "bn_local.h"    /* for definition of bn_mul_mont */

int bn_mul_mont(BN_ULONG *rp, const BN_ULONG *ap, const BN_ULONG *bp,
                const BN_ULONG *np, const BN_ULONG *n0, int num)
{
    int bn_mul_mont_vis3(BN_ULONG *rp, const BN_ULONG *ap, const BN_ULONG *bp,
                         const BN_ULONG *np, const BN_ULONG *n0, int num);
    int bn_mul_mont_fpu(BN_ULONG *rp, const BN_ULONG *ap, const BN_ULONG *bp,
                        const BN_ULONG *np, const BN_ULONG *n0, int num);
    int bn_mul_mont_int(BN_ULONG *rp, const BN_ULONG *ap, const BN_ULONG *bp,
                        const BN_ULONG *np, const BN_ULONG *n0, int num);

    if (!(num & 1) && num >= 6) {
        if ((num & 15) == 0 && num <= 64 &&
            (OPENSSL_sparcv9cap_P[1] & (CFR_MONTMUL | CFR_MONTSQR)) ==
            (CFR_MONTMUL | CFR_MONTSQR)) {
            typedef int (*bn_mul_mont_f) (BN_ULONG *rp, const BN_ULONG *ap,
                                          const BN_ULONG *bp,
                                          const BN_ULONG *np,
                                          const BN_ULONG *n0);
            int bn_mul_mont_t4_8(BN_ULONG *rp, const BN_ULONG *ap,
                                 const BN_ULONG *bp, const BN_ULONG *np,
                                 const BN_ULONG *n0);
            int bn_mul_mont_t4_16(BN_ULONG *rp, const BN_ULONG *ap,
                                  const BN_ULONG *bp, const BN_ULONG *np,
                                  const BN_ULONG *n0);
            int bn_mul_mont_t4_24(BN_ULONG *rp, const BN_ULONG *ap,
                                  const BN_ULONG *bp, const BN_ULONG *np,
                                  const BN_ULONG *n0);
            int bn_mul_mont_t4_32(BN_ULONG *rp, const BN_ULONG *ap,
                                  const BN_ULONG *bp, const BN_ULONG *np,
                                  const BN_ULONG *n0);
            static const bn_mul_mont_f funcs[4] = {
                bn_mul_mont_t4_8, bn_mul_mont_t4_16,
                bn_mul_mont_t4_24, bn_mul_mont_t4_32
            };
            bn_mul_mont_f worker = funcs[num / 16 - 1];

            if ((*worker) (rp, ap, bp, np, n0))
                return 1;
            /* retry once and fall back */
            if ((*worker) (rp, ap, bp, np, n0))
                return 1;
            return bn_mul_mont_vis3(rp, ap, bp, np, n0, num);
        }
        if ((OPENSSL_sparcv9cap_P[0] & SPARCV9_VIS3))
            return bn_mul_mont_vis3(rp, ap, bp, np, n0, num);
        else if (num >= 8 &&
                 /*
                  * bn_mul_mont_fpu doesn't use FMADD, we just use the
                  * flag to detect when FPU path is preferable in cases
                  * when current heuristics is unreliable. [it works
                  * out because FMADD-capable processors where FPU
                  * code path is undesirable are also VIS3-capable and
                  * VIS3 code path takes precedence.]
                  */
                 ( (OPENSSL_sparcv9cap_P[0] & SPARCV9_FMADD) ||
                   (OPENSSL_sparcv9cap_P[0] &
                    (SPARCV9_PREFER_FPU | SPARCV9_VIS1)) ==
                   (SPARCV9_PREFER_FPU | SPARCV9_VIS1) ))
            return bn_mul_mont_fpu(rp, ap, bp, np, n0, num);
    }
    return bn_mul_mont_int(rp, ap, bp, np, n0, num);
}
