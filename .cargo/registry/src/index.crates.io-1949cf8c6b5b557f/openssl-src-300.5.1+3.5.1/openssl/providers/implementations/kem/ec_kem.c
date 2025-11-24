/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * The following implementation is part of RFC 9180 related to DHKEM using
 * EC keys (i.e. P-256, P-384 and P-521)
 * References to Sections in the comments below refer to RFC 9180.
 */

#include "internal/deprecated.h"

#include <openssl/crypto.h>
#include <openssl/evp.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/ec.h>
#include <openssl/params.h>
#include <openssl/err.h>
#include <openssl/proverr.h>
#include <openssl/kdf.h>
#include <openssl/rand.h>
#include "prov/provider_ctx.h"
#include "prov/implementations.h"
#include "prov/securitycheck.h"
#include "prov/providercommon.h"

#include <openssl/hpke.h>
#include "internal/hpke_util.h"
#include "crypto/ec.h"
#include "prov/ecx.h"
#include "eckem.h"

typedef struct {
    EC_KEY *recipient_key;
    EC_KEY *sender_authkey;
    OSSL_LIB_CTX *libctx;
    char *propq;
    unsigned int mode;
    unsigned int op;
    unsigned char *ikm;
    size_t ikmlen;
    const char *kdfname;
    const OSSL_HPKE_KEM_INFO *info;
} PROV_EC_CTX;

static OSSL_FUNC_kem_newctx_fn eckem_newctx;
static OSSL_FUNC_kem_encapsulate_init_fn eckem_encapsulate_init;
static OSSL_FUNC_kem_auth_encapsulate_init_fn eckem_auth_encapsulate_init;
static OSSL_FUNC_kem_encapsulate_fn eckem_encapsulate;
static OSSL_FUNC_kem_decapsulate_init_fn eckem_decapsulate_init;
static OSSL_FUNC_kem_auth_decapsulate_init_fn eckem_auth_decapsulate_init;
static OSSL_FUNC_kem_decapsulate_fn eckem_decapsulate;
static OSSL_FUNC_kem_freectx_fn eckem_freectx;
static OSSL_FUNC_kem_set_ctx_params_fn eckem_set_ctx_params;
static OSSL_FUNC_kem_settable_ctx_params_fn eckem_settable_ctx_params;

/* ASCII: "KEM", in hex for EBCDIC compatibility */
static const char LABEL_KEM[] = "\x4b\x45\x4d";

static int eckey_check(const EC_KEY *ec, int requires_privatekey)
{
    int rv = 0;
    BN_CTX *bnctx = NULL;
    BIGNUM *rem = NULL;
    const BIGNUM *priv = EC_KEY_get0_private_key(ec);
    const EC_POINT *pub = EC_KEY_get0_public_key(ec);

    /* Keys always require a public component */
    if (pub == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_NOT_A_PUBLIC_KEY);
        return 0;
    }
    if (priv == NULL) {
        return (requires_privatekey == 0);
    } else {
        /* If there is a private key, check that is non zero (mod order) */
        const EC_GROUP *group = EC_KEY_get0_group(ec);
        const BIGNUM *order = EC_GROUP_get0_order(group);

        bnctx = BN_CTX_new_ex(ossl_ec_key_get_libctx(ec));
        rem = BN_new();

        if (order != NULL && rem != NULL && bnctx != NULL) {
             rv = BN_mod(rem, priv, order, bnctx)
                  && !BN_is_zero(rem);
        }
    }
    BN_free(rem);
    BN_CTX_free(bnctx);
    return rv;
}

/* Returns NULL if the curve is not supported */
static const char *ec_curvename_get0(const EC_KEY *ec)
{
    const EC_GROUP *group = EC_KEY_get0_group(ec);

    return EC_curve_nid2nist(EC_GROUP_get_curve_name(group));
}

/*
 * Set the recipient key, and free any existing key.
 * ec can be NULL.
 * The ec key may have only a private or public component
 * (but it must have a group).
 */
static int recipient_key_set(PROV_EC_CTX *ctx, EC_KEY *ec)
{
    EC_KEY_free(ctx->recipient_key);
    ctx->recipient_key = NULL;

    if (ec != NULL) {
        const char *curve = ec_curvename_get0(ec);

        if (curve == NULL)
            return -2;
        ctx->info = ossl_HPKE_KEM_INFO_find_curve(curve);
        if (ctx->info == NULL)
            return -2;
        if (!EC_KEY_up_ref(ec))
            return 0;
        ctx->recipient_key = ec;
        ctx->kdfname = "HKDF";
    }
    return 1;
}

