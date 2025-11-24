/*
 * Copyright 2015-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* We need to use some engine deprecated APIs */
#define OPENSSL_SUPPRESS_DEPRECATED

/*
 * SHA-1 low level APIs are deprecated for public use, but still ok for
 * internal use.  Note, that due to symbols not being exported, only the
 * #defines and structures can be accessed, in this case SHA_CBLOCK and
 * sizeof(SHA_CTX).
 */
#include "internal/deprecated.h"

#include <openssl/opensslconf.h>
#if defined(_WIN32)
# include <windows.h>
#endif

#include <stdio.h>
#include <string.h>

#include <openssl/engine.h>
#include <openssl/sha.h>
#include <openssl/aes.h>
#include <openssl/rsa.h>
#include <openssl/evp.h>
#include <openssl/async.h>
#include <openssl/bn.h>
#include <openssl/crypto.h>
#include <openssl/ssl.h>
#include <openssl/modes.h>

#if defined(OPENSSL_SYS_UNIX) && defined(OPENSSL_THREADS)
# undef ASYNC_POSIX
# define ASYNC_POSIX
# include <unistd.h>
#elif defined(_WIN32)
# undef ASYNC_WIN
# define ASYNC_WIN
#endif

#include "e_dasync_err.c"

/* Engine Id and Name */
static const char *engine_dasync_id = "dasync";
static const char *engine_dasync_name = "Dummy Async engine support";


/* Engine Lifetime functions */
static int dasync_destroy(ENGINE *e);
static int dasync_init(ENGINE *e);
static int dasync_finish(ENGINE *e);
void engine_load_dasync_int(void);


/* Set up digests. Just SHA1 for now */
static int dasync_digests(ENGINE *e, const EVP_MD **digest,
                          const int **nids, int nid);

static void dummy_pause_job(void);

/* SHA1 */
static int dasync_sha1_init(EVP_MD_CTX *ctx);
static int dasync_sha1_update(EVP_MD_CTX *ctx, const void *data,
                             size_t count);
static int dasync_sha1_final(EVP_MD_CTX *ctx, unsigned char *md);

/*
 * Holds the EVP_MD object for sha1 in this engine. Set up once only during
 * engine bind and can then be reused many times.
 */
static EVP_MD *_hidden_sha1_md = NULL;
static const EVP_MD *dasync_sha1(void)
{
    return _hidden_sha1_md;
}
static void destroy_digests(void)
{
    EVP_MD_meth_free(_hidden_sha1_md);
    _hidden_sha1_md = NULL;
}

static int dasync_digest_nids(const int **nids)
{
    static int digest_nids[2] = { 0, 0 };
    static int pos = 0;
    static int init = 0;

    if (!init) {
        const EVP_MD *md;
        if ((md = dasync_sha1()) != NULL)
            digest_nids[pos++] = EVP_MD_get_type(md);
        digest_nids[pos] = 0;
        init = 1;
    }
    *nids = digest_nids;
    return pos;
}

/* RSA */
static int dasync_pkey(ENGINE *e, EVP_PKEY_METHOD **pmeth,
                       const int **pnids, int nid);

static int dasync_rsa_init(EVP_PKEY_CTX *ctx);
static void dasync_rsa_cleanup(EVP_PKEY_CTX *ctx);
static int dasync_rsa_paramgen_init(EVP_PKEY_CTX *ctx);
static int dasync_rsa_paramgen(EVP_PKEY_CTX *ctx, EVP_PKEY *pkey);
static int dasync_rsa_keygen_init(EVP_PKEY_CTX *ctx);
static int dasync_rsa_keygen(EVP_PKEY_CTX *ctx, EVP_PKEY *pkey);
static int dasync_rsa_encrypt_init(EVP_PKEY_CTX *ctx);
static int dasync_rsa_encrypt(EVP_PKEY_CTX *ctx, unsigned char *out,
                              size_t *outlen, const unsigned char *in,
                              size_t inlen);
static int dasync_rsa_decrypt_init(EVP_PKEY_CTX *ctx);
static int dasync_rsa_decrypt(EVP_PKEY_CTX *ctx, unsigned char *out,
                              size_t *outlen, const unsigned char *in,
                              size_t inlen);
static int dasync_rsa_ctrl(EVP_PKEY_CTX *ctx, int type, int p1, void *p2);
static int dasync_rsa_ctrl_str(EVP_PKEY_CTX *ctx, const char *type,
                               const char *value);

static EVP_PKEY_METHOD *dasync_rsa;
static const EVP_PKEY_METHOD *dasync_rsa_orig;

/* AES */

static int dasync_aes128_cbc_ctrl(EVP_CIPHER_CTX *ctx, int type, int arg,
                                  void *ptr);
static int dasync_aes128_init_key(EVP_CIPHER_CTX *ctx, const unsigned char *key,
                                  const unsigned char *iv, int enc);
static int dasync_aes128_cbc_cipher(EVP_CIPHER_CTX *ctx, unsigned char *out,
                                    const unsigned char *in, size_t inl);
static int dasync_aes128_cbc_cleanup(EVP_CIPHER_CTX *ctx);

