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
 * ECX keys (i.e. X25519 and X448)
 * References to Sections in the comments below refer to RFC 9180.
 */

#include "internal/deprecated.h"

#include <string.h>
#include <openssl/crypto.h>
#include <openssl/evp.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/params.h>
#include <openssl/kdf.h>
#include <openssl/err.h>
#include <openssl/sha.h>
#include <openssl/rand.h>
#include <openssl/proverr.h>
#include "prov/provider_ctx.h"
#include "prov/implementations.h"
#include "prov/securitycheck.h"
#include "prov/providercommon.h"
#include "prov/ecx.h"
#include "crypto/ecx.h"
#include <openssl/hpke.h>
#include "internal/hpke_util.h"
#include "eckem.h"

#define MAX_ECX_KEYLEN X448_KEYLEN

/* KEM identifiers from Section 7.1 "Table 2 KEM IDs" */
#define KEMID_X25519_HKDF_SHA256 0x20
#define KEMID_X448_HKDF_SHA512   0x21

/* ASCII: "KEM", in hex for EBCDIC compatibility */
static const char LABEL_KEM[] = "\x4b\x45\x4d";

typedef struct {
    ECX_KEY *recipient_key;
    ECX_KEY *sender_authkey;
    OSSL_LIB_CTX *libctx;
    char *propq;
    unsigned int mode;
    unsigned int op;
    unsigned char *ikm;
    size_t ikmlen;
    const char *kdfname;
    const OSSL_HPKE_KEM_INFO *info;
} PROV_ECX_CTX;

static OSSL_FUNC_kem_newctx_fn ecxkem_newctx;
static OSSL_FUNC_kem_encapsulate_init_fn ecxkem_encapsulate_init;
static OSSL_FUNC_kem_encapsulate_fn ecxkem_encapsulate;
static OSSL_FUNC_kem_decapsulate_init_fn ecxkem_decapsulate_init;
static OSSL_FUNC_kem_decapsulate_fn ecxkem_decapsulate;
static OSSL_FUNC_kem_freectx_fn ecxkem_freectx;
static OSSL_FUNC_kem_set_ctx_params_fn ecxkem_set_ctx_params;
static OSSL_FUNC_kem_auth_encapsulate_init_fn ecxkem_auth_encapsulate_init;
static OSSL_FUNC_kem_auth_decapsulate_init_fn ecxkem_auth_decapsulate_init;

/*
 * Set KEM values as specified in Section 7.1 "Table 2 KEM IDs"
 * There is only one set of values for X25519 and X448.
 * Additional values could be set via set_params if required.
 */
static const OSSL_HPKE_KEM_INFO *get_kem_info(ECX_KEY *ecx)
{
    const char *name = NULL;

    if (ecx->type == ECX_KEY_TYPE_X25519)
        name = SN_X25519;
    else
        name = SN_X448;
    return ossl_HPKE_KEM_INFO_find_curve(name);
}

/*
 * Set the recipient key, and free any existing key.
 * ecx can be NULL. The ecx key may have only a private or public component.
 */
static int recipient_key_set(PROV_ECX_CTX *ctx, ECX_KEY *ecx)
{
    ossl_ecx_key_free(ctx->recipient_key);
    ctx->recipient_key = NULL;
    if (ecx != NULL) {
        ctx->info = get_kem_info(ecx);
        if (ctx->info == NULL)
            return -2;
        ctx->kdfname = "HKDF";
        if (!ossl_ecx_key_up_ref(ecx))
            return 0;
        ctx->recipient_key = ecx;
    }
    return 1;
}

/*
 * Set the senders auth key, and free any existing auth key.
 * ecx can be NULL.
 */
static int sender_authkey_set(PROV_ECX_CTX *ctx, ECX_KEY *ecx)
{
    ossl_ecx_key_free(ctx->sender_authkey);
    ctx->sender_authkey = NULL;

    if (ecx != NULL) {
        if (!ossl_ecx_key_up_ref(ecx))
            return 0;
        ctx->sender_authkey = ecx;
    }
    return 1;
}

/*
 * Serialize a public key from byte array's for the encoded public keys.
 * ctx is used to access the key type.
 * Returns: The created ECX_KEY or NULL on error.
 */
