/*
 * Copyright 2017-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* DH parameters from RFC7919 and RFC3526 */

/*
 * DH low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include "internal/cryptlib.h"
#include "internal/ffc.h"
#include "dh_local.h"
#include <openssl/bn.h>
#include <openssl/objects.h>
#include "internal/nelem.h"
#include "crypto/dh.h"

static DH *dh_param_init(OSSL_LIB_CTX *libctx, const DH_NAMED_GROUP *group)
{
    DH *dh = ossl_dh_new_ex(libctx);

    if (dh == NULL)
        return NULL;

    ossl_ffc_named_group_set(&dh->params, group);
    dh->params.nid = ossl_ffc_named_group_get_uid(group);
    dh->dirty_cnt++;
    return dh;
}

DH *ossl_dh_new_by_nid_ex(OSSL_LIB_CTX *libctx, int nid)
{
    const DH_NAMED_GROUP *group;

    if ((group = ossl_ffc_uid_to_dh_named_group(nid)) != NULL)
        return dh_param_init(libctx, group);

    ERR_raise(ERR_LIB_DH, DH_R_INVALID_PARAMETER_NID);
    return NULL;
}

DH *DH_new_by_nid(int nid)
{
    return ossl_dh_new_by_nid_ex(NULL, nid);
}

void ossl_dh_cache_named_group(DH *dh)
{
    const DH_NAMED_GROUP *group;

    if (dh == NULL)
        return;

    dh->params.nid = NID_undef; /* flush cached value */

    /* Exit if p or g is not set */
    if (dh->params.p == NULL
        || dh->params.g == NULL)
        return;

    if ((group = ossl_ffc_numbers_to_dh_named_group(dh->params.p,
                                                    dh->params.q,
                                                    dh->params.g)) != NULL) {
        if (dh->params.q == NULL)
            dh->params.q = (BIGNUM *)ossl_ffc_named_group_get_q(group);
        /* cache the nid and default key length */
        dh->params.nid = ossl_ffc_named_group_get_uid(group);
        dh->params.keylength = ossl_ffc_named_group_get_keylength(group);
        dh->dirty_cnt++;
    }
}

int ossl_dh_is_named_safe_prime_group(const DH *dh)
{
    int id = DH_get_nid(dh);

    /*
     * Exclude RFC5114 groups (id = 1..3) since they do not have
     * q = (p - 1) / 2
     */
    return (id > 3);
}

int DH_get_nid(const DH *dh)
{
    if (dh == NULL)
        return NID_undef;

    return dh->params.nid;
}
