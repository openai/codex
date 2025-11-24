/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#ifndef OSSL_CRYPTO_LOONGARCH_ARCH_H
# define OSSL_CRYPTO_LOONGARCH_ARCH_H

# ifndef __ASSEMBLER__
extern unsigned int OPENSSL_loongarch_hwcap_P;
# endif

# define LOONGARCH_HWCAP_LSX  (1 << 4)
# define LOONGARCH_HWCAP_LASX (1 << 5)

#endif
