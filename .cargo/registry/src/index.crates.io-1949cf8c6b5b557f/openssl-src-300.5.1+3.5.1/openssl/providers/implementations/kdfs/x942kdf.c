/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2019, Oracle and/or its affiliates.  All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/e_os.h"
#include <openssl/core_names.h>
#include <openssl/core_dispatch.h>
#include <openssl/err.h>
#include <openssl/evp.h>
#include <openssl/params.h>
#include <openssl/proverr.h>
#include "internal/packet.h"
#include "internal/der.h"
#include "internal/nelem.h"
#include "prov/provider_ctx.h"
#include "prov/providercommon.h"
#include "prov/implementations.h"
#include "prov/provider_util.h"
#include "prov/securitycheck.h"
#include "prov/der_wrap.h"

#define X942KDF_MAX_INLEN (1 << 30)

static OSSL_FUNC_kdf_newctx_fn x942kdf_new;
static OSSL_FUNC_kdf_dupctx_fn x942kdf_dup;
static OSSL_FUNC_kdf_freectx_fn x942kdf_free;
static OSSL_FUNC_kdf_reset_fn x942kdf_reset;
static OSSL_FUNC_kdf_derive_fn x942kdf_derive;
static OSSL_FUNC_kdf_settable_ctx_params_fn x942kdf_settable_ctx_params;
static OSSL_FUNC_kdf_set_ctx_params_fn x942kdf_set_ctx_params;
static OSSL_FUNC_kdf_gettable_ctx_params_fn x942kdf_gettable_ctx_params;
static OSSL_FUNC_kdf_get_ctx_params_fn x942kdf_get_ctx_params;

typedef struct {
    void *provctx;
    PROV_DIGEST digest;
    unsigned char *secret;
    size_t secret_len;
    unsigned char *acvpinfo;
    size_t acvpinfo_len;
    unsigned char *partyuinfo, *partyvinfo, *supp_pubinfo, *supp_privinfo;
    size_t partyuinfo_len, partyvinfo_len, supp_pubinfo_len, supp_privinfo_len;
    size_t dkm_len;
    const unsigned char *cek_oid;
    size_t cek_oid_len;
    int use_keybits;
    OSSL_FIPS_IND_DECLARE
} KDF_X942;

/*
 * A table of allowed wrapping algorithms, oids and the associated output
 * lengths.
 * NOTE: RC2wrap and camellia128_wrap have been removed as there are no
 * corresponding ciphers for these operations.
 */
static const struct {
    const char *name;
    const unsigned char *oid;
    size_t oid_len;
    size_t keklen; /* size in bytes */
} kek_algs[] = {
    { "AES-128-WRAP", ossl_der_oid_id_aes128_wrap, DER_OID_SZ_id_aes128_wrap,
      16 },
    { "AES-192-WRAP", ossl_der_oid_id_aes192_wrap, DER_OID_SZ_id_aes192_wrap,
      24 },
    { "AES-256-WRAP", ossl_der_oid_id_aes256_wrap, DER_OID_SZ_id_aes256_wrap,
      32 },
#ifndef FIPS_MODULE
    { "DES3-WRAP", ossl_der_oid_id_alg_CMS3DESwrap,
      DER_OID_SZ_id_alg_CMS3DESwrap, 24 },
#endif
};

static int find_alg_id(OSSL_LIB_CTX *libctx, const char *algname,
                       const char *propq, size_t *id)
{
    int ret = 1;
    size_t i;
    EVP_CIPHER *cipher;

    cipher = EVP_CIPHER_fetch(libctx, algname, propq);
    if (cipher != NULL) {
        for (i = 0; i < OSSL_NELEM(kek_algs); i++) {
            if (EVP_CIPHER_is_a(cipher, kek_algs[i].name)) {
                *id = i;
                goto end;
            }
        }
    }
    ret = 0;
    ERR_raise(ERR_LIB_PROV, PROV_R_UNSUPPORTED_CEK_ALG);
end:
    EVP_CIPHER_free(cipher);
    return ret;
}