static int dasync_aes256_ctr_ctrl(EVP_CIPHER_CTX *ctx, int type, int arg,
                                  void *ptr);
static int dasync_aes256_init_key(EVP_CIPHER_CTX *ctx, const unsigned char *key,
                                  const unsigned char *iv, int enc);
static int dasync_aes256_ctr_cipher(EVP_CIPHER_CTX *ctx, unsigned char *out,
                                    const unsigned char *in, size_t inl);
static int dasync_aes256_ctr_cleanup(EVP_CIPHER_CTX *ctx);

static int dasync_aes128_cbc_hmac_sha1_ctrl(EVP_CIPHER_CTX *ctx, int type,
                                             int arg, void *ptr);
static int dasync_aes128_cbc_hmac_sha1_init_key(EVP_CIPHER_CTX *ctx,
                                                 const unsigned char *key,
                                                 const unsigned char *iv,
                                                 int enc);
static int dasync_aes128_cbc_hmac_sha1_cipher(EVP_CIPHER_CTX *ctx,
                                               unsigned char *out,
                                               const unsigned char *in,
                                               size_t inl);
static int dasync_aes128_cbc_hmac_sha1_cleanup(EVP_CIPHER_CTX *ctx);

struct dasync_pipeline_ctx {
    void *inner_cipher_data;
    unsigned int numpipes;
    unsigned char **inbufs;
    unsigned char **outbufs;
    size_t *lens;
    unsigned char tlsaad[SSL_MAX_PIPELINES][EVP_AEAD_TLS1_AAD_LEN];
    unsigned int aadctr;
};

/*
 * Holds the EVP_CIPHER object for aes_128_cbc in this engine. Set up once only
 * during engine bind and can then be reused many times.
 */
static EVP_CIPHER *_hidden_aes_128_cbc = NULL;
static const EVP_CIPHER *dasync_aes_128_cbc(void)
{
    return _hidden_aes_128_cbc;
}

static EVP_CIPHER *_hidden_aes_256_ctr = NULL;
static const EVP_CIPHER *dasync_aes_256_ctr(void)
{
    return _hidden_aes_256_ctr;
}

/*
 * Holds the EVP_CIPHER object for aes_128_cbc_hmac_sha1 in this engine. Set up
 * once only during engine bind and can then be reused many times.
 *
 * This 'stitched' cipher depends on the EVP_aes_128_cbc_hmac_sha1() cipher,
 * which is implemented only if the AES-NI instruction set extension is available
 * (see OPENSSL_IA32CAP(3)). If that's not the case, then this cipher will not
 * be available either.
 *
 * Note: Since it is a legacy mac-then-encrypt cipher, modern TLS peers (which
 * negotiate the encrypt-then-mac extension) won't negotiate it anyway.
 */
static EVP_CIPHER *_hidden_aes_128_cbc_hmac_sha1 = NULL;
static const EVP_CIPHER *dasync_aes_128_cbc_hmac_sha1(void)
{
    return _hidden_aes_128_cbc_hmac_sha1;
}

static void destroy_ciphers(void)
{
    EVP_CIPHER_meth_free(_hidden_aes_128_cbc);
    EVP_CIPHER_meth_free(_hidden_aes_256_ctr);
    EVP_CIPHER_meth_free(_hidden_aes_128_cbc_hmac_sha1);
    _hidden_aes_128_cbc = NULL;
    _hidden_aes_256_ctr = NULL;
    _hidden_aes_128_cbc_hmac_sha1 = NULL;
}

static int dasync_ciphers(ENGINE *e, const EVP_CIPHER **cipher,
                                   const int **nids, int nid);

static int dasync_cipher_nids[] = {
    NID_aes_128_cbc,
    NID_aes_256_ctr,
    NID_aes_128_cbc_hmac_sha1,
    0
};