static ECX_KEY *ecxkey_pubfromdata(PROV_ECX_CTX *ctx,
                                   const unsigned char *pubbuf, size_t pubbuflen)
{
    ECX_KEY *ecx = NULL;
    OSSL_PARAM params[2], *p = params;

    *p++ = OSSL_PARAM_construct_octet_string(OSSL_PKEY_PARAM_PUB_KEY,
                                             (char *)pubbuf, pubbuflen);
    *p = OSSL_PARAM_construct_end();

    ecx = ossl_ecx_key_new(ctx->libctx, ctx->recipient_key->type, 1, ctx->propq);
    if (ecx == NULL)
        return NULL;
    if (ossl_ecx_key_fromdata(ecx, params, 0) <= 0) {
        ossl_ecx_key_free(ecx);
        ecx = NULL;
    }
    return ecx;
}

static unsigned char *ecx_pubkey(ECX_KEY *ecx)
{
    if (ecx == NULL || !ecx->haspubkey) {
        ERR_raise(ERR_LIB_PROV, PROV_R_NOT_A_PUBLIC_KEY);
        return 0;
    }
    return ecx->pubkey;
}

static void *ecxkem_newctx(void *provctx)
{
    PROV_ECX_CTX *ctx =  OPENSSL_zalloc(sizeof(PROV_ECX_CTX));

    if (ctx == NULL)
        return NULL;
    ctx->libctx = PROV_LIBCTX_OF(provctx);
    ctx->mode = KEM_MODE_DHKEM;

    return ctx;
}

static void ecxkem_freectx(void *vectx)
{
    PROV_ECX_CTX *ctx = (PROV_ECX_CTX *)vectx;

    OPENSSL_clear_free(ctx->ikm, ctx->ikmlen);
    recipient_key_set(ctx, NULL);
    sender_authkey_set(ctx, NULL);
    OPENSSL_free(ctx);
}

static int ecx_match_params(const ECX_KEY *key1, const ECX_KEY *key2)
{
    return (key1->type == key2->type && key1->keylen == key2->keylen);
}

static int ecx_key_check(const ECX_KEY *ecx, int requires_privatekey)
{
    if (ecx->privkey == NULL)
        return (requires_privatekey == 0);
    return 1;
}

static int ecxkem_init(void *vecxctx, int operation, void *vecx, void *vauth,
                       ossl_unused const OSSL_PARAM params[])
{
    int rv;
    PROV_ECX_CTX *ctx = (PROV_ECX_CTX *)vecxctx;
    ECX_KEY *ecx = vecx;
    ECX_KEY *auth = vauth;

    if (!ossl_prov_is_running())
        return 0;

    if (!ecx_key_check(ecx, operation == EVP_PKEY_OP_DECAPSULATE))
        return 0;
    rv = recipient_key_set(ctx, ecx);
    if (rv <= 0)
        return rv;

    if (auth != NULL) {
        if (!ecx_match_params(auth, ctx->recipient_key)
                || !ecx_key_check(auth, operation == EVP_PKEY_OP_ENCAPSULATE)
                || !sender_authkey_set(ctx, auth))
            return 0;
    }

    ctx->op = operation;
    return ecxkem_set_ctx_params(vecxctx, params);
}

static int ecxkem_encapsulate_init(void *vecxctx, void *vecx,
                                   const OSSL_PARAM params[])
{
    return ecxkem_init(vecxctx, EVP_PKEY_OP_ENCAPSULATE, vecx, NULL, params);
}

static int ecxkem_decapsulate_init(void *vecxctx, void *vecx,
                                   const OSSL_PARAM params[])
{
    return ecxkem_init(vecxctx, EVP_PKEY_OP_DECAPSULATE, vecx, NULL, params);
}

static int ecxkem_auth_encapsulate_init(void *vctx, void *vecx, void *vauthpriv,
                                        const OSSL_PARAM params[])
{
    return ecxkem_init(vctx, EVP_PKEY_OP_ENCAPSULATE, vecx, vauthpriv, params);
}

static int ecxkem_auth_decapsulate_init(void *vctx, void *vecx, void *vauthpub,
                                        const OSSL_PARAM params[])
{
    return ecxkem_init(vctx, EVP_PKEY_OP_DECAPSULATE, vecx, vauthpub, params);
}

static int ecxkem_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    PROV_ECX_CTX *ctx = (PROV_ECX_CTX *)vctx;
    const OSSL_PARAM *p;
    int mode;

    if (ctx == NULL)
        return 0;
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