/*
 * Set the senders auth key, and free any existing auth key.
 * ec can be NULL.
 */
static int sender_authkey_set(PROV_EC_CTX *ctx, EC_KEY *ec)
{
    EC_KEY_free(ctx->sender_authkey);
    ctx->sender_authkey = NULL;

    if (ec != NULL) {
        if (!EC_KEY_up_ref(ec))
            return 0;
        ctx->sender_authkey = ec;
    }
    return 1;
}

/*
 * Serializes a encoded public key buffer into a EC public key.
 * Params:
 *     in Contains the group.
 *     pubbuf The encoded public key buffer
 * Returns: The created public EC key, or NULL if there is an error.
 */
static EC_KEY *eckey_frompub(EC_KEY *in,
                             const unsigned char *pubbuf, size_t pubbuflen)
{
    EC_KEY *key;

    key = EC_KEY_new_ex(ossl_ec_key_get_libctx(in), ossl_ec_key_get0_propq(in));
    if (key == NULL)
        goto err;
    if (!EC_KEY_set_group(key, EC_KEY_get0_group(in)))
        goto err;
    if (!EC_KEY_oct2key(key, pubbuf, pubbuflen, NULL))
        goto err;
    return key;
err:
    EC_KEY_free(key);
    return NULL;
}

/*
 * Deserialises a EC public key into a encoded byte array.
 * Returns: 1 if successful or 0 otherwise.
 */
static int ecpubkey_todata(const EC_KEY *ec, unsigned char *out, size_t *outlen,
                           size_t maxoutlen)
{
    const EC_POINT *pub;
    const EC_GROUP *group;

    group = EC_KEY_get0_group(ec);
    pub = EC_KEY_get0_public_key(ec);
    *outlen = EC_POINT_point2oct(group, pub, POINT_CONVERSION_UNCOMPRESSED,
                                 out, maxoutlen, NULL);
    return *outlen != 0;
}

static void *eckem_newctx(void *provctx)
{
    PROV_EC_CTX *ctx =  OPENSSL_zalloc(sizeof(PROV_EC_CTX));

    if (ctx == NULL)
        return NULL;
    ctx->libctx = PROV_LIBCTX_OF(provctx);
    ctx->mode = KEM_MODE_DHKEM;

    return ctx;
}

static void eckem_freectx(void *vectx)
{
    PROV_EC_CTX *ctx = (PROV_EC_CTX *)vectx;

    OPENSSL_clear_free(ctx->ikm, ctx->ikmlen);
    recipient_key_set(ctx, NULL);
    sender_authkey_set(ctx, NULL);
    OPENSSL_free(ctx);
}

static int ossl_ec_match_params(const EC_KEY *key1, const EC_KEY *key2)
{
    int ret;
    BN_CTX *ctx = NULL;
    const EC_GROUP *group1 = EC_KEY_get0_group(key1);
    const EC_GROUP *group2 = EC_KEY_get0_group(key2);

    ctx = BN_CTX_new_ex(ossl_ec_key_get_libctx(key1));
    if (ctx == NULL)
        return 0;

    ret = group1 != NULL
          && group2 != NULL
          && EC_GROUP_cmp(group1, group2, ctx) == 0;
    if (!ret)
        ERR_raise(ERR_LIB_PROV, PROV_R_MISMATCHING_DOMAIN_PARAMETERS);
    BN_CTX_free(ctx);
    return ret;
}

static int eckem_init(void *vctx, int operation, void *vec, void *vauth,
                      const OSSL_PARAM params[])
{
    int rv;
    PROV_EC_CTX *ctx = (PROV_EC_CTX *)vctx;
    EC_KEY *ec = vec;
    EC_KEY *auth = vauth;

    if (!ossl_prov_is_running())
        return 0;

    if (!eckey_check(ec, operation == EVP_PKEY_OP_DECAPSULATE))
        return 0;
    rv = recipient_key_set(ctx, ec);
    if (rv <= 0)
        return rv;

    if (auth != NULL) {
        if (!ossl_ec_match_params(ec, auth)
            || !eckey_check(auth, operation == EVP_PKEY_OP_ENCAPSULATE)
            || !sender_authkey_set(ctx, auth))
        return 0;
    }

    ctx->op = operation;
    return eckem_set_ctx_params(vctx, params);
}