static int bind_dasync(ENGINE *e)
{
    /* Setup RSA */
    if ((dasync_rsa_orig = EVP_PKEY_meth_find(EVP_PKEY_RSA)) == NULL
        || (dasync_rsa = EVP_PKEY_meth_new(EVP_PKEY_RSA,
                                           EVP_PKEY_FLAG_AUTOARGLEN)) == NULL)
        return 0;
    EVP_PKEY_meth_set_init(dasync_rsa, dasync_rsa_init);
    EVP_PKEY_meth_set_cleanup(dasync_rsa, dasync_rsa_cleanup);
    EVP_PKEY_meth_set_paramgen(dasync_rsa, dasync_rsa_paramgen_init,
                               dasync_rsa_paramgen);
    EVP_PKEY_meth_set_keygen(dasync_rsa, dasync_rsa_keygen_init,
                             dasync_rsa_keygen);
    EVP_PKEY_meth_set_encrypt(dasync_rsa, dasync_rsa_encrypt_init,
                              dasync_rsa_encrypt);
    EVP_PKEY_meth_set_decrypt(dasync_rsa, dasync_rsa_decrypt_init,
                              dasync_rsa_decrypt);
    EVP_PKEY_meth_set_ctrl(dasync_rsa, dasync_rsa_ctrl,
                           dasync_rsa_ctrl_str);

    /* Ensure the dasync error handling is set up */
    ERR_load_DASYNC_strings();

    if (!ENGINE_set_id(e, engine_dasync_id)
        || !ENGINE_set_name(e, engine_dasync_name)
        || !ENGINE_set_pkey_meths(e, dasync_pkey)
        || !ENGINE_set_digests(e, dasync_digests)
        || !ENGINE_set_ciphers(e, dasync_ciphers)
        || !ENGINE_set_destroy_function(e, dasync_destroy)
        || !ENGINE_set_init_function(e, dasync_init)
        || !ENGINE_set_finish_function(e, dasync_finish)) {
        DASYNCerr(DASYNC_F_BIND_DASYNC, DASYNC_R_INIT_FAILED);
        return 0;
    }

    /*
     * Set up the EVP_CIPHER and EVP_MD objects for the ciphers/digests
     * supplied by this engine
     */
    _hidden_sha1_md = EVP_MD_meth_new(NID_sha1, NID_sha1WithRSAEncryption);
    if (_hidden_sha1_md == NULL
        || !EVP_MD_meth_set_result_size(_hidden_sha1_md, SHA_DIGEST_LENGTH)
        || !EVP_MD_meth_set_input_blocksize(_hidden_sha1_md, SHA_CBLOCK)
        || !EVP_MD_meth_set_app_datasize(_hidden_sha1_md,
                                         sizeof(EVP_MD *) + sizeof(SHA_CTX))
        || !EVP_MD_meth_set_flags(_hidden_sha1_md, EVP_MD_FLAG_DIGALGID_ABSENT)
        || !EVP_MD_meth_set_init(_hidden_sha1_md, dasync_sha1_init)
        || !EVP_MD_meth_set_update(_hidden_sha1_md, dasync_sha1_update)
        || !EVP_MD_meth_set_final(_hidden_sha1_md, dasync_sha1_final)) {
        EVP_MD_meth_free(_hidden_sha1_md);
        _hidden_sha1_md = NULL;
    }

    _hidden_aes_128_cbc = EVP_CIPHER_meth_new(NID_aes_128_cbc,
                                              16 /* block size */,
                                              16 /* key len */);
    if (_hidden_aes_128_cbc == NULL
            || !EVP_CIPHER_meth_set_iv_length(_hidden_aes_128_cbc,16)
            || !EVP_CIPHER_meth_set_flags(_hidden_aes_128_cbc,
                                          EVP_CIPH_FLAG_DEFAULT_ASN1
                                          | EVP_CIPH_CBC_MODE
                                          | EVP_CIPH_FLAG_PIPELINE
                                          | EVP_CIPH_CUSTOM_COPY)
            || !EVP_CIPHER_meth_set_init(_hidden_aes_128_cbc,
                                         dasync_aes128_init_key)
            || !EVP_CIPHER_meth_set_do_cipher(_hidden_aes_128_cbc,
                                              dasync_aes128_cbc_cipher)
            || !EVP_CIPHER_meth_set_cleanup(_hidden_aes_128_cbc,
                                            dasync_aes128_cbc_cleanup)
            || !EVP_CIPHER_meth_set_ctrl(_hidden_aes_128_cbc,
                                         dasync_aes128_cbc_ctrl)
            || !EVP_CIPHER_meth_set_impl_ctx_size(_hidden_aes_128_cbc,
                                sizeof(struct dasync_pipeline_ctx))) {
        EVP_CIPHER_meth_free(_hidden_aes_128_cbc);
        _hidden_aes_128_cbc = NULL;
    }

    _hidden_aes_256_ctr = EVP_CIPHER_meth_new(NID_aes_256_ctr,
                                              1  /* block size */,
                                              32 /* key len */);
    if (_hidden_aes_256_ctr == NULL
            || !EVP_CIPHER_meth_set_iv_length(_hidden_aes_256_ctr,16)
            || !EVP_CIPHER_meth_set_flags(_hidden_aes_256_ctr,
                                          EVP_CIPH_FLAG_DEFAULT_ASN1
                                          | EVP_CIPH_CTR_MODE
                                          | EVP_CIPH_FLAG_PIPELINE
                                          | EVP_CIPH_CUSTOM_COPY)
            || !EVP_CIPHER_meth_set_init(_hidden_aes_256_ctr,
                                         dasync_aes256_init_key)
            || !EVP_CIPHER_meth_set_do_cipher(_hidden_aes_256_ctr,
                                              dasync_aes256_ctr_cipher)
            || !EVP_CIPHER_meth_set_cleanup(_hidden_aes_256_ctr,
                                            dasync_aes256_ctr_cleanup)
            || !EVP_CIPHER_meth_set_ctrl(_hidden_aes_256_ctr,
                                         dasync_aes256_ctr_ctrl)
            || !EVP_CIPHER_meth_set_impl_ctx_size(_hidden_aes_256_ctr,
                                sizeof(struct dasync_pipeline_ctx))) {
        EVP_CIPHER_meth_free(_hidden_aes_256_ctr);
        _hidden_aes_256_ctr = NULL;
    }

    _hidden_aes_128_cbc_hmac_sha1 = EVP_CIPHER_meth_new(
                                                NID_aes_128_cbc_hmac_sha1,
                                                16 /* block size */,
                                                16 /* key len */);
    if (_hidden_aes_128_cbc_hmac_sha1 == NULL
            || !EVP_CIPHER_meth_set_iv_length(_hidden_aes_128_cbc_hmac_sha1,16)
            || !EVP_CIPHER_meth_set_flags(_hidden_aes_128_cbc_hmac_sha1,
                                            EVP_CIPH_CBC_MODE
                                          | EVP_CIPH_FLAG_DEFAULT_ASN1
                                          | EVP_CIPH_FLAG_AEAD_CIPHER
                                          | EVP_CIPH_FLAG_PIPELINE
                                          | EVP_CIPH_CUSTOM_COPY)
            || !EVP_CIPHER_meth_set_init(_hidden_aes_128_cbc_hmac_sha1,
                                         dasync_aes128_cbc_hmac_sha1_init_key)
            || !EVP_CIPHER_meth_set_do_cipher(_hidden_aes_128_cbc_hmac_sha1,
                                            dasync_aes128_cbc_hmac_sha1_cipher)
            || !EVP_CIPHER_meth_set_cleanup(_hidden_aes_128_cbc_hmac_sha1,
                                            dasync_aes128_cbc_hmac_sha1_cleanup)
            || !EVP_CIPHER_meth_set_ctrl(_hidden_aes_128_cbc_hmac_sha1,
                                         dasync_aes128_cbc_hmac_sha1_ctrl)
            || !EVP_CIPHER_meth_set_impl_ctx_size(_hidden_aes_128_cbc_hmac_sha1,
                                sizeof(struct dasync_pipeline_ctx))) {
        EVP_CIPHER_meth_free(_hidden_aes_128_cbc_hmac_sha1);
        _hidden_aes_128_cbc_hmac_sha1 = NULL;
    }

    return 1;
}

