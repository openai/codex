/*
 * Copyright 1998-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 *
 * Uses zstd compression library from https://github.com/facebook/zstd
 * Requires version 1.4.x (latest as of this writing is 1.4.5)
 * Using custom free functions require static linking, so that is disabled when
 * using the shared library.
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

COMP_METHOD *COMP_zstd(void);

#ifdef OPENSSL_NO_ZSTD
# undef ZSTD_SHARED
#else

# ifndef ZSTD_SHARED
#  define ZSTD_STATIC_LINKING_ONLY
# endif
# include <zstd.h>

/* Note: There is also a linux zstd.h file in the kernel source */
# ifndef ZSTD_H_235446
#  error Wrong (i.e. linux) zstd.h included.
# endif

# if ZSTD_VERSION_MAJOR != 1 && ZSTD_VERSION_MINOR < 4
#  error Expecting version 1.4 or greater of ZSTD
# endif

# ifndef ZSTD_SHARED
/* memory allocations functions for zstd initialisation */
static void *zstd_alloc(void *opaque, size_t size)
{
    return OPENSSL_zalloc(size);
}

static void zstd_free(void *opaque, void *address)
{
    OPENSSL_free(address);
}

static ZSTD_customMem zstd_mem_funcs = {
    zstd_alloc,
    zstd_free,
    NULL
};
# endif

/*
 * When OpenSSL is built on Windows, we do not want to require that
 * the LIBZSTD.DLL be available in order for the OpenSSL DLLs to
 * work.  Therefore, all ZSTD routines are loaded at run time
 * and we do not link to a .LIB file when ZSTD_SHARED is set.
 */
# if defined(OPENSSL_SYS_WINDOWS) || defined(OPENSSL_SYS_WIN32)
#  include <windows.h>
# endif

# ifdef ZSTD_SHARED
#  include "internal/dso.h"

/* Function pointers */
typedef ZSTD_CStream* (*createCStream_ft)(void);
typedef size_t (*initCStream_ft)(ZSTD_CStream*, int);
typedef size_t (*freeCStream_ft)(ZSTD_CStream*);
typedef size_t (*compressStream2_ft)(ZSTD_CCtx*, ZSTD_outBuffer*, ZSTD_inBuffer*, ZSTD_EndDirective);
typedef size_t (*flushStream_ft)(ZSTD_CStream*, ZSTD_outBuffer*);
typedef size_t (*endStream_ft)(ZSTD_CStream*, ZSTD_outBuffer*);
typedef size_t (*compress_ft)(void*, size_t, const void*, size_t, int);
typedef ZSTD_DStream* (*createDStream_ft)(void);
typedef size_t (*initDStream_ft)(ZSTD_DStream*);
typedef size_t (*freeDStream_ft)(ZSTD_DStream*);
typedef size_t (*decompressStream_ft)(ZSTD_DStream*, ZSTD_outBuffer*, ZSTD_inBuffer*);
typedef size_t (*decompress_ft)(void*, size_t, const void*, size_t);
typedef unsigned (*isError_ft)(size_t);
typedef const char* (*getErrorName_ft)(size_t);
typedef size_t (*DStreamInSize_ft)(void);
typedef size_t (*CStreamInSize_ft)(void);

static createCStream_ft p_createCStream = NULL;
static initCStream_ft p_initCStream = NULL;
static freeCStream_ft p_freeCStream = NULL;
static compressStream2_ft p_compressStream2 = NULL;
static flushStream_ft p_flushStream = NULL;
static endStream_ft p_endStream = NULL;
static compress_ft p_compress = NULL;
static createDStream_ft p_createDStream = NULL;
static initDStream_ft p_initDStream = NULL;
static freeDStream_ft p_freeDStream = NULL;
static decompressStream_ft p_decompressStream = NULL;
static decompress_ft p_decompress = NULL;
static isError_ft p_isError = NULL;
static getErrorName_ft p_getErrorName = NULL;
static DStreamInSize_ft p_DStreamInSize = NULL;
static CStreamInSize_ft p_CStreamInSize = NULL;

static DSO *zstd_dso = NULL;