static int eckem_encapsulate_init(void *vctx, void *vec,
                                   const OSSL_PARAM params[])
{
    return eckem_init(vctx, EVP_PKEY_OP_ENCAPSULATE, vec, NULL, params);
}

static int eckem_decapsulate_init(void *vctx, void *vec,
                                   const OSSL_PARAM params[])
{
    return eckem_init(vctx, EVP_PKEY_OP_DECAPSULATE, vec, NULL, params);
}

static int eckem_auth_encapsulate_init(void *vctx, void *vecx, void *vauthpriv,
                                       const OSSL_PARAM params[])
{
    return eckem_init(vctx, EVP_PKEY_OP_ENCAPSULATE, vecx, vauthpriv, params);
}

static int eckem_auth_decapsulate_init(void *vctx, void *vecx, void *vauthpub,
                                       const OSSL_PARAM params[])
{
    return eckem_init(vctx, EVP_PKEY_OP_DECAPSULATE, vecx, vauthpub, params);
}

static int eckem_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    PROV_EC_CTX *ctx = (PROV_EC_CTX *)vctx;
    const OSSL_PARAM *p;
    int mode;

    if (ossl_param_is_empty(params))
        return 1;

    p = OSSL_PARAM_locate_const(params, OSSL_KEM_PARAM_IKME);
    if (p != NULL) {
        void *tmp = NULL;
        size_t tmplen = 0;

        if (p->data != NULL && p->data_size != 0) {
            if (!OSSL_PARAM_get_octet_string(p, &tmp, 0, &tmplen))
                return 0;
        }
        OPENSSL_clear_free(ctx->ikm, ctx->ikmlen);
        /* Set the ephemeral seed */
        ctx->ikm = tmp;
        ctx->ikmlen = tmplen;
    }

    p = OSSL_PARAM_locate_const(params, OSSL_KEM_PARAM_OPERATION);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_UTF8_STRING)
            return 0;
        mode = ossl_eckem_modename2id(p->data);
        if (mode == KEM_MODE_UNDEFINED)
            return 0;
        ctx->mode = mode;
    }
    return 1;
}

static const OSSL_PARAM known_settable_eckem_ctx_params[] = {
    OSSL_PARAM_utf8_string(OSSL_KEM_PARAM_OPERATION, NULL, 0),
    OSSL_PARAM_octet_string(OSSL_KEM_PARAM_IKME, NULL, 0),
    OSSL_PARAM_END
};

static const OSSL_PARAM *eckem_settable_ctx_params(ossl_unused void *vctx,
                                                   ossl_unused void *provctx)
{
    return known_settable_eckem_ctx_params;
}

/*
 * See Section 4.1 DH-Based KEM (DHKEM) ExtractAndExpand
 */
static int dhkem_extract_and_expand(EVP_KDF_CTX *kctx,
                                    unsigned char *okm, size_t okmlen,
                                    uint16_t kemid,
                                    const unsigned char *dhkm, size_t dhkmlen,
                                    const unsigned char *kemctx,
                                    size_t kemctxlen)
{
    uint8_t suiteid[2];
    uint8_t prk[EVP_MAX_MD_SIZE];
    size_t prklen = okmlen;
    int ret;

    if (prklen > sizeof(prk))
        return 0;

    suiteid[0] = (kemid >> 8) & 0xff;
    suiteid[1] = kemid & 0xff;

    ret = ossl_hpke_labeled_extract(kctx, prk, prklen,
                                    NULL, 0, LABEL_KEM, suiteid, sizeof(suiteid),
                                    OSSL_DHKEM_LABEL_EAE_PRK, dhkm, dhkmlen)
          && ossl_hpke_labeled_expand(kctx, okm, okmlen, prk, prklen,
                                      LABEL_KEM, suiteid, sizeof(suiteid),
                                      OSSL_DHKEM_LABEL_SHARED_SECRET,
                                      kemctx, kemctxlen);
    OPENSSL_cleanse(prk, prklen);
    return ret;
}