static int DER_w_keyinfo(WPACKET *pkt,
                         const unsigned char *der_oid, size_t der_oidlen,
                         unsigned char **pcounter)
{
    return ossl_DER_w_begin_sequence(pkt, -1)
           /* Store the initial value of 1 into the counter */
           && ossl_DER_w_octet_string_uint32(pkt, -1, 1)
           /* Remember where we stored the counter in the buffer */
           && (pcounter == NULL
               || (*pcounter = WPACKET_get_curr(pkt)) != NULL)
           && ossl_DER_w_precompiled(pkt, -1, der_oid, der_oidlen)
           && ossl_DER_w_end_sequence(pkt, -1);
}

static int der_encode_sharedinfo(WPACKET *pkt, unsigned char *buf, size_t buflen,
                                 const unsigned char *der_oid, size_t der_oidlen,
                                 const unsigned char *acvp, size_t acvplen,
                                 const unsigned char *partyu, size_t partyulen,
                                 const unsigned char *partyv, size_t partyvlen,
                                 const unsigned char *supp_pub, size_t supp_publen,
                                 const unsigned char *supp_priv, size_t supp_privlen,
                                 uint32_t keylen_bits, unsigned char **pcounter)
{
    return (buf != NULL ? WPACKET_init_der(pkt, buf, buflen) :
                          WPACKET_init_null_der(pkt))
           && ossl_DER_w_begin_sequence(pkt, -1)
           && (supp_priv == NULL
               || ossl_DER_w_octet_string(pkt, 3, supp_priv, supp_privlen))
           && (supp_pub == NULL
               || ossl_DER_w_octet_string(pkt, 2, supp_pub, supp_publen))
           && (keylen_bits == 0
               || ossl_DER_w_octet_string_uint32(pkt, 2, keylen_bits))
           && (partyv == NULL || ossl_DER_w_octet_string(pkt, 1, partyv, partyvlen))
           && (partyu == NULL || ossl_DER_w_octet_string(pkt, 0, partyu, partyulen))
           && (acvp == NULL || ossl_DER_w_precompiled(pkt, -1, acvp, acvplen))
           && DER_w_keyinfo(pkt, der_oid, der_oidlen, pcounter)
           && ossl_DER_w_end_sequence(pkt, -1)
           && WPACKET_finish(pkt);
}

/*
 * Encode the other info structure.
 *
 * The ANS X9.42-2003 standard uses OtherInfo:
 *
 *  OtherInfo ::= SEQUENCE {
 *      keyInfo KeySpecificInfo,
 *      partyUInfo [0] OCTET STRING OPTIONAL,
 *      partyVInfo [1] OCTET STRING OPTIONAL,
 *      suppPubInfo [2] OCTET STRING OPTIONAL,
 *      suppPrivInfo [3] OCTET STRING OPTIONAL
 *  }
 *
 *  KeySpecificInfo ::= SEQUENCE {
 *      algorithm OBJECT IDENTIFIER,
 *      counter OCTET STRING SIZE (4..4)
 *  }
 *
 *  RFC2631 Section 2.1.2 Contains the following definition for OtherInfo
 *
 *  OtherInfo ::= SEQUENCE {
 *      keyInfo KeySpecificInfo,
 *      partyAInfo [0] OCTET STRING OPTIONAL,
 *      suppPubInfo [2] OCTET STRING
 *  }
 *  Where suppPubInfo is the key length (in bits) (stored into 4 bytes)
 *
 * |keylen| is the length (in bytes) of the generated KEK. It is stored into
 *   suppPubInfo (in bits). It is ignored if the value is 0.
 * |cek_oid| The oid of the key wrapping algorithm.
 * |cek_oidlen| The length (in bytes) of the key wrapping algorithm oid,
 * |acvp| is the optional blob of DER data representing one or more of the
 *   OtherInfo fields related to |partyu|, |partyv|, |supp_pub| and |supp_priv|.
 *   This field should normally be NULL. If |acvp| is non NULL then |partyu|,
 *   |partyv|, |supp_pub| and |supp_priv| should all be NULL.
 * |acvp_len| is the |acvp| length (in bytes).
 * |partyu| is the optional public info contributed by the initiator.
 *   It can be NULL. (It is also used as the ukm by CMS).
 * |partyu_len| is the |partyu| length (in bytes).
 * |partyv| is the optional public info contributed by the responder.
 *   It can be NULL.
 * |partyv_len| is the |partyv| length (in bytes).
 * |supp_pub| is the optional additional, mutually-known public information.
 *   It can be NULL. |keylen| should be 0 if this is not NULL.
 * |supp_pub_len| is the |supp_pub| length (in bytes).
 * |supp_priv| is the optional additional, mutually-known private information.
 *   It can be NULL.
 * |supp_priv_len| is the |supp_priv| length (in bytes).
 * |der| is the returned encoded data. It must be freed by the caller.
 * |der_len| is the returned size of the encoded data.
 * |out_ctr| returns a pointer to the counter data which is embedded inside the
 *   encoded data. This allows the counter bytes to be updated without
 *   re-encoding.
 *
 * Returns: 1 if successfully encoded, or 0 otherwise.
 * Assumptions: |der|, |der_len| & |out_ctr| are not NULL.
 */
