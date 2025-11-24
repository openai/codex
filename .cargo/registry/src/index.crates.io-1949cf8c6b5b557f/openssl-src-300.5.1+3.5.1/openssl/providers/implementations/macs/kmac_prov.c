/*
 * Copyright 2018-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * See SP800-185 "Appendix A - KMAC, .... in Terms of Keccak[c]"
 *
 * Inputs are:
 *    K = Key                  (len(K) < 2^2040 bits)
 *    X = Input
 *    L = Output length        (0 <= L < 2^2040 bits)
 *    S = Customization String Default="" (len(S) < 2^2040 bits)
 *
 * KMAC128(K, X, L, S)
 * {
 *     newX = bytepad(encode_string(K), 168) ||  X || right_encode(L).
 *     T = bytepad(encode_string("KMAC") || encode_string(S), 168).
 *     return KECCAK[256](T || newX || 00, L).
 * }
 *
 * KMAC256(K, X, L, S)
 * {
 *     newX = bytepad(encode_string(K), 136) ||  X || right_encode(L).
 *     T = bytepad(encode_string("KMAC") || encode_string(S), 136).
 *     return KECCAK[512](T || newX || 00, L).
 * }
 *
 * KMAC128XOF(K, X, L, S)
 * {
 *     newX = bytepad(encode_string(K), 168) ||  X || right_encode(0).
 *     T = bytepad(encode_string("KMAC") || encode_string(S), 168).
 *     return KECCAK[256](T || newX || 00, L).
 * }
 *
 * KMAC256XOF(K, X, L, S)
 * {
 *     newX = bytepad(encode_string(K), 136) ||  X || right_encode(0).
 *     T = bytepad(encode_string("KMAC") || encode_string(S), 136).
 *     return KECCAK[512](T || newX || 00, L).
 * }
 *
 */

#include <stdlib.h>
#include <string.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/params.h>
#include <openssl/evp.h>
#include <openssl/err.h>
#include <openssl/proverr.h>
#include <openssl/fips_names.h>
#include "prov/securitycheck.h"
#include "prov/implementations.h"
#include "prov/provider_ctx.h"
#include "prov/provider_util.h"
#include "prov/providercommon.h"
#include "internal/cryptlib.h" /* ossl_assert */

/*
 * Forward declaration of everything implemented here.  This is not strictly
 * necessary for the compiler, but provides an assurance that the signatures
 * of the functions in the dispatch table are correct.
 */
static OSSL_FUNC_mac_newctx_fn kmac128_new;
static OSSL_FUNC_mac_newctx_fn kmac256_new;
static OSSL_FUNC_mac_dupctx_fn kmac_dup;
static OSSL_FUNC_mac_freectx_fn kmac_free;
static OSSL_FUNC_mac_gettable_ctx_params_fn kmac_gettable_ctx_params;
static OSSL_FUNC_mac_get_ctx_params_fn kmac_get_ctx_params;
static OSSL_FUNC_mac_settable_ctx_params_fn kmac_settable_ctx_params;
static OSSL_FUNC_mac_set_ctx_params_fn kmac_set_ctx_params;
static OSSL_FUNC_mac_init_fn kmac_init;
static OSSL_FUNC_mac_update_fn kmac_update;
static OSSL_FUNC_mac_final_fn kmac_final;

#define KMAC_MAX_BLOCKSIZE ((1600 - 128 * 2) / 8) /* 168 */

/*
 * Length encoding will be  a 1 byte size + length in bits (3 bytes max)
 * This gives a range of 0..0XFFFFFF bits = 2097151 bytes).
 */
#define KMAC_MAX_OUTPUT_LEN (0xFFFFFF / 8)
#define KMAC_MAX_ENCODED_HEADER_LEN (1 + 3)

/*
 * Restrict the maximum length of the customisation string.  This must not
 * exceed 64 bits = 8k bytes.
 */
#define KMAC_MAX_CUSTOM 512

/* Maximum size of encoded custom string */
#define KMAC_MAX_CUSTOM_ENCODED (KMAC_MAX_CUSTOM + KMAC_MAX_ENCODED_HEADER_LEN)

/* Maximum key size in bytes = 512 (4096 bits) */
#define KMAC_MAX_KEY 512
#define KMAC_MIN_KEY 4

/*
 * Maximum Encoded Key size will be padded to a multiple of the blocksize
 * i.e KMAC_MAX_KEY + KMAC_MAX_ENCODED_HEADER_LEN = 512 + 4
 * Padded to a multiple of KMAC_MAX_BLOCKSIZE
 */
