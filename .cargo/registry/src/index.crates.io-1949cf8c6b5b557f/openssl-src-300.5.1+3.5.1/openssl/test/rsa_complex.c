/*
 * Copyright 2018 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Check to see if there is a conflict between complex.h and openssl/rsa.h.
 * The former defines "I" as a macro and earlier versions of the latter use
 * for function arguments.
 *
 * Will always succeed on djgpp, since its libc does not have complex.h.
 */

#if !defined(__DJGPP__)
# if defined(__STDC_VERSION__)
#  if __STDC_VERSION__ >= 199901L
#   include <complex.h>
#  endif
# endif
# include <openssl/rsa.h>
#endif
#include <stdlib.h>

int main(int argc, char *argv[])
{
    /* There are explicitly no run time checks for this one */
    return EXIT_SUCCESS;
}
