/*
 * Copyright 2002-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * ECDSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <openssl/ec.h>
#include "ec_local.h"
#include <openssl/err.h>

/*-
 * returns
 *      1: correct signature
 *      0: incorrect signature
 *     -1: error
 */
int ECDSA_do_verify(const unsigned char *dgst, int dgst_len,
                    const ECDSA_SIG *sig, EC_KEY *eckey)
{
    if (eckey->meth->verify_sig != NULL)
        return eckey->meth->verify_sig(dgst, dgst_len, sig, eckey);
    ERR_raise(ERR_LIB_EC, EC_R_OPERATION_NOT_SUPPORTED);
    return -1;
}

/*-
 * returns
 *      1: correct signature
 *      0: incorrect signature
 *     -1: error
 */
int ECDSA_verify(int type, const unsigned char *dgst, int dgst_len,
                 const unsigned char *sigbuf, int sig_len, EC_KEY *eckey)
{
    if (eckey->meth->verify != NULL)
        return eckey->meth->verify(type, dgst, dgst_len, sigbuf, sig_len,
                                   eckey);
    ERR_raise(ERR_LIB_EC, EC_R_OPERATION_NOT_SUPPORTED);
    return -1;
}
