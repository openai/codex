/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Low level APIs are deprecated for public use, but still ok for internal use.
 */
#include "internal/deprecated.h"

#include <openssl/core.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/bn.h>
#include <openssl/err.h>
#include <openssl/safestack.h>
#include <openssl/proverr.h>
#include "crypto/dh.h"           /* ossl_dh_get0_params() */
#include "crypto/dsa.h"          /* ossl_dsa_get0_params() */
#include "crypto/ec.h"           /* ossl_ec_key_get_libctx */
#include "crypto/ecx.h"          /* ECX_KEY, etc... */
#include "crypto/ml_kem.h"       /* ML_KEM_KEY, etc... */
#include "crypto/rsa.h"          /* RSA_PSS_PARAMS_30, etc... */
#include "crypto/ml_dsa.h"
#include "crypto/slh_dsa.h"
#include "prov/bio.h"
#include "prov/implementations.h"
#include "internal/encoder.h"
#include "endecoder_local.h"
#include "ml_dsa_codecs.h"
#include "ml_kem_codecs.h"

DEFINE_SPECIAL_STACK_OF_CONST(BIGNUM_const, BIGNUM)

/* ---------------------------------------------------------------------- */

#ifndef OPENSSL_NO_DH
static int dh_to_text(BIO *out, const void *key, int selection)
{
    const DH *dh = key;
    const char *type_label = NULL;
    const BIGNUM *priv_key = NULL, *pub_key = NULL;
    const FFC_PARAMS *params = NULL;
    const BIGNUM *p = NULL;
    long length;

    if (out == NULL || dh == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0)
        type_label = "DH Private-Key";
    else if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0)
        type_label = "DH Public-Key";
    else if ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) != 0)
        type_label = "DH Parameters";

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0) {
        priv_key = DH_get0_priv_key(dh);
        if (priv_key == NULL) {
            ERR_raise(ERR_LIB_PROV, PROV_R_NOT_A_PRIVATE_KEY);
            return 0;
        }
    }
    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
        pub_key = DH_get0_pub_key(dh);
        if (pub_key == NULL) {
            ERR_raise(ERR_LIB_PROV, PROV_R_NOT_A_PUBLIC_KEY);
            return 0;
        }
    }
    if ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) != 0) {
        params = ossl_dh_get0_params((DH *)dh);
        if (params == NULL) {
            ERR_raise(ERR_LIB_PROV, PROV_R_NOT_PARAMETERS);
            return 0;
        }
    }

    p = DH_get0_p(dh);
    if (p == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY);
        return 0;
    }

    if (BIO_printf(out, "%s: (%d bit)\n", type_label, BN_num_bits(p)) <= 0)
        return 0;
    if (priv_key != NULL
        && !ossl_bio_print_labeled_bignum(out, "private-key:", priv_key))
        return 0;
    if (pub_key != NULL
        && !ossl_bio_print_labeled_bignum(out, "public-key:", pub_key))
        return 0;
    if (params != NULL
        && !ossl_bio_print_ffc_params(out, params))
        return 0;
    length = DH_get_length(dh);
    if (length > 0
        && BIO_printf(out, "recommended-private-length: %ld bits\n",
                      length) <= 0)
        return 0;

    return 1;
}
#endif

/* ---------------------------------------------------------------------- */

#ifndef OPENSSL_NO_DSA
static int dsa_to_text(BIO *out, const void *key, int selection)
{
    const DSA *dsa = key;
    const char *type_label = NULL;
    const BIGNUM *priv_key = NULL, *pub_key = NULL;
    const FFC_PARAMS *params = NULL;
    const BIGNUM *p = NULL;

    if (out == NULL || dsa == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0)
        type_label = "Private-Key";
    else if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0)
        type_label = "Public-Key";
    else if ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) != 0)
        type_label = "DSA-Parameters";

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0) {
        priv_key = DSA_get0_priv_key(dsa);
        if (priv_key == NULL) {
            ERR_raise(ERR_LIB_PROV, PROV_R_NOT_A_PRIVATE_KEY);
            return 0;
        }
    }
    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
        pub_key = DSA_get0_pub_key(dsa);
        if (pub_key == NULL) {
            ERR_raise(ERR_LIB_PROV, PROV_R_NOT_A_PUBLIC_KEY);
            return 0;
        }
    }
    if ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) != 0) {
        params = ossl_dsa_get0_params((DSA *)dsa);
        if (params == NULL) {
            ERR_raise(ERR_LIB_PROV, PROV_R_NOT_PARAMETERS);
            return 0;
        }
    }

    p = DSA_get0_p(dsa);
    if (p == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY);
        return 0;
    }

    if (BIO_printf(out, "%s: (%d bit)\n", type_label, BN_num_bits(p)) <= 0)
        return 0;
    if (priv_key != NULL
        && !ossl_bio_print_labeled_bignum(out, "priv:", priv_key))
        return 0;
    if (pub_key != NULL
        && !ossl_bio_print_labeled_bignum(out, "pub: ", pub_key))
        return 0;
    if (params != NULL
        && !ossl_bio_print_ffc_params(out, params))
        return 0;

    return 1;
}
#endif