#  define ZSTD_createCStream p_createCStream
#  define ZSTD_initCStream p_initCStream
#  define ZSTD_freeCStream p_freeCStream
#  define ZSTD_compressStream2 p_compressStream2
#  define ZSTD_flushStream p_flushStream
#  define ZSTD_endStream p_endStream
#  define ZSTD_compress p_compress
#  define ZSTD_createDStream p_createDStream
#  define ZSTD_initDStream p_initDStream
#  define ZSTD_freeDStream p_freeDStream
#  define ZSTD_decompressStream p_decompressStream
#  define ZSTD_decompress p_decompress
#  define ZSTD_isError p_isError
#  define ZSTD_getErrorName p_getErrorName
#  define ZSTD_DStreamInSize p_DStreamInSize
#  define ZSTD_CStreamInSize p_CStreamInSize

# endif /* ifdef ZSTD_SHARED */

struct zstd_state {
    ZSTD_CStream *compressor;
    ZSTD_DStream *decompressor;
};

static int zstd_stateful_init(COMP_CTX *ctx)
{
    struct zstd_state *state = OPENSSL_zalloc(sizeof(*state));

    if (state == NULL)
        return 0;

# ifdef ZSTD_SHARED
    state->compressor = ZSTD_createCStream();
# else
    state->compressor = ZSTD_createCStream_advanced(zstd_mem_funcs);
# endif
    if (state->compressor == NULL)
        goto err;
    ZSTD_initCStream(state->compressor, ZSTD_CLEVEL_DEFAULT);

# ifdef ZSTD_SHARED
    state->decompressor = ZSTD_createDStream();
# else
    state->decompressor = ZSTD_createDStream_advanced(zstd_mem_funcs);
# endif
    if (state->decompressor == NULL)
        goto err;
    ZSTD_initDStream(state->decompressor);

    ctx->data = state;
    return 1;
 err:
    ZSTD_freeCStream(state->compressor);
    ZSTD_freeDStream(state->decompressor);
    OPENSSL_free(state);
    return 0;
}

static void zstd_stateful_finish(COMP_CTX *ctx)
{
    struct zstd_state *state = ctx->data;

    if (state != NULL) {
        ZSTD_freeCStream(state->compressor);
        ZSTD_freeDStream(state->decompressor);
        OPENSSL_free(state);
        ctx->data = NULL;
    }
}

static ossl_ssize_t zstd_stateful_compress_block(COMP_CTX *ctx, unsigned char *out,
                                                 size_t olen, unsigned char *in,
                                                 size_t ilen)
{
    ZSTD_inBuffer inbuf;
    ZSTD_outBuffer outbuf;
    size_t ret;
    ossl_ssize_t fret;
    struct zstd_state *state = ctx->data;

    inbuf.src = in;
    inbuf.size = ilen;
    inbuf.pos = 0;
    outbuf.dst = out;
    outbuf.size = olen;
    outbuf.pos = 0;

    if (state == NULL)
        return -1;

    /* If input length is zero, end the stream/frame ? */
    if (ilen == 0) {
        ret = ZSTD_endStream(state->compressor, &outbuf);
        if (ZSTD_isError(ret))
            return -1;
        goto end;
    }

    /*
     * The finish API does not provide a final output buffer,
     * so each compress operation has to be ended, if all
     * the input data can't be accepted, or there is more output,
     * this has to be considered an error, since there is no more
     * output buffer space.
     */
    do {
        ret = ZSTD_compressStream2(state->compressor, &outbuf, &inbuf, ZSTD_e_continue);
        if (ZSTD_isError(ret))
            return -1;
        /* do I need to check for ret == 0 ? */
    } while (inbuf.pos < inbuf.size);

    /* Did not consume all the data */
    if (inbuf.pos < inbuf.size)
        return -1;

    ret = ZSTD_flushStream(state->compressor, &outbuf);
    if (ZSTD_isError(ret))
        return -1;

 end:
    if (outbuf.pos > OSSL_SSIZE_MAX)
        return -1;
    fret = (ossl_ssize_t)outbuf.pos;
    if (fret < 0)
        return -1;
    return fret;
}

