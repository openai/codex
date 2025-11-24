/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/core_names.h>
#include <openssl/crypto.h>
#include <openssl/evp.h>
#include <openssl/params.h>
#include <openssl/err.h>
#include <openssl/proverr.h>
#include "internal/numbers.h"
#include "internal/sha3.h"
#include "prov/digestcommon.h"
#include "prov/implementations.h"

#define SHA3_FLAGS PROV_DIGEST_FLAG_ALGID_ABSENT
#define SHAKE_FLAGS (PROV_DIGEST_FLAG_XOF | PROV_DIGEST_FLAG_ALGID_ABSENT)
#define KMAC_FLAGS PROV_DIGEST_FLAG_XOF

/*
 * Forward declaration of any unique methods implemented here. This is not strictly
 * necessary for the compiler, but provides an assurance that the signatures
 * of the functions in the dispatch table are correct.
 */
static OSSL_FUNC_digest_init_fn keccak_init;
static OSSL_FUNC_digest_init_fn keccak_init_params;
static OSSL_FUNC_digest_update_fn keccak_update;
static OSSL_FUNC_digest_final_fn keccak_final;
static OSSL_FUNC_digest_freectx_fn keccak_freectx;
static OSSL_FUNC_digest_copyctx_fn keccak_copyctx;
static OSSL_FUNC_digest_dupctx_fn keccak_dupctx;
static OSSL_FUNC_digest_squeeze_fn shake_squeeze;
static OSSL_FUNC_digest_get_ctx_params_fn shake_get_ctx_params;
static OSSL_FUNC_digest_gettable_ctx_params_fn shake_gettable_ctx_params;
static OSSL_FUNC_digest_set_ctx_params_fn shake_set_ctx_params;
static OSSL_FUNC_digest_settable_ctx_params_fn shake_settable_ctx_params;
static sha3_absorb_fn generic_sha3_absorb;
static sha3_final_fn generic_sha3_final;
static sha3_squeeze_fn generic_sha3_squeeze;

#if defined(OPENSSL_CPUID_OBJ) && defined(__s390__) && defined(KECCAK1600_ASM)
/*
 * IBM S390X support
 */