static void destroy_pkey(void)
{
    /*
     * We don't actually need to free the dasync_rsa method since this is
     * automatically freed for us by libcrypto.
     */
    dasync_rsa_orig = NULL;
    dasync_rsa = NULL;
}

# ifndef OPENSSL_NO_DYNAMIC_ENGINE
static int bind_helper(ENGINE *e, const char *id)
{
    if (id && (strcmp(id, engine_dasync_id) != 0))
        return 0;
    if (!bind_dasync(e))
        return 0;
    return 1;
}

IMPLEMENT_DYNAMIC_CHECK_FN()
    IMPLEMENT_DYNAMIC_BIND_FN(bind_helper)
# endif

static ENGINE *engine_dasync(void)
{
    ENGINE *ret = ENGINE_new();
    if (!ret)
        return NULL;
    if (!bind_dasync(ret)) {
        ENGINE_free(ret);
        return NULL;
    }
    return ret;
}

void engine_load_dasync_int(void)
{
    ENGINE *toadd = engine_dasync();
    if (!toadd)
        return;
    ERR_set_mark();
    ENGINE_add(toadd);
    /*
     * If the "add" worked, it gets a structural reference. So either way, we
     * release our just-created reference.
     */
    ENGINE_free(toadd);
    /*
     * If the "add" didn't work, it was probably a conflict because it was
     * already added (eg. someone calling ENGINE_load_blah then calling
     * ENGINE_load_builtin_engines() perhaps).
     */
    ERR_pop_to_mark();
}

static int dasync_init(ENGINE *e)
{
    return 1;
}


static int dasync_finish(ENGINE *e)
{
    return 1;
}


static int dasync_destroy(ENGINE *e)
{
    destroy_digests();
    destroy_ciphers();
    destroy_pkey();
    ERR_unload_DASYNC_strings();
    return 1;
}

static int dasync_pkey(ENGINE *e, EVP_PKEY_METHOD **pmeth,
                       const int **pnids, int nid)
{
    static const int rnid = EVP_PKEY_RSA;

    if (pmeth == NULL) {
        *pnids = &rnid;
        return 1;
    }

    if (nid == EVP_PKEY_RSA) {
        *pmeth = dasync_rsa;
        return 1;
    }

    *pmeth = NULL;
    return 0;
}

static int dasync_digests(ENGINE *e, const EVP_MD **digest,
                          const int **nids, int nid)
{
    int ok = 1;
    if (!digest) {
        /* We are returning a list of supported nids */
        return dasync_digest_nids(nids);
    }
    /* We are being asked for a specific digest */
    switch (nid) {
    case NID_sha1:
        *digest = dasync_sha1();
        break;
    default:
        ok = 0;
        *digest = NULL;
        break;
    }
    return ok;
}

