/*
 * Copyright 1998-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 *
 * Uses brotli compression library from https://github.com/google/brotli
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <openssl/objects.h>
#include "internal/comp.h"
#include <openssl/err.h>
#include "crypto/cryptlib.h"
#include "internal/bio.h"
#include "internal/thread_once.h"
#include "comp_local.h"

COMP_METHOD *COMP_brotli(void);

#ifdef OPENSSL_NO_BROTLI
# undef BROTLI_SHARED
#else

# include <brotli/decode.h>
# include <brotli/encode.h>

/* memory allocations functions for brotli initialisation */
static void *brotli_alloc(void *opaque, size_t size)
{
    return OPENSSL_zalloc(size);
}

static void brotli_free(void *opaque, void *address)
{
    OPENSSL_free(address);
}

/*
 * When OpenSSL is built on Windows, we do not want to require that
 * the BROTLI.DLL be available in order for the OpenSSL DLLs to
 * work.  Therefore, all BROTLI routines are loaded at run time
 * and we do not link to a .LIB file when BROTLI_SHARED is set.
 */
# if defined(OPENSSL_SYS_WINDOWS) || defined(OPENSSL_SYS_WIN32)
#  include <windows.h>
# endif

# ifdef BROTLI_SHARED
#  include "internal/dso.h"

/* Function pointers */
typedef BrotliEncoderState *(*encode_init_ft)(brotli_alloc_func, brotli_free_func, void *);
typedef BROTLI_BOOL (*encode_stream_ft)(BrotliEncoderState *, BrotliEncoderOperation, size_t *, const uint8_t **, size_t *, uint8_t **, size_t *);
typedef BROTLI_BOOL (*encode_has_more_ft)(BrotliEncoderState *);
typedef void (*encode_end_ft)(BrotliEncoderState *);
typedef BROTLI_BOOL (*encode_oneshot_ft)(int, int, BrotliEncoderMode, size_t, const uint8_t in[], size_t *, uint8_t out[]);

typedef BrotliDecoderState *(*decode_init_ft)(brotli_alloc_func, brotli_free_func, void *);
typedef BROTLI_BOOL (*decode_stream_ft)(BrotliDecoderState *, size_t *, const uint8_t **, size_t *, uint8_t **, size_t *);
typedef BROTLI_BOOL (*decode_has_more_ft)(BrotliDecoderState *);
typedef void (*decode_end_ft)(BrotliDecoderState *);
typedef BrotliDecoderErrorCode (*decode_error_ft)(BrotliDecoderState *);
typedef const char *(*decode_error_string_ft)(BrotliDecoderErrorCode);
typedef BROTLI_BOOL (*decode_is_finished_ft)(BrotliDecoderState *);
typedef BrotliDecoderResult (*decode_oneshot_ft)(size_t, const uint8_t in[], size_t *, uint8_t out[]);

static encode_init_ft p_encode_init = NULL;
static encode_stream_ft p_encode_stream = NULL;
static encode_has_more_ft p_encode_has_more = NULL;
static encode_end_ft p_encode_end = NULL;
static encode_oneshot_ft p_encode_oneshot = NULL;

static decode_init_ft p_decode_init = NULL;
static decode_stream_ft p_decode_stream = NULL;
static decode_has_more_ft p_decode_has_more = NULL;
static decode_end_ft p_decode_end = NULL;
static decode_error_ft p_decode_error = NULL;
static decode_error_string_ft p_decode_error_string = NULL;
static decode_is_finished_ft p_decode_is_finished = NULL;
static decode_oneshot_ft p_decode_oneshot = NULL;

static DSO *brotli_encode_dso = NULL;
static DSO *brotli_decode_dso = NULL;

#  define BrotliEncoderCreateInstance p_encode_init
#  define BrotliEncoderCompressStream p_encode_stream
#  define BrotliEncoderHasMoreOutput p_encode_has_more
#  define BrotliEncoderDestroyInstance p_encode_end
#  define BrotliEncoderCompress p_encode_oneshot

