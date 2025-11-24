/*
 * Copyright 2020-2023 The OpenSSL Project Authors. All Rights Reserved.
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

#include <openssl/core_names.h>
#include <openssl/err.h>
#ifndef FIPS_MODULE
# include <openssl/x509.h>
#endif
#include "crypto/dsa.h"
#include "dsa_local.h"

/*
 * The intention with the "backend" source file is to offer backend support
 * for legacy backends (EVP_PKEY_ASN1_METHOD and EVP_PKEY_METHOD) and provider
 * implementations alike.
 */

int ossl_dsa_key_fromdata(DSA *dsa, const OSSL_PARAM params[],
                          int include_private)
{
    const OSSL_PARAM *param_priv_key = NULL, *param_pub_key;
    BIGNUM *priv_key = NULL, *pub_key = NULL;

    if (dsa == NULL)
        return 0;

    if (include_private) {
        param_priv_key =
            OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PRIV_KEY);
    }
    param_pub_key =
        OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PUB_KEY);

    /* It's ok if neither half is present */
    if (param_priv_key == NULL && param_pub_key == NULL)
        return 1;

    if (param_pub_key != NULL && !OSSL_PARAM_get_BN(param_pub_key, &pub_key))
        goto err;
    if (param_priv_key != NULL && !OSSL_PARAM_get_BN(param_priv_key, &priv_key))
        goto err;

    if (!DSA_set0_key(dsa, pub_key, priv_key))
        goto err;

    return 1;

 err:
    BN_clear_free(priv_key);
    BN_free(pub_key);
    return 0;
}

int ossl_dsa_is_foreign(const DSA *dsa)
{
#ifndef FIPS_MODULE
    if (dsa->engine != NULL || DSA_get_method((DSA *)dsa) != DSA_OpenSSL())
        return 1;
#endif
    return 0;
}

static ossl_inline int dsa_bn_dup_check(BIGNUM **out, const BIGNUM *f)
{
    if (f != NULL && (*out = BN_dup(f)) == NULL)
        return 0;
    return 1;
}

DSA *ossl_dsa_dup(const DSA *dsa, int selection)
{
    DSA *dupkey = NULL;

    /* Do not try to duplicate foreign DSA keys */
    if (ossl_dsa_is_foreign(dsa))
        return NULL;

    if ((dupkey = ossl_dsa_new(dsa->libctx)) == NULL)
        return NULL;

    if ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) != 0
        && !ossl_ffc_params_copy(&dupkey->params, &dsa->params))
        goto err;

    dupkey->flags = dsa->flags;

    if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0
        && ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) == 0
            || !dsa_bn_dup_check(&dupkey->pub_key, dsa->pub_key)))
        goto err;

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0
        && ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) == 0
            || !dsa_bn_dup_check(&dupkey->priv_key, dsa->priv_key)))
        goto err;

#ifndef FIPS_MODULE
    if (!CRYPTO_dup_ex_data(CRYPTO_EX_INDEX_DSA,
                            &dupkey->ex_data, &dsa->ex_data))
        goto err;
#endif

    return dupkey;

 err:
    DSA_free(dupkey);
    return NULL;
}

#ifndef FIPS_MODULE
DSA *ossl_dsa_key_from_pkcs8(const PKCS8_PRIV_KEY_INFO *p8inf,
                             OSSL_LIB_CTX *libctx, const char *propq)
{
    const unsigned char *p, *pm;
    int pklen, pmlen;
    int ptype;
    const void *pval;
    const ASN1_STRING *pstr;
    const X509_ALGOR *palg;
    ASN1_INTEGER *privkey = NULL;
    const BIGNUM *dsa_p, *dsa_g;
    BIGNUM *dsa_pubkey = NULL, *dsa_privkey = NULL;
    BN_CTX *ctx = NULL;

    DSA *dsa = NULL;

    if (!PKCS8_pkey_get0(NULL, &p, &pklen, &palg, p8inf))
        return 0;
    X509_ALGOR_get0(NULL, &ptype, &pval, palg);

    if ((privkey = d2i_ASN1_INTEGER(NULL, &p, pklen)) == NULL)
        goto decerr;
    if (privkey->type == V_ASN1_NEG_INTEGER || ptype != V_ASN1_SEQUENCE)
        goto decerr;

    pstr = pval;
    pm = pstr->data;
    pmlen = pstr->length;
    if ((dsa = d2i_DSAparams(NULL, &pm, pmlen)) == NULL)
        goto decerr;
    /* We have parameters now set private key */
    if ((dsa_privkey = BN_secure_new()) == NULL
        || !ASN1_INTEGER_to_BN(privkey, dsa_privkey)) {
        ERR_raise(ERR_LIB_DSA, DSA_R_BN_ERROR);
        goto dsaerr;
    }
    /* Calculate public key */
    if ((dsa_pubkey = BN_new()) == NULL) {
        ERR_raise(ERR_LIB_DSA, ERR_R_BN_LIB);
        goto dsaerr;
    }
    if ((ctx = BN_CTX_new()) == NULL) {
        ERR_raise(ERR_LIB_DSA, ERR_R_BN_LIB);
        goto dsaerr;
    }

    dsa_p = DSA_get0_p(dsa);
    dsa_g = DSA_get0_g(dsa);
    BN_set_flags(dsa_privkey, BN_FLG_CONSTTIME);
    if (!BN_mod_exp(dsa_pubkey, dsa_g, dsa_privkey, dsa_p, ctx)) {
        ERR_raise(ERR_LIB_DSA, DSA_R_BN_ERROR);
        goto dsaerr;
    }
    if (!DSA_set0_key(dsa, dsa_pubkey, dsa_privkey)) {
        ERR_raise(ERR_LIB_DSA, ERR_R_INTERNAL_ERROR);
        goto dsaerr;
    }

    goto done;

 decerr:
    ERR_raise(ERR_LIB_DSA, DSA_R_DECODE_ERROR);
 dsaerr:
    BN_free(dsa_privkey);
    BN_free(dsa_pubkey);
    DSA_free(dsa);
    dsa = NULL;
 done:
    BN_CTX_free(ctx);
    ASN1_STRING_clear_free(privkey);
    return dsa;
}
#endif