static ossl_ssize_t zstd_stateful_expand_block(COMP_CTX *ctx, unsigned char *out,
                                               size_t olen, unsigned char *in,
                                               size_t ilen)
{
    ZSTD_inBuffer inbuf;
    ZSTD_outBuffer outbuf;
    size_t ret;
    ossl_ssize_t fret;
    struct zstd_state *state = ctx->data;

    inbuf.src = in;
    inbuf.size = ilen;
    inbuf.pos = 0;
    outbuf.dst = out;
    outbuf.size = olen;
    outbuf.pos = 0;

    if (state == NULL)
        return -1;

    if (ilen == 0)
        return 0;

    do {
        ret = ZSTD_decompressStream(state->decompressor, &outbuf, &inbuf);
        if (ZSTD_isError(ret))
            return -1;
        /* If we completed a frame, and there's more data, try again */
    } while (ret == 0 && inbuf.pos < inbuf.size);

    /* Did not consume all the data */
    if (inbuf.pos < inbuf.size)
        return -1;

    if (outbuf.pos > OSSL_SSIZE_MAX)
        return -1;
    fret = (ossl_ssize_t)outbuf.pos;
    if (fret < 0)
        return -1;
    return fret;
}


static COMP_METHOD zstd_stateful_method = {
    NID_zstd,
    LN_zstd,
    zstd_stateful_init,
    zstd_stateful_finish,
    zstd_stateful_compress_block,
    zstd_stateful_expand_block
};

static int zstd_oneshot_init(COMP_CTX *ctx)
{
    return 1;
}

static void zstd_oneshot_finish(COMP_CTX *ctx)
{
}

static ossl_ssize_t zstd_oneshot_compress_block(COMP_CTX *ctx, unsigned char *out,
                                               size_t olen, unsigned char *in,
                                               size_t ilen)
{
    size_t out_size;
    ossl_ssize_t ret;

    if (ilen == 0)
        return 0;

    /* Note: uses STDLIB memory allocators */
    out_size = ZSTD_compress(out, olen, in, ilen, ZSTD_CLEVEL_DEFAULT);
    if (ZSTD_isError(out_size))
        return -1;

    if (out_size > OSSL_SSIZE_MAX)
        return -1;
    ret = (ossl_ssize_t)out_size;
    if (ret < 0)
        return -1;
    return ret;
}

static ossl_ssize_t zstd_oneshot_expand_block(COMP_CTX *ctx, unsigned char *out,
                                              size_t olen, unsigned char *in,
                                              size_t ilen)
{
    size_t out_size;
    ossl_ssize_t ret;

    if (ilen == 0)
        return 0;

    /* Note: uses STDLIB memory allocators */
    out_size = ZSTD_decompress(out, olen, in, ilen);
    if (ZSTD_isError(out_size))
        return -1;

    if (out_size > OSSL_SSIZE_MAX)
        return -1;
    ret = (ossl_ssize_t)out_size;
    if (ret < 0)
        return -1;
    return ret;
}

static COMP_METHOD zstd_oneshot_method = {
    NID_zstd,
    LN_zstd,
    zstd_oneshot_init,
    zstd_oneshot_finish,
    zstd_oneshot_compress_block,
    zstd_oneshot_expand_block
};

