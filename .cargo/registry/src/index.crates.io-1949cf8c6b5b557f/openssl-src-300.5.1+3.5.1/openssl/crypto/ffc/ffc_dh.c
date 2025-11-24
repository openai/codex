/*
 * Copyright 2020-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/ffc.h"
#include "internal/nelem.h"
#include "crypto/bn_dh.h"

#ifndef OPENSSL_NO_DH

# define FFDHE(sz, keylength) {                                             \
        SN_ffdhe##sz, NID_ffdhe##sz,                                        \
        sz,                                                                 \
        keylength,                                                          \
        &ossl_bignum_ffdhe##sz##_p, &ossl_bignum_ffdhe##sz##_q,             \
        &ossl_bignum_const_2,                                               \
    }

# define MODP(sz, keylength)  {                                             \
        SN_modp_##sz, NID_modp_##sz,                                        \
        sz,                                                                 \
        keylength,                                                          \
        &ossl_bignum_modp_##sz##_p, &ossl_bignum_modp_##sz##_q,             \
        &ossl_bignum_const_2                                                \
    }

# define RFC5114(name, uid, sz, tag) {                                      \
        name, uid,                                                          \
        sz,                                                                 \
        0,                                                                  \
        &ossl_bignum_dh##tag##_p, &ossl_bignum_dh##tag##_q,                 \
        &ossl_bignum_dh##tag##_g                                            \
    }

#else

# define FFDHE(sz, keylength)           { SN_ffdhe##sz, NID_ffdhe##sz }
# define MODP(sz, keylength)            { SN_modp_##sz, NID_modp_##sz }
# define RFC5114(name, uid, sz, tag)    { name, uid }

#endif

struct dh_named_group_st {
    const char *name;
    int uid;
#ifndef OPENSSL_NO_DH
    int32_t nbits;
    int keylength;
    const BIGNUM *p;
    const BIGNUM *q;
    const BIGNUM *g;
#endif
};

/*
 * The private key length values are taken from RFC7919 with the values for
 * MODP primes given the same lengths as the equivalent FFDHE.
 * The MODP 1536 value is approximated.
 */
static const DH_NAMED_GROUP dh_named_groups[] = {
    FFDHE(2048, 225),
    FFDHE(3072, 275),
    FFDHE(4096, 325),
    FFDHE(6144, 375),
    FFDHE(8192, 400),
#ifndef FIPS_MODULE
    MODP(1536, 200),
#endif
    MODP(2048, 225),
    MODP(3072, 275),
    MODP(4096, 325),
    MODP(6144, 375),
    MODP(8192, 400),
    /*
     * Additional dh named groups from RFC 5114 that have a different g.
     * The uid can be any unique identifier.
     */
#ifndef FIPS_MODULE
    RFC5114("dh_1024_160", 1, 1024, 1024_160),
    RFC5114("dh_2048_224", 2, 2048, 2048_224),
    RFC5114("dh_2048_256", 3, 2048, 2048_256),
#endif
};

const DH_NAMED_GROUP *ossl_ffc_name_to_dh_named_group(const char *name)
{
    size_t i;

    for (i = 0; i < OSSL_NELEM(dh_named_groups); ++i) {
        if (OPENSSL_strcasecmp(dh_named_groups[i].name, name) == 0)
            return &dh_named_groups[i];
    }
    return NULL;
}

const DH_NAMED_GROUP *ossl_ffc_uid_to_dh_named_group(int uid)
{
    size_t i;

    for (i = 0; i < OSSL_NELEM(dh_named_groups); ++i) {
        if (dh_named_groups[i].uid == uid)
            return &dh_named_groups[i];
    }
    return NULL;
}

#ifndef OPENSSL_NO_DH
const DH_NAMED_GROUP *ossl_ffc_numbers_to_dh_named_group(const BIGNUM *p,
                                                         const BIGNUM *q,
                                                         const BIGNUM *g)
{
    size_t i;

    for (i = 0; i < OSSL_NELEM(dh_named_groups); ++i) {
        /* Keep searching until a matching p and g is found */
        if (BN_cmp(p, dh_named_groups[i].p) == 0
            && BN_cmp(g, dh_named_groups[i].g) == 0
            /* Verify q is correct if it exists */
            && (q == NULL || BN_cmp(q, dh_named_groups[i].q) == 0))
            return &dh_named_groups[i];
    }
    return NULL;
}
#endif

int ossl_ffc_named_group_get_uid(const DH_NAMED_GROUP *group)
{
    if (group == NULL)
        return NID_undef;
    return group->uid;
}

const char *ossl_ffc_named_group_get_name(const DH_NAMED_GROUP *group)
{
    if (group == NULL)
        return NULL;
    return group->name;
}

#ifndef OPENSSL_NO_DH
int ossl_ffc_named_group_get_keylength(const DH_NAMED_GROUP *group)
{
    if (group == NULL)
        return 0;
    return group->keylength;
}

const BIGNUM *ossl_ffc_named_group_get_q(const DH_NAMED_GROUP *group)
{
    if (group == NULL)
        return NULL;
    return group->q;
}

int ossl_ffc_named_group_set(FFC_PARAMS *ffc, const DH_NAMED_GROUP *group)
{
    if (ffc == NULL || group == NULL)
        return 0;

    ossl_ffc_params_set0_pqg(ffc, (BIGNUM *)group->p, (BIGNUM *)group->q,
                             (BIGNUM *)group->g);
    ffc->keylength = group->keylength;

    /* flush the cached nid, The DH layer is responsible for caching */
    ffc->nid = NID_undef;
    return 1;
}
#endif