static int
x942_encode_otherinfo(size_t keylen,
                      const unsigned char *cek_oid, size_t cek_oid_len,
                      const unsigned char *acvp, size_t acvp_len,
                      const unsigned char *partyu, size_t partyu_len,
                      const unsigned char *partyv, size_t partyv_len,
                      const unsigned char *supp_pub, size_t supp_pub_len,
                      const unsigned char *supp_priv, size_t supp_priv_len,
                      unsigned char **der, size_t *der_len,
                      unsigned char **out_ctr)
{
    int ret = 0;
    unsigned char *pcounter = NULL, *der_buf = NULL;
    size_t der_buflen = 0;
    WPACKET pkt;
    uint32_t keylen_bits;

    /* keylenbits must fit into 4 bytes */
    if (keylen > 0xFFFFFF)
        return 0;
    keylen_bits = 8 * keylen;

    /* Calculate the size of the buffer */
    if (!der_encode_sharedinfo(&pkt, NULL, 0, cek_oid, cek_oid_len,
                               acvp, acvp_len,
                               partyu, partyu_len, partyv, partyv_len,
                               supp_pub, supp_pub_len, supp_priv, supp_priv_len,
                               keylen_bits, NULL)
        || !WPACKET_get_total_written(&pkt, &der_buflen))
        goto err;
    WPACKET_cleanup(&pkt);
    /* Alloc the buffer */
    der_buf = OPENSSL_zalloc(der_buflen);
    if (der_buf == NULL)
        goto err;
    /* Encode into the buffer */
    if (!der_encode_sharedinfo(&pkt, der_buf, der_buflen, cek_oid, cek_oid_len,
                               acvp, acvp_len,
                               partyu, partyu_len, partyv, partyv_len,
                               supp_pub, supp_pub_len, supp_priv, supp_priv_len,
                               keylen_bits, &pcounter))
        goto err;
    /*
     * Since we allocated the exact size required, the buffer should point to the
     * start of the allocated buffer at this point.
     */
    if (WPACKET_get_curr(&pkt) != der_buf)
        goto err;

    /*
     * The data for the DER encoded octet string of a 32 bit counter = 1
     * should be 04 04 00 00 00 01
     * So just check the header is correct and skip over it.
     * This counter will be incremented in the kdf update loop.
     */
    if (pcounter == NULL
        || pcounter[0] != 0x04
        || pcounter[1] != 0x04)
        goto err;
    *out_ctr = (pcounter + 2);
    *der = der_buf;
    *der_len = der_buflen;
    ret = 1;
err:
    WPACKET_cleanup(&pkt);
    return ret;
}