static CRYPTO_ONCE zstd_once = CRYPTO_ONCE_STATIC_INIT;
DEFINE_RUN_ONCE_STATIC(ossl_comp_zstd_init)
{
# ifdef ZSTD_SHARED
#  if defined(OPENSSL_SYS_WINDOWS) || defined(OPENSSL_SYS_WIN32)
#   define LIBZSTD "LIBZSTD"
#  else
#   define LIBZSTD  "zstd"
#  endif

    zstd_dso = DSO_load(NULL, LIBZSTD, NULL, 0);
    if (zstd_dso != NULL) {
        p_createCStream = (createCStream_ft)DSO_bind_func(zstd_dso, "ZSTD_createCStream");
        p_initCStream = (initCStream_ft)DSO_bind_func(zstd_dso, "ZSTD_initCStream");
        p_freeCStream = (freeCStream_ft)DSO_bind_func(zstd_dso, "ZSTD_freeCStream");
        p_compressStream2 = (compressStream2_ft)DSO_bind_func(zstd_dso, "ZSTD_compressStream2");
        p_flushStream = (flushStream_ft)DSO_bind_func(zstd_dso, "ZSTD_flushStream");
        p_endStream = (endStream_ft)DSO_bind_func(zstd_dso, "ZSTD_endStream");
        p_compress = (compress_ft)DSO_bind_func(zstd_dso, "ZSTD_compress");
        p_createDStream = (createDStream_ft)DSO_bind_func(zstd_dso, "ZSTD_createDStream");
        p_initDStream = (initDStream_ft)DSO_bind_func(zstd_dso, "ZSTD_initDStream");
        p_freeDStream = (freeDStream_ft)DSO_bind_func(zstd_dso, "ZSTD_freeDStream");
        p_decompressStream = (decompressStream_ft)DSO_bind_func(zstd_dso, "ZSTD_decompressStream");
        p_decompress = (decompress_ft)DSO_bind_func(zstd_dso, "ZSTD_decompress");
        p_isError = (isError_ft)DSO_bind_func(zstd_dso, "ZSTD_isError");
        p_getErrorName = (getErrorName_ft)DSO_bind_func(zstd_dso, "ZSTD_getErrorName");
        p_DStreamInSize = (DStreamInSize_ft)DSO_bind_func(zstd_dso, "ZSTD_DStreamInSize");
        p_CStreamInSize = (CStreamInSize_ft)DSO_bind_func(zstd_dso, "ZSTD_CStreamInSize");
    }

    if (p_createCStream == NULL || p_initCStream == NULL || p_freeCStream == NULL
            || p_compressStream2 == NULL || p_flushStream == NULL || p_endStream == NULL
            || p_compress == NULL || p_createDStream == NULL || p_initDStream == NULL
            || p_freeDStream == NULL || p_decompressStream == NULL || p_decompress == NULL
            || p_isError == NULL || p_getErrorName == NULL || p_DStreamInSize == NULL
            || p_CStreamInSize == NULL) {
        ossl_comp_zstd_cleanup();
        return 0;
    }
# endif
    return 1;
}
#endif /* ifndef ZSTD / else */

COMP_METHOD *COMP_zstd(void)
{
    COMP_METHOD *meth = NULL;

#ifndef OPENSSL_NO_ZSTD
    if (RUN_ONCE(&zstd_once, ossl_comp_zstd_init))
        meth = &zstd_stateful_method;
#endif
    return meth;
}

COMP_METHOD *COMP_zstd_oneshot(void)
{
    COMP_METHOD *meth = NULL;

#ifndef OPENSSL_NO_ZSTD
    if (RUN_ONCE(&zstd_once, ossl_comp_zstd_init))
        meth = &zstd_oneshot_method;
#endif
    return meth;
}

/* Also called from OPENSSL_cleanup() */
void ossl_comp_zstd_cleanup(void)
{
#ifdef ZSTD_SHARED
    DSO_free(zstd_dso);
    zstd_dso = NULL;
    p_createCStream = NULL;
    p_initCStream = NULL;
    p_freeCStream = NULL;
    p_compressStream2 = NULL;
    p_flushStream = NULL;
    p_endStream = NULL;
    p_compress = NULL;
    p_createDStream = NULL;
    p_initDStream = NULL;
    p_freeDStream = NULL;
    p_decompressStream = NULL;
    p_decompress = NULL;
    p_isError = NULL;
    p_getErrorName = NULL;
    p_DStreamInSize = NULL;
    p_CStreamInSize = NULL;
#endif
}

#ifndef OPENSSL_NO_ZSTD

/* Zstd-based compression/decompression filter BIO */

typedef struct {
    struct { /* input structure */
        ZSTD_DStream *state;
        ZSTD_inBuffer inbuf; /* has const src */
        size_t bufsize;
        void* buffer;
    } decompress;
    struct { /* output structure */
        ZSTD_CStream *state;
        ZSTD_outBuffer outbuf;
        size_t bufsize;
        size_t write_pos;
    } compress;
} BIO_ZSTD_CTX;

# define ZSTD_DEFAULT_BUFSIZE 1024

