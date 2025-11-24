/*
 * Copyright 2015-2020 The OpenSSL Project Authors. All Rights Reserved.
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

ECDSA_SIG *ECDSA_do_sign(const unsigned char *dgst, int dlen, EC_KEY *eckey)
{
    return ECDSA_do_sign_ex(dgst, dlen, NULL, NULL, eckey);
}

ECDSA_SIG *ECDSA_do_sign_ex(const unsigned char *dgst, int dlen,
                            const BIGNUM *kinv, const BIGNUM *rp,
                            EC_KEY *eckey)
{
    if (eckey->meth->sign_sig != NULL)
        return eckey->meth->sign_sig(dgst, dlen, kinv, rp, eckey);
    ERR_raise(ERR_LIB_EC, EC_R_OPERATION_NOT_SUPPORTED);
    return NULL;
}

int ECDSA_sign(int type, const unsigned char *dgst, int dlen, unsigned char
               *sig, unsigned int *siglen, EC_KEY *eckey)
{
    return ECDSA_sign_ex(type, dgst, dlen, sig, siglen, NULL, NULL, eckey);
}

int ECDSA_sign_ex(int type, const unsigned char *dgst, int dlen,
                  unsigned char *sig, unsigned int *siglen, const BIGNUM *kinv,
                  const BIGNUM *r, EC_KEY *eckey)
{
    if (eckey->meth->sign != NULL)
        return eckey->meth->sign(type, dgst, dlen, sig, siglen, kinv, r, eckey);
    ERR_raise(ERR_LIB_EC, EC_R_OPERATION_NOT_SUPPORTED);
    return 0;
}

int ECDSA_sign_setup(EC_KEY *eckey, BN_CTX *ctx_in, BIGNUM **kinvp,
                     BIGNUM **rp)
{
    if (eckey->meth->sign_setup != NULL)
        return eckey->meth->sign_setup(eckey, ctx_in, kinvp, rp);
    ERR_raise(ERR_LIB_EC, EC_R_OPERATION_NOT_SUPPORTED);
    return 0;
}