#  define BrotliDecoderCreateInstance p_decode_init
#  define BrotliDecoderDecompressStream p_decode_stream
#  define BrotliDecoderHasMoreOutput p_decode_has_more
#  define BrotliDecoderDestroyInstance p_decode_end
#  define BrotliDecoderGetErrorCode p_decode_error
#  define BrotliDecoderErrorString p_decode_error_string
#  define BrotliDecoderIsFinished p_decode_is_finished
#  define BrotliDecoderDecompress p_decode_oneshot

# endif /* ifdef BROTLI_SHARED */


struct brotli_state {
    BrotliEncoderState *encoder;
    BrotliDecoderState *decoder;
};

static int brotli_stateful_init(COMP_CTX *ctx)
{
    struct brotli_state *state = OPENSSL_zalloc(sizeof(*state));

    if (state == NULL)
        return 0;

    state->encoder = BrotliEncoderCreateInstance(brotli_alloc, brotli_free, NULL);
    if (state->encoder == NULL)
        goto err;

    state->decoder = BrotliDecoderCreateInstance(brotli_alloc, brotli_free, NULL);
    if (state->decoder == NULL)
        goto err;

    ctx->data = state;
    return 1;
 err:
    BrotliDecoderDestroyInstance(state->decoder);
    BrotliEncoderDestroyInstance(state->encoder);
    OPENSSL_free(state);
    return 0;
}

static void brotli_stateful_finish(COMP_CTX *ctx)
{
    struct brotli_state *state = ctx->data;

    if (state != NULL) {
        BrotliDecoderDestroyInstance(state->decoder);
        BrotliEncoderDestroyInstance(state->encoder);
        OPENSSL_free(state);
        ctx->data = NULL;
    }
}

static ossl_ssize_t brotli_stateful_compress_block(COMP_CTX *ctx, unsigned char *out,
                                                   size_t olen, unsigned char *in,
                                                   size_t ilen)
{
    BROTLI_BOOL done;
    struct brotli_state *state = ctx->data;
    size_t in_avail = ilen;
    size_t out_avail = olen;

    if (state == NULL || olen > OSSL_SSIZE_MAX)
        return -1;

    if (ilen == 0)
        return 0;

    /*
     * The finish API does not provide a final output buffer,
     * so each compress operation has to be flushed, if all
     * the input data can't be accepted, or there is more output,
     * this has to be considered an error, since there is no more
     * output buffer space
     */
    done = BrotliEncoderCompressStream(state->encoder, BROTLI_OPERATION_FLUSH,
                                       &in_avail, (const uint8_t**)&in,
                                       &out_avail, &out, NULL);
    if (done == BROTLI_FALSE
            || in_avail != 0
            || BrotliEncoderHasMoreOutput(state->encoder))
        return -1;

    if (out_avail > olen)
        return -1;
    return (ossl_ssize_t)(olen - out_avail);
}

static ossl_ssize_t brotli_stateful_expand_block(COMP_CTX *ctx, unsigned char *out,
                                                 size_t olen, unsigned char *in,
                                                 size_t ilen)
{
    BrotliDecoderResult result;
    struct brotli_state *state = ctx->data;
    size_t in_avail = ilen;
    size_t out_avail = olen;

    if (state == NULL || olen > OSSL_SSIZE_MAX)
        return -1;

    if (ilen == 0)
        return 0;

    result = BrotliDecoderDecompressStream(state->decoder, &in_avail,
                                           (const uint8_t**)&in, &out_avail,
                                           &out, NULL);
    if (result == BROTLI_DECODER_RESULT_ERROR
            || in_avail != 0
            || BrotliDecoderHasMoreOutput(state->decoder))
        return -1;

    if (out_avail > olen)
        return -1;
    return (ossl_ssize_t)(olen - out_avail);
}

static COMP_METHOD brotli_stateful_method = {
    NID_brotli,
    LN_brotli,
    brotli_stateful_init,
    brotli_stateful_finish,
    brotli_stateful_compress_block,
    brotli_stateful_expand_block
};

static int brotli_oneshot_init(COMP_CTX *ctx)
{
    return 1;
}

static void brotli_oneshot_finish(COMP_CTX *ctx)
{
}