static int dasync_ciphers(ENGINE *e, const EVP_CIPHER **cipher,
                                   const int **nids, int nid)
{
    int ok = 1;
    if (cipher == NULL) {
        /* We are returning a list of supported nids */
        *nids = dasync_cipher_nids;
        return (sizeof(dasync_cipher_nids) -
                1) / sizeof(dasync_cipher_nids[0]);
    }
    /* We are being asked for a specific cipher */
    switch (nid) {
    case NID_aes_128_cbc:
        *cipher = dasync_aes_128_cbc();
        break;
    case NID_aes_256_ctr:
        *cipher = dasync_aes_256_ctr();
        break;
    case NID_aes_128_cbc_hmac_sha1:
        *cipher = dasync_aes_128_cbc_hmac_sha1();
        break;
    default:
        ok = 0;
        *cipher = NULL;
        break;
    }
    return ok;
}

static void wait_cleanup(ASYNC_WAIT_CTX *ctx, const void *key,
                         OSSL_ASYNC_FD readfd, void *pvwritefd)
{
    OSSL_ASYNC_FD *pwritefd = (OSSL_ASYNC_FD *)pvwritefd;
#if defined(ASYNC_WIN)
    CloseHandle(readfd);
    CloseHandle(*pwritefd);
#elif defined(ASYNC_POSIX)
    close(readfd);
    close(*pwritefd);
#endif
    OPENSSL_free(pwritefd);
}

#define DUMMY_CHAR 'X'

static void dummy_pause_job(void) {
    ASYNC_JOB *job;
    ASYNC_WAIT_CTX *waitctx;
    ASYNC_callback_fn callback;
    void *callback_arg;
    OSSL_ASYNC_FD pipefds[2] = {0, 0};
    OSSL_ASYNC_FD *writefd;
#if defined(ASYNC_WIN)
    DWORD numwritten, numread;
    char buf = DUMMY_CHAR;
#elif defined(ASYNC_POSIX)
    char buf = DUMMY_CHAR;
#endif

    if ((job = ASYNC_get_current_job()) == NULL)
        return;

    waitctx = ASYNC_get_wait_ctx(job);

    if (ASYNC_WAIT_CTX_get_callback(waitctx, &callback, &callback_arg) && callback != NULL) {
        /*
         * In the Dummy async engine we are cheating. We call the callback that the job
         * is complete before the call to ASYNC_pause_job(). A real
         * async engine would only call the callback when the job was actually complete
         */
        (*callback)(callback_arg);
        ASYNC_pause_job();
        return;
    }


    if (ASYNC_WAIT_CTX_get_fd(waitctx, engine_dasync_id, &pipefds[0],
                              (void **)&writefd)) {
        pipefds[1] = *writefd;
    } else {
        writefd = OPENSSL_malloc(sizeof(*writefd));
        if (writefd == NULL)
            return;
#if defined(ASYNC_WIN)
        if (CreatePipe(&pipefds[0], &pipefds[1], NULL, 256) == 0) {
            OPENSSL_free(writefd);
            return;
        }
#elif defined(ASYNC_POSIX)
        if (pipe(pipefds) != 0) {
            OPENSSL_free(writefd);
            return;
        }
#endif
        *writefd = pipefds[1];

        if (!ASYNC_WAIT_CTX_set_wait_fd(waitctx, engine_dasync_id, pipefds[0],
                                        writefd, wait_cleanup)) {
            wait_cleanup(waitctx, engine_dasync_id, pipefds[0], writefd);
            return;
        }
    }
    /*
     * In the Dummy async engine we are cheating. We signal that the job
     * is complete by waking it before the call to ASYNC_pause_job(). A real
     * async engine would only wake when the job was actually complete
     */
#if defined(ASYNC_WIN)
    WriteFile(pipefds[1], &buf, 1, &numwritten, NULL);
#elif defined(ASYNC_POSIX)
    if (write(pipefds[1], &buf, 1) < 0)
        return;
#endif

    /* Ignore errors - we carry on anyway */
    ASYNC_pause_job();

    /* Clear the wake signal */
#if defined(ASYNC_WIN)
    ReadFile(pipefds[0], &buf, 1, &numread, NULL);
#elif defined(ASYNC_POSIX)
    if (read(pipefds[0], &buf, 1) < 0)
        return;
#endif
}

/*
 * SHA1 implementation. At the moment we just defer to the standard
 * implementation
 */
static int dasync_sha1_init(EVP_MD_CTX *ctx)
{
    dummy_pause_job();

    return EVP_MD_meth_get_init(EVP_sha1())(ctx);
}

static int dasync_sha1_update(EVP_MD_CTX *ctx, const void *data,
                             size_t count)
{
    dummy_pause_job();

    return EVP_MD_meth_get_update(EVP_sha1())(ctx, data, count);
}

static int dasync_sha1_final(EVP_MD_CTX *ctx, unsigned char *md)
{
    dummy_pause_job();

    return EVP_MD_meth_get_final(EVP_sha1())(ctx, md);
}

/* Cipher helper functions */