#define KMAC_MAX_KEY_ENCODED (KMAC_MAX_BLOCKSIZE * 4)

/* Fixed value of encode_string("KMAC") */
static const unsigned char kmac_string[] = {
    0x01, 0x20, 0x4B, 0x4D, 0x41, 0x43
};

#define KMAC_FLAG_XOF_MODE          1

struct kmac_data_st {
    void  *provctx;
    EVP_MD_CTX *ctx;
    PROV_DIGEST digest;
    size_t out_len;
    size_t key_len;
    size_t custom_len;
    /* If xof_mode = 1 then we use right_encode(0) */
    int xof_mode;
    /* key and custom are stored in encoded form */
    unsigned char key[KMAC_MAX_KEY_ENCODED];
    unsigned char custom[KMAC_MAX_CUSTOM_ENCODED];
#ifdef FIPS_MODULE
    /*
     * 'internal' is set to 1 if KMAC is used inside another algorithm such as a
     * KDF. In this case it is the parent algorithm that is responsible for
     * performing any conditional FIPS indicator related checks for KMAC.
     */
    int internal;
#endif
    OSSL_FIPS_IND_DECLARE
};

static int encode_string(unsigned char *out, size_t out_max_len, size_t *out_len,
                         const unsigned char *in, size_t in_len);
static int right_encode(unsigned char *out, size_t out_max_len, size_t *out_len,
                        size_t bits);
static int bytepad(unsigned char *out, size_t *out_len,
                   const unsigned char *in1, size_t in1_len,
                   const unsigned char *in2, size_t in2_len,
                   size_t w);
static int kmac_bytepad_encode_key(unsigned char *out, size_t out_max_len,
                                   size_t *out_len,
                                   const unsigned char *in, size_t in_len,
                                   size_t w);

static void kmac_free(void *vmacctx)
{
    struct kmac_data_st *kctx = vmacctx;

    if (kctx != NULL) {
        EVP_MD_CTX_free(kctx->ctx);
        ossl_prov_digest_reset(&kctx->digest);
        OPENSSL_cleanse(kctx->key, kctx->key_len);
        OPENSSL_cleanse(kctx->custom, kctx->custom_len);
        OPENSSL_free(kctx);
    }
}

/*
 * We have KMAC implemented as a hash, which we can use instead of
 * reimplementing the EVP functionality with direct use of
 * keccak_mac_init() and friends.
 */
static struct kmac_data_st *kmac_new(void *provctx)
{
    struct kmac_data_st *kctx;

    if (!ossl_prov_is_running())
        return NULL;

    if ((kctx = OPENSSL_zalloc(sizeof(*kctx))) == NULL
            || (kctx->ctx = EVP_MD_CTX_new()) == NULL) {
        kmac_free(kctx);
        return NULL;
    }
    kctx->provctx = provctx;
    OSSL_FIPS_IND_INIT(kctx)
    return kctx;
}

static void *kmac_fetch_new(void *provctx, const OSSL_PARAM *params)
{
    struct kmac_data_st *kctx = kmac_new(provctx);
    int md_size;

    if (kctx == NULL)
        return 0;
    if (!ossl_prov_digest_load_from_params(&kctx->digest, params,
                                      PROV_LIBCTX_OF(provctx))) {
        kmac_free(kctx);
        return 0;
    }

    md_size = EVP_MD_get_size(ossl_prov_digest_md(&kctx->digest));
    if (md_size <= 0) {
        kmac_free(kctx);
        return 0;
    }
    kctx->out_len = (size_t)md_size;
    return kctx;
}

static void *kmac128_new(void *provctx)
{
    static const OSSL_PARAM kmac128_params[] = {
        OSSL_PARAM_utf8_string("digest", OSSL_DIGEST_NAME_KECCAK_KMAC128,
                               sizeof(OSSL_DIGEST_NAME_KECCAK_KMAC128)),
        OSSL_PARAM_END
    };
    return kmac_fetch_new(provctx, kmac128_params);
}

static void *kmac256_new(void *provctx)
{
    static const OSSL_PARAM kmac256_params[] = {
        OSSL_PARAM_utf8_string("digest", OSSL_DIGEST_NAME_KECCAK_KMAC256,
                               sizeof(OSSL_DIGEST_NAME_KECCAK_KMAC256)),
        OSSL_PARAM_END
    };
    return kmac_fetch_new(provctx, kmac256_params);
}

