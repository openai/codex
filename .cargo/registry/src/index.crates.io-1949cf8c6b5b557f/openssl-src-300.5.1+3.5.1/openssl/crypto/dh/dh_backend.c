/*
 * Copyright 2020-2023 The OpenSSL Project Authors. All Rights Reserved.
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

#include <openssl/err.h>
#include <openssl/core_names.h>
#ifndef FIPS_MODULE
# include <openssl/x509.h>
#endif
#include "internal/param_build_set.h"
#include "crypto/dh.h"
#include "dh_local.h"

/*
 * The intention with the "backend" source file is to offer backend functions
 * for legacy backends (EVP_PKEY_ASN1_METHOD and EVP_PKEY_METHOD) and provider
 * implementations alike.
 */

static int dh_ffc_params_fromdata(DH *dh, const OSSL_PARAM params[])
{
    int ret;
    FFC_PARAMS *ffc = ossl_dh_get0_params(dh);

    ret = ossl_ffc_params_fromdata(ffc, params);
    if (ret)
        ossl_dh_cache_named_group(dh); /* This increments dh->dirty_cnt */
    return ret;
}

int ossl_dh_params_fromdata(DH *dh, const OSSL_PARAM params[])
{
    const OSSL_PARAM *param_priv_len;
    long priv_len;

    if (!dh_ffc_params_fromdata(dh, params))
        return 0;

    param_priv_len =
        OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_DH_PRIV_LEN);
    if (param_priv_len != NULL
        && (!OSSL_PARAM_get_long(param_priv_len, &priv_len)
            || !DH_set_length(dh, priv_len)))
        return 0;

    return 1;
}

int ossl_dh_key_fromdata(DH *dh, const OSSL_PARAM params[], int include_private)
{
    const OSSL_PARAM *param_priv_key, *param_pub_key;
    BIGNUM *priv_key = NULL, *pub_key = NULL;

    if (dh == NULL)
        return 0;

    param_priv_key = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PRIV_KEY);
    param_pub_key = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PUB_KEY);

    if (include_private
        && param_priv_key != NULL
        && !OSSL_PARAM_get_BN(param_priv_key, &priv_key))
        goto err;

    if (param_pub_key != NULL
        && !OSSL_PARAM_get_BN(param_pub_key, &pub_key))
        goto err;

    if (!DH_set0_key(dh, pub_key, priv_key))
        goto err;

    return 1;

 err:
    BN_clear_free(priv_key);
    BN_free(pub_key);
    return 0;
}

int ossl_dh_params_todata(DH *dh, OSSL_PARAM_BLD *bld, OSSL_PARAM params[])
{
    long l = DH_get_length(dh);

    if (!ossl_ffc_params_todata(ossl_dh_get0_params(dh), bld, params))
        return 0;
    if (l > 0
        && !ossl_param_build_set_long(bld, params, OSSL_PKEY_PARAM_DH_PRIV_LEN, l))
        return 0;
    return 1;
}

int ossl_dh_key_todata(DH *dh, OSSL_PARAM_BLD *bld, OSSL_PARAM params[],
                       int include_private)
{
    const BIGNUM *priv = NULL, *pub = NULL;

    if (dh == NULL)
        return 0;

    DH_get0_key(dh, &pub, &priv);
    if (priv != NULL
        && include_private
        && !ossl_param_build_set_bn(bld, params, OSSL_PKEY_PARAM_PRIV_KEY, priv))
        return 0;
    if (pub != NULL
        && !ossl_param_build_set_bn(bld, params, OSSL_PKEY_PARAM_PUB_KEY, pub))
        return 0;

    return 1;
}

int ossl_dh_is_foreign(const DH *dh)
{
#ifndef FIPS_MODULE
    if (dh->engine != NULL || ossl_dh_get_method(dh) != DH_OpenSSL())
        return 1;
#endif
    return 0;
}

static ossl_inline int dh_bn_dup_check(BIGNUM **out, const BIGNUM *f)
{
    if (f != NULL && (*out = BN_dup(f)) == NULL)
        return 0;
    return 1;
}

DH *ossl_dh_dup(const DH *dh, int selection)
{
    DH *dupkey = NULL;

    /* Do not try to duplicate foreign DH keys */
    if (ossl_dh_is_foreign(dh))
        return NULL;

    if ((dupkey = ossl_dh_new_ex(dh->libctx)) == NULL)
        return NULL;

    dupkey->length = DH_get_length(dh);
    if ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) != 0
        && !ossl_ffc_params_copy(&dupkey->params, &dh->params))
        goto err;

    dupkey->flags = dh->flags;

    if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0
        && ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) == 0
            || !dh_bn_dup_check(&dupkey->pub_key, dh->pub_key)))
        goto err;

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0
        && ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) == 0
            || !dh_bn_dup_check(&dupkey->priv_key, dh->priv_key)))
        goto err;

#ifndef FIPS_MODULE
    if (!CRYPTO_dup_ex_data(CRYPTO_EX_INDEX_DH,
                            &dupkey->ex_data, &dh->ex_data))
        goto err;
#endif

    return dupkey;

 err:
    DH_free(dupkey);
    return NULL;
}

#ifndef FIPS_MODULE
DH *ossl_dh_key_from_pkcs8(const PKCS8_PRIV_KEY_INFO *p8inf,
                           OSSL_LIB_CTX *libctx, const char *propq)
{
    const unsigned char *p, *pm;
    int pklen, pmlen;
    int ptype;
    const void *pval;
    const ASN1_STRING *pstr;
    const X509_ALGOR *palg;
    BIGNUM *privkey_bn = NULL;
    ASN1_INTEGER *privkey = NULL;
    DH *dh = NULL;

    if (!PKCS8_pkey_get0(NULL, &p, &pklen, &palg, p8inf))
        return 0;

    X509_ALGOR_get0(NULL, &ptype, &pval, palg);

    if (ptype != V_ASN1_SEQUENCE)
        goto decerr;
    if ((privkey = d2i_ASN1_INTEGER(NULL, &p, pklen)) == NULL)
        goto decerr;

    pstr = pval;
    pm = pstr->data;
    pmlen = pstr->length;
    switch (OBJ_obj2nid(palg->algorithm)) {
    case NID_dhKeyAgreement:
        dh = d2i_DHparams(NULL, &pm, pmlen);
        break;
    case NID_dhpublicnumber:
        dh = d2i_DHxparams(NULL, &pm, pmlen);
        break;
    default:
        goto decerr;
    }
    if (dh == NULL)
        goto decerr;

    /* We have parameters now set private key */
    if ((privkey_bn = BN_secure_new()) == NULL
        || !ASN1_INTEGER_to_BN(privkey, privkey_bn)) {
        ERR_raise(ERR_LIB_DH, DH_R_BN_ERROR);
        BN_clear_free(privkey_bn);
        goto dherr;
    }
    if (!DH_set0_key(dh, NULL, privkey_bn))
        goto dherr;
    /* Calculate public key, increments dirty_cnt */
    if (!DH_generate_key(dh))
        goto dherr;

    goto done;

 decerr:
    ERR_raise(ERR_LIB_DH, EVP_R_DECODE_ERROR);
 dherr:
    DH_free(dh);
    dh = NULL;
 done:
    ASN1_STRING_clear_free(privkey);
    return dh;
}
#endif