static int dasync_cipher_ctrl_helper(EVP_CIPHER_CTX *ctx, int type, int arg,
                                     void *ptr, int aeadcapable,
                                     const EVP_CIPHER *ciph)
{
    int ret;
    struct dasync_pipeline_ctx *pipe_ctx =
        (struct dasync_pipeline_ctx *)EVP_CIPHER_CTX_get_cipher_data(ctx);

    if (pipe_ctx == NULL)
        return 0;

    switch (type) {
        case EVP_CTRL_COPY:
            {
                size_t sz = EVP_CIPHER_impl_ctx_size(ciph);
                void *inner_cipher_data = OPENSSL_malloc(sz);

                if (inner_cipher_data == NULL)
                    return -1;
                memcpy(inner_cipher_data, pipe_ctx->inner_cipher_data, sz);
                pipe_ctx->inner_cipher_data = inner_cipher_data;
            }
            break;

        case EVP_CTRL_SET_PIPELINE_OUTPUT_BUFS:
            pipe_ctx->numpipes = arg;
            pipe_ctx->outbufs = (unsigned char **)ptr;
            break;

        case EVP_CTRL_SET_PIPELINE_INPUT_BUFS:
            pipe_ctx->numpipes = arg;
            pipe_ctx->inbufs = (unsigned char **)ptr;
            break;

        case EVP_CTRL_SET_PIPELINE_INPUT_LENS:
            pipe_ctx->numpipes = arg;
            pipe_ctx->lens = (size_t *)ptr;
            break;

        case EVP_CTRL_AEAD_SET_MAC_KEY:
            if (!aeadcapable)
                return -1;
            EVP_CIPHER_CTX_set_cipher_data(ctx, pipe_ctx->inner_cipher_data);
            ret = EVP_CIPHER_meth_get_ctrl(EVP_aes_128_cbc_hmac_sha1())
                                          (ctx, type, arg, ptr);
            EVP_CIPHER_CTX_set_cipher_data(ctx, pipe_ctx);
            return ret;

        case EVP_CTRL_AEAD_TLS1_AAD:
        {
            unsigned char *p = ptr;
            unsigned int len;

            if (!aeadcapable || arg != EVP_AEAD_TLS1_AAD_LEN)
                return -1;

            if (pipe_ctx->aadctr >= SSL_MAX_PIPELINES)
                return -1;

            memcpy(pipe_ctx->tlsaad[pipe_ctx->aadctr], ptr,
                   EVP_AEAD_TLS1_AAD_LEN);
            pipe_ctx->aadctr++;

            len = p[arg - 2] << 8 | p[arg - 1];

            if (EVP_CIPHER_CTX_is_encrypting(ctx)) {
                if ((p[arg - 4] << 8 | p[arg - 3]) >= TLS1_1_VERSION) {
                    if (len < AES_BLOCK_SIZE)
                        return 0;
                    len -= AES_BLOCK_SIZE;
                }

                return ((len + SHA_DIGEST_LENGTH + AES_BLOCK_SIZE)
                        & -AES_BLOCK_SIZE) - len;
            } else {
                return SHA_DIGEST_LENGTH;
            }
        }

        default:
            return 0;
    }

    return 1;
}

static int dasync_cipher_init_key_helper(EVP_CIPHER_CTX *ctx,
                                         const unsigned char *key,
                                         const unsigned char *iv, int enc,
                                         const EVP_CIPHER *cipher)
{
    int ret;
    struct dasync_pipeline_ctx *pipe_ctx =
        (struct dasync_pipeline_ctx *)EVP_CIPHER_CTX_get_cipher_data(ctx);

    if (pipe_ctx->inner_cipher_data == NULL
            && EVP_CIPHER_impl_ctx_size(cipher) != 0) {
        pipe_ctx->inner_cipher_data = OPENSSL_zalloc(
            EVP_CIPHER_impl_ctx_size(cipher));
        if (pipe_ctx->inner_cipher_data == NULL)
            return 0;
    }

    pipe_ctx->numpipes = 0;
    pipe_ctx->aadctr = 0;

    EVP_CIPHER_CTX_set_cipher_data(ctx, pipe_ctx->inner_cipher_data);
    ret = EVP_CIPHER_meth_get_init(cipher)(ctx, key, iv, enc);
    EVP_CIPHER_CTX_set_cipher_data(ctx, pipe_ctx);

    return ret;
}

static int dasync_cipher_helper(EVP_CIPHER_CTX *ctx, unsigned char *out,
                                const unsigned char *in, size_t inl,
                                const EVP_CIPHER *cipher)
{
    int ret = 1;
    unsigned int i, pipes;
    struct dasync_pipeline_ctx *pipe_ctx =
        (struct dasync_pipeline_ctx *)EVP_CIPHER_CTX_get_cipher_data(ctx);

    pipes = pipe_ctx->numpipes;
    EVP_CIPHER_CTX_set_cipher_data(ctx, pipe_ctx->inner_cipher_data);
    if (pipes == 0) {
        if (pipe_ctx->aadctr != 0) {
            if (pipe_ctx->aadctr != 1)
                return -1;
            EVP_CIPHER_meth_get_ctrl(cipher)
                                    (ctx, EVP_CTRL_AEAD_TLS1_AAD,
                                     EVP_AEAD_TLS1_AAD_LEN,
                                     pipe_ctx->tlsaad[0]);
        }
        ret = EVP_CIPHER_meth_get_do_cipher(cipher)
                                           (ctx, out, in, inl);
    } else {
        if (pipe_ctx->aadctr > 0 && pipe_ctx->aadctr != pipes)
            return -1;
        for (i = 0; i < pipes; i++) {
            if (pipe_ctx->aadctr > 0) {
                EVP_CIPHER_meth_get_ctrl(cipher)
                                        (ctx, EVP_CTRL_AEAD_TLS1_AAD,
                                         EVP_AEAD_TLS1_AAD_LEN,
                                         pipe_ctx->tlsaad[i]);
            }
            ret = ret && EVP_CIPHER_meth_get_do_cipher(cipher)
                                (ctx, pipe_ctx->outbufs[i], pipe_ctx->inbufs[i],
                                 pipe_ctx->lens[i]);
        }
        pipe_ctx->numpipes = 0;
    }
    pipe_ctx->aadctr = 0;
    EVP_CIPHER_CTX_set_cipher_data(ctx, pipe_ctx);
    return ret;
}

