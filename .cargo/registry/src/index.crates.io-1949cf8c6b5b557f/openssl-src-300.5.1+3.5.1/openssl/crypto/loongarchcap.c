/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#include <sys/auxv.h>
#include "loongarch_arch.h"

unsigned int OPENSSL_loongarch_hwcap_P = 0;

void OPENSSL_cpuid_setup(void)
{
	OPENSSL_loongarch_hwcap_P = getauxval(AT_HWCAP);
}