static void *kmac_dup(void *vsrc)
{
    struct kmac_data_st *src = vsrc;
    struct kmac_data_st *dst;

    if (!ossl_prov_is_running())
        return NULL;

    dst = kmac_new(src->provctx);
    if (dst == NULL)
        return NULL;

    if (!EVP_MD_CTX_copy(dst->ctx, src->ctx)
        || !ossl_prov_digest_copy(&dst->digest, &src->digest)) {
        kmac_free(dst);
        return NULL;
    }
#ifdef FIPS_MODULE
    dst->internal = src->internal;
#endif
    dst->out_len = src->out_len;
    dst->key_len = src->key_len;
    dst->custom_len = src->custom_len;
    dst->xof_mode = src->xof_mode;
    memcpy(dst->key, src->key, src->key_len);
    memcpy(dst->custom, src->custom, dst->custom_len);
    OSSL_FIPS_IND_COPY(dst, src)

    return dst;
}

static int kmac_setkey(struct kmac_data_st *kctx, const unsigned char *key,
                       size_t keylen)
{
    const EVP_MD *digest = ossl_prov_digest_md(&kctx->digest);
    int w = EVP_MD_get_block_size(digest);

    if (keylen < KMAC_MIN_KEY || keylen > KMAC_MAX_KEY) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
        return 0;
    }
#ifdef FIPS_MODULE
    /*
     * Only do the key check if KMAC is fetched directly.
     * Other algorithms that embed KMAC such as SSKDF will ignore this check.
     */
    if (!kctx->internal) {
        int approved = ossl_mac_check_key_size(keylen);

        if (!approved) {
            if (!OSSL_FIPS_IND_ON_UNAPPROVED(kctx, OSSL_FIPS_IND_SETTABLE1,
                                             PROV_LIBCTX_OF(kctx->provctx),
                                             "KMAC", "Key size",
                                             ossl_fips_config_kmac_key_check)) {
                ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
                return 0;
            }
        }
    }
#endif
    if (w <= 0) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_DIGEST_LENGTH);
        return 0;
    }
    if (!kmac_bytepad_encode_key(kctx->key, sizeof(kctx->key), &kctx->key_len,
                                 key, keylen, (size_t)w))
        return 0;
    return 1;
}

/*
 * The init() assumes that any ctrl methods are set beforehand for
 * md, key and custom. Setting the fields afterwards will have no
 * effect on the output mac.
 */
static int kmac_init(void *vmacctx, const unsigned char *key,
                     size_t keylen, const OSSL_PARAM params[])
{
    struct kmac_data_st *kctx = vmacctx;
    EVP_MD_CTX *ctx = kctx->ctx;
    unsigned char *out;
    size_t out_len, block_len;
    int res, t;

    if (!ossl_prov_is_running() || !kmac_set_ctx_params(kctx, params))
        return 0;

    if (key != NULL) {
        if (!kmac_setkey(kctx, key, keylen))
            return 0;
    } else if (kctx->key_len == 0) {
        /* Check key has been set */
        ERR_raise(ERR_LIB_PROV, PROV_R_NO_KEY_SET);
        return 0;
    }
    if (!EVP_DigestInit_ex(kctx->ctx, ossl_prov_digest_md(&kctx->digest),
                           NULL))
        return 0;

    t = EVP_MD_get_block_size(ossl_prov_digest_md(&kctx->digest));
    if (t <= 0) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_DIGEST_LENGTH);
        return 0;
    }
    block_len = t;

    /* Set default custom string if it is not already set */
    if (kctx->custom_len == 0) {
        const OSSL_PARAM cparams[] = {
            OSSL_PARAM_octet_string(OSSL_MAC_PARAM_CUSTOM, "", 0),
            OSSL_PARAM_END
        };
        (void)kmac_set_ctx_params(kctx, cparams);
    }

    if (!bytepad(NULL, &out_len, kmac_string, sizeof(kmac_string),
                 kctx->custom, kctx->custom_len, block_len)) {
        ERR_raise(ERR_LIB_PROV, ERR_R_INTERNAL_ERROR);
        return 0;
    }
    out = OPENSSL_malloc(out_len);
    if (out == NULL)
        return 0;
    res = bytepad(out, NULL, kmac_string, sizeof(kmac_string),
                  kctx->custom, kctx->custom_len, block_len)
          && EVP_DigestUpdate(ctx, out, out_len)
          && EVP_DigestUpdate(ctx, kctx->key, kctx->key_len);
    OPENSSL_free(out);
    return res;
}