static int dasync_cipher_cleanup_helper(EVP_CIPHER_CTX *ctx,
                                        const EVP_CIPHER *cipher)
{
    struct dasync_pipeline_ctx *pipe_ctx =
        (struct dasync_pipeline_ctx *)EVP_CIPHER_CTX_get_cipher_data(ctx);

    OPENSSL_clear_free(pipe_ctx->inner_cipher_data,
                       EVP_CIPHER_impl_ctx_size(cipher));

    return 1;
}

/*
 * AES128 CBC Implementation
 */

static int dasync_aes128_cbc_ctrl(EVP_CIPHER_CTX *ctx, int type, int arg,
                                  void *ptr)
{
    return dasync_cipher_ctrl_helper(ctx, type, arg, ptr, 0, EVP_aes_128_cbc());
}

static int dasync_aes128_init_key(EVP_CIPHER_CTX *ctx, const unsigned char *key,
                             const unsigned char *iv, int enc)
{
    return dasync_cipher_init_key_helper(ctx, key, iv, enc, EVP_aes_128_cbc());
}

static int dasync_aes128_cbc_cipher(EVP_CIPHER_CTX *ctx, unsigned char *out,
                               const unsigned char *in, size_t inl)
{
    return dasync_cipher_helper(ctx, out, in, inl, EVP_aes_128_cbc());
}

static int dasync_aes128_cbc_cleanup(EVP_CIPHER_CTX *ctx)
{
    return dasync_cipher_cleanup_helper(ctx, EVP_aes_128_cbc());
}

static int dasync_aes256_ctr_ctrl(EVP_CIPHER_CTX *ctx, int type, int arg,
                                  void *ptr)
{
    return dasync_cipher_ctrl_helper(ctx, type, arg, ptr, 0, EVP_aes_256_ctr());
}

static int dasync_aes256_init_key(EVP_CIPHER_CTX *ctx, const unsigned char *key,
                             const unsigned char *iv, int enc)
{
    return dasync_cipher_init_key_helper(ctx, key, iv, enc, EVP_aes_256_ctr());
}

static int dasync_aes256_ctr_cipher(EVP_CIPHER_CTX *ctx, unsigned char *out,
                               const unsigned char *in, size_t inl)
{
    return dasync_cipher_helper(ctx, out, in, inl, EVP_aes_256_ctr());
}

static int dasync_aes256_ctr_cleanup(EVP_CIPHER_CTX *ctx)
{
    return dasync_cipher_cleanup_helper(ctx, EVP_aes_256_ctr());
}


/*
 * AES128 CBC HMAC SHA1 Implementation
 */

static int dasync_aes128_cbc_hmac_sha1_ctrl(EVP_CIPHER_CTX *ctx, int type,
                                             int arg, void *ptr)
{
    return dasync_cipher_ctrl_helper(ctx, type, arg, ptr, 1, EVP_aes_128_cbc_hmac_sha1());
}

static int dasync_aes128_cbc_hmac_sha1_init_key(EVP_CIPHER_CTX *ctx,
                                                const unsigned char *key,
                                                const unsigned char *iv,
                                                int enc)
{
    /*
     * We can safely assume that EVP_aes_128_cbc_hmac_sha1() != NULL,
     * see comment before the definition of dasync_aes_128_cbc_hmac_sha1().
     */
    return dasync_cipher_init_key_helper(ctx, key, iv, enc,
                                         EVP_aes_128_cbc_hmac_sha1());
}

static int dasync_aes128_cbc_hmac_sha1_cipher(EVP_CIPHER_CTX *ctx,
                                               unsigned char *out,
                                               const unsigned char *in,
                                               size_t inl)
{
    return dasync_cipher_helper(ctx, out, in, inl, EVP_aes_128_cbc_hmac_sha1());
}

static int dasync_aes128_cbc_hmac_sha1_cleanup(EVP_CIPHER_CTX *ctx)
{
    /*
     * We can safely assume that EVP_aes_128_cbc_hmac_sha1() != NULL,
     * see comment before the definition of dasync_aes_128_cbc_hmac_sha1().
     */
    return dasync_cipher_cleanup_helper(ctx, EVP_aes_128_cbc_hmac_sha1());
}


/*
 * RSA implementation
 */
