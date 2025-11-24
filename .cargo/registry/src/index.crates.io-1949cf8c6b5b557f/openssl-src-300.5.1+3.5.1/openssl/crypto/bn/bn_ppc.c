/*
 * Copyright 2009-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/crypto.h>
#include <openssl/bn.h>
#include "crypto/ppc_arch.h"
#include "bn_local.h"

int bn_mul_mont(BN_ULONG *rp, const BN_ULONG *ap, const BN_ULONG *bp,
                const BN_ULONG *np, const BN_ULONG *n0, int num)
{
    int bn_mul_mont_int(BN_ULONG *rp, const BN_ULONG *ap, const BN_ULONG *bp,
                        const BN_ULONG *np, const BN_ULONG *n0, int num);
    int bn_mul4x_mont_int(BN_ULONG *rp, const BN_ULONG *ap, const BN_ULONG *bp,
                          const BN_ULONG *np, const BN_ULONG *n0, int num);
    int bn_mul_mont_fixed_n6(BN_ULONG *rp, const BN_ULONG *ap,
                             const BN_ULONG *bp, const BN_ULONG *np,
                             const BN_ULONG *n0, int num);
    int bn_mul_mont_300_fixed_n6(BN_ULONG *rp, const BN_ULONG *ap,
                                 const BN_ULONG *bp, const BN_ULONG *np,
                                 const BN_ULONG *n0, int num);

    if (num < 4)
        return 0;

    if ((num & 3) == 0)
        return bn_mul4x_mont_int(rp, ap, bp, np, n0, num);

    /*
     * There used to be [optional] call to bn_mul_mont_fpu64 here,
     * but above subroutine is faster on contemporary processors.
     * Formulation means that there might be old processors where
     * FPU code path would be faster, POWER6 perhaps, but there was
     * no opportunity to figure it out...
     */

#if defined(_ARCH_PPC64) && !defined(__ILP32__)
    /* Minerva side-channel fix danny */
# if defined(USE_FIXED_N6)
    if (num == 6) {
        if (OPENSSL_ppccap_P & PPC_MADD300)
            return bn_mul_mont_300_fixed_n6(rp, ap, bp, np, n0, num);
        else
            return bn_mul_mont_fixed_n6(rp, ap, bp, np, n0, num);
    }
# endif
#endif

    return bn_mul_mont_int(rp, ap, bp, np, n0, num);
}