static int x942kdf_hash_kdm(const EVP_MD *kdf_md,
                            const unsigned char *z, size_t z_len,
                            const unsigned char *other, size_t other_len,
                            unsigned char *ctr,
                            unsigned char *derived_key, size_t derived_key_len)
{
    int ret = 0, hlen;
    size_t counter, out_len, len = derived_key_len;
    unsigned char mac[EVP_MAX_MD_SIZE];
    unsigned char *out = derived_key;
    EVP_MD_CTX *ctx = NULL, *ctx_init = NULL;

    if (z_len > X942KDF_MAX_INLEN
        || other_len > X942KDF_MAX_INLEN
        || derived_key_len > X942KDF_MAX_INLEN
        || derived_key_len == 0) {
        ERR_raise(ERR_LIB_PROV, PROV_R_BAD_LENGTH);
        return 0;
    }

    hlen = EVP_MD_get_size(kdf_md);
    if (hlen <= 0)
        return 0;
    out_len = (size_t)hlen;

    ctx = EVP_MD_CTX_create();
    ctx_init = EVP_MD_CTX_create();
    if (ctx == NULL || ctx_init == NULL)
        goto end;

    if (!EVP_DigestInit(ctx_init, kdf_md))
        goto end;

    for (counter = 1;; counter++) {
        /* updating the ctr modifies 4 bytes in the 'other' buffer */
        ctr[0] = (unsigned char)((counter >> 24) & 0xff);
        ctr[1] = (unsigned char)((counter >> 16) & 0xff);
        ctr[2] = (unsigned char)((counter >> 8) & 0xff);
        ctr[3] = (unsigned char)(counter & 0xff);

        if (!EVP_MD_CTX_copy_ex(ctx, ctx_init)
            || !EVP_DigestUpdate(ctx, z, z_len)
            || !EVP_DigestUpdate(ctx, other, other_len))
            goto end;
        if (len >= out_len) {
            if (!EVP_DigestFinal_ex(ctx, out, NULL))
                goto end;
            out += out_len;
            len -= out_len;
            if (len == 0)
                break;
        } else {
            if (!EVP_DigestFinal_ex(ctx, mac, NULL))
                goto end;
            memcpy(out, mac, len);
            break;
        }
    }
    ret = 1;
end:
    EVP_MD_CTX_free(ctx);
    EVP_MD_CTX_free(ctx_init);
    OPENSSL_cleanse(mac, sizeof(mac));
    return ret;
}

static void *x942kdf_new(void *provctx)
{
    KDF_X942 *ctx;

    if (!ossl_prov_is_running())
        return NULL;

    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx == NULL)
        return NULL;

    ctx->provctx = provctx;
    OSSL_FIPS_IND_INIT(ctx)
    ctx->use_keybits = 1;
    return ctx;
}

static void x942kdf_reset(void *vctx)
{
    KDF_X942 *ctx = (KDF_X942 *)vctx;
    void *provctx = ctx->provctx;

    ossl_prov_digest_reset(&ctx->digest);
    OPENSSL_clear_free(ctx->secret, ctx->secret_len);
    OPENSSL_clear_free(ctx->acvpinfo, ctx->acvpinfo_len);
    OPENSSL_clear_free(ctx->partyuinfo, ctx->partyuinfo_len);
    OPENSSL_clear_free(ctx->partyvinfo, ctx->partyvinfo_len);
    OPENSSL_clear_free(ctx->supp_pubinfo, ctx->supp_pubinfo_len);
    OPENSSL_clear_free(ctx->supp_privinfo, ctx->supp_privinfo_len);
    memset(ctx, 0, sizeof(*ctx));
    ctx->provctx = provctx;
    ctx->use_keybits = 1;
}

static void x942kdf_free(void *vctx)
{
    KDF_X942 *ctx = (KDF_X942 *)vctx;

    if (ctx != NULL) {
        x942kdf_reset(ctx);
        OPENSSL_free(ctx);
    }
}