static ossl_ssize_t brotli_oneshot_compress_block(COMP_CTX *ctx, unsigned char *out,
                                                  size_t olen, unsigned char *in,
                                                  size_t ilen)
{
    size_t out_size = olen;
    ossl_ssize_t ret;

    if (ilen == 0)
        return 0;

    if (BrotliEncoderCompress(BROTLI_DEFAULT_QUALITY, BROTLI_DEFAULT_WINDOW,
                              BROTLI_DEFAULT_MODE, ilen, in,
                              &out_size, out) == BROTLI_FALSE)
        return -1;

    if (out_size > OSSL_SSIZE_MAX)
        return -1;
    ret = (ossl_ssize_t)out_size;
    if (ret < 0)
        return -1;
    return ret;
}

static ossl_ssize_t brotli_oneshot_expand_block(COMP_CTX *ctx, unsigned char *out,
                                                size_t olen, unsigned char *in,
                                                size_t ilen)
{
    size_t out_size = olen;
    ossl_ssize_t ret;

    if (ilen == 0)
        return 0;

    if (BrotliDecoderDecompress(ilen, in, &out_size, out) != BROTLI_DECODER_RESULT_SUCCESS)
        return -1;

    if (out_size > OSSL_SSIZE_MAX)
        return -1;
    ret = (ossl_ssize_t)out_size;
    if (ret < 0)
        return -1;
    return ret;
}

static COMP_METHOD brotli_oneshot_method = {
    NID_brotli,
    LN_brotli,
    brotli_oneshot_init,
    brotli_oneshot_finish,
    brotli_oneshot_compress_block,
    brotli_oneshot_expand_block
};

static CRYPTO_ONCE brotli_once = CRYPTO_ONCE_STATIC_INIT;
DEFINE_RUN_ONCE_STATIC(ossl_comp_brotli_init)
{
# ifdef BROTLI_SHARED
#  if defined(OPENSSL_SYS_WINDOWS) || defined(OPENSSL_SYS_WIN32)
#   define LIBBROTLIENC "BROTLIENC"
#   define LIBBROTLIDEC "BROTLIDEC"
#  else
#   define LIBBROTLIENC "brotlienc"
#   define LIBBROTLIDEC "brotlidec"
#  endif

    brotli_encode_dso = DSO_load(NULL, LIBBROTLIENC, NULL, 0);
    if (brotli_encode_dso != NULL) {
        p_encode_init = (encode_init_ft)DSO_bind_func(brotli_encode_dso, "BrotliEncoderCreateInstance");
        p_encode_stream = (encode_stream_ft)DSO_bind_func(brotli_encode_dso, "BrotliEncoderCompressStream");
        p_encode_has_more = (encode_has_more_ft)DSO_bind_func(brotli_encode_dso, "BrotliEncoderHasMoreOutput");
        p_encode_end = (encode_end_ft)DSO_bind_func(brotli_encode_dso, "BrotliEncoderDestroyInstance");
        p_encode_oneshot = (encode_oneshot_ft)DSO_bind_func(brotli_encode_dso, "BrotliEncoderCompress");
    }

    brotli_decode_dso = DSO_load(NULL, LIBBROTLIDEC, NULL, 0);
    if (brotli_decode_dso != NULL) {
        p_decode_init = (decode_init_ft)DSO_bind_func(brotli_decode_dso, "BrotliDecoderCreateInstance");
        p_decode_stream = (decode_stream_ft)DSO_bind_func(brotli_decode_dso, "BrotliDecoderDecompressStream");
        p_decode_has_more = (decode_has_more_ft)DSO_bind_func(brotli_decode_dso, "BrotliDecoderHasMoreOutput");
        p_decode_end = (decode_end_ft)DSO_bind_func(brotli_decode_dso, "BrotliDecoderDestroyInstance");
        p_decode_error = (decode_error_ft)DSO_bind_func(brotli_decode_dso, "BrotliDecoderGetErrorCode");
        p_decode_error_string = (decode_error_string_ft)DSO_bind_func(brotli_decode_dso, "BrotliDecoderErrorString");
        p_decode_is_finished = (decode_is_finished_ft)DSO_bind_func(brotli_decode_dso, "BrotliDecoderIsFinished");
        p_decode_oneshot = (decode_oneshot_ft)DSO_bind_func(brotli_decode_dso, "BrotliDecoderDecompress");
    }

    if (p_encode_init == NULL || p_encode_stream == NULL || p_encode_has_more == NULL
            || p_encode_end == NULL || p_encode_oneshot == NULL || p_decode_init == NULL
            || p_decode_stream == NULL || p_decode_has_more == NULL || p_decode_end == NULL
            || p_decode_error == NULL || p_decode_error_string == NULL || p_decode_is_finished == NULL
            || p_decode_oneshot == NULL) {
        ossl_comp_brotli_cleanup();
        return 0;
    }
# endif
    return 1;
}
#endif /* ifndef BROTLI / else */

