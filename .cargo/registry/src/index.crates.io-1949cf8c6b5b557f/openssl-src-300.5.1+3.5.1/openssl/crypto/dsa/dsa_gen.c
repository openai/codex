/*
 * Copyright 1995-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * DSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <openssl/opensslconf.h>
#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/evp.h>
#include <openssl/bn.h>
#include <openssl/rand.h>
#include <openssl/sha.h>
#include "crypto/dsa.h"
#include "dsa_local.h"

int ossl_dsa_generate_ffc_parameters(DSA *dsa, int type, int pbits, int qbits,
                                     BN_GENCB *cb)
{
    int ret = 0, res;

#ifndef FIPS_MODULE
    if (type == DSA_PARAMGEN_TYPE_FIPS_186_2)
        ret = ossl_ffc_params_FIPS186_2_generate(dsa->libctx, &dsa->params,
                                                 FFC_PARAM_TYPE_DSA,
                                                 pbits, qbits, &res, cb);
    else
#endif
        ret = ossl_ffc_params_FIPS186_4_generate(dsa->libctx, &dsa->params,
                                                 FFC_PARAM_TYPE_DSA,
                                                 pbits, qbits, &res, cb);
    if (ret > 0)
        dsa->dirty_cnt++;
    return ret;
}

#ifndef FIPS_MODULE
int DSA_generate_parameters_ex(DSA *dsa, int bits,
                               const unsigned char *seed_in, int seed_len,
                               int *counter_ret, unsigned long *h_ret,
                               BN_GENCB *cb)
{
    if (dsa->meth->dsa_paramgen)
        return dsa->meth->dsa_paramgen(dsa, bits, seed_in, seed_len,
                                       counter_ret, h_ret, cb);
    if (seed_in != NULL
        && !ossl_ffc_params_set_validate_params(&dsa->params, seed_in, seed_len,
                                                -1))
        return 0;

    /* The old code used FIPS 186-2 DSA Parameter generation */
    if (bits < 2048 && seed_len <= 20) {
        if (!ossl_dsa_generate_ffc_parameters(dsa, DSA_PARAMGEN_TYPE_FIPS_186_2,
                                              bits, 160, cb))
            return 0;
    } else {
        if (!ossl_dsa_generate_ffc_parameters(dsa, DSA_PARAMGEN_TYPE_FIPS_186_4,
                                              bits, 0, cb))
            return 0;
    }

    if (counter_ret != NULL)
        *counter_ret = dsa->params.pcounter;
    if (h_ret != NULL)
        *h_ret = dsa->params.h;
    return 1;
}
#endif