static int kmac_update(void *vmacctx, const unsigned char *data,
                       size_t datalen)
{
    struct kmac_data_st *kctx = vmacctx;

    return EVP_DigestUpdate(kctx->ctx, data, datalen);
}

static int kmac_final(void *vmacctx, unsigned char *out, size_t *outl,
                      size_t outsize)
{
    struct kmac_data_st *kctx = vmacctx;
    EVP_MD_CTX *ctx = kctx->ctx;
    size_t lbits, len;
    unsigned char encoded_outlen[KMAC_MAX_ENCODED_HEADER_LEN];
    int ok;

    if (!ossl_prov_is_running())
        return 0;

    /* KMAC XOF mode sets the encoded length to 0 */
    lbits = (kctx->xof_mode ? 0 : (kctx->out_len * 8));

    ok = right_encode(encoded_outlen, sizeof(encoded_outlen), &len, lbits)
        && EVP_DigestUpdate(ctx, encoded_outlen, len)
        && EVP_DigestFinalXOF(ctx, out, kctx->out_len);
    *outl = kctx->out_len;
    return ok;
}

static const OSSL_PARAM known_gettable_ctx_params[] = {
    OSSL_PARAM_size_t(OSSL_MAC_PARAM_SIZE, NULL),
    OSSL_PARAM_size_t(OSSL_MAC_PARAM_BLOCK_SIZE, NULL),
    OSSL_FIPS_IND_GETTABLE_CTX_PARAM()
    OSSL_PARAM_END
};
static const OSSL_PARAM *kmac_gettable_ctx_params(ossl_unused void *ctx,
                                                  ossl_unused void *provctx)
{
    return known_gettable_ctx_params;
}

static int kmac_get_ctx_params(void *vmacctx, OSSL_PARAM params[])
{
    struct kmac_data_st *kctx = vmacctx;
    OSSL_PARAM *p;
    int sz;

    if ((p = OSSL_PARAM_locate(params, OSSL_MAC_PARAM_SIZE)) != NULL
            && !OSSL_PARAM_set_size_t(p, kctx->out_len))
        return 0;

    if ((p = OSSL_PARAM_locate(params, OSSL_MAC_PARAM_BLOCK_SIZE)) != NULL) {
        sz = EVP_MD_block_size(ossl_prov_digest_md(&kctx->digest));
        if (!OSSL_PARAM_set_int(p, sz))
            return 0;
    }

    if (!OSSL_FIPS_IND_GET_CTX_PARAM(kctx, params))
        return 0;

    return 1;
}

static const OSSL_PARAM known_settable_ctx_params[] = {
    OSSL_PARAM_int(OSSL_MAC_PARAM_XOF, NULL),
    OSSL_PARAM_size_t(OSSL_MAC_PARAM_SIZE, NULL),
    OSSL_PARAM_octet_string(OSSL_MAC_PARAM_KEY, NULL, 0),
    OSSL_PARAM_octet_string(OSSL_MAC_PARAM_CUSTOM, NULL, 0),
    OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_MAC_PARAM_FIPS_NO_SHORT_MAC)
    OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_MAC_PARAM_FIPS_KEY_CHECK)
    OSSL_PARAM_END
};
static const OSSL_PARAM *kmac_settable_ctx_params(ossl_unused void *ctx,
                                                  ossl_unused void *provctx)
{
    return known_settable_ctx_params;
}

/*
 * The following params can be set any time before final():
 *     - "outlen" or "size":    The requested output length.
 *     - "xof":                 If set, this indicates that right_encoded(0)
 *                              is part of the digested data, otherwise it
 *                              uses right_encoded(requested output length).
 *
 * All other params should be set before init().
 */
static int kmac_set_ctx_params(void *vmacctx, const OSSL_PARAM *params)
{
    struct kmac_data_st *kctx = vmacctx;
    const OSSL_PARAM *p;

    if (ossl_param_is_empty(params))
        return 1;

    if (!OSSL_FIPS_IND_SET_CTX_PARAM(kctx, OSSL_FIPS_IND_SETTABLE0, params,
                                     OSSL_MAC_PARAM_FIPS_NO_SHORT_MAC))
        return 0;
    if (!OSSL_FIPS_IND_SET_CTX_PARAM(kctx, OSSL_FIPS_IND_SETTABLE1, params,
                                     OSSL_MAC_PARAM_FIPS_KEY_CHECK))
        return 0;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_MAC_PARAM_XOF)) != NULL
        && !OSSL_PARAM_get_int(p, &kctx->xof_mode))
        return 0;
    if ((p = OSSL_PARAM_locate_const(params, OSSL_MAC_PARAM_SIZE)) != NULL) {
        size_t sz = 0;

        if (!OSSL_PARAM_get_size_t(p, &sz))
            return 0;
        if (sz > KMAC_MAX_OUTPUT_LEN) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_OUTPUT_LENGTH);
            return 0;
        }