static void *x942kdf_dup(void *vctx)
{
    const KDF_X942 *src = (const KDF_X942 *)vctx;
    KDF_X942 *dest;

    dest = x942kdf_new(src->provctx);
    if (dest != NULL) {
        if (!ossl_prov_memdup(src->secret, src->secret_len,
                              &dest->secret , &dest->secret_len)
                || !ossl_prov_memdup(src->acvpinfo, src->acvpinfo_len,
                                     &dest->acvpinfo , &dest->acvpinfo_len)
                || !ossl_prov_memdup(src->partyuinfo, src->partyuinfo_len,
                                     &dest->partyuinfo , &dest->partyuinfo_len)
                || !ossl_prov_memdup(src->partyvinfo, src->partyvinfo_len,
                                     &dest->partyvinfo , &dest->partyvinfo_len)
                || !ossl_prov_memdup(src->supp_pubinfo, src->supp_pubinfo_len,
                                     &dest->supp_pubinfo,
                                     &dest->supp_pubinfo_len)
                || !ossl_prov_memdup(src->supp_privinfo, src->supp_privinfo_len,
                                     &dest->supp_privinfo,
                                     &dest->supp_privinfo_len)
                || !ossl_prov_digest_copy(&dest->digest, &src->digest))
            goto err;
        dest->cek_oid = src->cek_oid;
        dest->cek_oid_len = src->cek_oid_len;
        dest->dkm_len = src->dkm_len;
        dest->use_keybits = src->use_keybits;
        OSSL_FIPS_IND_COPY(dest, src)
    }
    return dest;

 err:
    x942kdf_free(dest);
    return NULL;
}

static int x942kdf_set_buffer(unsigned char **out, size_t *out_len,
                              const OSSL_PARAM *p)
{
    if (p->data_size == 0 || p->data == NULL)
        return 1;

    OPENSSL_free(*out);
    *out = NULL;
    return OSSL_PARAM_get_octet_string(p, (void **)out, 0, out_len);
}

static size_t x942kdf_size(KDF_X942 *ctx)
{
    int len;
    const EVP_MD *md = ossl_prov_digest_md(&ctx->digest);

    if (md == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_MESSAGE_DIGEST);
        return 0;
    }
    len = EVP_MD_get_size(md);
    return (len <= 0) ? 0 : (size_t)len;
}

#ifdef FIPS_MODULE
static int fips_x942kdf_key_check_passed(KDF_X942 *ctx)
{
    OSSL_LIB_CTX *libctx = PROV_LIBCTX_OF(ctx->provctx);
    int key_approved = ossl_kdf_check_key_size(ctx->secret_len);

    if (!key_approved) {
        if (!OSSL_FIPS_IND_ON_UNAPPROVED(ctx, OSSL_FIPS_IND_SETTABLE0,
                                         libctx, "X942KDF", "Key size",
                                         ossl_fips_config_x942kdf_key_check)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
            return 0;
        }
    }
    return 1;
}
#endif