static const OSSL_PARAM known_settable_ecxkem_ctx_params[] = {
    OSSL_PARAM_utf8_string(OSSL_KEM_PARAM_OPERATION, NULL, 0),
    OSSL_PARAM_octet_string(OSSL_KEM_PARAM_IKME, NULL, 0),
    OSSL_PARAM_END
};

static const OSSL_PARAM *ecxkem_settable_ctx_params(ossl_unused void *vctx,
                                                   ossl_unused void *provctx)
{
    return known_settable_ecxkem_ctx_params;
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
    size_t prklen = okmlen; /* Nh */
    int ret;

    if (prklen > sizeof(prk))
        return 0;

    suiteid[0] = (kemid >> 8) &0xff;
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
 * This function is used by ecx keygen.
 * (For this reason it does not use any of the state stored in PROV_ECX_CTX).
 *
 * Params:
 *     ecx An initialized ecx key.
 *     privout The buffer to store the generated private key into (it is assumed
 *             this is of length ecx->keylen).
 *     ikm buffer containing the input key material (seed). This must be non NULL.
 *     ikmlen size of the ikm buffer in bytes
 * Returns:
 *     1 if successful or 0 otherwise.
 */
int ossl_ecx_dhkem_derive_private(ECX_KEY *ecx, unsigned char *privout,
                                  const unsigned char *ikm, size_t ikmlen)
{
    int ret = 0;
    EVP_KDF_CTX *kdfctx = NULL;
    unsigned char prk[EVP_MAX_MD_SIZE];
    uint8_t suiteid[2];
    const OSSL_HPKE_KEM_INFO *info = get_kem_info(ecx);

    /* ikmlen should have a length of at least Nsk */
    if (ikmlen < info->Nsk) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_INPUT_LENGTH,
                       "ikm length is :%zu, should be at least %zu",
                       ikmlen, info->Nsk);
        goto err;
    }

    kdfctx = ossl_kdf_ctx_create("HKDF", info->mdname, ecx->libctx, ecx->propq);
    if (kdfctx == NULL)
        return 0;

    suiteid[0] = info->kem_id / 256;
    suiteid[1] = info->kem_id % 256;

    if (!ossl_hpke_labeled_extract(kdfctx, prk, info->Nsecret,
                                   NULL, 0, LABEL_KEM, suiteid, sizeof(suiteid),
                                   OSSL_DHKEM_LABEL_DKP_PRK, ikm, ikmlen))
        goto err;

    if (!ossl_hpke_labeled_expand(kdfctx, privout, info->Nsk, prk, info->Nsecret,
                                  LABEL_KEM, suiteid, sizeof(suiteid),
                                  OSSL_DHKEM_LABEL_SK, NULL, 0))
        goto err;
    ret = 1;
err:
    OPENSSL_cleanse(prk, sizeof(prk));
    EVP_KDF_CTX_free(kdfctx);
    return ret;
}

/*
 * Do a keygen operation without having to use EVP_PKEY.
 * Params:
 *     ctx Context object
 *     ikm The seed material - if this is NULL, then a random seed is used.
 * Returns:
 *     The generated ECX key, or NULL on failure.
 */
static ECX_KEY *derivekey(PROV_ECX_CTX *ctx,
                          const unsigned char *ikm, size_t ikmlen)
{
    int ok = 0;
    ECX_KEY *key;
    unsigned char *privkey;
    unsigned char *seed = (unsigned char *)ikm;
    size_t seedlen = ikmlen;
    unsigned char tmpbuf[OSSL_HPKE_MAX_PRIVATE];
    const OSSL_HPKE_KEM_INFO *info = ctx->info;

    key = ossl_ecx_key_new(ctx->libctx, ctx->recipient_key->type, 0, ctx->propq);
    if (key == NULL)
        return NULL;
    privkey = ossl_ecx_key_allocate_privkey(key);
    if (privkey == NULL)
        goto err;

    /* Generate a random seed if there is no input ikm */
    if (seed == NULL || seedlen == 0) {
        if (info->Nsk > sizeof(tmpbuf))
            goto err;
        if (RAND_priv_bytes_ex(ctx->libctx, tmpbuf, info->Nsk, 0) <= 0)
            goto err;
        seed = tmpbuf;
        seedlen = info->Nsk;
    }
    if (!ossl_ecx_dhkem_derive_private(key, privkey, seed, seedlen))
        goto err;
    if (!ossl_ecx_public_from_private(key))
        goto err;
    key->haspubkey = 1;
    ok = 1;
err:
    if (!ok) {
        ossl_ecx_key_free(key);
        key = NULL;
    }
    if (seed != ikm)
        OPENSSL_cleanse(seed, seedlen);
    return key;
}

