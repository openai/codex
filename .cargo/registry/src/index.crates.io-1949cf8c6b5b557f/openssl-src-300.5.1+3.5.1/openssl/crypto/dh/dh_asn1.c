/*
 * Copyright 2000-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * DH low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/bn.h>
#include "dh_local.h"
#include <openssl/objects.h>
#include <openssl/asn1t.h>
#include "crypto/dh.h"

/* Override the default free and new methods */
static int dh_cb(int operation, ASN1_VALUE **pval, const ASN1_ITEM *it,
                 void *exarg)
{
    if (operation == ASN1_OP_NEW_PRE) {
        *pval = (ASN1_VALUE *)DH_new();
        if (*pval != NULL)
            return 2;
        return 0;
    } else if (operation == ASN1_OP_FREE_PRE) {
        DH_free((DH *)*pval);
        *pval = NULL;
        return 2;
    } else if (operation == ASN1_OP_D2I_POST) {
        DH *dh = (DH *)*pval;

        DH_clear_flags(dh, DH_FLAG_TYPE_MASK);
        DH_set_flags(dh, DH_FLAG_TYPE_DH);
        ossl_dh_cache_named_group(dh);
        dh->dirty_cnt++;
    }
    return 1;
}

ASN1_SEQUENCE_cb(DHparams, dh_cb) = {
        ASN1_SIMPLE(DH, params.p, BIGNUM),
        ASN1_SIMPLE(DH, params.g, BIGNUM),
        ASN1_OPT_EMBED(DH, length, ZINT32),
} ASN1_SEQUENCE_END_cb(DH, DHparams)

IMPLEMENT_ASN1_ENCODE_FUNCTIONS_fname(DH, DHparams, DHparams)

/*
 * Internal only structures for handling X9.42 DH: this gets translated to or
 * from a DH structure straight away.
 */

typedef struct {
    ASN1_BIT_STRING *seed;
    BIGNUM *counter;
} int_dhvparams;

typedef struct {
    BIGNUM *p;
    BIGNUM *q;
    BIGNUM *g;
    BIGNUM *j;
    int_dhvparams *vparams;
} int_dhx942_dh;

ASN1_SEQUENCE(DHvparams) = {
        ASN1_SIMPLE(int_dhvparams, seed, ASN1_BIT_STRING),
        ASN1_SIMPLE(int_dhvparams, counter, BIGNUM)
} static_ASN1_SEQUENCE_END_name(int_dhvparams, DHvparams)

ASN1_SEQUENCE(DHxparams) = {
        ASN1_SIMPLE(int_dhx942_dh, p, BIGNUM),
        ASN1_SIMPLE(int_dhx942_dh, g, BIGNUM),
        ASN1_SIMPLE(int_dhx942_dh, q, BIGNUM),
        ASN1_OPT(int_dhx942_dh, j, BIGNUM),
        ASN1_OPT(int_dhx942_dh, vparams, DHvparams),
} static_ASN1_SEQUENCE_END_name(int_dhx942_dh, DHxparams)

int_dhx942_dh *d2i_int_dhx(int_dhx942_dh **a,
                           const unsigned char **pp, long length);
int i2d_int_dhx(const int_dhx942_dh *a, unsigned char **pp);

IMPLEMENT_ASN1_ENCODE_FUNCTIONS_fname(int_dhx942_dh, DHxparams, int_dhx)

DH *d2i_DHxparams(DH **a, const unsigned char **pp, long length)
{
    FFC_PARAMS *params;
    int_dhx942_dh *dhx = NULL;
    DH *dh = NULL;

    dh = DH_new();
    if (dh == NULL)
        return NULL;
    dhx = d2i_int_dhx(NULL, pp, length);
    if (dhx == NULL) {
        DH_free(dh);
        return NULL;
    }

    if (a != NULL) {
        DH_free(*a);
        *a = dh;
    }

    params = &dh->params;
    DH_set0_pqg(dh, dhx->p, dhx->q, dhx->g);
    ossl_ffc_params_set0_j(params, dhx->j);

    if (dhx->vparams != NULL) {
        /* The counter has a maximum value of 4 * numbits(p) - 1 */
        size_t counter = (size_t)BN_get_word(dhx->vparams->counter);
        ossl_ffc_params_set_validate_params(params, dhx->vparams->seed->data,
                                            dhx->vparams->seed->length,
                                            counter);
        ASN1_BIT_STRING_free(dhx->vparams->seed);
        BN_free(dhx->vparams->counter);
        OPENSSL_free(dhx->vparams);
        dhx->vparams = NULL;
    }

    OPENSSL_free(dhx);
    DH_clear_flags(dh, DH_FLAG_TYPE_MASK);
    DH_set_flags(dh, DH_FLAG_TYPE_DHX);
    return dh;
}

int i2d_DHxparams(const DH *dh, unsigned char **pp)
{
    int ret = 0;
    int_dhx942_dh dhx;
    int_dhvparams dhv = { NULL, NULL };
    ASN1_BIT_STRING seed;
    size_t seedlen = 0;
    const FFC_PARAMS *params = &dh->params;
    int counter;

    ossl_ffc_params_get0_pqg(params, (const BIGNUM **)&dhx.p,
                             (const BIGNUM **)&dhx.q, (const BIGNUM **)&dhx.g);
    dhx.j = params->j;
    ossl_ffc_params_get_validate_params(params, &seed.data, &seedlen, &counter);
    seed.length = (int)seedlen;

    if (counter != -1 && seed.data != NULL && seed.length > 0) {
        seed.flags = ASN1_STRING_FLAG_BITS_LEFT;
        dhv.seed = &seed;
        dhv.counter = BN_new();
        if (dhv.counter == NULL)
            return 0;
        if (!BN_set_word(dhv.counter, (BN_ULONG)counter))
            goto err;
        dhx.vparams = &dhv;
    } else {
        dhx.vparams = NULL;
    }
    ret = i2d_int_dhx(&dhx, pp);
err:
    BN_free(dhv.counter);
    return ret;
}
