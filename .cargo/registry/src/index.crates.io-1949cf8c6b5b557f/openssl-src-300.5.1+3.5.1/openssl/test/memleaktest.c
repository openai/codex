/*
 * Copyright 2016-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/bio.h>
#include <openssl/crypto.h>

#include "testutil.h"

/* __has_feature is a clang-ism, while __SANITIZE_ADDRESS__ is a gcc-ism */
#if defined(__has_feature)
# if __has_feature(address_sanitizer)
#  define __SANITIZE_ADDRESS__ 1
# endif
#endif
/* If __SANITIZE_ADDRESS__ isn't defined, define it to be false */
/* Leak detection is not yet supported with MSVC on Windows, so */
/* set __SANITIZE_ADDRESS__ to false in this case as well.      */
#if !defined(__SANITIZE_ADDRESS__) || defined(_MSC_VER)
# undef __SANITIZE_ADDRESS__
# define __SANITIZE_ADDRESS__ 0
#endif

/*
 * We use a proper main function here instead of the custom main from the
 * test framework to avoid CRYPTO_mem_leaks stuff.
 */

int main(int argc, char *argv[])
{
#if __SANITIZE_ADDRESS__
    int exitcode = EXIT_SUCCESS;
#else
    /*
     * When we don't sanitize, we set the exit code to what we would expect
     * to get when we are sanitizing.  This makes it easy for wrapper scripts
     * to detect that we get the result we expect.
     */
    int exitcode = EXIT_FAILURE;
#endif
    char *volatile lost;

    lost = OPENSSL_malloc(3);
    if (!TEST_ptr(lost))
        return EXIT_FAILURE;

    strcpy(lost, "ab");

    if (argv[1] && strcmp(argv[1], "freeit") == 0) {
        OPENSSL_free(lost);
        exitcode = EXIT_SUCCESS;
    }

    lost = NULL;
    return exitcode;
}