# include "s390x_arch.h"
# define S390_SHA3 1
# define S390_SHA3_CAPABLE(name) \
    ((OPENSSL_s390xcap_P.kimd[0] & S390X_CAPBIT(S390X_##name)) && \
     (OPENSSL_s390xcap_P.klmd[0] & S390X_CAPBIT(S390X_##name)))

#endif

static int keccak_init(void *vctx, ossl_unused const OSSL_PARAM params[])
{
    if (!ossl_prov_is_running())
        return 0;
    /* The newctx() handles most of the ctx fixed setup. */
    ossl_sha3_reset((KECCAK1600_CTX *)vctx);
    return 1;
}

static int keccak_init_params(void *vctx, const OSSL_PARAM params[])
{
    return keccak_init(vctx, NULL)
            && shake_set_ctx_params(vctx, params);
}

static int keccak_update(void *vctx, const unsigned char *inp, size_t len)
{
    KECCAK1600_CTX *ctx = vctx;
    const size_t bsz = ctx->block_size;
    size_t num, rem;

    if (len == 0)
        return 1;

    /* Is there anything in the buffer already ? */
    if ((num = ctx->bufsz) != 0) {
        /* Calculate how much space is left in the buffer */
        rem = bsz - num;
        /* If the new input does not fill the buffer then just add it */
        if (len < rem) {
            memcpy(ctx->buf + num, inp, len);
            ctx->bufsz += len;
            return 1;
        }
        /* otherwise fill up the buffer and absorb the buffer */
        memcpy(ctx->buf + num, inp, rem);
        /* Update the input pointer */
        inp += rem;
        len -= rem;
        ctx->meth.absorb(ctx, ctx->buf, bsz);
        ctx->bufsz = 0;
    }
    /* Absorb the input - rem = leftover part of the input < blocksize) */
    rem = ctx->meth.absorb(ctx, inp, len);
    /* Copy the leftover bit of the input into the buffer */
    if (rem) {
        memcpy(ctx->buf, inp + len - rem, rem);
        ctx->bufsz = rem;
    }
    return 1;
}

static int keccak_final(void *vctx, unsigned char *out, size_t *outl,
                        size_t outlen)
{
    int ret = 1;
    KECCAK1600_CTX *ctx = vctx;

    if (!ossl_prov_is_running())
        return 0;
    if (ctx->md_size == SIZE_MAX) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_DIGEST_LENGTH);
        return 0;
    }
    if (outlen > 0)
        ret = ctx->meth.final(ctx, out, ctx->md_size);

    *outl = ctx->md_size;
    return ret;
}

static int shake_squeeze(void *vctx, unsigned char *out, size_t *outl,
                         size_t outlen)
{
    int ret = 1;
    KECCAK1600_CTX *ctx = vctx;

    if (!ossl_prov_is_running())
        return 0;
    if (ctx->meth.squeeze == NULL)
        return 0;
    if (outlen > 0)
        ret = ctx->meth.squeeze(ctx, out, outlen);

    *outl = outlen;
    return ret;
}

/*-
 * Generic software version of the absorb() and final().
 */
static size_t generic_sha3_absorb(void *vctx, const void *inp, size_t len)
{
    KECCAK1600_CTX *ctx = vctx;

    if (!(ctx->xof_state == XOF_STATE_INIT ||
          ctx->xof_state == XOF_STATE_ABSORB))
        return 0;
    ctx->xof_state = XOF_STATE_ABSORB;
    return SHA3_absorb(ctx->A, inp, len, ctx->block_size);
}

static int generic_sha3_final(void *vctx, unsigned char *out, size_t outlen)
{
    return ossl_sha3_final((KECCAK1600_CTX *)vctx, out, outlen);
}

static int generic_sha3_squeeze(void *vctx, unsigned char *out, size_t outlen)
{
    return ossl_sha3_squeeze((KECCAK1600_CTX *)vctx, out, outlen);
}

static PROV_SHA3_METHOD sha3_generic_md = {
    generic_sha3_absorb,
    generic_sha3_final,
    NULL
};

static PROV_SHA3_METHOD shake_generic_md =
{
    generic_sha3_absorb,
    generic_sha3_final,
    generic_sha3_squeeze
};

#if defined(S390_SHA3)

static sha3_absorb_fn s390x_sha3_absorb;
static sha3_final_fn s390x_sha3_final;
static sha3_final_fn s390x_shake_final;

/*-
 * The platform specific parts of the absorb() and final() for S390X.
 */
static size_t s390x_sha3_absorb(void *vctx, const void *inp, size_t len)
{
    KECCAK1600_CTX *ctx = vctx;
    size_t rem = len % ctx->block_size;
    unsigned int fc;

    if (!(ctx->xof_state == XOF_STATE_INIT ||
          ctx->xof_state == XOF_STATE_ABSORB))
        return 0;
    if (len - rem > 0) {
        fc = ctx->pad;
        fc |= ctx->xof_state == XOF_STATE_INIT ? S390X_KIMD_NIP : 0;
        ctx->xof_state = XOF_STATE_ABSORB;
        s390x_kimd(inp, len - rem, fc, ctx->A);
    }
    return rem;
}

static int s390x_sha3_final(void *vctx, unsigned char *out, size_t outlen)
{
    KECCAK1600_CTX *ctx = vctx;
    unsigned int fc;

    if (!ossl_prov_is_running())
        return 0;
    if (!(ctx->xof_state == XOF_STATE_INIT ||
          ctx->xof_state == XOF_STATE_ABSORB))
        return 0;
    fc = ctx->pad | S390X_KLMD_DUFOP;
    fc |= ctx->xof_state == XOF_STATE_INIT ? S390X_KLMD_NIP : 0;
    ctx->xof_state = XOF_STATE_FINAL;
    s390x_klmd(ctx->buf, ctx->bufsz, NULL, 0, fc, ctx->A);
    memcpy(out, ctx->A, outlen);
    return 1;
}

static int s390x_shake_final(void *vctx, unsigned char *out, size_t outlen)
{
    KECCAK1600_CTX *ctx = vctx;
    unsigned int fc;

    if (!ossl_prov_is_running())
        return 0;
    if (!(ctx->xof_state == XOF_STATE_INIT ||
          ctx->xof_state == XOF_STATE_ABSORB))
        return 0;
    fc = ctx->pad | S390X_KLMD_DUFOP;
    fc |= ctx->xof_state == XOF_STATE_INIT ? S390X_KLMD_NIP : 0;
    ctx->xof_state = XOF_STATE_FINAL;
    s390x_klmd(ctx->buf, ctx->bufsz, out, outlen, fc, ctx->A);
    return 1;
}

static int s390x_shake_squeeze(void *vctx, unsigned char *out, size_t outlen)
{
    KECCAK1600_CTX *ctx = vctx;
    unsigned int fc;
    size_t len;

    if (!ossl_prov_is_running())
        return 0;
    if (ctx->xof_state == XOF_STATE_FINAL)
        return 0;
    /*
     * On the first squeeze call, finish the absorb process (incl. padding).
     */
    if (ctx->xof_state != XOF_STATE_SQUEEZE) {
        fc = ctx->pad;
        fc |= ctx->xof_state == XOF_STATE_INIT ? S390X_KLMD_NIP : 0;
        ctx->xof_state = XOF_STATE_SQUEEZE;
        s390x_klmd(ctx->buf, ctx->bufsz, out, outlen, fc, ctx->A);
        ctx->bufsz = outlen % ctx->block_size;
        /* reuse ctx->bufsz to count bytes squeezed from current sponge */
        return 1;
    }
    ctx->xof_state = XOF_STATE_SQUEEZE;
    if (ctx->bufsz != 0) {
        len = ctx->block_size - ctx->bufsz;
        if (outlen < len)
            len = outlen;
        memcpy(out, (char *)ctx->A + ctx->bufsz, len);
        out += len;
        outlen -= len;
        ctx->bufsz += len;
        if (ctx->bufsz == ctx->block_size)
            ctx->bufsz = 0;
    }
    if (outlen == 0)
        return 1;
    s390x_klmd(NULL, 0, out, outlen, ctx->pad | S390X_KLMD_PS, ctx->A);
    ctx->bufsz = outlen % ctx->block_size;

    return 1;
}

static int s390x_keccakc_final(void *vctx, unsigned char *out, size_t outlen,
                               int padding)
{
    KECCAK1600_CTX *ctx = vctx;
    size_t bsz = ctx->block_size;
    size_t num = ctx->bufsz;
    size_t needed = outlen;
    unsigned int fc;

    if (!ossl_prov_is_running())
        return 0;
    if (!(ctx->xof_state == XOF_STATE_INIT ||
          ctx->xof_state == XOF_STATE_ABSORB))
        return 0;
    fc = ctx->pad;
    fc |= ctx->xof_state == XOF_STATE_INIT ? S390X_KIMD_NIP : 0;
    ctx->xof_state = XOF_STATE_FINAL;
    if (outlen == 0)
        return 1;
    memset(ctx->buf + num, 0, bsz - num);
    ctx->buf[num] = padding;
    ctx->buf[bsz - 1] |= 0x80;
    s390x_kimd(ctx->buf, bsz, fc, ctx->A);
    num = needed > bsz ? bsz : needed;
    memcpy(out, ctx->A, num);
    needed -= num;
    if (needed > 0)
        s390x_klmd(NULL, 0, out + bsz, needed,
                   ctx->pad | S390X_KLMD_PS | S390X_KLMD_DUFOP, ctx->A);

    return 1;
}

static int s390x_keccak_final(void *vctx, unsigned char *out, size_t outlen)
{
    return s390x_keccakc_final(vctx, out, outlen, 0x01);
}

static int s390x_kmac_final(void *vctx, unsigned char *out, size_t outlen)
{
    return s390x_keccakc_final(vctx, out, outlen, 0x04);
}

static int s390x_keccakc_squeeze(void *vctx, unsigned char *out, size_t outlen,
                                 int padding)
{
    KECCAK1600_CTX *ctx = vctx;
    size_t len;
    unsigned int fc;

    if (!ossl_prov_is_running())
        return 0;
    if (ctx->xof_state == XOF_STATE_FINAL)
        return 0;
    /*
     * On the first squeeze call, finish the absorb process
     * by adding the trailing padding and then doing
     * a final absorb.
     */
    if (ctx->xof_state != XOF_STATE_SQUEEZE) {
        len = ctx->block_size - ctx->bufsz;
        memset(ctx->buf + ctx->bufsz, 0, len);
        ctx->buf[ctx->bufsz] = padding;
        ctx->buf[ctx->block_size - 1] |= 0x80;
        fc = ctx->pad;
        fc |= ctx->xof_state == XOF_STATE_INIT ? S390X_KIMD_NIP : 0;
        s390x_kimd(ctx->buf, ctx->block_size, fc, ctx->A);
        ctx->bufsz = 0;
        /* reuse ctx->bufsz to count bytes squeezed from current sponge */
    }
    if (ctx->bufsz != 0 || ctx->xof_state != XOF_STATE_SQUEEZE) {
        len = ctx->block_size - ctx->bufsz;
        if (outlen < len)
            len = outlen;
        memcpy(out, (char *)ctx->A + ctx->bufsz, len);
        out += len;
        outlen -= len;
        ctx->bufsz += len;
        if (ctx->bufsz == ctx->block_size)
            ctx->bufsz = 0;
    }
    ctx->xof_state = XOF_STATE_SQUEEZE;
    if (outlen == 0)
        return 1;
    s390x_klmd(NULL, 0, out, outlen, ctx->pad | S390X_KLMD_PS, ctx->A);
    ctx->bufsz = outlen % ctx->block_size;

    return 1;
}

static int s390x_keccak_squeeze(void *vctx, unsigned char *out, size_t outlen)
{
     return s390x_keccakc_squeeze(vctx, out, outlen, 0x01);
}

static int s390x_kmac_squeeze(void *vctx, unsigned char *out, size_t outlen)
{
     return s390x_keccakc_squeeze(vctx, out, outlen, 0x04);
}

static PROV_SHA3_METHOD sha3_s390x_md = {
    s390x_sha3_absorb,
    s390x_sha3_final,
    NULL,
};

static PROV_SHA3_METHOD keccak_s390x_md = {
    s390x_sha3_absorb,
    s390x_keccak_final,
    s390x_keccak_squeeze,
};

static PROV_SHA3_METHOD shake_s390x_md = {
    s390x_sha3_absorb,
    s390x_shake_final,
    s390x_shake_squeeze,
};

static PROV_SHA3_METHOD kmac_s390x_md = {
    s390x_sha3_absorb,
    s390x_kmac_final,
    s390x_kmac_squeeze,
};

# define SHAKE_SET_MD(uname, typ)                                              \
    if (S390_SHA3_CAPABLE(uname)) {                                            \
        ctx->pad = S390X_##uname;                                              \
        ctx->meth = typ##_s390x_md;                                            \
    } else {                                                                   \
        ctx->meth = shake_generic_md;                                          \
    }

# define SHA3_SET_MD(uname, typ)                                               \
    if (S390_SHA3_CAPABLE(uname)) {                                            \
        ctx->pad = S390X_##uname;                                              \
        ctx->meth = typ##_s390x_md;                                            \
    } else {                                                                   \
        ctx->meth = sha3_generic_md;                                           \
    }
# define KMAC_SET_MD(bitlen)                                                   \
    if (S390_SHA3_CAPABLE(SHAKE_##bitlen)) {                                   \
        ctx->pad = S390X_SHAKE_##bitlen;                                       \
        ctx->meth = kmac_s390x_md;                                             \
    } else {                                                                   \
        ctx->meth = sha3_generic_md;                                           \
    }
#elif defined(__aarch64__) && defined(KECCAK1600_ASM)
# include "arm_arch.h"

static sha3_absorb_fn armsha3_sha3_absorb;

size_t SHA3_absorb_cext(uint64_t A[5][5], const unsigned char *inp, size_t len,
                        size_t r);
/*-
 * Hardware-assisted ARMv8.2 SHA3 extension version of the absorb()
 */
static size_t armsha3_sha3_absorb(void *vctx, const void *inp, size_t len)
{
    KECCAK1600_CTX *ctx = vctx;

    return SHA3_absorb_cext(ctx->A, inp, len, ctx->block_size);
}

static PROV_SHA3_METHOD sha3_ARMSHA3_md = {
    armsha3_sha3_absorb,
    generic_sha3_final
};
static PROV_SHA3_METHOD shake_ARMSHA3_md =
{
    armsha3_sha3_absorb,
    generic_sha3_final,
    generic_sha3_squeeze
};
# define SHAKE_SET_MD(uname, typ)                                              \
    if (OPENSSL_armcap_P & ARMV8_HAVE_SHA3_AND_WORTH_USING) {                  \
        ctx->meth = shake_ARMSHA3_md;                                          \
    } else {                                                                   \
        ctx->meth = shake_generic_md;                                          \
    }

# define SHA3_SET_MD(uname, typ)                                               \
    if (OPENSSL_armcap_P & ARMV8_HAVE_SHA3_AND_WORTH_USING) {                  \
        ctx->meth = sha3_ARMSHA3_md;                                           \
    } else {                                                                   \
        ctx->meth = sha3_generic_md;                                           \
    }
# define KMAC_SET_MD(bitlen)                                                   \
    if (OPENSSL_armcap_P & ARMV8_HAVE_SHA3_AND_WORTH_USING) {                  \
        ctx->meth = sha3_ARMSHA3_md;                                           \
    } else {                                                                   \
        ctx->meth = sha3_generic_md;                                           \
    }
#else
# define SHA3_SET_MD(uname, typ) ctx->meth = sha3_generic_md;
# define KMAC_SET_MD(bitlen) ctx->meth = sha3_generic_md;
# define SHAKE_SET_MD(uname, typ) ctx->meth = shake_generic_md;
#endif /* S390_SHA3 */

#define SHA3_newctx(typ, uname, name, bitlen, pad)                             \
static OSSL_FUNC_digest_newctx_fn name##_newctx;                               \
static void *name##_newctx(void *provctx)                                      \
{                                                                              \
    KECCAK1600_CTX *ctx = ossl_prov_is_running() ? OPENSSL_zalloc(sizeof(*ctx)) \
                                                : NULL;                        \
                                                                               \
    if (ctx == NULL)                                                           \
        return NULL;                                                           \
    ossl_sha3_init(ctx, pad, bitlen);                                          \
    SHA3_SET_MD(uname, typ)                                                    \
    return ctx;                                                                \
}

#define SHAKE_newctx(typ, uname, name, bitlen, mdlen, pad)                     \
static OSSL_FUNC_digest_newctx_fn name##_newctx;                               \
static void *name##_newctx(void *provctx)                                      \
{                                                                              \
    KECCAK1600_CTX *ctx = ossl_prov_is_running() ? OPENSSL_zalloc(sizeof(*ctx))\
                                                : NULL;                        \
                                                                               \
    if (ctx == NULL)                                                           \
        return NULL;                                                           \
    ossl_keccak_init(ctx, pad, bitlen, mdlen);                                 \
    if (mdlen == 0)                                                            \
        ctx->md_size = SIZE_MAX;                                               \
    SHAKE_SET_MD(uname, typ)                                                   \
    return ctx;                                                                \
}

#define KMAC_newctx(uname, bitlen, pad)                                        \
static OSSL_FUNC_digest_newctx_fn uname##_newctx;                              \
static void *uname##_newctx(void *provctx)                                     \
{                                                                              \
    KECCAK1600_CTX *ctx = ossl_prov_is_running() ? OPENSSL_zalloc(sizeof(*ctx)) \
                                                : NULL;                        \
                                                                               \
    if (ctx == NULL)                                                           \
        return NULL;                                                           \
    ossl_keccak_init(ctx, pad, bitlen, 2 * bitlen);                            \
    KMAC_SET_MD(bitlen)                                                        \
    return ctx;                                                                \
}

#define PROV_FUNC_SHA3_DIGEST_COMMON(name, bitlen, blksize, dgstsize, flags)   \
PROV_FUNC_DIGEST_GET_PARAM(name, blksize, dgstsize, flags)                     \
const OSSL_DISPATCH ossl_##name##_functions[] = {                              \
    { OSSL_FUNC_DIGEST_NEWCTX, (void (*)(void))name##_newctx },                \
    { OSSL_FUNC_DIGEST_UPDATE, (void (*)(void))keccak_update },                \
    { OSSL_FUNC_DIGEST_FINAL, (void (*)(void))keccak_final },                  \
    { OSSL_FUNC_DIGEST_FREECTX, (void (*)(void))keccak_freectx },              \
    { OSSL_FUNC_DIGEST_DUPCTX, (void (*)(void))keccak_dupctx },                \
    { OSSL_FUNC_DIGEST_COPYCTX, (void (*)(void))keccak_copyctx },              \
    PROV_DISPATCH_FUNC_DIGEST_GET_PARAMS(name)

#define PROV_FUNC_SHA3_DIGEST(name, bitlen, blksize, dgstsize, flags)          \
    PROV_FUNC_SHA3_DIGEST_COMMON(name, bitlen, blksize, dgstsize, flags),      \
    { OSSL_FUNC_DIGEST_INIT, (void (*)(void))keccak_init },                    \
    PROV_DISPATCH_FUNC_DIGEST_CONSTRUCT_END

#define PROV_FUNC_SHAKE_DIGEST(name, bitlen, blksize, dgstsize, flags)         \
    PROV_FUNC_SHA3_DIGEST_COMMON(name, bitlen, blksize, dgstsize, flags),      \
    { OSSL_FUNC_DIGEST_SQUEEZE, (void (*)(void))shake_squeeze },               \
    { OSSL_FUNC_DIGEST_INIT, (void (*)(void))keccak_init_params },             \
    { OSSL_FUNC_DIGEST_SET_CTX_PARAMS, (void (*)(void))shake_set_ctx_params }, \
    { OSSL_FUNC_DIGEST_SETTABLE_CTX_PARAMS,                                    \
     (void (*)(void))shake_settable_ctx_params },                              \
    { OSSL_FUNC_DIGEST_GET_CTX_PARAMS, (void (*)(void))shake_get_ctx_params }, \
    { OSSL_FUNC_DIGEST_GETTABLE_CTX_PARAMS,                                    \
     (void (*)(void))shake_gettable_ctx_params },                              \
    PROV_DISPATCH_FUNC_DIGEST_CONSTRUCT_END

static void keccak_freectx(void *vctx)
{
    KECCAK1600_CTX *ctx = (KECCAK1600_CTX *)vctx;

    OPENSSL_clear_free(ctx,  sizeof(*ctx));
}

static void keccak_copyctx(void *voutctx, void *vinctx)
{
    KECCAK1600_CTX *outctx = (KECCAK1600_CTX *)voutctx;
    KECCAK1600_CTX *inctx = (KECCAK1600_CTX *)vinctx;

    *outctx = *inctx;
}

static void *keccak_dupctx(void *ctx)
{
    KECCAK1600_CTX *in = (KECCAK1600_CTX *)ctx;
    KECCAK1600_CTX *ret = ossl_prov_is_running() ? OPENSSL_malloc(sizeof(*ret))
                                                 : NULL;

    if (ret != NULL)
        *ret = *in;
    return ret;
}

static const OSSL_PARAM *shake_gettable_ctx_params(ossl_unused void *ctx,
                                                   ossl_unused void *provctx)
{
    static const OSSL_PARAM known_shake_gettable_ctx_params[] = {
        {OSSL_DIGEST_PARAM_XOFLEN, OSSL_PARAM_UNSIGNED_INTEGER, NULL, 0, 0},
        {OSSL_DIGEST_PARAM_SIZE, OSSL_PARAM_UNSIGNED_INTEGER, NULL, 0, 0},
        OSSL_PARAM_END
    };
    return known_shake_gettable_ctx_params;
}

static int shake_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
    OSSL_PARAM *p;
    KECCAK1600_CTX *ctx = (KECCAK1600_CTX *)vctx;

    if (ctx == NULL)
        return 0;
    if (ossl_param_is_empty(params))
        return 1;

    p = OSSL_PARAM_locate(params, OSSL_DIGEST_PARAM_XOFLEN);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, ctx->md_size)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
        return 0;
    }
    /* Size is an alias of xoflen */
    p = OSSL_PARAM_locate(params, OSSL_DIGEST_PARAM_SIZE);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, ctx->md_size)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
        return 0;
    }
    return 1;
}