/*
 * Do an ecxdh key exchange.
 * dhkm = DH(sender, peer)
 *
 * NOTE: Instead of using EVP_PKEY_derive() API's, we use ECX_KEY operations
 *       to avoid messy conversions back to EVP_PKEY.
 *
 * Returns the size of the secret if successful, or 0 otherwise,
 */
static int generate_ecxdhkm(const ECX_KEY *sender, const ECX_KEY *peer,
                           unsigned char *out,  size_t maxout,
                           unsigned int secretsz)
{
    size_t len = 0;

    /* NOTE: ossl_ecx_compute_key checks for shared secret being all zeros */
    return ossl_ecx_compute_key((ECX_KEY *)peer, (ECX_KEY *)sender,
                                 sender->keylen, out, &len, maxout);
}

/*
 * Derive a secret using ECXDH (code is shared by the encap and decap)
 *
 * dhkm = Concat(ecxdh(privkey1, peerkey1), ecdh(privkey2, peerkey2)
 * kemctx = Concat(sender_pub, recipient_pub, ctx->sender_authkey)
 * secret = dhkem_extract_and_expand(kemid, dhkm, kemctx);
 *
 * Params:
 *     ctx Object that contains algorithm state and constants.
 *     secret The returned secret (with a length ctx->alg->secretlen bytes).
 *     privkey1 A private key used for ECXDH key derivation.
 *     peerkey1 A public key used for ECXDH key derivation with privkey1
 *     privkey2 A optional private key used for a second ECXDH key derivation.
 *              It can be NULL.
 *     peerkey2 A optional public key used for a second ECXDH key derivation
 *              with privkey2,. It can be NULL.
 *     sender_pub The senders public key in encoded form.
 *     recipient_pub The recipients public key in encoded form.
 * Notes:
 *     The second ecdh() is only used for the HPKE auth modes when both privkey2
 *     and peerkey2 are non NULL (i.e. ctx->sender_authkey is not NULL).
 */
static int derive_secret(PROV_ECX_CTX *ctx, unsigned char *secret,
                         const ECX_KEY *privkey1, const ECX_KEY *peerkey1,
                         const ECX_KEY *privkey2, const ECX_KEY *peerkey2,
                         const unsigned char *sender_pub,
                         const unsigned char *recipient_pub)
{
    int ret = 0;
    EVP_KDF_CTX *kdfctx = NULL;
    unsigned char *sender_authpub = NULL;
    unsigned char dhkm[MAX_ECX_KEYLEN * 2];
    unsigned char kemctx[MAX_ECX_KEYLEN * 3];
    size_t kemctxlen = 0, dhkmlen = 0;
    const OSSL_HPKE_KEM_INFO *info = ctx->info;
    int auth = ctx->sender_authkey != NULL;
    size_t encodedkeylen = info->Npk;

    if (!generate_ecxdhkm(privkey1, peerkey1, dhkm, sizeof(dhkm), encodedkeylen))
        goto err;
    dhkmlen = encodedkeylen;

    /* Concat the optional second ECXDH (used for Auth) */
    if (auth) {
        if (!generate_ecxdhkm(privkey2, peerkey2,
                              dhkm + dhkmlen, sizeof(dhkm) - dhkmlen,
                              encodedkeylen))
            goto err;
        /* Get the public key of the auth sender in encoded form */
        sender_authpub = ecx_pubkey(ctx->sender_authkey);
        if (sender_authpub == NULL)
            goto err;
        dhkmlen += encodedkeylen;
    }
    kemctxlen = encodedkeylen + dhkmlen;
    if (kemctxlen > sizeof(kemctx))
        goto err;

    /* kemctx is the concat of both sides encoded public key */
    memcpy(kemctx, sender_pub, encodedkeylen);
    memcpy(kemctx + encodedkeylen, recipient_pub, encodedkeylen);
    if (auth)
        memcpy(kemctx + 2 * encodedkeylen, sender_authpub, encodedkeylen);
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
static int dhkem_encap(PROV_ECX_CTX *ctx,
                       unsigned char *enc, size_t *enclen,
                       unsigned char *secret, size_t *secretlen)
{
    int ret = 0;
    ECX_KEY *sender_ephemkey = NULL;
    unsigned char *sender_ephempub, *recipient_pub;
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

    sender_ephempub = ecx_pubkey(sender_ephemkey);
    recipient_pub = ecx_pubkey(ctx->recipient_key);
    if (sender_ephempub == NULL || recipient_pub == NULL)
        goto err;

    if (!derive_secret(ctx, secret,
                       sender_ephemkey, ctx->recipient_key,
                       ctx->sender_authkey, ctx->recipient_key,
                       sender_ephempub, recipient_pub))
        goto err;

    /* Return the public part of the ephemeral key */
    memcpy(enc, sender_ephempub, info->Nenc);
    *enclen = info->Nenc;
    *secretlen = info->Nsecret;
    ret = 1;
err:
    ossl_ecx_key_free(sender_ephemkey);
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
static int dhkem_decap(PROV_ECX_CTX *ctx,
                       unsigned char *secret, size_t *secretlen,
                       const unsigned char *enc, size_t enclen)
{
    int ret = 0;
    ECX_KEY *recipient_privkey = ctx->recipient_key;
    ECX_KEY *sender_ephempubkey = NULL;
    const OSSL_HPKE_KEM_INFO *info = ctx->info;
    unsigned char *recipient_pub;

    if (secret == NULL) {
        *secretlen = info->Nsecret;
        return 1;
    }
    if (*secretlen < info->Nsecret) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_BAD_LENGTH, "*secretlen too small");
        return 0;
    }
    if (enclen != info->Nenc) {
        ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_KEY, "Invalid enc public key");
        return 0;
    }

    /* Get the public part of the ephemeral key created by encap */
    sender_ephempubkey = ecxkey_pubfromdata(ctx, enc, enclen);
    if (sender_ephempubkey == NULL)
        goto err;

    recipient_pub = ecx_pubkey(recipient_privkey);
    if (recipient_pub == NULL)
        goto err;

    if (!derive_secret(ctx, secret,
                       ctx->recipient_key, sender_ephempubkey,
                       ctx->recipient_key, ctx->sender_authkey,
                       enc, recipient_pub))
        goto err;

    *secretlen = info->Nsecret;
    ret = 1;