/*
 * See Section 7.1.3 DeriveKeyPair.
 *
 * This function is used by ec keygen.
 * (For this reason it does not use any of the state stored in PROV_EC_CTX).
 *
 * Params:
 *     ec An initialized ec key.
 *     priv The buffer to store the generated private key into (it is assumed
 *          this is of length alg->encodedprivlen).
 *     ikm buffer containing the input key material (seed). This must be set.
 *     ikmlen size of the ikm buffer in bytes
 * Returns:
 *     1 if successful or 0 otherwise.
 */
int ossl_ec_dhkem_derive_private(EC_KEY *ec, BIGNUM *priv,
                                 const unsigned char *ikm, size_t ikmlen)
{
    int ret = 0;
    EVP_KDF_CTX *kdfctx = NULL;
    uint8_t suiteid[2];
    unsigned char prk[OSSL_HPKE_MAX_SECRET];
    unsigned char privbuf[OSSL_HPKE_MAX_PRIVATE];
    const BIGNUM *order;
    unsigned char counter = 0;
    const char *curve = ec_curvename_get0(ec);
    const OSSL_HPKE_KEM_INFO *info;

    if (curve == NULL)
        return -2;

    info = ossl_HPKE_KEM_INFO_find_curve(curve);
    if (info == NULL)
        return -2;

    kdfctx = ossl_kdf_ctx_create("HKDF", info->mdname,
                                 ossl_ec_key_get_libctx(ec),
                                 ossl_ec_key_get0_propq(ec));
    if (kdfctx == NULL)
        return 0;

    /* ikmlen should have a length of at least Nsk */
    if (ikmlen < info->Nsk) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_INPUT_LENGTH,
                       "ikm length is :%zu, should be at least %zu",
                       ikmlen, info->Nsk);
        goto err;
    }

    suiteid[0] = info->kem_id / 256;
    suiteid[1] = info->kem_id % 256;

    if (!ossl_hpke_labeled_extract(kdfctx, prk, info->Nsecret,
                                   NULL, 0, LABEL_KEM, suiteid, sizeof(suiteid),
                                   OSSL_DHKEM_LABEL_DKP_PRK, ikm, ikmlen))
        goto err;

    order = EC_GROUP_get0_order(EC_KEY_get0_group(ec));
    do {
        if (!ossl_hpke_labeled_expand(kdfctx, privbuf, info->Nsk,
                                      prk, info->Nsecret,
                                      LABEL_KEM, suiteid, sizeof(suiteid),
                                      OSSL_DHKEM_LABEL_CANDIDATE,
                                      &counter, 1))
            goto err;
        privbuf[0] &= info->bitmask;
        if (BN_bin2bn(privbuf, info->Nsk, priv) == NULL)
            goto err;
        if (counter == 0xFF) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GENERATE_KEY);
            goto err;
        }
        counter++;
    } while (BN_is_zero(priv) || BN_cmp(priv, order) >= 0);
    ret = 1;
err:
    OPENSSL_cleanse(prk, sizeof(prk));
    OPENSSL_cleanse(privbuf, sizeof(privbuf));
    EVP_KDF_CTX_free(kdfctx);
    return ret;
}

/*
 * Do a keygen operation without having to use EVP_PKEY.
 * Params:
 *     ctx Context object
 *     ikm The seed material - if this is NULL, then a random seed is used.
 * Returns:
 *     The generated EC key, or NULL on failure.
 */
static EC_KEY *derivekey(PROV_EC_CTX *ctx,
                         const unsigned char *ikm, size_t ikmlen)
{
    int ret = 0;
    EC_KEY *key;
    unsigned char *seed = (unsigned char *)ikm;
    size_t seedlen = ikmlen;
    unsigned char tmpbuf[OSSL_HPKE_MAX_PRIVATE];

    key = EC_KEY_new_ex(ctx->libctx, ctx->propq);
    if (key == NULL)
        goto err;
    if (!EC_KEY_set_group(key, EC_KEY_get0_group(ctx->recipient_key)))
        goto err;

    /* Generate a random seed if there is no input ikm */
    if (seed == NULL || seedlen == 0) {
        seedlen = ctx->info->Nsk;
        if (seedlen > sizeof(tmpbuf))
            goto err;
        if (RAND_priv_bytes_ex(ctx->libctx, tmpbuf, seedlen, 0) <= 0)
            goto err;
        seed = tmpbuf;
    }
    ret = ossl_ec_generate_key_dhkem(key, seed, seedlen);
err:
    if (seed != ikm)
        OPENSSL_cleanse(seed, seedlen);
    if (ret <= 0) {
        EC_KEY_free(key);
        key = NULL;
    }
    return key;
}