static const OSSL_PARAM *shake_settable_ctx_params(ossl_unused void *ctx,
                                                   ossl_unused void *provctx)
{
    static const OSSL_PARAM known_shake_settable_ctx_params[] = {
        {OSSL_DIGEST_PARAM_XOFLEN, OSSL_PARAM_UNSIGNED_INTEGER, NULL, 0, 0},
        {OSSL_DIGEST_PARAM_SIZE, OSSL_PARAM_UNSIGNED_INTEGER, NULL, 0, 0},
        OSSL_PARAM_END
    };

    return known_shake_settable_ctx_params;
}

static int shake_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    const OSSL_PARAM *p;
    KECCAK1600_CTX *ctx = (KECCAK1600_CTX *)vctx;

    if (ctx == NULL)
        return 0;
    if (ossl_param_is_empty(params))
        return 1;

    p = OSSL_PARAM_locate_const(params, OSSL_DIGEST_PARAM_XOFLEN);
    if (p == NULL)
        p = OSSL_PARAM_locate_const(params, OSSL_DIGEST_PARAM_SIZE);

    if (p != NULL && !OSSL_PARAM_get_size_t(p, &ctx->md_size)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_TO_GET_PARAMETER);
        return 0;
    }
    return 1;
}

#define IMPLEMENT_SHA3_functions(bitlen)                                       \
    SHA3_newctx(sha3, SHA3_##bitlen, sha3_##bitlen, bitlen, '\x06')            \
    PROV_FUNC_SHA3_DIGEST(sha3_##bitlen, bitlen,                               \
                          SHA3_BLOCKSIZE(bitlen), SHA3_MDSIZE(bitlen),         \
                          SHA3_FLAGS)

#define IMPLEMENT_KECCAK_functions(bitlen)                                     \
    SHA3_newctx(keccak, KECCAK_##bitlen, keccak_##bitlen, bitlen, '\x01')      \
    PROV_FUNC_SHA3_DIGEST(keccak_##bitlen, bitlen,                             \
                          SHA3_BLOCKSIZE(bitlen), SHA3_MDSIZE(bitlen),         \
                          SHA3_FLAGS)

#define IMPLEMENT_SHAKE_functions(bitlen)                                      \
    SHAKE_newctx(shake, SHAKE_##bitlen, shake_##bitlen, bitlen,                \
                 0 /* no default md length */, '\x1f')                         \
    PROV_FUNC_SHAKE_DIGEST(shake_##bitlen, bitlen,                             \
                           SHA3_BLOCKSIZE(bitlen), 0,                          \
                           SHAKE_FLAGS)

#define IMPLEMENT_KMAC_functions(bitlen)                                       \
    KMAC_newctx(keccak_kmac_##bitlen, bitlen, '\x04')                          \
    PROV_FUNC_SHAKE_DIGEST(keccak_kmac_##bitlen, bitlen,                       \
                           SHA3_BLOCKSIZE(bitlen), KMAC_MDSIZE(bitlen),        \
                           KMAC_FLAGS)

/* ossl_sha3_224_functions */
IMPLEMENT_SHA3_functions(224)
/* ossl_sha3_256_functions */
IMPLEMENT_SHA3_functions(256)
/* ossl_sha3_384_functions */
IMPLEMENT_SHA3_functions(384)
/* ossl_sha3_512_functions */
IMPLEMENT_SHA3_functions(512)
/* ossl_keccak_224_functions */
IMPLEMENT_KECCAK_functions(224)
/* ossl_keccak_256_functions */
IMPLEMENT_KECCAK_functions(256)
/* ossl_keccak_384_functions */
IMPLEMENT_KECCAK_functions(384)
/* ossl_keccak_512_functions */
IMPLEMENT_KECCAK_functions(512)
/* ossl_shake_128_functions */
IMPLEMENT_SHAKE_functions(128)
/* ossl_shake_256_functions */
IMPLEMENT_SHAKE_functions(256)
/* ossl_keccak_kmac_128_functions */
IMPLEMENT_KMAC_functions(128)
/* ossl_keccak_kmac_256_functions */
IMPLEMENT_KMAC_functions(256)