static int x942kdf_derive(void *vctx, unsigned char *key, size_t keylen,
                          const OSSL_PARAM params[])
{
    KDF_X942 *ctx = (KDF_X942 *)vctx;
    const EVP_MD *md;
    int ret = 0;
    unsigned char *ctr;
    unsigned char *der = NULL;
    size_t der_len = 0;

    if (!ossl_prov_is_running() || !x942kdf_set_ctx_params(ctx, params))
        return 0;

    /*
     * These 2 options encode to the same field so only one of them should be
     * active at once.
     */
    if (ctx->use_keybits && ctx->supp_pubinfo != NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_PUBINFO);
        return 0;
    }
    /*
     * If the blob of acvp data is used then the individual info fields that it
     * replaces should not also be defined.
     */
    if (ctx->acvpinfo != NULL
        && (ctx->partyuinfo != NULL
            || ctx->partyvinfo != NULL
            || ctx->supp_pubinfo != NULL
            || ctx->supp_privinfo != NULL)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_DATA);
        return 0;
    }
    if (ctx->secret == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_SECRET);
        return 0;
    }
    md = ossl_prov_digest_md(&ctx->digest);
    if (md == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_MESSAGE_DIGEST);
        return 0;
    }
    if (ctx->cek_oid == NULL || ctx->cek_oid_len == 0) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_CEK_ALG);
        return 0;
    }
    if (ctx->partyuinfo != NULL && ctx->partyuinfo_len >= X942KDF_MAX_INLEN) {
        /*
         * Note the ukm length MUST be 512 bits if it is used.
         * For backwards compatibility the old check is being done.
         */
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_UKM_LENGTH);
        return 0;
    }
    /* generate the otherinfo der */
    if (!x942_encode_otherinfo(ctx->use_keybits ? ctx->dkm_len : 0,
                               ctx->cek_oid, ctx->cek_oid_len,
                               ctx->acvpinfo, ctx->acvpinfo_len,
                               ctx->partyuinfo, ctx->partyuinfo_len,
                               ctx->partyvinfo, ctx->partyvinfo_len,
                               ctx->supp_pubinfo, ctx->supp_pubinfo_len,
                               ctx->supp_privinfo, ctx->supp_privinfo_len,
                               &der, &der_len, &ctr)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_BAD_ENCODING);
        return 0;
    }
    ret = x942kdf_hash_kdm(md, ctx->secret, ctx->secret_len,
                           der, der_len, ctr, key, keylen);
    OPENSSL_free(der);
    return ret;
}

static int x942kdf_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    const OSSL_PARAM *p, *pq;
    KDF_X942 *ctx = vctx;
    OSSL_LIB_CTX *provctx = PROV_LIBCTX_OF(ctx->provctx);
    const char *propq = NULL;
    const EVP_MD *md;
    size_t id;

    if (ossl_param_is_empty(params))
        return 1;

    if (!OSSL_FIPS_IND_SET_CTX_PARAM(ctx, OSSL_FIPS_IND_SETTABLE0, params,
                                     OSSL_KDF_PARAM_FIPS_KEY_CHECK))
        return 0;

    if (OSSL_PARAM_locate_const(params, OSSL_ALG_PARAM_DIGEST) != NULL) {
        if (!ossl_prov_digest_load_from_params(&ctx->digest, params, provctx))
            return 0;
        md = ossl_prov_digest_md(&ctx->digest);
        if (EVP_MD_xof(md)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_XOF_DIGESTS_NOT_ALLOWED);
            return 0;
        }
    }

    p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_SECRET);
    if (p == NULL)
        p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_KEY);
    if (p != NULL) {
        if (!x942kdf_set_buffer(&ctx->secret, &ctx->secret_len, p))
            return 0;
#ifdef FIPS_MODULE
        if (!fips_x942kdf_key_check_passed(ctx))
            return 0;
#endif
    }

    p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_X942_ACVPINFO);
    if (p != NULL
        && !x942kdf_set_buffer(&ctx->acvpinfo, &ctx->acvpinfo_len, p))
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_X942_PARTYUINFO);
    if (p == NULL)
        p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_UKM);
    if (p != NULL
        && !x942kdf_set_buffer(&ctx->partyuinfo, &ctx->partyuinfo_len, p))
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_X942_PARTYVINFO);
    if (p != NULL
        && !x942kdf_set_buffer(&ctx->partyvinfo, &ctx->partyvinfo_len, p))
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_X942_USE_KEYBITS);
    if (p != NULL && !OSSL_PARAM_get_int(p, &ctx->use_keybits))
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_X942_SUPP_PUBINFO);
    if (p != NULL) {
        if (!x942kdf_set_buffer(&ctx->supp_pubinfo, &ctx->supp_pubinfo_len, p))
            return 0;
        ctx->use_keybits = 0;
    }

    p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_X942_SUPP_PRIVINFO);
    if (p != NULL
        && !x942kdf_set_buffer(&ctx->supp_privinfo, &ctx->supp_privinfo_len, p))
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_KDF_PARAM_CEK_ALG);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_UTF8_STRING)
            return 0;
        pq = OSSL_PARAM_locate_const(params, OSSL_ALG_PARAM_PROPERTIES);
        /*
         * We already grab the properties during ossl_prov_digest_load_from_params()
         * so there is no need to check the validity again..
         */
        if (pq != NULL)
            propq = p->data;
        if (find_alg_id(provctx, p->data, propq, &id) == 0)
            return 0;
        ctx->cek_oid = kek_algs[id].oid;
        ctx->cek_oid_len = kek_algs[id].oid_len;
        ctx->dkm_len = kek_algs[id].keklen;
    }
    return 1;
}