/*
 * Before doing a key exchange the public key of the peer needs to be checked
 * Note that the group check is not done here as we have already checked
 * that it only uses one of the approved curve names when the key was set.
 *
 * Returns 1 if the public key is valid, or 0 if it fails.
 */
static int check_publickey(const EC_KEY *pub)
{
    int ret = 0;
    BN_CTX *bnctx = BN_CTX_new_ex(ossl_ec_key_get_libctx(pub));

    if (bnctx == NULL)
        return 0;
    ret = ossl_ec_key_public_check(pub, bnctx);
    BN_CTX_free(bnctx);

    return ret;
}

/*
 * Do an ecdh key exchange.
 * dhkm = DH(sender, peer)
 *
 * NOTE: Instead of using EVP_PKEY_derive() API's, we use EC_KEY operations
 *       to avoid messy conversions back to EVP_PKEY.
 *
 * Returns the size of the secret if successful, or 0 otherwise,
 */
static int generate_ecdhkm(const EC_KEY *sender, const EC_KEY *peer,
                           unsigned char *out, size_t maxout,
                           unsigned int secretsz)
{
    const EC_GROUP *group = EC_KEY_get0_group(sender);
    size_t secretlen = (EC_GROUP_get_degree(group) + 7) / 8;

    if (secretlen != secretsz || secretlen > maxout) {
        ERR_raise_data(ERR_LIB_PROV,  PROV_R_BAD_LENGTH, "secretsz invalid");
        return 0;
    }

    if (!check_publickey(peer))
        return 0;
    return ECDH_compute_key(out, secretlen, EC_KEY_get0_public_key(peer),
                            sender, NULL) > 0;
}

/*
 * Derive a secret using ECDH (code is shared by the encap and decap)
 *
 * dhkm = Concat(ecdh(privkey1, peerkey1), ecdh(privkey2, peerkey2)
 * kemctx = Concat(sender_pub, recipient_pub, ctx->sender_authkey)
 * secret = dhkem_extract_and_expand(kemid, dhkm, kemctx);
 *
 * Params:
 *     ctx Object that contains algorithm state and constants.
 *     secret The returned secret (with a length ctx->alg->secretlen bytes).
 *     privkey1 A private key used for ECDH key derivation.
 *     peerkey1 A public key used for ECDH key derivation with privkey1
 *     privkey2 A optional private key used for a second ECDH key derivation.
 *              It can be NULL.
 *     peerkey2 A optional public key used for a second ECDH key derivation
 *              with privkey2,. It can be NULL.
 *     sender_pub The senders public key in encoded form.
 *     recipient_pub The recipients public key in encoded form.
 * Notes:
 *     The second ecdh() is only used for the HPKE auth modes when both privkey2
 *     and peerkey2 are non NULL (i.e. ctx->sender_authkey is not NULL).
 */
static int derive_secret(PROV_EC_CTX *ctx, unsigned char *secret,
                         const EC_KEY *privkey1, const EC_KEY *peerkey1,
                         const EC_KEY *privkey2, const EC_KEY *peerkey2,
                         const unsigned char *sender_pub,
                         const unsigned char *recipient_pub)
{
    int ret = 0;
    EVP_KDF_CTX *kdfctx = NULL;
    unsigned char sender_authpub[OSSL_HPKE_MAX_PUBLIC];
    unsigned char dhkm[OSSL_HPKE_MAX_PRIVATE * 2];
    unsigned char kemctx[OSSL_HPKE_MAX_PUBLIC * 3];
    size_t sender_authpublen;
    size_t kemctxlen = 0, dhkmlen = 0;
    const OSSL_HPKE_KEM_INFO *info = ctx->info;
    size_t encodedpublen = info->Npk;
    size_t encodedprivlen = info->Nsk;
    int auth = ctx->sender_authkey != NULL;

    if (!generate_ecdhkm(privkey1, peerkey1, dhkm, sizeof(dhkm), encodedprivlen))
        goto err;
    dhkmlen = encodedprivlen;
    kemctxlen = 2 * encodedpublen;

    /* Concat the optional second ECDH (used for Auth) */
    if (auth) {
        /* Get the public key of the auth sender in encoded form */
        if (!ecpubkey_todata(ctx->sender_authkey, sender_authpub,
                             &sender_authpublen, sizeof(sender_authpub)))
            goto err;
        if (sender_authpublen != encodedpublen) {
            ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_KEY,
                           "Invalid sender auth public key");
            goto err;
        }
        if (!generate_ecdhkm(privkey2, peerkey2,
                             dhkm + dhkmlen, sizeof(dhkm) - dhkmlen,
                             encodedprivlen))
            goto err;
        dhkmlen += encodedprivlen;
        kemctxlen += encodedpublen;
    }
    if (kemctxlen > sizeof(kemctx))
        goto err;

    /* kemctx is the concat of both sides encoded public key */
    memcpy(kemctx, sender_pub, info->Npk);
    memcpy(kemctx + info->Npk, recipient_pub, info->Npk);
    if (auth)
        memcpy(kemctx + 2 * encodedpublen, sender_authpub, encodedpublen);
    kdfctx = ossl_kdf_ctx_create(ctx->kdfname, info->mdname,
                                 ctx->libctx, ctx->propq);
    if (kdfctx == NULL)
        goto err;
    if (!dhkem_extract_and_expand(kdfctx, secret, info->Nsecret,
                                  info->kem_id, dhkm, dhkmlen,
                                  kemctx, kemctxlen))
        goto err;
    ret = 1;
