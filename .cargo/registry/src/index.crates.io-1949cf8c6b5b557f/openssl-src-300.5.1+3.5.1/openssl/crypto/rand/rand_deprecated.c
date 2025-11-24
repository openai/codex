/*
 * Copyright 1995-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/e_os.h"
#include <openssl/macros.h>
#include <openssl/rand.h>

#if defined(OPENSSL_SYS_WINDOWS) || defined(OPENSSL_SYS_WIN32)
# include <windows.h>
# if OPENSSL_API_COMPAT < 0x10100000L

# define DEPRECATED_RAND_FUNCTIONS_DEFINED

int RAND_event(UINT iMsg, WPARAM wParam, LPARAM lParam)
{
    RAND_poll();
    return RAND_status();
}

void RAND_screen(void)
{
    RAND_poll();
}
# endif
#endif

#ifndef DEPRECATED_RAND_FUNCTIONS_DEFINED
NON_EMPTY_TRANSLATION_UNIT
#endif