static const OSSL_PARAM *x942kdf_settable_ctx_params(ossl_unused void *ctx,
                                                     ossl_unused void *provctx)
{
    static const OSSL_PARAM known_settable_ctx_params[] = {
        OSSL_PARAM_utf8_string(OSSL_KDF_PARAM_PROPERTIES, NULL, 0),
        OSSL_PARAM_utf8_string(OSSL_KDF_PARAM_DIGEST, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_KDF_PARAM_SECRET, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_KDF_PARAM_KEY, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_KDF_PARAM_UKM, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_KDF_PARAM_X942_ACVPINFO, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_KDF_PARAM_X942_PARTYUINFO, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_KDF_PARAM_X942_PARTYVINFO, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_KDF_PARAM_X942_SUPP_PUBINFO, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_KDF_PARAM_X942_SUPP_PRIVINFO, NULL, 0),
        OSSL_PARAM_int(OSSL_KDF_PARAM_X942_USE_KEYBITS, NULL),
        OSSL_PARAM_utf8_string(OSSL_KDF_PARAM_CEK_ALG, NULL, 0),
        OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_KDF_PARAM_FIPS_KEY_CHECK)
        OSSL_PARAM_END
    };
    return known_settable_ctx_params;
}

static int x942kdf_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
    KDF_X942 *ctx = (KDF_X942 *)vctx;
    OSSL_PARAM *p;

    p = OSSL_PARAM_locate(params, OSSL_KDF_PARAM_SIZE);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, x942kdf_size(ctx)))
        return 0;

    if (!OSSL_FIPS_IND_GET_CTX_PARAM(ctx, params))
        return 0;
    return 1;
}

static const OSSL_PARAM *x942kdf_gettable_ctx_params(ossl_unused void *ctx,
                                                     ossl_unused void *provctx)
{
    static const OSSL_PARAM known_gettable_ctx_params[] = {
        OSSL_PARAM_size_t(OSSL_KDF_PARAM_SIZE, NULL),
        OSSL_FIPS_IND_GETTABLE_CTX_PARAM()
        OSSL_PARAM_END
    };
    return known_gettable_ctx_params;
}

const OSSL_DISPATCH ossl_kdf_x942_kdf_functions[] = {
    { OSSL_FUNC_KDF_NEWCTX, (void(*)(void))x942kdf_new },
    { OSSL_FUNC_KDF_DUPCTX, (void(*)(void))x942kdf_dup },
    { OSSL_FUNC_KDF_FREECTX, (void(*)(void))x942kdf_free },
    { OSSL_FUNC_KDF_RESET, (void(*)(void))x942kdf_reset },
    { OSSL_FUNC_KDF_DERIVE, (void(*)(void))x942kdf_derive },
    { OSSL_FUNC_KDF_SETTABLE_CTX_PARAMS,
      (void(*)(void))x942kdf_settable_ctx_params },
    { OSSL_FUNC_KDF_SET_CTX_PARAMS, (void(*)(void))x942kdf_set_ctx_params },
    { OSSL_FUNC_KDF_GETTABLE_CTX_PARAMS,
      (void(*)(void))x942kdf_gettable_ctx_params },
    { OSSL_FUNC_KDF_GET_CTX_PARAMS, (void(*)(void))x942kdf_get_ctx_params },
    OSSL_DISPATCH_END
};