err:
    OPENSSL_cleanse(dhkm, dhkmlen);
    EVP_KDF_CTX_free(kdfctx);
    return ret;
}

/*
 * Do a DHKEM encapsulate operation.
 *
 * See Section 4.1 Encap() and AuthEncap()
 *
 * Params:
 *     ctx A context object holding the recipients public key and the
 *         optional senders auth private key.
 *     enc A buffer to return the senders ephemeral public key.
 *         Setting this to NULL allows the enclen and secretlen to return
 *         values, without calculating the secret.
 *     enclen Passes in the max size of the enc buffer and returns the
 *            encoded public key length.
 *     secret A buffer to return the calculated shared secret.
 *     secretlen Passes in the max size of the secret buffer and returns the
 *               secret length.
 * Returns: 1 on success or 0 otherwise.
 */
static int dhkem_encap(PROV_EC_CTX *ctx,
                       unsigned char *enc, size_t *enclen,
                       unsigned char *secret, size_t *secretlen)
{
    int ret = 0;
    EC_KEY *sender_ephemkey = NULL;
    unsigned char sender_pub[OSSL_HPKE_MAX_PUBLIC];
    unsigned char recipient_pub[OSSL_HPKE_MAX_PUBLIC];
    size_t sender_publen, recipient_publen;
    const OSSL_HPKE_KEM_INFO *info = ctx->info;

    if (enc == NULL) {
        if (enclen == NULL && secretlen == NULL)
            return 0;
        if (enclen != NULL)
            *enclen = info->Nenc;
        if (secretlen != NULL)
            *secretlen = info->Nsecret;
       return 1;
    }

    if (*secretlen < info->Nsecret) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_BAD_LENGTH, "*secretlen too small");
        return 0;
    }
    if (*enclen < info->Nenc) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_BAD_LENGTH, "*enclen too small");
        return 0;
    }

    /* Create an ephemeral key */
    sender_ephemkey = derivekey(ctx, ctx->ikm, ctx->ikmlen);
    if (sender_ephemkey == NULL)
        goto err;
    if (!ecpubkey_todata(sender_ephemkey, sender_pub, &sender_publen,
                         sizeof(sender_pub))
            || !ecpubkey_todata(ctx->recipient_key, recipient_pub,
                                &recipient_publen, sizeof(recipient_pub)))
        goto err;

    if (sender_publen != info->Npk
            || recipient_publen != sender_publen) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_KEY, "Invalid public key");
        goto err;
    }

    if (!derive_secret(ctx, secret,
                       sender_ephemkey, ctx->recipient_key,
                       ctx->sender_authkey, ctx->recipient_key,
                       sender_pub, recipient_pub))
        goto err;

    /* Return the senders ephemeral public key in encoded form */
    memcpy(enc, sender_pub, sender_publen);
    *enclen = sender_publen;
    *secretlen = info->Nsecret;
    ret = 1;
err:
    EC_KEY_free(sender_ephemkey);
    return ret;
}

