/*
 * Copyright 2020-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core_names.h>
#include "internal/ffc.h"
#include "internal/sizes.h"

/*
 * The intention with the "backend" source file is to offer backend support
 * for legacy backends (EVP_PKEY_ASN1_METHOD and EVP_PKEY_METHOD) and provider
 * implementations alike.
 */

int ossl_ffc_params_fromdata(FFC_PARAMS *ffc, const OSSL_PARAM params[])
{
    const OSSL_PARAM *prm;
    const OSSL_PARAM *param_p, *param_q, *param_g;
    BIGNUM *p = NULL, *q = NULL, *g = NULL, *j = NULL;
    int i;

    prm  = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_GROUP_NAME);
    if (prm != NULL) {
        /*
         * In a no-dh build we just go straight to err because we have no
         * support for this.
         */
#ifndef OPENSSL_NO_DH
        const DH_NAMED_GROUP *group = NULL;

        if (prm->data_type != OSSL_PARAM_UTF8_STRING
            || prm->data == NULL
            || (group = ossl_ffc_name_to_dh_named_group(prm->data)) == NULL
            || !ossl_ffc_named_group_set(ffc, group))
#endif
            goto err;
    }

    param_p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_P);
    param_g = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_G);
    param_q = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_Q);

    if ((param_p != NULL && !OSSL_PARAM_get_BN(param_p, &p))
        || (param_q != NULL && !OSSL_PARAM_get_BN(param_q, &q))
        || (param_g != NULL && !OSSL_PARAM_get_BN(param_g, &g)))
        goto err;

    prm = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_GINDEX);
    if (prm != NULL) {
        if (!OSSL_PARAM_get_int(prm, &i))
            goto err;
        ffc->gindex =  i;
    }
    prm = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_PCOUNTER);
    if (prm != NULL) {
        if (!OSSL_PARAM_get_int(prm, &i))
            goto err;
        ffc->pcounter = i;
    }
    prm = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_COFACTOR);
    if (prm != NULL && !OSSL_PARAM_get_BN(prm, &j))
        goto err;
    prm = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_H);
    if (prm != NULL) {
        if (!OSSL_PARAM_get_int(prm, &i))
            goto err;
        ffc->h =  i;
    }
    prm  = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_SEED);
    if (prm != NULL) {
        if (prm->data_type != OSSL_PARAM_OCTET_STRING
            || !ossl_ffc_params_set_seed(ffc, prm->data, prm->data_size))
            goto err;
    }
    prm  = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_VALIDATE_PQ);
    if (prm != NULL) {
        if (!OSSL_PARAM_get_int(prm, &i))
            goto err;
        ossl_ffc_params_enable_flags(ffc, FFC_PARAM_FLAG_VALIDATE_PQ, i);
    }
    prm  = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_VALIDATE_G);
    if (prm != NULL) {
        if (!OSSL_PARAM_get_int(prm, &i))
            goto err;
        ossl_ffc_params_enable_flags(ffc, FFC_PARAM_FLAG_VALIDATE_G, i);
    }
    prm  = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_VALIDATE_LEGACY);
    if (prm != NULL) {
        if (!OSSL_PARAM_get_int(prm, &i))
            goto err;
        ossl_ffc_params_enable_flags(ffc, FFC_PARAM_FLAG_VALIDATE_LEGACY, i);
    }

    prm = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_DIGEST);
    if (prm != NULL) {
        const OSSL_PARAM *p1;
        const char *props = NULL;

        if (prm->data_type != OSSL_PARAM_UTF8_STRING)
            goto err;
        p1 = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_DIGEST_PROPS);
        if (p1 != NULL) {
            if (p1->data_type != OSSL_PARAM_UTF8_STRING)
                goto err;
            props = p1->data;
        }
        ossl_ffc_set_digest(ffc, prm->data, props);
    }
    ossl_ffc_params_set0_pqg(ffc, p, q, g);
    ossl_ffc_params_set0_j(ffc, j);
    return 1;

 err:
    BN_free(j);
    BN_free(p);
    BN_free(q);
    BN_free(g);
    return 0;
}