static int dasync_rsa_init(EVP_PKEY_CTX *ctx)
{
    static int (*pinit)(EVP_PKEY_CTX *ctx);

    if (pinit == NULL)
        EVP_PKEY_meth_get_init(dasync_rsa_orig, &pinit);
    return pinit(ctx);
}

static void dasync_rsa_cleanup(EVP_PKEY_CTX *ctx)
{
    static void (*pcleanup)(EVP_PKEY_CTX *ctx);

    if (pcleanup == NULL)
        EVP_PKEY_meth_get_cleanup(dasync_rsa_orig, &pcleanup);
    pcleanup(ctx);
}

static int dasync_rsa_paramgen_init(EVP_PKEY_CTX *ctx)
{
    static int (*pparamgen_init)(EVP_PKEY_CTX *ctx);

    if (pparamgen_init == NULL)
        EVP_PKEY_meth_get_paramgen(dasync_rsa_orig, &pparamgen_init, NULL);
    return pparamgen_init != NULL ? pparamgen_init(ctx) : 1;
}

static int dasync_rsa_paramgen(EVP_PKEY_CTX *ctx, EVP_PKEY *pkey)
{
    static int (*pparamgen)(EVP_PKEY_CTX *c, EVP_PKEY *pkey);

    if (pparamgen == NULL)
        EVP_PKEY_meth_get_paramgen(dasync_rsa_orig, NULL, &pparamgen);
    return pparamgen != NULL ? pparamgen(ctx, pkey) : 1;
}

static int dasync_rsa_keygen_init(EVP_PKEY_CTX *ctx)
{
    static int (*pkeygen_init)(EVP_PKEY_CTX *ctx);

    if (pkeygen_init == NULL)
        EVP_PKEY_meth_get_keygen(dasync_rsa_orig, &pkeygen_init, NULL);
    return pkeygen_init != NULL ? pkeygen_init(ctx) : 1;
}

static int dasync_rsa_keygen(EVP_PKEY_CTX *ctx, EVP_PKEY *pkey)
{
    static int (*pkeygen)(EVP_PKEY_CTX *c, EVP_PKEY *pkey);

    if (pkeygen == NULL)
        EVP_PKEY_meth_get_keygen(dasync_rsa_orig, NULL, &pkeygen);
    return pkeygen(ctx, pkey);
}

static int dasync_rsa_encrypt_init(EVP_PKEY_CTX *ctx)
{
    static int (*pencrypt_init)(EVP_PKEY_CTX *ctx);

    if (pencrypt_init == NULL)
        EVP_PKEY_meth_get_encrypt(dasync_rsa_orig, &pencrypt_init, NULL);
    return pencrypt_init != NULL ? pencrypt_init(ctx) : 1;
}

static int dasync_rsa_encrypt(EVP_PKEY_CTX *ctx, unsigned char *out,
                              size_t *outlen, const unsigned char *in,
                              size_t inlen)
{
    static int (*pencryptfn)(EVP_PKEY_CTX *ctx, unsigned char *out,
                             size_t *outlen, const unsigned char *in,
                             size_t inlen);

    if (pencryptfn == NULL)
        EVP_PKEY_meth_get_encrypt(dasync_rsa_orig, NULL, &pencryptfn);
    return pencryptfn(ctx, out, outlen, in, inlen);
}

static int dasync_rsa_decrypt_init(EVP_PKEY_CTX *ctx)
{
    static int (*pdecrypt_init)(EVP_PKEY_CTX *ctx);

    if (pdecrypt_init == NULL)
        EVP_PKEY_meth_get_decrypt(dasync_rsa_orig, &pdecrypt_init, NULL);
    return pdecrypt_init != NULL ? pdecrypt_init(ctx) : 1;
}

static int dasync_rsa_decrypt(EVP_PKEY_CTX *ctx, unsigned char *out,
                              size_t *outlen, const unsigned char *in,
                              size_t inlen)
{
    static int (*pdecrypt)(EVP_PKEY_CTX *ctx, unsigned char *out,
                             size_t *outlen, const unsigned char *in,
                             size_t inlen);

    if (pdecrypt == NULL)
        EVP_PKEY_meth_get_decrypt(dasync_rsa_orig, NULL, &pdecrypt);
    return pdecrypt(ctx, out, outlen, in, inlen);
}

static int dasync_rsa_ctrl(EVP_PKEY_CTX *ctx, int type, int p1, void *p2)
{
    static int (*pctrl)(EVP_PKEY_CTX *ctx, int type, int p1, void *p2);

    if (pctrl == NULL)
        EVP_PKEY_meth_get_ctrl(dasync_rsa_orig, &pctrl, NULL);
    return pctrl(ctx, type, p1, p2);
}

static int dasync_rsa_ctrl_str(EVP_PKEY_CTX *ctx, const char *type,
                               const char *value)
{
    static int (*pctrl_str)(EVP_PKEY_CTX *ctx, const char *type,
                            const char *value);

    if (pctrl_str == NULL)
        EVP_PKEY_meth_get_ctrl(dasync_rsa_orig, NULL, &pctrl_str);
    return pctrl_str(ctx, type, value);
}