/* ---------------------------------------------------------------------- */

#ifndef OPENSSL_NO_EC
static int ec_param_explicit_curve_to_text(BIO *out, const EC_GROUP *group,
                                           BN_CTX *ctx)
{
    const char *plabel = "Prime:";
    BIGNUM *p = NULL, *a = NULL, *b = NULL;

    p = BN_CTX_get(ctx);
    a = BN_CTX_get(ctx);
    b = BN_CTX_get(ctx);
    if (b == NULL
        || !EC_GROUP_get_curve(group, p, a, b, ctx))
        return 0;

    if (EC_GROUP_get_field_type(group) == NID_X9_62_characteristic_two_field) {
        int basis_type = EC_GROUP_get_basis_type(group);

        /* print the 'short name' of the base type OID */
        if (basis_type == NID_undef
            || BIO_printf(out, "Basis Type: %s\n", OBJ_nid2sn(basis_type)) <= 0)
            return 0;
        plabel = "Polynomial:";
    }
    return ossl_bio_print_labeled_bignum(out, plabel, p)
        && ossl_bio_print_labeled_bignum(out, "A:   ", a)
        && ossl_bio_print_labeled_bignum(out, "B:   ", b);
}

static int ec_param_explicit_gen_to_text(BIO *out, const EC_GROUP *group,
                                         BN_CTX *ctx)
{
    int ret;
    size_t buflen;
    point_conversion_form_t form;
    const EC_POINT *point = NULL;
    const char *glabel = NULL;
    unsigned char *buf = NULL;

    form = EC_GROUP_get_point_conversion_form(group);
    point = EC_GROUP_get0_generator(group);

    if (point == NULL)
        return 0;

    switch (form) {
    case POINT_CONVERSION_COMPRESSED:
       glabel = "Generator (compressed):";
       break;
    case POINT_CONVERSION_UNCOMPRESSED:
        glabel = "Generator (uncompressed):";
        break;
    case POINT_CONVERSION_HYBRID:
        glabel = "Generator (hybrid):";
        break;
    default:
        return 0;
    }

    buflen = EC_POINT_point2buf(group, point, form, &buf, ctx);
    if (buflen == 0)
        return 0;

    ret = ossl_bio_print_labeled_buf(out, glabel, buf, buflen);
    OPENSSL_clear_free(buf, buflen);
    return ret;
}

/* Print explicit parameters */
static int ec_param_explicit_to_text(BIO *out, const EC_GROUP *group,
                                     OSSL_LIB_CTX *libctx)
{
    int ret = 0, tmp_nid;
    BN_CTX *ctx = NULL;
    const BIGNUM *order = NULL, *cofactor = NULL;
    const unsigned char *seed;
    size_t seed_len = 0;

    ctx = BN_CTX_new_ex(libctx);
    if (ctx == NULL)
        return 0;
    BN_CTX_start(ctx);

    tmp_nid = EC_GROUP_get_field_type(group);
    order = EC_GROUP_get0_order(group);
    if (order == NULL)
        goto err;

    seed = EC_GROUP_get0_seed(group);
    if (seed != NULL)
        seed_len = EC_GROUP_get_seed_len(group);
    cofactor = EC_GROUP_get0_cofactor(group);

    /* print the 'short name' of the field type */
    if (BIO_printf(out, "Field Type: %s\n", OBJ_nid2sn(tmp_nid)) <= 0
        || !ec_param_explicit_curve_to_text(out, group, ctx)
        || !ec_param_explicit_gen_to_text(out, group, ctx)
        || !ossl_bio_print_labeled_bignum(out, "Order: ", order)
        || (cofactor != NULL
            && !ossl_bio_print_labeled_bignum(out, "Cofactor: ", cofactor))
        || (seed != NULL
            && !ossl_bio_print_labeled_buf(out, "Seed:", seed, seed_len)))
        goto err;
    ret = 1;
err:
    BN_CTX_end(ctx);
    BN_CTX_free(ctx);
    return ret;
}