COMP_METHOD *COMP_brotli(void)
{
    COMP_METHOD *meth = NULL;

#ifndef OPENSSL_NO_BROTLI
    if (RUN_ONCE(&brotli_once, ossl_comp_brotli_init))
        meth = &brotli_stateful_method;
#endif
    return meth;
}

COMP_METHOD *COMP_brotli_oneshot(void)
{
    COMP_METHOD *meth = NULL;

#ifndef OPENSSL_NO_BROTLI
    if (RUN_ONCE(&brotli_once, ossl_comp_brotli_init))
        meth = &brotli_oneshot_method;
#endif
    return meth;
}

/* Also called from OPENSSL_cleanup() */
void ossl_comp_brotli_cleanup(void)
{
#ifdef BROTLI_SHARED
    DSO_free(brotli_encode_dso);
    brotli_encode_dso = NULL;
    DSO_free(brotli_decode_dso);
    brotli_decode_dso = NULL;
    p_encode_init = NULL;
    p_encode_stream = NULL;
    p_encode_has_more = NULL;
    p_encode_end = NULL;
    p_encode_oneshot = NULL;
    p_decode_init = NULL;
    p_decode_stream = NULL;
    p_decode_has_more = NULL;
    p_decode_end = NULL;
    p_decode_error = NULL;
    p_decode_error_string = NULL;
    p_decode_is_finished = NULL;
    p_decode_oneshot = NULL;
#endif
}

#ifndef OPENSSL_NO_BROTLI

/* Brotli-based compression/decompression filter BIO */

typedef struct {
    struct { /* input structure */
        size_t avail_in;
        unsigned char *next_in;
        size_t avail_out;
        unsigned char *next_out;
        unsigned char *buf;
        size_t bufsize;
        BrotliDecoderState *state;
    } decode;
    struct { /* output structure */
        size_t avail_in;
        unsigned char *next_in;
        size_t avail_out;
        unsigned char *next_out;
        unsigned char *buf;
        size_t bufsize;
        BrotliEncoderState *state;
        int mode;                      /* Encoder mode to use */
        int done;
        unsigned char *ptr;
        size_t count;
    } encode;
} BIO_BROTLI_CTX;

# define BROTLI_DEFAULT_BUFSIZE 1024

static int bio_brotli_new(BIO *bi);
static int bio_brotli_free(BIO *bi);
static int bio_brotli_read(BIO *b, char *out, int outl);
static int bio_brotli_write(BIO *b, const char *in, int inl);
static long bio_brotli_ctrl(BIO *b, int cmd, long num, void *ptr);
static long bio_brotli_callback_ctrl(BIO *b, int cmd, BIO_info_cb *fp);

static const BIO_METHOD bio_meth_brotli = {
    BIO_TYPE_COMP,
    "brotli",
    /* TODO: Convert to new style write function */
    bwrite_conv,
    bio_brotli_write,
    /* TODO: Convert to new style read function */
    bread_conv,
    bio_brotli_read,
    NULL,                      /* bio_brotli_puts, */
    NULL,                      /* bio_brotli_gets, */
    bio_brotli_ctrl,
    bio_brotli_new,
    bio_brotli_free,
    bio_brotli_callback_ctrl
};
#endif

const BIO_METHOD *BIO_f_brotli(void)
{
#ifndef OPENSSL_NO_BROTLI
    if (RUN_ONCE(&brotli_once, ossl_comp_brotli_init))
        return &bio_meth_brotli;
#endif
    return NULL;
}

#ifndef OPENSSL_NO_BROTLI