err:
    ossl_ecx_key_free(sender_ephempubkey);
    return ret;
}

static int ecxkem_encapsulate(void *vctx, unsigned char *out, size_t *outlen,
                              unsigned char *secret, size_t *secretlen)
{
    PROV_ECX_CTX *ctx = (PROV_ECX_CTX *)vctx;

    switch (ctx->mode) {
        case KEM_MODE_DHKEM:
            return dhkem_encap(ctx, out, outlen, secret, secretlen);
        default:
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_MODE);
            return -2;
    }
}

static int ecxkem_decapsulate(void *vctx, unsigned char *out, size_t *outlen,
                              const unsigned char *in, size_t inlen)
{
    PROV_ECX_CTX *ctx = (PROV_ECX_CTX *)vctx;

    switch (ctx->mode) {
        case KEM_MODE_DHKEM:
            return dhkem_decap(vctx, out, outlen, in, inlen);
        default:
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_MODE);
            return -2;
    }
}

const OSSL_DISPATCH ossl_ecx_asym_kem_functions[] = {
    { OSSL_FUNC_KEM_NEWCTX, (void (*)(void))ecxkem_newctx },
    { OSSL_FUNC_KEM_ENCAPSULATE_INIT,
      (void (*)(void))ecxkem_encapsulate_init },
    { OSSL_FUNC_KEM_ENCAPSULATE, (void (*)(void))ecxkem_encapsulate },
    { OSSL_FUNC_KEM_DECAPSULATE_INIT,
      (void (*)(void))ecxkem_decapsulate_init },
    { OSSL_FUNC_KEM_DECAPSULATE, (void (*)(void))ecxkem_decapsulate },
    { OSSL_FUNC_KEM_FREECTX, (void (*)(void))ecxkem_freectx },
    { OSSL_FUNC_KEM_SET_CTX_PARAMS,
      (void (*)(void))ecxkem_set_ctx_params },
    { OSSL_FUNC_KEM_SETTABLE_CTX_PARAMS,
      (void (*)(void))ecxkem_settable_ctx_params },
    { OSSL_FUNC_KEM_AUTH_ENCAPSULATE_INIT,
      (void (*)(void))ecxkem_auth_encapsulate_init },
    { OSSL_FUNC_KEM_AUTH_DECAPSULATE_INIT,
      (void (*)(void))ecxkem_auth_decapsulate_init },
    OSSL_DISPATCH_END
};