static int ec_param_to_text(BIO *out, const EC_GROUP *group,
                            OSSL_LIB_CTX *libctx)
{
    if (EC_GROUP_get_asn1_flag(group) & OPENSSL_EC_NAMED_CURVE) {
        const char *curve_name;
        int curve_nid = EC_GROUP_get_curve_name(group);

        /* Explicit parameters */
        if (curve_nid == NID_undef)
            return 0;

        if (BIO_printf(out, "%s: %s\n", "ASN1 OID", OBJ_nid2sn(curve_nid)) <= 0)
            return 0;

        curve_name = EC_curve_nid2nist(curve_nid);
        return (curve_name == NULL
                || BIO_printf(out, "%s: %s\n", "NIST CURVE", curve_name) > 0);
    } else {
        return ec_param_explicit_to_text(out, group, libctx);
    }
}

static int ec_to_text(BIO *out, const void *key, int selection)
{
    const EC_KEY *ec = key;
    const char *type_label = NULL;
    unsigned char *priv = NULL, *pub = NULL;
    size_t priv_len = 0, pub_len = 0;
    const EC_GROUP *group;
    int ret = 0;

    if (out == NULL || ec == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if ((group = EC_KEY_get0_group(ec)) == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY);
        return 0;
    }

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0)
        type_label = "Private-Key";
    else if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0)
        type_label = "Public-Key";
    else if ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) != 0)
        if (EC_GROUP_get_curve_name(group) != NID_sm2)
            type_label = "EC-Parameters";

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0) {
        const BIGNUM *priv_key = EC_KEY_get0_private_key(ec);

        if (priv_key == NULL) {
            ERR_raise(ERR_LIB_PROV, PROV_R_NOT_A_PRIVATE_KEY);
            goto err;
        }
        priv_len = EC_KEY_priv2buf(ec, &priv);
        if (priv_len == 0)
            goto err;
    }
    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
        const EC_POINT *pub_pt = EC_KEY_get0_public_key(ec);

        if (pub_pt == NULL) {
            ERR_raise(ERR_LIB_PROV, PROV_R_NOT_A_PUBLIC_KEY);
            goto err;
        }

        pub_len = EC_KEY_key2buf(ec, EC_KEY_get_conv_form(ec), &pub, NULL);
        if (pub_len == 0)
            goto err;
    }

    if (type_label != NULL
        && BIO_printf(out, "%s: (%d bit)\n", type_label,
                      EC_GROUP_order_bits(group)) <= 0)
        goto err;
    if (priv != NULL
        && !ossl_bio_print_labeled_buf(out, "priv:", priv, priv_len))
        goto err;
    if (pub != NULL
        && !ossl_bio_print_labeled_buf(out, "pub:", pub, pub_len))
        goto err;
    if ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) != 0)
        ret = ec_param_to_text(out, group, ossl_ec_key_get_libctx(ec));
err:
    OPENSSL_clear_free(priv, priv_len);
    OPENSSL_free(pub);
    return ret;
}
#endif

/* ---------------------------------------------------------------------- */

#ifndef OPENSSL_NO_ECX
static int ecx_to_text(BIO *out, const void *key, int selection)
{
    const ECX_KEY *ecx = key;
    const char *type_label = NULL;

    if (out == NULL || ecx == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    switch (ecx->type) {
    case ECX_KEY_TYPE_X25519:
        type_label = "X25519";
        break;
    case ECX_KEY_TYPE_X448:
        type_label = "X448";
        break;
    case ECX_KEY_TYPE_ED25519:
        type_label = "ED25519";
        break;
    case ECX_KEY_TYPE_ED448:
        type_label = "ED448";
        break;
    }

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0) {
        if (ecx->privkey == NULL) {
            ERR_raise(ERR_LIB_PROV, PROV_R_NOT_A_PRIVATE_KEY);
            return 0;
        }

        if (BIO_printf(out, "%s Private-Key:\n", type_label) <= 0)
            return 0;
        if (!ossl_bio_print_labeled_buf(out, "priv:", ecx->privkey, ecx->keylen))
            return 0;
    } else if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0) {
        /* ecx->pubkey is an array, not a pointer... */
        if (!ecx->haspubkey) {
            ERR_raise(ERR_LIB_PROV, PROV_R_NOT_A_PUBLIC_KEY);
            return 0;
        }

        if (BIO_printf(out, "%s Public-Key:\n", type_label) <= 0)
            return 0;
    }

    if (!ossl_bio_print_labeled_buf(out, "pub:", ecx->pubkey, ecx->keylen))
        return 0;

    return 1;
}
#endif