#ifdef FIPS_MODULE
        /* SP 800-185 8.4.2 mandates a minimum of 32 bits of output */
        if (sz < 32 / 8) {
            if (!OSSL_FIPS_IND_ON_UNAPPROVED(kctx, OSSL_FIPS_IND_SETTABLE0,
                                             PROV_LIBCTX_OF(kctx->provctx),
                                             "KMAC", "length",
                                             ossl_fips_config_no_short_mac)) {
                ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_OUTPUT_LENGTH);
                return 0;
            }
        }
#endif
        kctx->out_len = sz;
    }
    if ((p = OSSL_PARAM_locate_const(params, OSSL_MAC_PARAM_KEY)) != NULL
            && !kmac_setkey(kctx, p->data, p->data_size))
        return 0;
    if ((p = OSSL_PARAM_locate_const(params, OSSL_MAC_PARAM_CUSTOM))
        != NULL) {
        if (p->data_size > KMAC_MAX_CUSTOM) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_CUSTOM_LENGTH);
            return 0;
        }
        if (!encode_string(kctx->custom, sizeof(kctx->custom), &kctx->custom_len,
                           p->data, p->data_size))
            return 0;
    }
    return 1;
}

/* Encoding/Padding Methods. */

/* Returns the number of bytes required to store 'bits' into a byte array */
static unsigned int get_encode_size(size_t bits)
{
    unsigned int cnt = 0, sz = sizeof(size_t);

    while (bits && (cnt < sz)) {
        ++cnt;
        bits >>= 8;
    }
    /* If bits is zero 1 byte is required */
    if (cnt == 0)
        cnt = 1;
    return cnt;
}

/*
 * Convert an integer into bytes . The number of bytes is appended
 * to the end of the buffer. Returns an array of bytes 'out' of size
 * *out_len.
 *
 * e.g if bits = 32, out[2] = { 0x20, 0x01 }
 */
static int right_encode(unsigned char *out, size_t out_max_len, size_t *out_len,
                        size_t bits)
{
    unsigned int len = get_encode_size(bits);
    int i;

    if (len >= out_max_len) {
        ERR_raise(ERR_LIB_PROV, PROV_R_LENGTH_TOO_LARGE);
        return 0;
    }

    /* MSB's are at the start of the bytes array */
    for (i = len - 1; i >= 0; --i) {
        out[i] = (unsigned char)(bits & 0xFF);
        bits >>= 8;
    }
    /* Tack the length onto the end */
    out[len] = (unsigned char)len;

    /* The Returned length includes the tacked on byte */
    *out_len = len + 1;
    return 1;
}

/*
 * Encodes a string with a left encoded length added. Note that the
 * in_len is converted to bits (*8).
 *
 * e.g- in="KMAC" gives out[6] = { 0x01, 0x20, 0x4B, 0x4D, 0x41, 0x43 }
 *                                 len   bits    K     M     A     C
 */
static int encode_string(unsigned char *out, size_t out_max_len, size_t *out_len,
                         const unsigned char *in, size_t in_len)
{
    if (in == NULL) {
        *out_len = 0;
    } else {
        size_t i, bits, len, sz;

        bits = 8 * in_len;
        len = get_encode_size(bits);
        sz = 1 + len + in_len;

        if (sz > out_max_len) {
            ERR_raise(ERR_LIB_PROV, PROV_R_LENGTH_TOO_LARGE);
            return 0;
        }

        out[0] = (unsigned char)len;
        for (i = len; i > 0; --i) {
            out[i] = (bits & 0xFF);
            bits >>= 8;
        }
        memcpy(out + len + 1, in, in_len);
        *out_len = sz;
    }
    return 1;
}

/*
 * Returns a zero padded encoding of the inputs in1 and an optional
 * in2 (can be NULL). The padded output must be a multiple of the blocksize 'w'.
 * The value of w is in bytes (< 256).
 *
 * The returned output is:
 *    zero_padded(multiple of w, (left_encode(w) || in1 [|| in2])
 */
