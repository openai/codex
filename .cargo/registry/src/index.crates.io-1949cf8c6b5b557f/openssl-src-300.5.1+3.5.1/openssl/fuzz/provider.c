/*
 * Copyright 2023-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 * https://www.openssl.org/source/license.html
 * or in the file LICENSE in the source distribution.
 */
#include <string.h>
#include <openssl/types.h>
#include <openssl/crypto.h>
#include <openssl/core_names.h>
#include <openssl/kdf.h>
#include <openssl/evp.h>
#include <openssl/provider.h>
#include "fuzzer.h"

#define DEFINE_ALGORITHMS(name, evp) DEFINE_STACK_OF(evp) \
    static int cmp_##evp(const evp *const *a, const evp *const *b); \
    static void collect_##evp(evp *obj, void *stack); \
    static void init_##name(OSSL_LIB_CTX *libctx); \
    static void cleanup_##name(void); \
    static STACK_OF(evp) *name##_collection; \
    static int cmp_##evp(const evp *const *a, const evp *const *b) \
    { \
        return strcmp(OSSL_PROVIDER_get0_name(evp##_get0_provider(*a)), \
                      OSSL_PROVIDER_get0_name(evp##_get0_provider(*b))); \
    } \
    static void collect_##evp(evp *obj, void *stack) \
    { \
        STACK_OF(evp) *obj_stack = stack;  \
        \
        if (sk_##evp##_push(obj_stack, obj) > 0) \
            evp##_up_ref(obj); \
    } \
    static void init_##name(OSSL_LIB_CTX *libctx) \
    { \
        name##_collection = sk_##evp##_new(cmp_##evp); \
        evp##_do_all_provided(libctx, collect_##evp, name##_collection); \
    } \
    static void cleanup_##name(void) \
    { \
        sk_##evp##_pop_free(name##_collection, evp##_free); \
    }

DEFINE_ALGORITHMS(digests, EVP_MD)

DEFINE_ALGORITHMS(kdf, EVP_KDF)

DEFINE_ALGORITHMS(cipher, EVP_CIPHER)

DEFINE_ALGORITHMS(kem, EVP_KEM)

DEFINE_ALGORITHMS(keyexch, EVP_KEYEXCH)

DEFINE_ALGORITHMS(rand, EVP_RAND)

DEFINE_ALGORITHMS(mac, EVP_MAC)

DEFINE_ALGORITHMS(keymgmt, EVP_KEYMGMT)

DEFINE_ALGORITHMS(signature, EVP_SIGNATURE)

DEFINE_ALGORITHMS(asym_ciphers, EVP_ASYM_CIPHER)

static OSSL_LIB_CTX *libctx = NULL;

int FuzzerInitialize(int *argc, char ***argv)
{
    libctx = OSSL_LIB_CTX_new();
    if (libctx == NULL)
        return 0;

    init_digests(libctx);
    init_kdf(libctx);
    init_cipher(libctx);
    init_kem(libctx);
    init_keyexch(libctx);
    init_rand(libctx);
    init_mac(libctx);
    init_keymgmt(libctx);
    init_signature(libctx);
    init_asym_ciphers(libctx);
    return 1;
}

void FuzzerCleanup(void)
{
    cleanup_digests();
    cleanup_kdf();
    cleanup_cipher();
    cleanup_kem();
    cleanup_keyexch();
    cleanup_rand();
    cleanup_mac();
    cleanup_keymgmt();
    cleanup_signature();
    cleanup_asym_ciphers();

    OSSL_LIB_CTX_free(libctx);
}

static int read_uint(const uint8_t **buf, size_t *len, uint64_t **res)
{
    int r = 1;

    if (*len < sizeof(uint64_t)) {
        r = 0;
        goto end;
    }

    *res = OPENSSL_malloc(sizeof(uint64_t));
    **res = (uint64_t) **buf;

    *buf += sizeof(uint64_t);
    *len -= sizeof(uint64_t);
end:
    return r;
}

static int read_int(const uint8_t **buf, size_t *len, int64_t **res)
{
    int r = 1;

    if (*len < sizeof(int64_t)) {
        r = 0;
        goto end;
    }

    *res = OPENSSL_malloc(sizeof(int64_t));
    **res = (int64_t) **buf;

    *buf += sizeof(int64_t);
    *len -= sizeof(int64_t);
end:
    return r;
}

static int read_double(const uint8_t **buf, size_t *len, double **res)
{
    int r = 1;

    if (*len < sizeof(double)) {
        r = 0;
        goto end;
    }

    *res = OPENSSL_malloc(sizeof(double));
    **res = (double) **buf;

    *buf += sizeof(double);
    *len -= sizeof(double);
end:
    return r;
}

static int read_utf8_string(const uint8_t **buf, size_t *len, char **res)
{
    size_t found_len;
    int r;

    found_len = OPENSSL_strnlen((const char *) *buf, *len);

    if (found_len == *len) {
        r = -1;
        goto end;
    }

    found_len++; /* skip over the \0 byte */

    r = (int) found_len;

    *res = (char *) *buf;
    *len -= found_len;
    *buf = *buf + found_len; /* continue after the \0 byte */
end:
    return r;
}

static int read_utf8_ptr(const uint8_t **buf, size_t *len, char **res)
{
    if (*len > 0 && **buf == 0xFF) {
        /* represent NULL somehow */
        *res = NULL;
        *buf += 1;
        *len -= 1;
        return 0;
    }
    return read_utf8_string(buf, len, res);
}

static int read_octet_string(const uint8_t **buf, size_t *len, char **res)
{
    int r;
    size_t i;
    const uint8_t *ptr = *buf;
    int found = 0;

    for (i = 0; i < *len; ++i) {
        if (*ptr == 0xFF &&
            (i + 1 < *len && *(ptr + 1) == 0xFF)) {
            ptr++;
            found = 1;
            break;
        }
        ptr++;
    }

    if (!found) {
        r = -1;
        goto end;
    }

    *res = (char *) *buf;

    r = ptr - *buf;
    *len -= r;
    *buf = ptr;

end:
    return r;
}

static int read_octet_ptr(const uint8_t **buf, size_t *len, char **res)
{
    /* TODO: This representation could need an improvement potentially. */
    if (*len > 1 && **buf == 0xFF && *(*buf + 1) == 0xFF) {
        /* represent NULL somehow */
        *res = NULL;
        *buf += 2;
        *len -= 2;
        return 0;
    }
    return read_octet_string(buf, len, res);
}

static char *DFLT_STR = "";
static char *DFLT_UTF8_PTR = NULL;
static char *DFLT_OCTET_STRING = "";
static char *DFLT_OCTET_PTR = NULL;

static int64_t ITERS = 1;
static uint64_t UITERS = 1;
static int64_t BLOCKSIZE = 8;
static uint64_t UBLOCKSIZE = 8;


static void free_params(OSSL_PARAM *param)
{
    for (; param != NULL && param->key != NULL; param++) {
        switch (param->data_type) {
            case OSSL_PARAM_INTEGER:
            case OSSL_PARAM_UNSIGNED_INTEGER:
            case OSSL_PARAM_REAL:
                if (param->data != NULL) {
                    OPENSSL_free(param->data);
                }
                break;
        }
    }
}

static OSSL_PARAM *fuzz_params(OSSL_PARAM *param, const uint8_t **buf, size_t *len)
{
    OSSL_PARAM *p;
    OSSL_PARAM *fuzzed_parameters;
    int p_num = 0;

    for (p = param; p != NULL && p->key != NULL; p++)
        p_num++;

    fuzzed_parameters = OPENSSL_zalloc(sizeof(OSSL_PARAM) *(p_num + 1));
    p = fuzzed_parameters;

    for (; param != NULL && param->key != NULL; param++) {
        int64_t *use_param = NULL;
        int64_t *p_value_int = NULL;
        uint64_t *p_value_uint = NULL;
        double *p_value_double = NULL;
        char *p_value_utf8_str = DFLT_STR;
        char *p_value_octet_str = DFLT_OCTET_STRING;
        char *p_value_utf8_ptr = DFLT_UTF8_PTR;
        char *p_value_octet_ptr = DFLT_OCTET_PTR;

        int data_len = 0;

        if (!read_int(buf, len, &use_param)) {
            use_param = OPENSSL_malloc(sizeof(uint64_t));
            *use_param = 0;
        }

        switch (param->data_type) {
        case OSSL_PARAM_INTEGER:
            if (strcmp(param->key, OSSL_KDF_PARAM_ITER) == 0) {
                p_value_int = OPENSSL_malloc(sizeof(ITERS));
                *p_value_int = ITERS;
            } else if (strcmp(param->key, OSSL_KDF_PARAM_SCRYPT_N) == 0) {
                p_value_int = OPENSSL_malloc(sizeof(ITERS));
                *p_value_int = ITERS;
            } else if (strcmp(param->key, OSSL_KDF_PARAM_SCRYPT_R) == 0) {
                p_value_int = OPENSSL_malloc(sizeof(BLOCKSIZE));
                *p_value_int = BLOCKSIZE;
            } else if (strcmp(param->key, OSSL_KDF_PARAM_SCRYPT_P) == 0) {
                p_value_int = OPENSSL_malloc(sizeof(BLOCKSIZE));
                *p_value_int = BLOCKSIZE;
            } else if (!*use_param || !read_int(buf, len, &p_value_int)) {
                p_value_int = OPENSSL_malloc(sizeof(int64_t));
                *p_value_int = 0;
            }

            *p = *param;
            p->data = p_value_int;
            p++;
            break;
        case OSSL_PARAM_UNSIGNED_INTEGER:
            if (strcmp(param->key, OSSL_KDF_PARAM_ITER) == 0) {
                p_value_uint = OPENSSL_malloc(sizeof(UITERS));
                *p_value_uint = UITERS;
            } else if (strcmp(param->key, OSSL_KDF_PARAM_SCRYPT_N) == 0) {
                p_value_uint = OPENSSL_malloc(sizeof(UITERS));
                *p_value_uint = UITERS;
            } else if (strcmp(param->key, OSSL_KDF_PARAM_SCRYPT_R) == 0) {
                p_value_uint = OPENSSL_malloc(sizeof(UBLOCKSIZE));
                *p_value_uint = UBLOCKSIZE;
            } else if (strcmp(param->key, OSSL_KDF_PARAM_SCRYPT_P) == 0) {
                p_value_uint = OPENSSL_malloc(sizeof(UBLOCKSIZE));
                *p_value_uint = UBLOCKSIZE;
            } else if (!*use_param || !read_uint(buf, len, &p_value_uint)) {
                p_value_uint = OPENSSL_malloc(sizeof(uint64_t));
                *p_value_uint = 0;
            }

            *p = *param;
            p->data = p_value_uint;
            p++;
            break;
        case OSSL_PARAM_REAL:
            if (!*use_param || !read_double(buf, len, &p_value_double)) {
                p_value_double = OPENSSL_malloc(sizeof(double));
                *p_value_double = 0;
            }

            *p = *param;
            p->data = p_value_double;
            p++;
            break;
        case OSSL_PARAM_UTF8_STRING:
            if (*use_param && (data_len = read_utf8_string(buf, len, &p_value_utf8_str)) < 0)
                data_len = 0;
            *p = *param;
            p->data = p_value_utf8_str;
            p->data_size = data_len;
            p++;
            break;
        case OSSL_PARAM_OCTET_STRING:
            if (*use_param && (data_len = read_octet_string(buf, len, &p_value_octet_str)) < 0)
                data_len = 0;
            *p = *param;
            p->data = p_value_octet_str;
            p->data_size = data_len;
            p++;
            break;
        case OSSL_PARAM_UTF8_PTR:
            if (*use_param && (data_len = read_utf8_ptr(buf, len, &p_value_utf8_ptr)) < 0)
                data_len = 0;
            *p = *param;
            p->data = p_value_utf8_ptr;
            p->data_size = data_len;
            p++;
            break;
        case OSSL_PARAM_OCTET_PTR:
            if (*use_param && (data_len = read_octet_ptr(buf, len, &p_value_octet_ptr)) < 0)
                data_len = 0;
            *p = *param;
            p->data = p_value_octet_ptr;
            p->data_size = data_len;
            p++;
            break;
        default:
            break;
        }

        OPENSSL_free(use_param);
    }

    return fuzzed_parameters;
}

static int do_evp_cipher(const EVP_CIPHER *evp_cipher, const OSSL_PARAM param[])
{
    unsigned char outbuf[1024];
    int outlen, tmplen;
    unsigned char key[] = {0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15};
    unsigned char iv[] = {1, 2, 3, 4, 5, 6, 7, 8};
    const char intext[] = "text";
    EVP_CIPHER_CTX *ctx;

    ctx = EVP_CIPHER_CTX_new();

    if (!EVP_CIPHER_CTX_set_params(ctx, param)) {
        EVP_CIPHER_CTX_free(ctx);
        return 0;
    }

    if (!EVP_EncryptInit_ex2(ctx, evp_cipher, key, iv, NULL)) {
        /* Error */
        EVP_CIPHER_CTX_free(ctx);
        return 0;
    }

    if (!EVP_EncryptUpdate(ctx, outbuf, &outlen, (const unsigned char *) intext, strlen(intext))) {
        /* Error */
        EVP_CIPHER_CTX_free(ctx);
        return 0;
    }
    /*
     * Buffer passed to EVP_EncryptFinal() must be after data just
     * encrypted to avoid overwriting it.
     */
    if (!EVP_EncryptFinal_ex(ctx, outbuf + outlen, &tmplen)) {
        /* Error */
        EVP_CIPHER_CTX_free(ctx);
        return 0;
    }
    outlen += tmplen;
    EVP_CIPHER_CTX_free(ctx);
    return 1;
}

static int do_evp_kdf(EVP_KDF *evp_kdf, const OSSL_PARAM params[])
{
    int r = 1;
    EVP_KDF_CTX *kctx = NULL;
    unsigned char derived[32];

    kctx = EVP_KDF_CTX_new(evp_kdf);

    if (kctx == NULL) {
        r = 0;
        goto end;
    }

    if (EVP_KDF_CTX_set_params(kctx, params) <= 0) {
        r = 0;
        goto end;
    }

    if (EVP_KDF_derive(kctx, derived, sizeof(derived), NULL) <= 0) {
        r = 0;
        goto end;
    }

end:
    EVP_KDF_CTX_free(kctx);
    return r;
}

static int do_evp_mac(EVP_MAC *evp_mac, const OSSL_PARAM params[])
{
    int r = 1;
    const char *key = "mac_key";
    char text[] = "Some Crypto Text";
    EVP_MAC_CTX *ctx = NULL;
    unsigned char buf[4096];
    size_t final_l;

    if ((ctx = EVP_MAC_CTX_new(evp_mac)) == NULL
        || !EVP_MAC_init(ctx, (const unsigned char *) key, strlen(key),
                         params)) {
        r = 0;
        goto end;
    }

    if (EVP_MAC_CTX_set_params(ctx, params) <= 0) {
        r = 0;
        goto end;
    }

    if (!EVP_MAC_update(ctx, (unsigned char *) text, sizeof(text))) {
        r = 0;
        goto end;
    }

    if (!EVP_MAC_final(ctx, buf, &final_l, sizeof(buf))) {
        r = 0;
        goto end;
    }

end:
    EVP_MAC_CTX_free(ctx);
    return r;
}

static int do_evp_rand(EVP_RAND *evp_rand, const OSSL_PARAM params[])
{
    int r = 1;
    EVP_RAND_CTX *ctx = NULL;
    unsigned char buf[4096];

    if (!(ctx = EVP_RAND_CTX_new(evp_rand, NULL))) {
        r = 0;
        goto end;
    }

    if (EVP_RAND_CTX_set_params(ctx, params) <= 0) {
        r = 0;
        goto end;
    }

    if (!EVP_RAND_generate(ctx, buf, sizeof(buf), 0, 0, NULL, 0)) {
        r = 0;
        goto end;
    }

    if (!EVP_RAND_reseed(ctx, 0, 0, 0, NULL, 0)) {
        r = 0;
        goto end;
    }

end:
    EVP_RAND_CTX_free(ctx);
    return r;
}

static int do_evp_sig(EVP_SIGNATURE *evp_sig, const OSSL_PARAM params[])
{
    return 0;
}

static int do_evp_asym_cipher(EVP_ASYM_CIPHER *evp_asym_cipher, const OSSL_PARAM params[])
{
    return 0;
}

static int do_evp_kem(EVP_KEM *evp_kem, const OSSL_PARAM params[])
{
    return 0;
}

static int do_evp_key_exch(EVP_KEYEXCH *evp_kdf, const OSSL_PARAM params[])
{
    return 0;
}

static int do_evp_md(EVP_MD *evp_md, const OSSL_PARAM params[])
{
    int r = 1;
    unsigned char md_value[EVP_MAX_MD_SIZE];
    unsigned int md_len;
    EVP_MD_CTX *mdctx = NULL;

    if (!(mdctx = EVP_MD_CTX_new())) {
        r = 0;
        goto end;
    }

    if (!EVP_MD_CTX_set_params(mdctx, params)) {
        r = 0;
        goto end;
    }

    if (!EVP_DigestInit_ex2(mdctx, evp_md, NULL)) {
        r = 0;
        goto end;
    }
    if (!EVP_DigestUpdate(mdctx, "Test", strlen("Test"))) {
        r = 0;
        goto end;
    }
    if (!EVP_DigestFinal_ex(mdctx, md_value, &md_len)) {
        r = 0;
        goto end;
    }

end:
    EVP_MD_CTX_free(mdctx);
    return r;
}

#define EVP_FUZZ(source, evp, f) \
    do { \
        evp *alg = sk_##evp##_value(source, *algorithm % sk_##evp##_num(source)); \
        OSSL_PARAM *fuzzed_params; \
        \
        if (alg == NULL) \
            break; \
        fuzzed_params = fuzz_params((OSSL_PARAM*) evp##_settable_ctx_params(alg), &buf, &len); \
        if (fuzzed_params != NULL) \
            f(alg, fuzzed_params); \
        free_params(fuzzed_params); \
        OSSL_PARAM_free(fuzzed_params); \
    } while (0);

int FuzzerTestOneInput(const uint8_t *buf, size_t len)
{
    int r = 1;
    uint64_t *operation = NULL;
    int64_t *algorithm = NULL;

    if (!read_uint(&buf, &len, &operation)) {
        r = 0;
        goto end;
    }

    if (!read_int(&buf, &len, &algorithm)) {
        r = 0;
        goto end;
    }

    switch (*operation % 10) {
    case 0:
        EVP_FUZZ(digests_collection, EVP_MD, do_evp_md);
        break;
    case 1:
        EVP_FUZZ(cipher_collection, EVP_CIPHER, do_evp_cipher);
        break;
    case 2:
        EVP_FUZZ(kdf_collection, EVP_KDF, do_evp_kdf);
        break;
    case 3:
        EVP_FUZZ(mac_collection, EVP_MAC, do_evp_mac);
        break;
    case 4:
        EVP_FUZZ(kem_collection, EVP_KEM, do_evp_kem);
        break;
    case 5:
        EVP_FUZZ(rand_collection, EVP_RAND, do_evp_rand);
        break;
    case 6:
        EVP_FUZZ(asym_ciphers_collection, EVP_ASYM_CIPHER, do_evp_asym_cipher);
        break;
    case 7:
        EVP_FUZZ(signature_collection, EVP_SIGNATURE, do_evp_sig);
        break;
    case 8:
        EVP_FUZZ(keyexch_collection, EVP_KEYEXCH, do_evp_key_exch);
        break;
    case 9:
        /*
        Implement and call:
        static int do_evp_keymgmt(EVP_KEYMGMT *evp_kdf, const OSSL_PARAM params[])
        {
            return 0;
        }
        */
        /* not yet implemented */
        break;
    default:
        r = 0;
        goto end;
    }

end:
    OPENSSL_free(operation);
    OPENSSL_free(algorithm);
    return r;
}