/* ---------------------------------------------------------------------- */

#ifndef OPENSSL_NO_ML_KEM
static int ml_kem_to_text(BIO *out, const void *vkey, int selection)
{
    return ossl_ml_kem_key_to_text(out, (ML_KEM_KEY *)vkey, selection);
}
#endif

/* ---------------------------------------------------------------------- */

#ifndef OPENSSL_NO_SLH_DSA
static int slh_dsa_to_text(BIO *out, const void *key, int selection)
{
    return ossl_slh_dsa_key_to_text(out, (SLH_DSA_KEY *)key, selection);
}
#endif /* OPENSSL_NO_SLH_DSA */

static int rsa_to_text(BIO *out, const void *key, int selection)
{
    const RSA *rsa = key;
    const char *type_label = "RSA key";
    const char *modulus_label = NULL;
    const char *exponent_label = NULL;
    const BIGNUM *rsa_d = NULL, *rsa_n = NULL, *rsa_e = NULL;
    STACK_OF(BIGNUM_const) *factors = NULL;
    STACK_OF(BIGNUM_const) *exps = NULL;
    STACK_OF(BIGNUM_const) *coeffs = NULL;
    int primes;
    const RSA_PSS_PARAMS_30 *pss_params = ossl_rsa_get0_pss_params_30((RSA *)rsa);
    int ret = 0;

    if (out == NULL || rsa == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_NULL_PARAMETER);
        goto err;
    }

    factors = sk_BIGNUM_const_new_null();
    exps = sk_BIGNUM_const_new_null();
    coeffs = sk_BIGNUM_const_new_null();

    if (factors == NULL || exps == NULL || coeffs == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_CRYPTO_LIB);
        goto err;
    }

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0) {
        type_label = "Private-Key";
        modulus_label = "modulus:";
        exponent_label = "publicExponent:";
    } else if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0) {
        type_label = "Public-Key";
        modulus_label = "Modulus:";
        exponent_label = "Exponent:";
    }

    RSA_get0_key(rsa, &rsa_n, &rsa_e, &rsa_d);
    ossl_rsa_get0_all_params((RSA *)rsa, factors, exps, coeffs);
    primes = sk_BIGNUM_const_num(factors);

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0) {
        if (BIO_printf(out, "%s: (%d bit, %d primes)\n",
                       type_label, BN_num_bits(rsa_n), primes) <= 0)
            goto err;
    } else {
        if (BIO_printf(out, "%s: (%d bit)\n",
                       type_label, BN_num_bits(rsa_n)) <= 0)
            goto err;
    }

    if (!ossl_bio_print_labeled_bignum(out, modulus_label, rsa_n))
        goto err;
    if (!ossl_bio_print_labeled_bignum(out, exponent_label, rsa_e))
        goto err;
    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0) {
        int i;

        if (!ossl_bio_print_labeled_bignum(out, "privateExponent:", rsa_d))
            goto err;
        if (!ossl_bio_print_labeled_bignum(out, "prime1:",
                                           sk_BIGNUM_const_value(factors, 0)))
            goto err;
        if (!ossl_bio_print_labeled_bignum(out, "prime2:",
                                           sk_BIGNUM_const_value(factors, 1)))
            goto err;
        if (!ossl_bio_print_labeled_bignum(out, "exponent1:",
                                           sk_BIGNUM_const_value(exps, 0)))
            goto err;
        if (!ossl_bio_print_labeled_bignum(out, "exponent2:",
                                           sk_BIGNUM_const_value(exps, 1)))
            goto err;
        if (!ossl_bio_print_labeled_bignum(out, "coefficient:",
                                           sk_BIGNUM_const_value(coeffs, 0)))
            goto err;
        for (i = 2; i < sk_BIGNUM_const_num(factors); i++) {
            if (BIO_printf(out, "prime%d:", i + 1) <= 0)
                goto err;
            if (!ossl_bio_print_labeled_bignum(out, NULL,
                                               sk_BIGNUM_const_value(factors, i)))
                goto err;
            if (BIO_printf(out, "exponent%d:", i + 1) <= 0)
                goto err;
            if (!ossl_bio_print_labeled_bignum(out, NULL,
                                               sk_BIGNUM_const_value(exps, i)))
                goto err;
            if (BIO_printf(out, "coefficient%d:", i + 1) <= 0)
                goto err;
            if (!ossl_bio_print_labeled_bignum(out, NULL,
                                               sk_BIGNUM_const_value(coeffs, i - 1)))
                goto err;
        }
    }

    if ((selection & OSSL_KEYMGMT_SELECT_OTHER_PARAMETERS) != 0) {
        switch (RSA_test_flags(rsa, RSA_FLAG_TYPE_MASK)) {
        case RSA_FLAG_TYPE_RSA:
            if (!ossl_rsa_pss_params_30_is_unrestricted(pss_params)) {
                if (BIO_printf(out, "(INVALID PSS PARAMETERS)\n") <= 0)
                    goto err;
            }
            break;
        case RSA_FLAG_TYPE_RSASSAPSS:
            if (ossl_rsa_pss_params_30_is_unrestricted(pss_params)) {
                if (BIO_printf(out, "No PSS parameter restrictions\n") <= 0)
                    goto err;
            } else {
                int hashalg_nid = ossl_rsa_pss_params_30_hashalg(pss_params);
                int maskgenalg_nid =
                    ossl_rsa_pss_params_30_maskgenalg(pss_params);
                int maskgenhashalg_nid =
                    ossl_rsa_pss_params_30_maskgenhashalg(pss_params);
                int saltlen = ossl_rsa_pss_params_30_saltlen(pss_params);
                int trailerfield =
                    ossl_rsa_pss_params_30_trailerfield(pss_params);

                if (BIO_printf(out, "PSS parameter restrictions:\n") <= 0)
                    goto err;
                if (BIO_printf(out, "  Hash Algorithm: %s%s\n",
                               ossl_rsa_oaeppss_nid2name(hashalg_nid),
                               (hashalg_nid == NID_sha1
                                ? " (default)" : "")) <= 0)
                    goto err;
                if (BIO_printf(out, "  Mask Algorithm: %s with %s%s\n",
                               ossl_rsa_mgf_nid2name(maskgenalg_nid),
                               ossl_rsa_oaeppss_nid2name(maskgenhashalg_nid),
                               (maskgenalg_nid == NID_mgf1
                                && maskgenhashalg_nid == NID_sha1
                                ? " (default)" : "")) <= 0)
                    goto err;
                if (BIO_printf(out, "  Minimum Salt Length: %d%s\n",
                               saltlen,
                               (saltlen == 20 ? " (default)" : "")) <= 0)
                    goto err;
                if (BIO_printf(out, "  Trailer Field: 0x%x%s\n",
                               trailerfield,
                               (trailerfield == 1 ? " (default)" : "")) <= 0)
                    goto err;
            }
            break;
        }
    }

    ret = 1;
 err:
    sk_BIGNUM_const_free(factors);
    sk_BIGNUM_const_free(exps);
    sk_BIGNUM_const_free(coeffs);
    return ret;
}