static int bio_brotli_new(BIO *bi)
{
    BIO_BROTLI_CTX *ctx;

# ifdef BROTLI_SHARED
    if (!RUN_ONCE(&brotli_once, ossl_comp_brotli_init)) {
        ERR_raise(ERR_LIB_COMP, COMP_R_BROTLI_NOT_SUPPORTED);
        return 0;
    }
# endif
    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_COMP, ERR_R_MALLOC_FAILURE);
        return 0;
    }
    ctx->decode.bufsize = BROTLI_DEFAULT_BUFSIZE;
    ctx->decode.state = BrotliDecoderCreateInstance(brotli_alloc, brotli_free, NULL);
    if (ctx->decode.state == NULL)
        goto err;
    ctx->encode.bufsize = BROTLI_DEFAULT_BUFSIZE;
    ctx->encode.state = BrotliEncoderCreateInstance(brotli_alloc, brotli_free, NULL);
    if (ctx->encode.state == NULL)
        goto err;
    ctx->encode.mode = BROTLI_DEFAULT_MODE;
    BIO_set_init(bi, 1);
    BIO_set_data(bi, ctx);

    return 1;

 err:
    ERR_raise(ERR_LIB_COMP, ERR_R_MALLOC_FAILURE);
    BrotliDecoderDestroyInstance(ctx->decode.state);
    BrotliEncoderDestroyInstance(ctx->encode.state);
    OPENSSL_free(ctx);
    return 0;
}

static int bio_brotli_free(BIO *bi)
{
    BIO_BROTLI_CTX *ctx;

    if (bi == NULL)
        return 0;

    ctx = BIO_get_data(bi);
    if (ctx != NULL) {
        BrotliDecoderDestroyInstance(ctx->decode.state);
        OPENSSL_free(ctx->decode.buf);
        BrotliEncoderDestroyInstance(ctx->encode.state);
        OPENSSL_free(ctx->encode.buf);
        OPENSSL_free(ctx);
    }
    BIO_set_data(bi, NULL);
    BIO_set_init(bi, 0);

    return 1;
}

