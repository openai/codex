/*
 * Copyright 2014-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CRYPTO_PPC_ARCH_H
# define OSSL_CRYPTO_PPC_ARCH_H

extern unsigned int OPENSSL_ppccap_P;

/*
 * Flags' usage can appear ambiguous, because they are set rather
 * to reflect OpenSSL performance preferences than actual processor
 * capabilities.
 */
# define PPC_FPU64       (1<<0)
# define PPC_ALTIVEC     (1<<1)
# define PPC_CRYPTO207   (1<<2)
# define PPC_FPU         (1<<3)
# define PPC_MADD300     (1<<4)
# define PPC_MFTB        (1<<5)
# define PPC_MFSPR268    (1<<6)
# define PPC_BRD31       (1<<7)

#endif