static int bio_zstd_new(BIO *bi);
static int bio_zstd_free(BIO *bi);
static int bio_zstd_read(BIO *b, char *out, int outl);
static int bio_zstd_write(BIO *b, const char *in, int inl);
static long bio_zstd_ctrl(BIO *b, int cmd, long num, void *ptr);
static long bio_zstd_callback_ctrl(BIO *b, int cmd, BIO_info_cb *fp);

static const BIO_METHOD bio_meth_zstd = {
    BIO_TYPE_COMP,
    "zstd",
    /* TODO: Convert to new style write function */
    bwrite_conv,
    bio_zstd_write,
    /* TODO: Convert to new style read function */
    bread_conv,
    bio_zstd_read,
    NULL,                      /* bio_zstd_puts, */
    NULL,                      /* bio_zstd_gets, */
    bio_zstd_ctrl,
    bio_zstd_new,
    bio_zstd_free,
    bio_zstd_callback_ctrl
};
#endif

const BIO_METHOD *BIO_f_zstd(void)
{
#ifndef OPENSSL_NO_ZSTD
    if (RUN_ONCE(&zstd_once, ossl_comp_zstd_init))
        return &bio_meth_zstd;
#endif
    return NULL;
}

#ifndef OPENSSL_NO_ZSTD
static int bio_zstd_new(BIO *bi)
{
    BIO_ZSTD_CTX *ctx;

# ifdef ZSTD_SHARED
    (void)COMP_zstd();
    if (zstd_dso == NULL) {
        ERR_raise(ERR_LIB_COMP, COMP_R_ZSTD_NOT_SUPPORTED);
        return 0;
    }
# endif
    ctx = OPENSSL_zalloc(sizeof(*ctx));
    if (ctx == NULL) {
        ERR_raise(ERR_LIB_COMP, ERR_R_MALLOC_FAILURE);
        return 0;
    }

# ifdef ZSTD_SHARED
    ctx->decompress.state =  ZSTD_createDStream();
# else
    ctx->decompress.state =  ZSTD_createDStream_advanced(zstd_mem_funcs);
# endif
    if (ctx->decompress.state == NULL)
        goto err;
    ZSTD_initDStream(ctx->decompress.state);
    ctx->decompress.bufsize = ZSTD_DStreamInSize();

# ifdef ZSTD_SHARED
    ctx->compress.state = ZSTD_createCStream();
# else
    ctx->compress.state = ZSTD_createCStream_advanced(zstd_mem_funcs);
# endif
    if (ctx->compress.state == NULL)
        goto err;
    ZSTD_initCStream(ctx->compress.state, ZSTD_CLEVEL_DEFAULT);
    ctx->compress.bufsize = ZSTD_CStreamInSize();

    BIO_set_init(bi, 1);
    BIO_set_data(bi, ctx);

    return 1;
 err:
    ERR_raise(ERR_LIB_COMP, ERR_R_MALLOC_FAILURE);
    ZSTD_freeDStream(ctx->decompress.state);
    ZSTD_freeCStream(ctx->compress.state);
    OPENSSL_free(ctx);
    return 0;
}

static int bio_zstd_free(BIO *bi)
{
    BIO_ZSTD_CTX *ctx;

    if (bi == NULL)
        return 0;

    ctx = BIO_get_data(bi);
    if (ctx != NULL) {
        ZSTD_freeDStream(ctx->decompress.state);
        OPENSSL_free(ctx->decompress.buffer);
        ZSTD_freeCStream(ctx->compress.state);
        OPENSSL_free(ctx->compress.outbuf.dst);
        OPENSSL_free(ctx);
    }
    BIO_set_data(bi, NULL);
    BIO_set_init(bi, 0);

    return 1;
}