static int bio_brotli_read(BIO *b, char *out, int outl)
{
    BIO_BROTLI_CTX *ctx;
    BrotliDecoderResult bret;
    int ret;
    BIO *next = BIO_next(b);

    if (out == NULL || outl <= 0) {
        ERR_raise(ERR_LIB_COMP, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
#if INT_MAX > SIZE_MAX
    if ((unsigned int)outl > SIZE_MAX) {
        ERR_raise(ERR_LIB_COMP, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
#endif

    ctx = BIO_get_data(b);
    BIO_clear_retry_flags(b);
    if (ctx->decode.buf == NULL) {
        ctx->decode.buf = OPENSSL_malloc(ctx->decode.bufsize);
        if (ctx->decode.buf == NULL) {
            ERR_raise(ERR_LIB_COMP, ERR_R_MALLOC_FAILURE);
            return 0;
        }
        ctx->decode.next_in = ctx->decode.buf;
        ctx->decode.avail_in = 0;
    }

    /* Copy output data directly to supplied buffer */
    ctx->decode.next_out = (unsigned char *)out;
    ctx->decode.avail_out = (size_t)outl;
    for (;;) {
        /* Decompress while data available */
        while (ctx->decode.avail_in > 0 || BrotliDecoderHasMoreOutput(ctx->decode.state)) {
            bret = BrotliDecoderDecompressStream(ctx->decode.state, &ctx->decode.avail_in, (const uint8_t**)&ctx->decode.next_in,
                                                  &ctx->decode.avail_out, &ctx->decode.next_out, NULL);
            if (bret == BROTLI_DECODER_RESULT_ERROR) {
                ERR_raise(ERR_LIB_COMP, COMP_R_BROTLI_DECODE_ERROR);
                ERR_add_error_data(1, BrotliDecoderErrorString(BrotliDecoderGetErrorCode(ctx->decode.state)));
                return 0;
            }
            /* If EOF or we've read everything then return */
            if (BrotliDecoderIsFinished(ctx->decode.state) || ctx->decode.avail_out == 0)
                return (int)(outl - ctx->decode.avail_out);
        }

        /* If EOF */
        if (BrotliDecoderIsFinished(ctx->decode.state))
            return 0;

        /*
         * No data in input buffer try to read some in, if an error then
         * return the total data read.
         */
        ret = BIO_read(next, ctx->decode.buf, ctx->decode.bufsize);
        if (ret <= 0) {
            /* Total data read */
            int tot = outl - ctx->decode.avail_out;

            BIO_copy_next_retry(b);
            if (ret < 0)
                return (tot > 0) ? tot : ret;
            return tot;
        }
        ctx->decode.avail_in = ret;
        ctx->decode.next_in = ctx->decode.buf;
    }
}

static int bio_brotli_write(BIO *b, const char *in, int inl)
{
    BIO_BROTLI_CTX *ctx;
    BROTLI_BOOL brret;
    int ret;
    BIO *next = BIO_next(b);

    if (in == NULL || inl <= 0) {
        ERR_raise(ERR_LIB_COMP, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
#if INT_MAX > SIZE_MAX
    if ((unsigned int)inl > SIZE_MAX) {
        ERR_raise(ERR_LIB_COMP, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
#endif

    ctx = BIO_get_data(b);
    if (ctx->encode.done)
        return 0;

    BIO_clear_retry_flags(b);
    if (ctx->encode.buf == NULL) {
        ctx->encode.buf = OPENSSL_malloc(ctx->encode.bufsize);
        if (ctx->encode.buf == NULL) {
            ERR_raise(ERR_LIB_COMP, ERR_R_MALLOC_FAILURE);
            return 0;
        }
        ctx->encode.ptr = ctx->encode.buf;
        ctx->encode.count = 0;
        ctx->encode.next_out = ctx->encode.buf;
        ctx->encode.avail_out = ctx->encode.bufsize;
    }
    /* Obtain input data directly from supplied buffer */
    ctx->encode.next_in = (unsigned char *)in;
    ctx->encode.avail_in = (size_t)inl;
    for (;;) {
        /* If data in output buffer write it first */
        while (ctx->encode.count > 0) {
            ret = BIO_write(next, ctx->encode.ptr, ctx->encode.count);
            if (ret <= 0) {
                /* Total data written */
                int tot = inl - ctx->encode.avail_in;

                BIO_copy_next_retry(b);
                if (ret < 0)
                    return (tot > 0) ? tot : ret;
                return tot;
            }
            ctx->encode.ptr += ret;
            ctx->encode.count -= ret;
        }

        /* Have we consumed all supplied data? */
        if (ctx->encode.avail_in == 0 && !BrotliEncoderHasMoreOutput(ctx->encode.state))
            return inl;

        /* Compress some more */

        /* Reset buffer */
        ctx->encode.ptr = ctx->encode.buf;
        ctx->encode.next_out = ctx->encode.buf;
        ctx->encode.avail_out = ctx->encode.bufsize;
        /* Compress some more */
        brret = BrotliEncoderCompressStream(ctx->encode.state, BROTLI_OPERATION_FLUSH, &ctx->encode.avail_in, (const uint8_t**)&ctx->encode.next_in,
                                            &ctx->encode.avail_out, &ctx->encode.next_out, NULL);
        if (brret != BROTLI_TRUE) {
            ERR_raise(ERR_LIB_COMP, COMP_R_BROTLI_ENCODE_ERROR);
            ERR_add_error_data(1, "brotli encoder error");
            return 0;
        }
        ctx->encode.count = ctx->encode.bufsize - ctx->encode.avail_out;
    }
}

static int bio_brotli_flush(BIO *b)
{
    BIO_BROTLI_CTX *ctx;
    BROTLI_BOOL brret;
    int ret;
    BIO *next = BIO_next(b);

    ctx = BIO_get_data(b);

    /* If no data written or already flush show success */
    if (ctx->encode.buf == NULL || (ctx->encode.done && ctx->encode.count == 0))
        return 1;

    BIO_clear_retry_flags(b);
    /* No more input data */
    ctx->encode.next_in = NULL;
    ctx->encode.avail_in = 0;
    for (;;) {
        /* If data in output buffer write it first */
        while (ctx->encode.count > 0) {
            ret = BIO_write(next, ctx->encode.ptr, ctx->encode.count);
            if (ret <= 0) {
                BIO_copy_next_retry(b);
                return ret;
            }
            ctx->encode.ptr += ret;
            ctx->encode.count -= ret;
        }
        if (ctx->encode.done)
            return 1;

        /* Compress some more */

        /* Reset buffer */
        ctx->encode.ptr = ctx->encode.buf;
        ctx->encode.next_out = ctx->encode.buf;
        ctx->encode.avail_out = ctx->encode.bufsize;
        /* Compress some more */
        brret = BrotliEncoderCompressStream(ctx->encode.state, BROTLI_OPERATION_FINISH, &ctx->encode.avail_in,
                                            (const uint8_t**)&ctx->encode.next_in, &ctx->encode.avail_out, &ctx->encode.next_out, NULL);
        if (brret != BROTLI_TRUE) {
            ERR_raise(ERR_LIB_COMP, COMP_R_BROTLI_DECODE_ERROR);
            ERR_add_error_data(1, "brotli encoder error");
            return 0;
        }
        if (!BrotliEncoderHasMoreOutput(ctx->encode.state) && ctx->encode.avail_in == 0)
            ctx->encode.done = 1;
        ctx->encode.count = ctx->encode.bufsize - ctx->encode.avail_out;
    }
}

static long bio_brotli_ctrl(BIO *b, int cmd, long num, void *ptr)
{
    BIO_BROTLI_CTX *ctx;
    unsigned char *tmp;
    int ret = 0, *ip;
    size_t ibs, obs;
    BIO *next = BIO_next(b);

    if (next == NULL)
        return 0;
    ctx = BIO_get_data(b);
    switch (cmd) {

    case BIO_CTRL_RESET:
        ctx->encode.count = 0;
        ctx->encode.done = 0;
        ret = 1;
        break;

    case BIO_CTRL_FLUSH:
        ret = bio_brotli_flush(b);
        if (ret > 0) {
            ret = BIO_flush(next);
            BIO_copy_next_retry(b);
        }
        break;

    case BIO_C_SET_BUFF_SIZE:
        ibs = ctx->decode.bufsize;
        obs = ctx->encode.bufsize;
        if (ptr != NULL) {
            ip = ptr;
            if (*ip == 0)
                ibs = (size_t)num;
            else
                obs = (size_t)num;
        } else {
            ibs = (size_t)num;
            obs = ibs;
        }

        if (ibs > 0 && ibs != ctx->decode.bufsize) {
            /* Do not free/alloc, only reallocate */
            if (ctx->decode.buf != NULL) {
                tmp = OPENSSL_realloc(ctx->decode.buf, ibs);
                if (tmp == NULL)
                    return 0;
                ctx->decode.buf = tmp;
            }
            ctx->decode.bufsize = ibs;
        }

        if (obs > 0 && obs != ctx->encode.bufsize) {
            /* Do not free/alloc, only reallocate */
            if (ctx->encode.buf != NULL) {
                tmp = OPENSSL_realloc(ctx->encode.buf, obs);
                if (tmp == NULL)
                    return 0;
                ctx->encode.buf = tmp;
            }
            ctx->encode.bufsize = obs;
        }
        ret = 1;
        break;

    case BIO_C_DO_STATE_MACHINE:
        BIO_clear_retry_flags(b);
        ret = BIO_ctrl(next, cmd, num, ptr);
        BIO_copy_next_retry(b);
        break;

   case BIO_CTRL_WPENDING:
        if (BrotliEncoderHasMoreOutput(ctx->encode.state))
            ret = 1;
        else
            ret = BIO_ctrl(next, cmd, num, ptr);
        break;

    case BIO_CTRL_PENDING:
        if (!BrotliDecoderIsFinished(ctx->decode.state))
            ret = 1;
        else
            ret = BIO_ctrl(next, cmd, num, ptr);
        break;

    default:
        ret = BIO_ctrl(next, cmd, num, ptr);
        break;

    }

    return ret;
}

static long bio_brotli_callback_ctrl(BIO *b, int cmd, BIO_info_cb *fp)
{
    BIO *next = BIO_next(b);
    if (next == NULL)
        return 0;
    return BIO_callback_ctrl(next, cmd, fp);
}

#endif