static int bytepad(unsigned char *out, size_t *out_len,
                   const unsigned char *in1, size_t in1_len,
                   const unsigned char *in2, size_t in2_len, size_t w)
{
    int len;
    unsigned char *p = out;
    int sz = w;

    if (out == NULL) {
        if (out_len == NULL) {
            ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_NULL_PARAMETER);
            return 0;
        }
        sz = 2 + in1_len + (in2 != NULL ? in2_len : 0);
        *out_len = (sz + w - 1) / w * w;
        return 1;
    }

    if (!ossl_assert(w <= 255))
        return 0;

    /* Left encoded w */
    *p++ = 1;
    *p++ = (unsigned char)w;
    /* || in1 */
    memcpy(p, in1, in1_len);
    p += in1_len;
    /* [ || in2 ] */
    if (in2 != NULL && in2_len > 0) {
        memcpy(p, in2, in2_len);
        p += in2_len;
    }
    /* Figure out the pad size (divisible by w) */
    len = p - out;
    sz = (len + w - 1) / w * w;
    /* zero pad the end of the buffer */
    if (sz != len)
        memset(p, 0, sz - len);
    if (out_len != NULL)
        *out_len = sz;
    return 1;
}

/* Returns out = bytepad(encode_string(in), w) */
static int kmac_bytepad_encode_key(unsigned char *out, size_t out_max_len,
                                   size_t *out_len,
                                   const unsigned char *in, size_t in_len,
                                   size_t w)
{
    unsigned char tmp[KMAC_MAX_KEY + KMAC_MAX_ENCODED_HEADER_LEN];
    size_t tmp_len;

    if (!encode_string(tmp, sizeof(tmp), &tmp_len, in, in_len))
        return 0;
    if (!bytepad(NULL, out_len, tmp, tmp_len, NULL, 0, w))
        return 0;
    if (!ossl_assert(*out_len <= out_max_len))
        return 0;
    return bytepad(out, NULL, tmp, tmp_len, NULL, 0, w);
}

#define IMPLEMENT_KMAC_TABLE(size, funcname, newname)                          \
const OSSL_DISPATCH ossl_kmac##size##_##funcname[] =                           \
{                                                                              \
    { OSSL_FUNC_MAC_NEWCTX, (void (*)(void))kmac##size##_##newname },          \
    { OSSL_FUNC_MAC_DUPCTX, (void (*)(void))kmac_dup },                        \
    { OSSL_FUNC_MAC_FREECTX, (void (*)(void))kmac_free },                      \
    { OSSL_FUNC_MAC_INIT, (void (*)(void))kmac_init },                         \
    { OSSL_FUNC_MAC_UPDATE, (void (*)(void))kmac_update },                     \
    { OSSL_FUNC_MAC_FINAL, (void (*)(void))kmac_final },                       \
    { OSSL_FUNC_MAC_GETTABLE_CTX_PARAMS,                                       \
      (void (*)(void))kmac_gettable_ctx_params },                              \
    { OSSL_FUNC_MAC_GET_CTX_PARAMS, (void (*)(void))kmac_get_ctx_params },     \
    { OSSL_FUNC_MAC_SETTABLE_CTX_PARAMS,                                       \
      (void (*)(void))kmac_settable_ctx_params },                              \
    { OSSL_FUNC_MAC_SET_CTX_PARAMS, (void (*)(void))kmac_set_ctx_params },     \
    OSSL_DISPATCH_END                                                          \
}

#define KMAC_TABLE(size) IMPLEMENT_KMAC_TABLE(size, functions, new)

KMAC_TABLE(128);
KMAC_TABLE(256);

#ifdef FIPS_MODULE
# define KMAC_INTERNAL_TABLE(size)                                             \
static OSSL_FUNC_mac_newctx_fn kmac##size##_internal_new;                      \
static void *kmac##size##_internal_new(void *provctx)                          \
{                                                                              \
    struct kmac_data_st *macctx = kmac##size##_new(provctx);                   \
                                                                               \
    if (macctx != NULL)                                                        \
        macctx->internal = 1;                                                  \
    return macctx;                                                             \
}                                                                              \
IMPLEMENT_KMAC_TABLE(size, internal_functions, internal_new)

KMAC_INTERNAL_TABLE(128);
KMAC_INTERNAL_TABLE(256);
#endif /* FIPS_MODULE */