/* ---------------------------------------------------------------------- */

#ifndef OPENSSL_NO_ML_DSA
static int ml_dsa_to_text(BIO *out, const void *key, int selection)
{
    return ossl_ml_dsa_key_to_text(out, (ML_DSA_KEY *)key, selection);
}
#endif /* OPENSSL_NO_ML_DSA */
/* ---------------------------------------------------------------------- */

static void *key2text_newctx(void *provctx)
{
    return provctx;
}

static void key2text_freectx(ossl_unused void *vctx)
{
}

static int key2text_encode(void *vctx, const void *key, int selection,
                           OSSL_CORE_BIO *cout,
                           int (*key2text)(BIO *out, const void *key,
                                           int selection),
                           OSSL_PASSPHRASE_CALLBACK *cb, void *cbarg)
{
    BIO *out = ossl_bio_new_from_core_bio(vctx, cout);
    int ret;

    if (out == NULL)
        return 0;

    ret = key2text(out, key, selection);
    BIO_free(out);

    return ret;
}

#define MAKE_TEXT_ENCODER(impl, type)                                   \
    static OSSL_FUNC_encoder_import_object_fn                           \
    impl##2text_import_object;                                          \
    static OSSL_FUNC_encoder_free_object_fn                             \
    impl##2text_free_object;                                            \
    static OSSL_FUNC_encoder_encode_fn impl##2text_encode;              \
                                                                        \
    static void *impl##2text_import_object(void *ctx, int selection,    \
                                           const OSSL_PARAM params[])   \
    {                                                                   \
        return ossl_prov_import_key(ossl_##impl##_keymgmt_functions,    \
                                    ctx, selection, params);            \
    }                                                                   \
    static void impl##2text_free_object(void *key)                      \
    {                                                                   \
        ossl_prov_free_key(ossl_##impl##_keymgmt_functions, key);       \
    }                                                                   \
    static int impl##2text_encode(void *vctx, OSSL_CORE_BIO *cout,      \
                                  const void *key,                      \
                                  const OSSL_PARAM key_abstract[],      \
                                  int selection,                        \
                                  OSSL_PASSPHRASE_CALLBACK *cb,         \
                                  void *cbarg)                          \
    {                                                                   \
        /* We don't deal with abstract objects */                       \
        if (key_abstract != NULL) {                                     \
            ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_INVALID_ARGUMENT);     \
            return 0;                                                   \
        }                                                               \
        return key2text_encode(vctx, key, selection, cout,              \
                               type##_to_text, cb, cbarg);              \
    }                                                                   \
    const OSSL_DISPATCH ossl_##impl##_to_text_encoder_functions[] = {   \
        { OSSL_FUNC_ENCODER_NEWCTX,                                     \
          (void (*)(void))key2text_newctx },                            \
        { OSSL_FUNC_ENCODER_FREECTX,                                    \
          (void (*)(void))key2text_freectx },                           \
        { OSSL_FUNC_ENCODER_IMPORT_OBJECT,                              \
          (void (*)(void))impl##2text_import_object },                  \
        { OSSL_FUNC_ENCODER_FREE_OBJECT,                                \
          (void (*)(void))impl##2text_free_object },                    \
        { OSSL_FUNC_ENCODER_ENCODE,                                     \
          (void (*)(void))impl##2text_encode },                         \
        OSSL_DISPATCH_END                                               \
    }

#ifndef OPENSSL_NO_DH
MAKE_TEXT_ENCODER(dh, dh);
MAKE_TEXT_ENCODER(dhx, dh);
#endif
#ifndef OPENSSL_NO_DSA
MAKE_TEXT_ENCODER(dsa, dsa);
#endif
#ifndef OPENSSL_NO_EC
MAKE_TEXT_ENCODER(ec, ec);
# ifndef OPENSSL_NO_SM2
MAKE_TEXT_ENCODER(sm2, ec);
# endif
# ifndef OPENSSL_NO_ECX
MAKE_TEXT_ENCODER(ed25519, ecx);
MAKE_TEXT_ENCODER(ed448, ecx);
MAKE_TEXT_ENCODER(x25519, ecx);
MAKE_TEXT_ENCODER(x448, ecx);
# endif
#endif
#ifndef OPENSSL_NO_ML_KEM
MAKE_TEXT_ENCODER(ml_kem_512, ml_kem);
MAKE_TEXT_ENCODER(ml_kem_768, ml_kem);
MAKE_TEXT_ENCODER(ml_kem_1024, ml_kem);
#endif
MAKE_TEXT_ENCODER(rsa, rsa);
MAKE_TEXT_ENCODER(rsapss, rsa);

#ifndef OPENSSL_NO_ML_DSA
MAKE_TEXT_ENCODER(ml_dsa_44, ml_dsa);
MAKE_TEXT_ENCODER(ml_dsa_65, ml_dsa);
MAKE_TEXT_ENCODER(ml_dsa_87, ml_dsa);
#endif

#ifndef OPENSSL_NO_SLH_DSA
MAKE_TEXT_ENCODER(slh_dsa_sha2_128s, slh_dsa);
MAKE_TEXT_ENCODER(slh_dsa_sha2_128f, slh_dsa);
MAKE_TEXT_ENCODER(slh_dsa_sha2_192s, slh_dsa);
MAKE_TEXT_ENCODER(slh_dsa_sha2_192f, slh_dsa);
MAKE_TEXT_ENCODER(slh_dsa_sha2_256s, slh_dsa);
MAKE_TEXT_ENCODER(slh_dsa_sha2_256f, slh_dsa);
MAKE_TEXT_ENCODER(slh_dsa_shake_128s, slh_dsa);
MAKE_TEXT_ENCODER(slh_dsa_shake_128f, slh_dsa);
MAKE_TEXT_ENCODER(slh_dsa_shake_192s, slh_dsa);
MAKE_TEXT_ENCODER(slh_dsa_shake_192f, slh_dsa);
MAKE_TEXT_ENCODER(slh_dsa_shake_256s, slh_dsa);
MAKE_TEXT_ENCODER(slh_dsa_shake_256f, slh_dsa);
#endif