static int bio_zstd_read(BIO *b, char *out, int outl)
{
    BIO_ZSTD_CTX *ctx;
    size_t zret;
    int ret;
    ZSTD_outBuffer outBuf;
    BIO *next = BIO_next(b);

    if (out == NULL || outl <= 0)
        return 0;

    ctx = BIO_get_data(b);
    BIO_clear_retry_flags(b);
    if (ctx->decompress.buffer == NULL) {
        ctx->decompress.buffer = OPENSSL_malloc(ctx->decompress.bufsize);
        if (ctx->decompress.buffer == NULL) {
            ERR_raise(ERR_LIB_COMP, ERR_R_MALLOC_FAILURE);
            return 0;
        }
        ctx->decompress.inbuf.src = ctx->decompress.buffer;
        ctx->decompress.inbuf.size = 0;
        ctx->decompress.inbuf.pos = 0;
    }

    /* Copy output data directly to supplied buffer */
    outBuf.dst = out;
    outBuf.size = (size_t)outl;
    outBuf.pos = 0;
    for (;;) {
        /* Decompress while data available */
        do {
            zret = ZSTD_decompressStream(ctx->decompress.state, &outBuf, &ctx->decompress.inbuf);
            if (ZSTD_isError(zret)) {
                ERR_raise(ERR_LIB_COMP, COMP_R_ZSTD_DECOMPRESS_ERROR);
                ERR_add_error_data(1, ZSTD_getErrorName(zret));
                return -1;
            }
            /* No more output space */
            if (outBuf.pos == outBuf.size)
                return outBuf.pos;
        } while (ctx->decompress.inbuf.pos < ctx->decompress.inbuf.size);

        /*
         * No data in input buffer try to read some in, if an error then
         * return the total data read.
         */
        ret = BIO_read(next, ctx->decompress.buffer, ctx->decompress.bufsize);
        if (ret <= 0) {
            BIO_copy_next_retry(b);
            if (ret < 0 && outBuf.pos == 0)
                return ret;
            return outBuf.pos;
        }
        ctx->decompress.inbuf.size = ret;
        ctx->decompress.inbuf.pos = 0;
    }
}

static int bio_zstd_write(BIO *b, const char *in, int inl)
{
    BIO_ZSTD_CTX *ctx;
    size_t zret;
    ZSTD_inBuffer inBuf;
    int ret;
    int done = 0;
    BIO *next = BIO_next(b);

    if (in == NULL || inl <= 0)
        return 0;

    ctx = BIO_get_data(b);

    BIO_clear_retry_flags(b);
    if (ctx->compress.outbuf.dst == NULL) {
        ctx->compress.outbuf.dst = OPENSSL_malloc(ctx->compress.bufsize);
        if (ctx->compress.outbuf.dst == NULL) {
            ERR_raise(ERR_LIB_COMP, ERR_R_MALLOC_FAILURE);
            return 0;
        }
        ctx->compress.outbuf.size = ctx->compress.bufsize;
        ctx->compress.outbuf.pos = 0;
        ctx->compress.write_pos = 0;
    }
    /* Obtain input data directly from supplied buffer */
    inBuf.src = in;
    inBuf.size = inl;
    inBuf.pos = 0;
    for (;;) {
        /* If data in output buffer write it first */
        while (ctx->compress.write_pos < ctx->compress.outbuf.pos) {
            ret = BIO_write(next, (unsigned char*)ctx->compress.outbuf.dst + ctx->compress.write_pos,
                            ctx->compress.outbuf.pos - ctx->compress.write_pos);
            if (ret <= 0) {
                BIO_copy_next_retry(b);
                if (ret < 0 && inBuf.pos == 0)
                    return ret;
                return inBuf.pos;
            }
            ctx->compress.write_pos += ret;
        }

        /* Have we consumed all supplied data? */
        if (done)
            return inBuf.pos;

        /* Reset buffer */
        ctx->compress.outbuf.pos = 0;
        ctx->compress.outbuf.size = ctx->compress.bufsize;
        ctx->compress.write_pos = 0;
        /* Compress some more */
        zret = ZSTD_compressStream2(ctx->compress.state, &ctx->compress.outbuf, &inBuf, ZSTD_e_end);
        if (ZSTD_isError(zret)) {
            ERR_raise(ERR_LIB_COMP, COMP_R_ZSTD_COMPRESS_ERROR);
            ERR_add_error_data(1, ZSTD_getErrorName(zret));
            return 0;
        } else if (zret == 0) {
            done = 1;
        }
    }
}