/*
 * Do a DHKEM decapsulate operation.
 * See Section 4.1 Decap() and Auth Decap()
 *
 * Params:
 *     ctx A context object holding the recipients private key and the
 *         optional senders auth public key.
 *     secret A buffer to return the calculated shared secret. Setting this to
 *            NULL can be used to return the secretlen.
 *     secretlen Passes in the max size of the secret buffer and returns the
 *               secret length.
 *     enc A buffer containing the senders ephemeral public key that was returned
 *         from dhkem_encap().
 *     enclen The length in bytes of enc.
 * Returns: 1 If the shared secret is returned or 0 on error.
 */
static int dhkem_decap(PROV_EC_CTX *ctx,
                       unsigned char *secret, size_t *secretlen,
                       const unsigned char *enc, size_t enclen)
{
    int ret = 0;
    EC_KEY *sender_ephempubkey = NULL;
    const OSSL_HPKE_KEM_INFO *info = ctx->info;
    unsigned char recipient_pub[OSSL_HPKE_MAX_PUBLIC];
    size_t recipient_publen;
    size_t encodedpublen = info->Npk;

    if (secret == NULL) {
        *secretlen = info->Nsecret;
        return 1;
    }

    if (*secretlen < info->Nsecret) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_BAD_LENGTH, "*secretlen too small");
        return 0;
    }
    if (enclen != encodedpublen) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_KEY, "Invalid enc public key");
        return 0;
    }

    sender_ephempubkey = eckey_frompub(ctx->recipient_key, enc, enclen);
    if (sender_ephempubkey == NULL)
        goto err;
    if (!ecpubkey_todata(ctx->recipient_key, recipient_pub, &recipient_publen,
                         sizeof(recipient_pub)))
        goto err;
    if (recipient_publen != encodedpublen) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_KEY, "Invalid recipient public key");
        goto err;
    }

    if (!derive_secret(ctx, secret,
                       ctx->recipient_key, sender_ephempubkey,
                       ctx->recipient_key, ctx->sender_authkey,
                       enc, recipient_pub))
        goto err;
    *secretlen = info->Nsecret;
    ret = 1;
err:
    EC_KEY_free(sender_ephempubkey);
    return ret;
}

static int eckem_encapsulate(void *vctx, unsigned char *out, size_t *outlen,
                             unsigned char *secret, size_t *secretlen)
{
    PROV_EC_CTX *ctx = (PROV_EC_CTX *)vctx;

    switch (ctx->mode) {
        case KEM_MODE_DHKEM:
            return dhkem_encap(ctx, out, outlen, secret, secretlen);
        default:
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_MODE);
            return -2;
    }
}

static int eckem_decapsulate(void *vctx, unsigned char *out, size_t *outlen,
                             const unsigned char *in, size_t inlen)
{
    PROV_EC_CTX *ctx = (PROV_EC_CTX *)vctx;

    switch (ctx->mode) {
        case KEM_MODE_DHKEM:
            return dhkem_decap(ctx, out, outlen, in, inlen);
        default:
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_MODE);
            return -2;
    }
}

const OSSL_DISPATCH ossl_ec_asym_kem_functions[] = {
    { OSSL_FUNC_KEM_NEWCTX, (void (*)(void))eckem_newctx },
    { OSSL_FUNC_KEM_ENCAPSULATE_INIT,
      (void (*)(void))eckem_encapsulate_init },
    { OSSL_FUNC_KEM_ENCAPSULATE, (void (*)(void))eckem_encapsulate },
    { OSSL_FUNC_KEM_DECAPSULATE_INIT,
      (void (*)(void))eckem_decapsulate_init },
    { OSSL_FUNC_KEM_DECAPSULATE, (void (*)(void))eckem_decapsulate },
    { OSSL_FUNC_KEM_FREECTX, (void (*)(void))eckem_freectx },
    { OSSL_FUNC_KEM_SET_CTX_PARAMS,
      (void (*)(void))eckem_set_ctx_params },
    { OSSL_FUNC_KEM_SETTABLE_CTX_PARAMS,
      (void (*)(void))eckem_settable_ctx_params },
    { OSSL_FUNC_KEM_AUTH_ENCAPSULATE_INIT,
      (void (*)(void))eckem_auth_encapsulate_init },
    { OSSL_FUNC_KEM_AUTH_DECAPSULATE_INIT,
      (void (*)(void))eckem_auth_decapsulate_init },
    OSSL_DISPATCH_END
};