static int bio_zstd_flush(BIO *b)
{
    BIO_ZSTD_CTX *ctx;
    size_t zret;
    int ret;
    BIO *next = BIO_next(b);

    ctx = BIO_get_data(b);

    /* If no data written or already flush show success */
    if (ctx->compress.outbuf.dst == NULL)
        return 1;

    BIO_clear_retry_flags(b);
    /* No more input data */
    ctx->compress.outbuf.pos = 0;
    ctx->compress.outbuf.size = ctx->compress.bufsize;
    ctx->compress.write_pos = 0;
    for (;;) {
        /* If data in output buffer write it first */
        while (ctx->compress.write_pos < ctx->compress.outbuf.pos) {
            ret = BIO_write(next, (unsigned char*)ctx->compress.outbuf.dst + ctx->compress.write_pos,
                            ctx->compress.outbuf.pos - ctx->compress.write_pos);
            if (ret <= 0) {
                BIO_copy_next_retry(b);
                return ret;
            }
            ctx->compress.write_pos += ret;
        }

        /* Reset buffer */
        ctx->compress.outbuf.pos = 0;
        ctx->compress.outbuf.size = ctx->compress.bufsize;
        ctx->compress.write_pos = 0;
        /* Compress some more */
        zret = ZSTD_flushStream(ctx->compress.state, &ctx->compress.outbuf);
        if (ZSTD_isError(zret)) {
            ERR_raise(ERR_LIB_COMP, COMP_R_ZSTD_DECODE_ERROR);
            ERR_add_error_data(1, ZSTD_getErrorName(zret));
            return 0;
        }
        if (zret == 0)
            return 1;
    }
}

static long bio_zstd_ctrl(BIO *b, int cmd, long num, void *ptr)
{
    BIO_ZSTD_CTX *ctx;
    int ret = 0, *ip;
    size_t ibs, obs;
    unsigned char *tmp;
    BIO *next = BIO_next(b);

    if (next == NULL)
        return 0;
    ctx = BIO_get_data(b);
    switch (cmd) {

    case BIO_CTRL_RESET:
        ctx->compress.write_pos = 0;
        ctx->compress.bufsize = 0;
        ret = 1;
        break;

    case BIO_CTRL_FLUSH:
        ret = bio_zstd_flush(b);
        if (ret > 0) {
            ret = BIO_flush(next);
            BIO_copy_next_retry(b);
        }
        break;

    case BIO_C_SET_BUFF_SIZE:
        ibs = ctx->decompress.bufsize;
        obs = ctx->compress.bufsize;
        if (ptr != NULL) {
            ip = ptr;
            if (*ip == 0)
                ibs = (size_t)num;
            else
                obs = (size_t)num;
        } else {
            obs = ibs = (size_t)num;
        }

        if (ibs > 0 && ibs != ctx->decompress.bufsize) {
            if (ctx->decompress.buffer != NULL) {
                tmp = OPENSSL_realloc(ctx->decompress.buffer, ibs);
                if (tmp == NULL)
                    return 0;
                if (ctx->decompress.inbuf.src == ctx->decompress.buffer)
                    ctx->decompress.inbuf.src = tmp;
                ctx->decompress.buffer = tmp;
            }
            ctx->decompress.bufsize = ibs;
        }

        if (obs > 0 && obs != ctx->compress.bufsize) {
            if (ctx->compress.outbuf.dst != NULL) {
                tmp = OPENSSL_realloc(ctx->compress.outbuf.dst, obs);
                if (tmp == NULL)
                    return 0;
                ctx->compress.outbuf.dst = tmp;
            }
            ctx->compress.bufsize = obs;
        }
        ret = 1;
        break;

    case BIO_C_DO_STATE_MACHINE:
        BIO_clear_retry_flags(b);
        ret = BIO_ctrl(next, cmd, num, ptr);
        BIO_copy_next_retry(b);
        break;

   case BIO_CTRL_WPENDING:
        if (ctx->compress.outbuf.pos < ctx->compress.outbuf.size)
            ret = 1;
        else
            ret = BIO_ctrl(next, cmd, num, ptr);
        break;

    case BIO_CTRL_PENDING:
        if (ctx->decompress.inbuf.pos < ctx->decompress.inbuf.size)
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

static long bio_zstd_callback_ctrl(BIO *b, int cmd, BIO_info_cb *fp)
{
    BIO *next = BIO_next(b);
    if (next == NULL)
        return 0;
    return BIO_callback_ctrl(next, cmd, fp);
}

#endif
