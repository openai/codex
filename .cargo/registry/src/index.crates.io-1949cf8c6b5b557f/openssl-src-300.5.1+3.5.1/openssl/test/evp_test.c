/*
 * Copyright 2015-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#define OPENSSL_SUPPRESS_DEPRECATED /* EVP_PKEY_new_CMAC_key */
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <ctype.h>
#include <openssl/evp.h>
#include <openssl/pem.h>
#include <openssl/err.h>
#include <openssl/provider.h>
#include <openssl/x509v3.h>
#include <openssl/pkcs12.h>
#include <openssl/kdf.h>
#include <openssl/params.h>
#include <openssl/param_build.h>
#include <openssl/core_names.h>
#include <openssl/fips_names.h>
#include <openssl/thread.h>
#include "internal/numbers.h"
#include "internal/nelem.h"
#include "internal/sizes.h"
#include "crypto/evp.h"
#include "testutil.h"

typedef struct evp_test_buffer_st EVP_TEST_BUFFER;
DEFINE_STACK_OF(EVP_TEST_BUFFER)

#define AAD_NUM 4

typedef struct evp_test_method_st EVP_TEST_METHOD;

/* Structure holding test information */
typedef struct evp_test_st {
    STANZA s;                     /* Common test stanza */
    char *name;
    int skip;                     /* Current test should be skipped */
    const EVP_TEST_METHOD *meth;  /* method for this test */
    const char *err, *aux_err;    /* Error string for test */
    char *expected_err;           /* Expected error value of test */
    char *reason;                 /* Expected error reason string */
    void *data;                   /* test specific data */
    int expect_unapproved;
} EVP_TEST;

/* Test method structure */
struct evp_test_method_st {
    /* Name of test as it appears in file */
    const char *name;
    /* Initialise test for "alg" */
    int (*init) (EVP_TEST *t, const char *alg);
    /* Clean up method */
    void (*cleanup) (EVP_TEST *t);
    /* Test specific name value pair processing */
    int (*parse) (EVP_TEST *t, const char *name, const char *value);
    /* Run the test itself */
    int (*run_test) (EVP_TEST *t);
};

/* Linked list of named keys. */
typedef struct key_list_st {
    char *name;
    EVP_PKEY *key;
    struct key_list_st *next;
} KEY_LIST;

typedef enum OPTION_choice {
    OPT_ERR = -1,
    OPT_EOF = 0,
    OPT_CONFIG_FILE,
    OPT_IN_PLACE,
    OPT_PROVIDER_NAME,
    OPT_PROV_PROPQUERY,
    OPT_DATA_CHUNK,
    OPT_TEST_ENUM
} OPTION_CHOICE;

static OSSL_PROVIDER *prov_null = NULL;
static OSSL_PROVIDER *libprov = NULL;
static OSSL_LIB_CTX *libctx = NULL;
static int fips_indicator_callback_unapproved_count = 0;
static int extended_tests = 0;

/* List of public and private keys */
static KEY_LIST *private_keys;
static KEY_LIST *public_keys;

static int find_key(EVP_PKEY **ppk, const char *name, KEY_LIST *lst);
static int parse_bin(const char *value, unsigned char **buf, size_t *buflen);
static int parse_bin_chunk(const char *value, size_t offset, size_t max,
                           unsigned char **buf, size_t *buflen, size_t *out_offset);
static int is_digest_disabled(const char *name);
static int is_pkey_disabled(const char *name);
static int is_mac_disabled(const char *name);
static int is_cipher_disabled(const char *name);
static int is_kdf_disabled(const char *name);

/* A callback that is triggered if fips unapproved mode is detected */
static int fips_indicator_cb(const char *type, const char *desc,
                             const OSSL_PARAM params[])
{
    fips_indicator_callback_unapproved_count++;
    TEST_info("(Indicator Callback received %s : %s is not approved)", type, desc);
    return 1;
}

static int check_fips_approved(EVP_TEST *t, int approved)
{
    /*
     * If the expected result is approved
     * then it is expected that approved will be 1
     * and the fips indicator callback has not been triggered, otherwise
     * approved should be 0 and the fips indicator callback should be triggered.
     */
    if (t->expect_unapproved) {
        if (approved == 1 || fips_indicator_callback_unapproved_count == 0) {
            TEST_error("Test is not expected to be FIPS approved");
            return 0;
        }
    } else {
        if (approved == 0 || fips_indicator_callback_unapproved_count > 0) {
            TEST_error("Test is expected to be FIPS approved");
            return 0;
        }
    }
    return 1;
}

static int mac_check_fips_approved(EVP_MAC_CTX *ctx, EVP_TEST *t)
{
    OSSL_PARAM params[2] = { OSSL_PARAM_END, OSSL_PARAM_END };
    /*
     * For any getters that do not handle the FIPS indicator assume a default
     * value of approved.
     */
    int approved = 1;

    if (EVP_MAC_CTX_gettable_params(ctx) == NULL)
        return 1;

    params[0] = OSSL_PARAM_construct_int(OSSL_MAC_PARAM_FIPS_APPROVED_INDICATOR,
                                         &approved);
    if (!EVP_MAC_CTX_get_params(ctx, params))
        return 0;
    return check_fips_approved(t, approved);
}

static int pkey_check_fips_approved(EVP_PKEY_CTX *ctx, EVP_TEST *t)
{
    OSSL_PARAM params[2] = { OSSL_PARAM_END, OSSL_PARAM_END };
    /*
     * For any getters that do not handle the FIPS indicator assume a default
     * value of approved.
     */
    int approved = 1;
    const OSSL_PARAM *gettables = EVP_PKEY_CTX_gettable_params(ctx);

    if (gettables == NULL
            || OSSL_PARAM_locate_const(gettables,
                                       OSSL_ALG_PARAM_FIPS_APPROVED_INDICATOR)
            == NULL)
        return 1;

    /* Older providers dont have a gettable */
    if (EVP_PKEY_CTX_gettable_params(ctx) == NULL)
        return 1;

    params[0] = OSSL_PARAM_construct_int(OSSL_ALG_PARAM_FIPS_APPROVED_INDICATOR,
                                         &approved);
    if (!EVP_PKEY_CTX_get_params(ctx, params))
        return 0;
    return check_fips_approved(t, approved);
}

static int rand_check_fips_approved(EVP_RAND_CTX *ctx, EVP_TEST *t)
{
    OSSL_PARAM params[2] = { OSSL_PARAM_END, OSSL_PARAM_END };
    /*
     * For any getters that do not handle the FIPS indicator assume a default
     * value of approved.
     */
    int approved = 1;

    if (EVP_RAND_CTX_gettable_params(ctx) == NULL)
        return 1;

    params[0] = OSSL_PARAM_construct_int(OSSL_DRBG_PARAM_FIPS_APPROVED_INDICATOR,
                                         &approved);
    if (!EVP_RAND_CTX_get_params(ctx, params))
        return 0;
    return check_fips_approved(t, approved);
}

static int ctrladd(STACK_OF(OPENSSL_STRING) *controls, const char *value)
{
    char *data = OPENSSL_strdup(value);

    if (data == NULL)
        return -1;

    if (sk_OPENSSL_STRING_push(controls, data) <= 0) {
        OPENSSL_free(data);
        return -1;
    }

    return 1;
}

/* Because OPENSSL_free is a macro, it can't be passed as a function pointer */
static void openssl_free(char *m)
{
    OPENSSL_free(m);
}

static void ctrlfree(STACK_OF(OPENSSL_STRING) *controls)
{
    sk_OPENSSL_STRING_pop_free(controls, openssl_free);
}

/*
 * This is used if ctrl2params() passes settables as NULL.
 * A default list of settable OSSL_PARAM that may be set during an operations
 * init().
 * Using the algorithms settable list is problematic since it requires that the
 * init() has already run.
 */
static const OSSL_PARAM settable_ctx_params[] = {
    OSSL_PARAM_int("key-check", NULL),
    OSSL_PARAM_int("digest-check", NULL),
    OSSL_PARAM_int("ems_check", NULL),
    OSSL_PARAM_int("sign-check", NULL),
    OSSL_PARAM_int("encrypt-check", NULL),
    OSSL_PARAM_int("rsa-pss-saltlen-check", NULL),
    OSSL_PARAM_int("sign-x931-pad-check", NULL),
    OSSL_PARAM_END
};

static int ctrl2params(EVP_TEST *t, STACK_OF(OPENSSL_STRING) *controls,
                       const OSSL_PARAM *settables,
                       OSSL_PARAM params[], size_t params_sz, size_t *params_n)
{
    int i;

    if (settables == NULL)
        settables = settable_ctx_params;
    /* check bounds */
    if (*params_n + sk_OPENSSL_STRING_num(controls) >= params_sz) {
        t->err = "ERR_TOO_MANY_PARAMETERS";
        goto err;
    }
    for (i = 0; i < sk_OPENSSL_STRING_num(controls); i++) {
        char *tmpkey, *tmpval;
        char *value = sk_OPENSSL_STRING_value(controls, i);

        if (!TEST_ptr(tmpkey = OPENSSL_strdup(value))) {
            t->err = "ERR_PARAM_ERROR";
            goto err;
        }
        tmpval = strchr(tmpkey, ':');
        if (tmpval != NULL)
            *tmpval++ = '\0';

        if (tmpval == NULL
            || !OSSL_PARAM_allocate_from_text(&params[*params_n],
                                              settables,
                                              tmpkey, tmpval,
                                              strlen(tmpval), NULL)) {
            OPENSSL_free(tmpkey);
            t->err = "ERR_PARAM_ERROR";
            goto err;
        }
        *params_n += 1;
        OPENSSL_free(tmpkey);
    }
    params[*params_n] = OSSL_PARAM_construct_end();
    return 1;
err:
    return 0;
}

static void ctrl2params_free(OSSL_PARAM params[],
                             size_t params_n, size_t params_n_allocstart)
{
    while (params_n-- > params_n_allocstart) {
        OPENSSL_free(params[params_n].data);
    }
}

static int kdf_check_fips_approved(EVP_KDF_CTX *ctx, EVP_TEST *t)
{
    OSSL_PARAM params[2] = { OSSL_PARAM_END, OSSL_PARAM_END };
    int approved = 1;

    params[0] = OSSL_PARAM_construct_int(OSSL_KDF_PARAM_FIPS_APPROVED_INDICATOR,
                                         &approved);
    if (!EVP_KDF_CTX_get_params(ctx, params))
        return 0;
    return check_fips_approved(t, approved);
}

static int cipher_check_fips_approved(EVP_CIPHER_CTX *ctx, EVP_TEST *t)
{
    OSSL_PARAM params[2] = { OSSL_PARAM_END, OSSL_PARAM_END };
    int approved = 1;

    params[0] = OSSL_PARAM_construct_int(OSSL_CIPHER_PARAM_FIPS_APPROVED_INDICATOR,
                                         &approved);
    if (!EVP_CIPHER_CTX_get_params(ctx, params))
        return 0;
    return check_fips_approved(t, approved);
}

/*
 * Compare two memory regions for equality, returning zero if they differ.
 * However, if there is expected to be an error and the actual error
 * matches then the memory is expected to be different so handle this
 * case without producing unnecessary test framework output.
 */
static int memory_err_compare(EVP_TEST *t, const char *err,
                              const void *expected, size_t expected_len,
                              const void *got, size_t got_len)
{
    int r;

    if (t->expected_err != NULL && strcmp(t->expected_err, err) == 0)
        r = !TEST_mem_ne(expected, expected_len, got, got_len);
    else
        r = TEST_mem_eq(expected, expected_len, got, got_len);
    if (!r)
        t->err = err;
    return r;
}

/* Option specific for evp test */
static int process_mode_in_place;
static const char *propquery = NULL;
static int data_chunk_size;

static int evp_test_process_mode(char *mode)
{
    if (strcmp(mode, "in_place") == 0)
        return 1;
    else if (strcmp(mode, "both") == 0)
        return 0;
    return -1;
}

/*
 * Structure used to hold a list of blocks of memory to test
 * calls to "update" like functions.
 */
struct evp_test_buffer_st {
    unsigned char *buf;
    size_t buflen;
    size_t count;
    int count_set;
};

static void evp_test_buffer_free(EVP_TEST_BUFFER *db)
{
    if (db != NULL) {
        OPENSSL_free(db->buf);
        OPENSSL_free(db);
    }
}

/* append buffer to a list */
static int evp_test_buffer_append(const char *value, size_t max_len,
                                  STACK_OF(EVP_TEST_BUFFER) **sk)
{
    EVP_TEST_BUFFER *db = NULL;
    int rv = 0;
    size_t offset = 0;

    if (*sk == NULL && !TEST_ptr(*sk = sk_EVP_TEST_BUFFER_new_null()))
        goto err;

    do {
        if (!TEST_ptr(db = OPENSSL_zalloc(sizeof(*db))))
            goto err;
        if (max_len == 0) {
            /* parse all in one shot */
            if ((rv = parse_bin(value, &db->buf, &db->buflen)) != 1)
                goto err;
        } else {
            /* parse in chunks */
            size_t new_offset = 0;

            if ((rv = parse_bin_chunk(value, offset, max_len, &db->buf,
                                      &db->buflen, &new_offset)) == -1)
                goto err;
            offset = new_offset;
        }

        db->count = 1;
        db->count_set = 0;

        if (db->buf == NULL)
            evp_test_buffer_free(db);
        else if (db->buf != NULL && !sk_EVP_TEST_BUFFER_push(*sk, db))
            goto err;
        /* if processing by chunks, continue until the whole value is parsed */
    } while (rv == 1 && max_len != 0);

    return 1;

err:
    evp_test_buffer_free(db);
    return 0;
}

/* replace last buffer in list with copies of itself */
static int evp_test_buffer_ncopy(const char *value,
                                 STACK_OF(EVP_TEST_BUFFER) *sk)
{
    EVP_TEST_BUFFER *db;
    unsigned char *tbuf, *p;
    size_t tbuflen;
    int ncopy = atoi(value);
    int i;

    if (ncopy <= 0)
        return 0;
    if (sk == NULL || sk_EVP_TEST_BUFFER_num(sk) == 0)
        return 0;
    db = sk_EVP_TEST_BUFFER_value(sk, sk_EVP_TEST_BUFFER_num(sk) - 1);

    tbuflen = db->buflen * ncopy;
    if (!TEST_ptr(tbuf = OPENSSL_malloc(tbuflen)))
        return 0;
    for (i = 0, p = tbuf; i < ncopy; i++, p += db->buflen)
        memcpy(p, db->buf, db->buflen);

    OPENSSL_free(db->buf);
    db->buf = tbuf;
    db->buflen = tbuflen;
    return 1;
}

/* set repeat count for last buffer in list */
static int evp_test_buffer_set_count(const char *value,
                                     STACK_OF(EVP_TEST_BUFFER) *sk)
{
    EVP_TEST_BUFFER *db;
    int count = atoi(value);

    if (count <= 0)
        return 0;

    if (sk == NULL || sk_EVP_TEST_BUFFER_num(sk) == 0)
        return 0;

    db = sk_EVP_TEST_BUFFER_value(sk, sk_EVP_TEST_BUFFER_num(sk) - 1);
    if (db->count_set != 0)
        return 0;

    db->count = (size_t)count;
    db->count_set = 1;
    return 1;
}

/* call "fn" with each element of the list in turn */
static int evp_test_buffer_do(STACK_OF(EVP_TEST_BUFFER) *sk,
                              int (*fn)(void *ctx,
                                        const unsigned char *buf,
                                        size_t buflen),
                              void *ctx)
{
    int i;

    for (i = 0; i < sk_EVP_TEST_BUFFER_num(sk); i++) {
        EVP_TEST_BUFFER *tb = sk_EVP_TEST_BUFFER_value(sk, i);
        size_t j;

        for (j = 0; j < tb->count; j++) {
            if (fn(ctx, tb->buf, tb->buflen) <= 0)
                return 0;
        }
    }
    return 1;
}

/*
 * Unescape some sequences in string literals (only \n for now).
 * Return an allocated buffer, set |out_len|.  If |input_len|
 * is zero, get an empty buffer but set length to zero.
 */
static unsigned char* unescape(const char *input, size_t input_len,
                               size_t *out_len)
{
    unsigned char *ret, *p;
    size_t i;

    if (input_len == 0) {
        *out_len = 0;
        return OPENSSL_zalloc(1);
    }

    /* Escaping is non-expanding; over-allocate original size for simplicity. */
    if (!TEST_ptr(ret = p = OPENSSL_malloc(input_len)))
        return NULL;

    for (i = 0; i < input_len; i++) {
        if (*input == '\\') {
            if (i == input_len - 1 || *++input != 'n') {
                TEST_error("Bad escape sequence in file");
                goto err;
            }
            *p++ = '\n';
            i++;
            input++;
        } else {
            *p++ = *input++;
        }
    }

    *out_len = p - ret;
    return ret;

 err:
    OPENSSL_free(ret);
    return NULL;
}

/*
 * For a hex string "value" convert to a binary allocated buffer.
 * Return 1 on success or 0 on failure.
 */
static int parse_bin(const char *value, unsigned char **buf, size_t *buflen)
{
    long len;

    /* Check for NULL literal */
    if (strcmp(value, "NULL") == 0) {
        *buf = NULL;
        *buflen = 0;
        return 1;
    }

    /* Check for empty value */
    if (*value == '\0') {
        /*
         * Don't return NULL for zero length buffer. This is needed for
         * some tests with empty keys: HMAC_Init_ex() expects a non-NULL key
         * buffer even if the key length is 0, in order to detect key reset.
         */
        *buf = OPENSSL_malloc(1);
        if (*buf == NULL)
            return 0;
        **buf = 0;
        *buflen = 0;
        return 1;
    }

    /* Check for string literal */
    if (value[0] == '"') {
        size_t vlen = strlen(++value);

        if (vlen == 0 || value[vlen - 1] != '"')
            return 0;
        vlen--;
        *buf = unescape(value, vlen, buflen);
        return *buf == NULL ? 0 : 1;
    }

    /* Otherwise assume as hex literal and convert it to binary buffer */
    if (!TEST_ptr(*buf = OPENSSL_hexstr2buf(value, &len))) {
        TEST_info("Can't convert %s", value);
        TEST_openssl_errors();
        return -1;
    }
    /* Size of input buffer means we'll never overflow */
    *buflen = len;
    return 1;
}

/*
 * Convert at maximum "max" bytes to a binary allocated buffer.
 * Return 1 on success, -1 on failure or 0 for end of value string.
 */
static int parse_bin_chunk(const char *value, size_t offset, size_t max,
                           unsigned char **buf, size_t *buflen, size_t *out_offset)
{
    size_t vlen;
    size_t chunk_len;
    const char *value_str = value[0] == '"' ? value + offset + 1 : value + offset;

    if (max < 1)
        return -1;

    if (*value == '\0' || strcmp(value, "\"\"") == 0) {
        *buf = OPENSSL_malloc(1);
        if (*buf == NULL)
            return 0;
        **buf = 0;
        *buflen = 0;
        return 0;
    }

    if (*value_str == '\0')
        return 0;

    vlen = strlen(value_str);
    if (value[0] == '"') {
        /* Parse string literal */
        if (vlen == 1 && value_str[0] != '"')
            /* Missing ending quotation mark */
            return -1;
        if (vlen == 1 && value_str[0] == '"')
            /* End of value */
            return 0;
        vlen--;
        chunk_len = max > vlen ? vlen : max;
        if ((*buf = unescape(value_str, chunk_len, buflen)) == NULL)
            return -1;
    } else {
        /* Parse hex string chunk */
        long len;
        char *chunk = NULL;

        chunk_len = 2 * max > vlen ? vlen : 2 * max;
        chunk = OPENSSL_strndup(value_str, chunk_len);
        if (chunk == NULL)
            return -1;
        if (!TEST_ptr(*buf = OPENSSL_hexstr2buf(chunk, &len))) {
            OPENSSL_free(chunk);
            TEST_openssl_errors();
            return -1;
        }
        OPENSSL_free(chunk);
        *buflen = len;
    }
    *out_offset = value[0] == '"' ? offset + (*buflen) : offset + 2 * (*buflen);
    return 1;
}

/**
 **  MESSAGE DIGEST TESTS
 **/

typedef struct digest_data_st {
    /* Digest this test is for */
    const EVP_MD *digest;
    EVP_MD *fetched_digest;
    /* Input to digest */
    STACK_OF(EVP_TEST_BUFFER) *input;
    /* Expected output */
    unsigned char *output;
    size_t output_len;
    /* Padding type */
    int pad_type;
    /* XOF mode? */
    int xof;
    /* Size for variable output length but non-XOF */
    size_t digest_size;
} DIGEST_DATA;

static int digest_test_init(EVP_TEST *t, const char *alg)
{
    DIGEST_DATA *mdat;
    const EVP_MD *digest;
    EVP_MD *fetched_digest;

    if (is_digest_disabled(alg)) {
        TEST_info("skipping, '%s' is disabled", alg);
        t->skip = 1;
        return 1;
    }

    if ((digest = fetched_digest = EVP_MD_fetch(libctx, alg, propquery)) == NULL
        && (digest = EVP_get_digestbyname(alg)) == NULL)
        return 0;
    if (!TEST_ptr(mdat = OPENSSL_zalloc(sizeof(*mdat)))) {
        EVP_MD_free(fetched_digest);
        return 0;
    }
    t->data = mdat;
    mdat->digest = digest;
    mdat->fetched_digest = fetched_digest;
    mdat->pad_type = 0;
    mdat->xof = 0;
    if (fetched_digest != NULL)
        TEST_info("%s is fetched", alg);
    return 1;
}

static void digest_test_cleanup(EVP_TEST *t)
{
    DIGEST_DATA *mdat = t->data;

    sk_EVP_TEST_BUFFER_pop_free(mdat->input, evp_test_buffer_free);
    OPENSSL_free(mdat->output);
    EVP_MD_free(mdat->fetched_digest);
}

static int digest_test_parse(EVP_TEST *t,
                             const char *keyword, const char *value)
{
    DIGEST_DATA *mdata = t->data;

    if (strcmp(keyword, "Input") == 0)
        return evp_test_buffer_append(value, data_chunk_size, &mdata->input);
    if (strcmp(keyword, "Output") == 0)
        return parse_bin(value, &mdata->output, &mdata->output_len);
    if (strcmp(keyword, "Count") == 0)
        return evp_test_buffer_set_count(value, mdata->input);
    if (strcmp(keyword, "Ncopy") == 0)
        return evp_test_buffer_ncopy(value, mdata->input);
    if (strcmp(keyword, "Padding") == 0)
        return (mdata->pad_type = atoi(value)) > 0;
    if (strcmp(keyword, "XOF") == 0)
        return (mdata->xof = atoi(value)) > 0;
    if (strcmp(keyword, "OutputSize") == 0) {
        int sz;

        sz = atoi(value);
        if (sz < 0)
            return -1;
        mdata->digest_size = sz;
        return 1;
    }
    return 0;
}

static int digest_update_fn(void *ctx, const unsigned char *buf, size_t buflen)
{
    return EVP_DigestUpdate(ctx, buf, buflen);
}

static int test_duplicate_md_ctx(EVP_TEST *t, EVP_MD_CTX *mctx)
{
    char dont[] = "touch";

    if (!TEST_ptr(mctx))
        return 0;
    if (!EVP_DigestFinalXOF(mctx, (unsigned char *)dont, 0)) {
        EVP_MD_CTX_free(mctx);
        t->err = "DIGESTFINALXOF_ERROR";
        return 0;
    }
    if (!TEST_str_eq(dont, "touch")) {
        EVP_MD_CTX_free(mctx);
        t->err = "DIGESTFINALXOF_ERROR";
        return 0;
    }
    EVP_MD_CTX_free(mctx);
    return 1;
}

static int digest_test_run(EVP_TEST *t)
{
    DIGEST_DATA *expected = t->data;
    EVP_TEST_BUFFER *inbuf;
    EVP_MD_CTX *mctx;
    unsigned char *got = NULL;
    unsigned int got_len;
    size_t size = 0;
    int xof = 0;
    OSSL_PARAM params[4], *p = &params[0];

    t->err = "TEST_FAILURE";
    if (!TEST_ptr(mctx = EVP_MD_CTX_new()))
        goto err;

    got = OPENSSL_malloc(expected->output_len > EVP_MAX_MD_SIZE ?
                         expected->output_len : EVP_MAX_MD_SIZE);
    if (!TEST_ptr(got))
        goto err;

    if (expected->xof > 0) {
        xof |= 1;
        *p++ = OSSL_PARAM_construct_size_t(OSSL_DIGEST_PARAM_XOFLEN,
                                           &expected->output_len);
    }
    if (expected->digest_size > 0) {
        *p++ = OSSL_PARAM_construct_size_t(OSSL_DIGEST_PARAM_SIZE,
                                           &expected->digest_size);
    }
    if (expected->pad_type > 0)
        *p++ = OSSL_PARAM_construct_int(OSSL_DIGEST_PARAM_PAD_TYPE,
                                        &expected->pad_type);
    *p++ = OSSL_PARAM_construct_end();

    if (!EVP_DigestInit_ex2(mctx, expected->digest, params)) {
        t->err = "DIGESTINIT_ERROR";
        goto err;
    }

    if (!evp_test_buffer_do(expected->input, digest_update_fn, mctx)) {
        t->err = "DIGESTUPDATE_ERROR";
        goto err;
    }

    xof |= EVP_MD_xof(expected->digest);
    if (xof) {
        EVP_MD_CTX *mctx_cpy;

        if (!TEST_ptr(mctx_cpy = EVP_MD_CTX_new())) {
            goto err;
        }
        if (!TEST_true(EVP_MD_CTX_copy(mctx_cpy, mctx))) {
            EVP_MD_CTX_free(mctx_cpy);
            goto err;
        } else if (!test_duplicate_md_ctx(t, mctx_cpy)) {
            goto err;
        }

        if (!test_duplicate_md_ctx(t, EVP_MD_CTX_dup(mctx)))
            goto err;

        got_len = expected->output_len;
        if (!EVP_DigestFinalXOF(mctx, got, got_len)) {
            t->err = "DIGESTFINALXOF_ERROR";
            goto err;
        }
    } else {
        if (!EVP_DigestFinal(mctx, got, &got_len)) {
            t->err = "DIGESTFINAL_ERROR";
            goto err;
        }
    }
    if (!TEST_int_eq(expected->output_len, got_len)) {
        t->err = "DIGEST_LENGTH_MISMATCH";
        goto err;
    }
    if (!memory_err_compare(t, "DIGEST_MISMATCH",
                            expected->output, expected->output_len,
                            got, got_len))
        goto err;

    t->err = NULL;

    /* Test the EVP_Q_digest interface as well */
    if (sk_EVP_TEST_BUFFER_num(expected->input) == 1
            && !xof
            /* This should never fail but we need the returned pointer now */
            && !TEST_ptr(inbuf = sk_EVP_TEST_BUFFER_value(expected->input, 0))
            && !inbuf->count_set) {
        OPENSSL_cleanse(got, got_len);
        if (!TEST_true(EVP_Q_digest(libctx,
                                    EVP_MD_get0_name(expected->fetched_digest),
                                    NULL, inbuf->buf, inbuf->buflen,
                                    got, &size))
                || !TEST_mem_eq(got, size,
                                expected->output, expected->output_len)) {
            t->err = "EVP_Q_digest failed";
            goto err;
        }
    }

 err:
    OPENSSL_free(got);
    EVP_MD_CTX_free(mctx);
    return 1;
}

static const EVP_TEST_METHOD digest_test_method = {
    "Digest",
    digest_test_init,
    digest_test_cleanup,
    digest_test_parse,
    digest_test_run
};

/**
***  CIPHER TESTS
**/

typedef struct cipher_data_st {
    const EVP_CIPHER *cipher;
    EVP_CIPHER *fetched_cipher;
    int enc;
    /* EVP_CIPH_GCM_MODE, EVP_CIPH_CCM_MODE or EVP_CIPH_OCB_MODE if AEAD */
    int aead;
    unsigned char *key;
    size_t key_len;
    size_t key_bits; /* Used by RC2 */
    unsigned char *iv;
    unsigned char *next_iv; /* Expected IV state after operation */
    unsigned int rounds;
    size_t iv_len;
    unsigned char *plaintext;
    size_t plaintext_len;
    unsigned char *ciphertext;
    size_t ciphertext_len;
    /* AEAD ciphers only */
    unsigned char *aad[AAD_NUM];
    size_t aad_len[AAD_NUM];
    int tls_aad;
    int tls_version;
    unsigned char *tag;
    const char *cts_mode;
    size_t tag_len;
    int tag_late;
    unsigned char *mac_key;
    size_t mac_key_len;
    const char *xts_standard;
    STACK_OF(OPENSSL_STRING) *init_controls; /* collection of controls */
} CIPHER_DATA;


/*
 * XTS, SIV, CCM, stitched ciphers and Wrap modes have special
 * requirements about input lengths so we don't fragment for those
 */
static int cipher_test_valid_fragmentation(CIPHER_DATA *cdat)
{
    return (cdat->aead == EVP_CIPH_CCM_MODE
            || cdat->aead == EVP_CIPH_CBC_MODE
            || (cdat->aead == -1
                && EVP_CIPHER_get_mode(cdat->cipher) == EVP_CIPH_STREAM_CIPHER)
            || ((EVP_CIPHER_get_flags(cdat->cipher) & EVP_CIPH_FLAG_CTS) != 0)
            || EVP_CIPHER_get_mode(cdat->cipher) == EVP_CIPH_SIV_MODE
            || EVP_CIPHER_get_mode(cdat->cipher) == EVP_CIPH_GCM_SIV_MODE
            || EVP_CIPHER_get_mode(cdat->cipher) == EVP_CIPH_XTS_MODE
            || EVP_CIPHER_get_mode(cdat->cipher) == EVP_CIPH_WRAP_MODE) ? 0 : 1;
}

static int cipher_test_init(EVP_TEST *t, const char *alg)
{
    const EVP_CIPHER *cipher;
    EVP_CIPHER *fetched_cipher;
    CIPHER_DATA *cdat;
    int m;

    if (is_cipher_disabled(alg)) {
        t->skip = 1;
        TEST_info("skipping, '%s' is disabled", alg);
        return 1;
    }

    ERR_set_mark();
    if ((cipher = fetched_cipher = EVP_CIPHER_fetch(libctx, alg, propquery)) == NULL
        && (cipher = EVP_get_cipherbyname(alg)) == NULL) {
        /* a stitched cipher might not be available */
        if (strstr(alg, "HMAC") != NULL) {
            ERR_pop_to_mark();
            t->skip = 1;
            TEST_info("skipping, '%s' is not available", alg);
            return 1;
        }
        ERR_clear_last_mark();
        return 0;
    }
    ERR_clear_last_mark();

    if (!TEST_ptr(cdat = OPENSSL_zalloc(sizeof(*cdat))))
        return 0;

    cdat->init_controls = sk_OPENSSL_STRING_new_null();
    cdat->cipher = cipher;
    cdat->fetched_cipher = fetched_cipher;
    cdat->enc = -1;
    m = EVP_CIPHER_get_mode(cipher);
    if (EVP_CIPHER_get_flags(cipher) & EVP_CIPH_FLAG_AEAD_CIPHER)
        cdat->aead = m != 0 ? m : -1;
    else
        cdat->aead = 0;

    if (data_chunk_size != 0 && !cipher_test_valid_fragmentation(cdat)) {
        ERR_pop_to_mark();
        EVP_CIPHER_free(fetched_cipher);
        OPENSSL_free(cdat);
        t->skip = 1;
        TEST_info("skipping, '%s' does not support fragmentation", alg);
        return 1;
    }

    t->data = cdat;
    if (fetched_cipher != NULL)
        TEST_info("%s is fetched", alg);
    return 1;
}

static void cipher_test_cleanup(EVP_TEST *t)
{
    int i;
    CIPHER_DATA *cdat = t->data;

    OPENSSL_free(cdat->key);
    OPENSSL_free(cdat->iv);
    OPENSSL_free(cdat->next_iv);
    OPENSSL_free(cdat->ciphertext);
    OPENSSL_free(cdat->plaintext);
    for (i = 0; i < AAD_NUM; i++)
        OPENSSL_free(cdat->aad[i]);
    OPENSSL_free(cdat->tag);
    OPENSSL_free(cdat->mac_key);
    EVP_CIPHER_free(cdat->fetched_cipher);
    ctrlfree(cdat->init_controls);
}

static int cipher_test_parse(EVP_TEST *t, const char *keyword,
                             const char *value)
{
    CIPHER_DATA *cdat = t->data;
    int i;

    if (strcmp(keyword, "Key") == 0)
        return parse_bin(value, &cdat->key, &cdat->key_len);
    if (strcmp(keyword, "Rounds") == 0) {
        i = atoi(value);
        if (i < 0)
            return -1;
        cdat->rounds = (unsigned int)i;
        return 1;
    }
    if (strcmp(keyword, "IV") == 0)
        return parse_bin(value, &cdat->iv, &cdat->iv_len);
    if (strcmp(keyword, "NextIV") == 0)
        return parse_bin(value, &cdat->next_iv, &cdat->iv_len);
    if (strcmp(keyword, "Plaintext") == 0)
        return parse_bin(value, &cdat->plaintext, &cdat->plaintext_len);
    if (strcmp(keyword, "Ciphertext") == 0)
        return parse_bin(value, &cdat->ciphertext, &cdat->ciphertext_len);
    if (strcmp(keyword, "KeyBits") == 0) {
        i = atoi(value);
        if (i < 0)
            return -1;
        cdat->key_bits = (size_t)i;
        return 1;
    }
    if (cdat->aead) {
        int tls_aad = 0;

        if (strcmp(keyword, "TLSAAD") == 0)
            cdat->tls_aad = tls_aad = 1;
        if (strcmp(keyword, "AAD") == 0 || tls_aad) {
            for (i = 0; i < AAD_NUM; i++) {
                if (cdat->aad[i] == NULL)
                    return parse_bin(value, &cdat->aad[i], &cdat->aad_len[i]);
            }
            return -1;
        }
        if (strcmp(keyword, "Tag") == 0)
            return parse_bin(value, &cdat->tag, &cdat->tag_len);
        if (strcmp(keyword, "SetTagLate") == 0) {
            if (strcmp(value, "TRUE") == 0)
                cdat->tag_late = 1;
            else if (strcmp(value, "FALSE") == 0)
                cdat->tag_late = 0;
            else
                return -1;
            return 1;
        }
        if (strcmp(keyword, "MACKey") == 0)
            return parse_bin(value, &cdat->mac_key, &cdat->mac_key_len);
        if (strcmp(keyword, "TLSVersion") == 0) {
            char *endptr;

            cdat->tls_version = (int)strtol(value, &endptr, 0);
            return value[0] != '\0' && endptr[0] == '\0';
        }
    }

    if (strcmp(keyword, "Operation") == 0) {
        if (strcmp(value, "ENCRYPT") == 0)
            cdat->enc = 1;
        else if (strcmp(value, "DECRYPT") == 0)
            cdat->enc = 0;
        else
            return -1;
        return 1;
    }
    if (strcmp(keyword, "CTSMode") == 0) {
        cdat->cts_mode = value;
        return 1;
    }
    if (strcmp(keyword, "XTSStandard") == 0) {
        cdat->xts_standard = value;
        return 1;
    }
    if (strcmp(keyword, "CtrlInit") == 0)
        return ctrladd(cdat->init_controls, value);
    return 0;
}

static int cipher_test_enc(EVP_TEST *t, int enc, size_t out_misalign,
                           size_t inp_misalign, int frag, int in_place,
                           const OSSL_PARAM initparams[])
{
    CIPHER_DATA *expected = t->data;
    unsigned char *in, *expected_out, *tmp = NULL;
    size_t in_len, out_len, donelen = 0;
    int ok = 0, tmplen, chunklen, tmpflen, i;
    EVP_CIPHER_CTX *ctx_base = NULL;
    EVP_CIPHER_CTX *ctx = NULL, *duped;
    int fips_dupctx_supported = fips_provider_version_ge(libctx, 3, 2, 0);

    t->err = "TEST_FAILURE";
    if (!TEST_ptr(ctx_base = EVP_CIPHER_CTX_new()))
        goto err;
    if (!TEST_ptr(ctx = EVP_CIPHER_CTX_new()))
        goto err;
    EVP_CIPHER_CTX_set_flags(ctx_base, EVP_CIPHER_CTX_FLAG_WRAP_ALLOW);
    if (enc) {
        in = expected->plaintext;
        in_len = expected->plaintext_len;
        expected_out = expected->ciphertext;
        out_len = expected->ciphertext_len;
    } else {
        in = expected->ciphertext;
        in_len = expected->ciphertext_len;
        expected_out = expected->plaintext;
        out_len = expected->plaintext_len;
    }
    if (in_place == 1) {
        /* Exercise in-place encryption */
        tmp = OPENSSL_malloc(out_misalign + in_len + 2 * EVP_MAX_BLOCK_LENGTH);
        if (!tmp)
            goto err;
        in = memcpy(tmp + out_misalign, in, in_len);
    } else {
        inp_misalign += 16 - ((out_misalign + in_len) & 15);
        /*
         * 'tmp' will store both output and copy of input. We make the copy
         * of input to specifically aligned part of 'tmp'. So we just
         * figured out how much padding would ensure the required alignment,
         * now we allocate extended buffer and finally copy the input just
         * past inp_misalign in expression below. Output will be written
         * past out_misalign...
         */
        tmp = OPENSSL_malloc(out_misalign + in_len + 2 * EVP_MAX_BLOCK_LENGTH +
                             inp_misalign + in_len);
        if (!tmp)
            goto err;
        in = memcpy(tmp + out_misalign + in_len + 2 * EVP_MAX_BLOCK_LENGTH +
                    inp_misalign, in, in_len);
    }
    if (!EVP_CipherInit_ex2(ctx_base, expected->cipher, NULL, NULL, enc,
                            initparams)) {
        t->err = "CIPHERINIT_ERROR";
        goto err;
    }
    if (expected->cts_mode != NULL) {
        OSSL_PARAM params[2];

        params[0] = OSSL_PARAM_construct_utf8_string(OSSL_CIPHER_PARAM_CTS_MODE,
                                                     (char *)expected->cts_mode,
                                                     0);
        params[1] = OSSL_PARAM_construct_end();
        if (!EVP_CIPHER_CTX_set_params(ctx_base, params)) {
            t->err = "INVALID_CTS_MODE";
            goto err;
        }
    }
    if (expected->iv) {
        if (expected->aead) {
            if (EVP_CIPHER_CTX_ctrl(ctx_base, EVP_CTRL_AEAD_SET_IVLEN,
                                     expected->iv_len, 0) <= 0) {
                t->err = "INVALID_IV_LENGTH";
                goto err;
            }
        } else if (expected->iv_len != (size_t)EVP_CIPHER_CTX_get_iv_length(ctx_base)) {
            t->err = "INVALID_IV_LENGTH";
            goto err;
        }
    }
    if (expected->aead && !expected->tls_aad) {
        unsigned char *tag;
        /*
         * If encrypting or OCB just set tag length initially, otherwise
         * set tag length and value.
         */
        if (enc || expected->aead == EVP_CIPH_OCB_MODE || expected->tag_late) {
            t->err = "TAG_LENGTH_SET_ERROR";
            tag = NULL;
        } else {
            t->err = "TAG_SET_ERROR";
            tag = expected->tag;
        }
        if (tag || expected->aead != EVP_CIPH_GCM_MODE) {
            if (EVP_CIPHER_CTX_ctrl(ctx_base, EVP_CTRL_AEAD_SET_TAG,
                                     expected->tag_len, tag) <= 0)
                goto err;
        }
    }

    if (expected->rounds > 0) {
        int  rounds = (int)expected->rounds;

        if (EVP_CIPHER_CTX_ctrl(ctx_base, EVP_CTRL_SET_RC5_ROUNDS, rounds, NULL) <= 0) {
            t->err = "INVALID_ROUNDS";
            goto err;
        }
    }

    if (!EVP_CIPHER_CTX_set_key_length(ctx_base, expected->key_len)) {
        t->err = "INVALID_KEY_LENGTH";
        goto err;
    }
    if (expected->key_bits > 0) {
        int bits = (int)expected->key_bits;

        if (EVP_CIPHER_CTX_ctrl(ctx_base, EVP_CTRL_SET_RC2_KEY_BITS, bits, NULL) <= 0) {
            t->err = "INVALID KEY BITS";
            goto err;
        }
    }
    if (!EVP_CipherInit_ex(ctx_base, NULL, NULL, expected->key, expected->iv, -1)) {
        t->err = "KEY_SET_ERROR";
        goto err;
    }

    /* Check that we get the same IV back */
    if (expected->iv != NULL) {
        /* Some (e.g., GCM) tests use IVs longer than EVP_MAX_IV_LENGTH. */
        unsigned char iv[128];
        if (!TEST_true(EVP_CIPHER_CTX_get_updated_iv(ctx_base, iv, sizeof(iv)))
            || ((EVP_CIPHER_get_flags(expected->cipher) & EVP_CIPH_CUSTOM_IV) == 0
                && !TEST_mem_eq(expected->iv, expected->iv_len, iv,
                                expected->iv_len))) {
            t->err = "INVALID_IV";
            goto err;
        }
    }

    /* Test that the cipher dup functions correctly if it is supported */
    ERR_set_mark();
    if (!EVP_CIPHER_CTX_copy(ctx, ctx_base)) {
        if (fips_dupctx_supported) {
            TEST_info("Doing a copy of Cipher %s Fails!\n",
                      EVP_CIPHER_get0_name(expected->cipher));
            ERR_print_errors_fp(stderr);
            goto err;
        } else {
            TEST_info("Allowing copy fail as an old fips provider is in use.");
        }
        EVP_CIPHER_CTX_free(ctx);
        ctx = ctx_base;
    } else {
        EVP_CIPHER_CTX_free(ctx_base);
        ctx_base = NULL;
    }
    /* Likewise for dup */
    duped = EVP_CIPHER_CTX_dup(ctx);
    if (duped != NULL) {
        EVP_CIPHER_CTX_free(ctx);
        ctx = duped;
    } else {
        if (fips_dupctx_supported) {
            TEST_info("Doing a dup of Cipher %s Fails!\n",
                      EVP_CIPHER_get0_name(expected->cipher));
            ERR_print_errors_fp(stderr);
            goto err;
        } else {
            TEST_info("Allowing dup fail as an old fips provider is in use.");
        }
    }
    ERR_pop_to_mark();

    if (expected->mac_key != NULL
        && EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_SET_MAC_KEY,
                                (int)expected->mac_key_len,
                                (void *)expected->mac_key) <= 0) {
        t->err = "SET_MAC_KEY_ERROR";
        goto err;
    }

    if (expected->tls_version) {
        OSSL_PARAM params[2];

        params[0] = OSSL_PARAM_construct_int(OSSL_CIPHER_PARAM_TLS_VERSION,
                                             &expected->tls_version);
        params[1] = OSSL_PARAM_construct_end();
        if (!EVP_CIPHER_CTX_set_params(ctx, params)) {
            t->err = "SET_TLS_VERSION_ERROR";
            goto err;
        }
    }

    if (expected->aead == EVP_CIPH_CCM_MODE) {
        if (!EVP_CipherUpdate(ctx, NULL, &tmplen, NULL, out_len)) {
            t->err = "CCM_PLAINTEXT_LENGTH_SET_ERROR";
            goto err;
        }
    }
    if (expected->aad[0] != NULL && !expected->tls_aad) {
        t->err = "AAD_SET_ERROR";
        if (!frag) {
            /* Supply the data all in one go or according to data_chunk_size */
            for (i = 0; expected->aad[i] != NULL; i++) {
                size_t aad_len = expected->aad_len[i];
                donelen = 0;

                do {
                    size_t current_aad_len = (size_t) data_chunk_size;

                    if (data_chunk_size == 0 || (size_t) data_chunk_size > aad_len)
                        current_aad_len = aad_len;
                    if (!EVP_CipherUpdate(ctx, NULL, &chunklen,
                                          expected->aad[i] + donelen,
                                          current_aad_len))
                        goto err;
                    donelen += current_aad_len;
                    aad_len -= current_aad_len;
                } while (aad_len > 0);
            }
        } else {
            /* Supply the AAD in chunks less than the block size where possible */
            for (i = 0; expected->aad[i] != NULL; i++) {
                if (expected->aad_len[i] > 0) {
                    if (!EVP_CipherUpdate(ctx, NULL, &chunklen, expected->aad[i], 1))
                        goto err;
                    donelen++;
                }
                if (expected->aad_len[i] > 2) {
                    if (!EVP_CipherUpdate(ctx, NULL, &chunklen,
                                          expected->aad[i] + donelen,
                                          expected->aad_len[i] - 2))
                        goto err;
                    donelen += expected->aad_len[i] - 2;
                }
                if (expected->aad_len[i] > 1
                    && !EVP_CipherUpdate(ctx, NULL, &chunklen,
                                         expected->aad[i] + donelen, 1))
                    goto err;
            }
        }
    }

    if (expected->tls_aad) {
        OSSL_PARAM params[2];
        char *tls_aad;

        /* duplicate the aad as the implementation might modify it */
        if ((tls_aad = OPENSSL_memdup(expected->aad[0],
                                      expected->aad_len[0])) == NULL)
            goto err;
        params[0] = OSSL_PARAM_construct_octet_string(OSSL_CIPHER_PARAM_AEAD_TLS1_AAD,
                                                      tls_aad,
                                                      expected->aad_len[0]);
        params[1] = OSSL_PARAM_construct_end();
        if (!EVP_CIPHER_CTX_set_params(ctx, params)) {
            OPENSSL_free(tls_aad);
            t->err = "TLS1_AAD_ERROR";
            goto err;
        }
        OPENSSL_free(tls_aad);
    } else if (!enc && (expected->aead == EVP_CIPH_OCB_MODE
                        || expected->tag_late)) {
        if (EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_SET_TAG,
                                 expected->tag_len, expected->tag) <= 0) {
            t->err = "TAG_SET_ERROR";
            goto err;
        }
    }
    if (expected->xts_standard != NULL) {
        OSSL_PARAM params[2];

        params[0] =
            OSSL_PARAM_construct_utf8_string(OSSL_CIPHER_PARAM_XTS_STANDARD,
                                             (char *)expected->xts_standard, 0);
        params[1] = OSSL_PARAM_construct_end();
        if (!EVP_CIPHER_CTX_set_params(ctx, params)) {
            t->err = "SET_XTS_STANDARD_ERROR";
            goto err;
        }
    }
    EVP_CIPHER_CTX_set_padding(ctx, 0);
    t->err = "CIPHERUPDATE_ERROR";
    tmplen = 0;
    if (!frag) {
        do {
            /* Supply the data all in one go or according to data_chunk_size */
            size_t current_in_len = (size_t) data_chunk_size;

            if (data_chunk_size == 0 || (size_t) data_chunk_size > in_len)
                current_in_len = in_len;
            if (!EVP_CipherUpdate(ctx, tmp + out_misalign + tmplen, &chunklen,
                                  in, current_in_len))
                goto err;
            tmplen += chunklen;
            in += current_in_len;
            in_len -= current_in_len;
        } while (in_len > 0);
    } else {
        /* Supply the data in chunks less than the block size where possible */
        if (in_len > 0) {
            if (!EVP_CipherUpdate(ctx, tmp + out_misalign, &chunklen, in, 1))
                goto err;
            tmplen += chunklen;
            in++;
            in_len--;
        }
        if (in_len > 1) {
            if (!EVP_CipherUpdate(ctx, tmp + out_misalign + tmplen, &chunklen,
                                  in, in_len - 1))
                goto err;
            tmplen += chunklen;
            in += in_len - 1;
            in_len = 1;
        }
        if (in_len > 0) {
            if (!EVP_CipherUpdate(ctx, tmp + out_misalign + tmplen, &chunklen,
                                  in, 1))
                goto err;
            tmplen += chunklen;
        }
    }
    if (!EVP_CipherFinal_ex(ctx, tmp + out_misalign + tmplen, &tmpflen)) {
        t->err = "CIPHERFINAL_ERROR";
        goto err;
    }
    if (!cipher_check_fips_approved(ctx, t)) {
        t->err = "FIPSAPPROVED_ERROR";
        goto err;
    }

    if (!enc && expected->tls_aad) {
        if (expected->tls_version >= TLS1_1_VERSION
            && (EVP_CIPHER_is_a(expected->cipher, "AES-128-CBC-HMAC-SHA1")
                || EVP_CIPHER_is_a(expected->cipher, "AES-256-CBC-HMAC-SHA1"))) {
            tmplen -= expected->iv_len;
            expected_out += expected->iv_len;
            out_misalign += expected->iv_len;
        }
        if ((int)out_len > tmplen + tmpflen)
            out_len = tmplen + tmpflen;
    }
    if (!memory_err_compare(t, "VALUE_MISMATCH", expected_out, out_len,
                            tmp + out_misalign, tmplen + tmpflen))
        goto err;
    if (enc && expected->aead && !expected->tls_aad) {
        unsigned char rtag[48]; /* longest known for TLS_SHA384_SHA384 */

        if (!TEST_size_t_le(expected->tag_len, sizeof(rtag))) {
            t->err = "TAG_LENGTH_INTERNAL_ERROR";
            goto err;
        }
        if (EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_GET_TAG,
                                 expected->tag_len, rtag) <= 0) {
            t->err = "TAG_RETRIEVE_ERROR";
            goto err;
        }
        if (!memory_err_compare(t, "TAG_VALUE_MISMATCH",
                                expected->tag, expected->tag_len,
                                rtag, expected->tag_len))
            goto err;
    }
    /* Check the updated IV */
    if (expected->next_iv != NULL) {
        /* Some (e.g., GCM) tests use IVs longer than EVP_MAX_IV_LENGTH. */
        unsigned char iv[128];
        if (!TEST_true(EVP_CIPHER_CTX_get_updated_iv(ctx, iv, sizeof(iv)))
            || ((EVP_CIPHER_get_flags(expected->cipher) & EVP_CIPH_CUSTOM_IV) == 0
                && !TEST_mem_eq(expected->next_iv, expected->iv_len, iv,
                                expected->iv_len))) {
            t->err = "INVALID_NEXT_IV";
            goto err;
        }
    }

    t->err = NULL;
    ok = 1;
 err:
    OPENSSL_free(tmp);
    if (ctx != ctx_base)
        EVP_CIPHER_CTX_free(ctx_base);
    EVP_CIPHER_CTX_free(ctx);
    return ok;
}

static int cipher_test_run(EVP_TEST *t)
{
    CIPHER_DATA *cdat = t->data;
    int rv, frag, fragmax, in_place;
    size_t out_misalign, inp_misalign;
    OSSL_PARAM initparams[2] = { OSSL_PARAM_END, OSSL_PARAM_END };
    size_t params_n = 0;

    TEST_info("RUNNING TEST FOR CIPHER %s\n", EVP_CIPHER_get0_name(cdat->cipher));
    if (!cdat->key) {
        t->err = "NO_KEY";
        return 0;
    }
    if (!cdat->iv && EVP_CIPHER_get_iv_length(cdat->cipher) > 0) {
        /* IV is optional and usually omitted in wrap mode */
        if (EVP_CIPHER_get_mode(cdat->cipher) != EVP_CIPH_WRAP_MODE) {
            t->err = "NO_IV";
            return 0;
        }
    }
    if (cdat->aead && cdat->tag == NULL && !cdat->tls_aad) {
        t->err = "NO_TAG";
        return 0;
    }

    if (sk_OPENSSL_STRING_num(cdat->init_controls) > 0) {
        if (!ctrl2params(t, cdat->init_controls, NULL,
                         initparams, OSSL_NELEM(initparams), &params_n))
            return 0;
    }

    fragmax = (cipher_test_valid_fragmentation(cdat) == 0) ? 0 : 1;
    for (in_place = 1; in_place >= 0; in_place--) {
        static char aux_err[64];

        t->aux_err = aux_err;
        /* Test only in-place data processing */
        if (process_mode_in_place == 1 && in_place == 0)
            break;

        for (frag = 0; frag <= fragmax; frag++) {
            if (frag == 1 && data_chunk_size != 0)
                break;
            for (out_misalign = 0; out_misalign <= 1; out_misalign++) {
                for (inp_misalign = 0; inp_misalign <= 1; inp_misalign++) {
                    /* Skip input misalign tests for in-place processing */
                    if (inp_misalign == 1 && in_place == 1)
                        break;
                    if (in_place == 1) {
                        BIO_snprintf(aux_err, sizeof(aux_err),
                                        "%s in-place, %sfragmented",
                                        out_misalign ? "misaligned" : "aligned",
                                        frag ? "" : "not ");
                    } else {
                        BIO_snprintf(aux_err, sizeof(aux_err),
                                        "%s output and %s input, %sfragmented",
                                        out_misalign ? "misaligned" : "aligned",
                                        inp_misalign ? "misaligned" : "aligned",
                                        frag ? "" : "not ");
                    }
                    if (cdat->enc) {
                        rv = cipher_test_enc(t, 1, out_misalign, inp_misalign,
                                             frag, in_place, initparams);
                        if (rv != 1)
                            goto end;
                    }
                    if (cdat->enc != 1) {
                        rv = cipher_test_enc(t, 0, out_misalign, inp_misalign,
                                             frag, in_place, initparams);
                        if (rv != 1)
                            goto end;
                    }
                }
            }
        }
    }
    ctrl2params_free(initparams, params_n, 0);
    t->aux_err = NULL;
    return 1;
 end:
    ctrl2params_free(initparams, params_n, 0);
    return (rv < 0 ? 0 : 1);
}

static const EVP_TEST_METHOD cipher_test_method = {
    "Cipher",
    cipher_test_init,
    cipher_test_cleanup,
    cipher_test_parse,
    cipher_test_run
};


/**
 **  MAC TESTS
 **/

typedef struct mac_data_st {
    /* MAC type in one form or another */
    char *mac_name;
    EVP_MAC *mac;                /* for mac_test_run_mac */
    int type;                    /* for mac_test_run_pkey */
    /* Algorithm string for this MAC */
    char *alg;
    /* MAC key */
    unsigned char *key;
    size_t key_len;
    /* MAC IV (GMAC) */
    unsigned char *iv;
    size_t iv_len;
    /* Input to MAC */
    unsigned char *input;
    size_t input_len;
    /* Expected output */
    unsigned char *output;
    size_t output_len;
    unsigned char *custom;
    size_t custom_len;
    /* MAC salt (blake2) */
    unsigned char *salt;
    size_t salt_len;
    /* XOF mode? */
    int xof;
    /* Reinitialization fails */
    int no_reinit;
    /* Collection of controls */
    STACK_OF(OPENSSL_STRING) *controls;
    /* Output size */
    int output_size;
    /* Block size */
    int block_size;
} MAC_DATA;

static int mac_test_init(EVP_TEST *t, const char *alg)
{
    EVP_MAC *mac = NULL;
    int type = NID_undef;
    MAC_DATA *mdat;

    if (is_mac_disabled(alg)) {
        TEST_info("skipping, '%s' is disabled", alg);
        t->skip = 1;
        return 1;
    }
    if ((mac = EVP_MAC_fetch(libctx, alg, propquery)) == NULL) {
        /*
         * Since we didn't find an EVP_MAC, we check for known EVP_PKEY methods
         * For debugging purposes, we allow 'NNNN by EVP_PKEY' to force running
         * the EVP_PKEY method.
         */
        size_t sz = strlen(alg);
        static const char epilogue[] = " by EVP_PKEY";

        if (sz >= sizeof(epilogue)
            && strcmp(alg + sz - (sizeof(epilogue) - 1), epilogue) == 0)
            sz -= sizeof(epilogue) - 1;

        if (strncmp(alg, "HMAC", sz) == 0)
            type = EVP_PKEY_HMAC;
        else if (strncmp(alg, "CMAC", sz) == 0)
            type = EVP_PKEY_CMAC;
        else if (strncmp(alg, "Poly1305", sz) == 0)
            type = EVP_PKEY_POLY1305;
        else if (strncmp(alg, "SipHash", sz) == 0)
            type = EVP_PKEY_SIPHASH;
        else
            return 0;
    }

    if (!TEST_ptr(mdat = OPENSSL_zalloc(sizeof(*mdat))))
        return 0;

    mdat->type = type;
    if (!TEST_ptr(mdat->mac_name = OPENSSL_strdup(alg))) {
        OPENSSL_free(mdat);
        return 0;
    }

    mdat->mac = mac;
    if (!TEST_ptr(mdat->controls = sk_OPENSSL_STRING_new_null())) {
        OPENSSL_free(mdat->mac_name);
        OPENSSL_free(mdat);
        return 0;
    }

    mdat->output_size = mdat->block_size = -1;
    t->data = mdat;
    return 1;
}

static void mac_test_cleanup(EVP_TEST *t)
{
    MAC_DATA *mdat = t->data;

    EVP_MAC_free(mdat->mac);
    OPENSSL_free(mdat->mac_name);
    sk_OPENSSL_STRING_pop_free(mdat->controls, openssl_free);
    OPENSSL_free(mdat->alg);
    OPENSSL_free(mdat->key);
    OPENSSL_free(mdat->iv);
    OPENSSL_free(mdat->custom);
    OPENSSL_free(mdat->salt);
    OPENSSL_free(mdat->input);
    OPENSSL_free(mdat->output);
}

static int mac_test_parse(EVP_TEST *t,
                          const char *keyword, const char *value)
{
    MAC_DATA *mdata = t->data;

    if (strcmp(keyword, "Key") == 0)
        return parse_bin(value, &mdata->key, &mdata->key_len);
    if (strcmp(keyword, "IV") == 0)
        return parse_bin(value, &mdata->iv, &mdata->iv_len);
    if (strcmp(keyword, "Custom") == 0)
        return parse_bin(value, &mdata->custom, &mdata->custom_len);
    if (strcmp(keyword, "Salt") == 0)
        return parse_bin(value, &mdata->salt, &mdata->salt_len);
    if (strcmp(keyword, "Algorithm") == 0) {
        mdata->alg = OPENSSL_strdup(value);
        if (mdata->alg == NULL)
            return -1;
        return 1;
    }
    if (strcmp(keyword, "Input") == 0)
        return parse_bin(value, &mdata->input, &mdata->input_len);
    if (strcmp(keyword, "Output") == 0)
        return parse_bin(value, &mdata->output, &mdata->output_len);
    if (strcmp(keyword, "XOF") == 0)
        return mdata->xof = 1;
    if (strcmp(keyword, "NoReinit") == 0)
        return mdata->no_reinit = 1;
    if (strcmp(keyword, "Ctrl") == 0)
        return ctrladd(mdata->controls, value);
    if (strcmp(keyword, "OutputSize") == 0) {
        mdata->output_size = atoi(value);
        if (mdata->output_size < 0)
            return -1;
        return 1;
    }
    if (strcmp(keyword, "BlockSize") == 0) {
        mdata->block_size = atoi(value);
        if (mdata->block_size < 0)
            return -1;
        return 1;
    }
    return 0;
}

static int mac_test_ctrl_pkey(EVP_TEST *t, EVP_PKEY_CTX *pctx,
                              const char *value)
{
    int rv = 0;
    char *p, *tmpval;

    if (!TEST_ptr(tmpval = OPENSSL_strdup(value)))
        return 0;
    p = strchr(tmpval, ':');
    if (p != NULL) {
        *p++ = '\0';
        rv = EVP_PKEY_CTX_ctrl_str(pctx, tmpval, p);
    }
    if (rv == -2)
        t->err = "PKEY_CTRL_INVALID";
    else if (rv <= 0)
        t->err = "PKEY_CTRL_ERROR";
    else
        rv = 1;
    OPENSSL_free(tmpval);
    return rv > 0;
}

static int mac_test_run_pkey(EVP_TEST *t)
{
    MAC_DATA *expected = t->data;
    EVP_MD_CTX *mctx = NULL;
    EVP_PKEY_CTX *pctx = NULL, *genctx = NULL;
    EVP_PKEY *key = NULL;
    const char *mdname = NULL;
    EVP_CIPHER *cipher = NULL;
    unsigned char *got = NULL;
    size_t got_len;
    int i;
    size_t input_len, donelen;

    /* We don't do XOF mode via PKEY */
    if (expected->xof)
        return 1;

    if (expected->alg == NULL)
        TEST_info("Trying the EVP_PKEY %s test", OBJ_nid2sn(expected->type));
    else
        TEST_info("Trying the EVP_PKEY %s test with %s",
                  OBJ_nid2sn(expected->type), expected->alg);

    if (expected->type == EVP_PKEY_CMAC) {
#ifdef OPENSSL_NO_DEPRECATED_3_0
        TEST_info("skipping, PKEY CMAC '%s' is disabled", expected->alg);
        t->skip = 1;
        t->err = NULL;
        goto err;
#else
        OSSL_LIB_CTX *tmpctx;

        if (expected->alg != NULL && is_cipher_disabled(expected->alg)) {
            TEST_info("skipping, PKEY CMAC '%s' is disabled", expected->alg);
            t->skip = 1;
            t->err = NULL;
            goto err;
        }
        if (!TEST_ptr(cipher = EVP_CIPHER_fetch(libctx, expected->alg, propquery))) {
            t->err = "MAC_KEY_CREATE_ERROR";
            goto err;
        }
        tmpctx = OSSL_LIB_CTX_set0_default(libctx);
        key = EVP_PKEY_new_CMAC_key(NULL, expected->key, expected->key_len,
                                    cipher);
        OSSL_LIB_CTX_set0_default(tmpctx);
#endif
    } else {
        key = EVP_PKEY_new_raw_private_key_ex(libctx,
                                              OBJ_nid2sn(expected->type), NULL,
                                              expected->key, expected->key_len);
    }
    if (key == NULL) {
        t->err = "MAC_KEY_CREATE_ERROR";
        goto err;
    }

    if (expected->type == EVP_PKEY_HMAC && expected->alg != NULL) {
        if (is_digest_disabled(expected->alg)) {
            TEST_info("skipping, HMAC '%s' is disabled", expected->alg);
            t->skip = 1;
            t->err = NULL;
            goto err;
        }
        mdname = expected->alg;
    }
    if (!TEST_ptr(mctx = EVP_MD_CTX_new())) {
        t->err = "INTERNAL_ERROR";
        goto err;
    }
    if (!EVP_DigestSignInit_ex(mctx, &pctx, mdname, libctx, NULL, key, NULL)) {
        t->err = "DIGESTSIGNINIT_ERROR";
        goto err;
    }
    for (i = 0; i < sk_OPENSSL_STRING_num(expected->controls); i++)
        if (!mac_test_ctrl_pkey(t, pctx,
                                sk_OPENSSL_STRING_value(expected->controls,
                                                        i))) {
            t->err = "EVPPKEYCTXCTRL_ERROR";
            goto err;
        }
    input_len = expected->input_len;
    donelen = 0;
    do {
        size_t current_len = (size_t) data_chunk_size;

        if (data_chunk_size == 0 || (size_t) data_chunk_size > input_len)
            current_len = input_len;
        if (!EVP_DigestSignUpdate(mctx, expected->input + donelen, current_len)) {
            t->err = "DIGESTSIGNUPDATE_ERROR";
            goto err;
        }
        donelen += current_len;
        input_len -= current_len;
    } while (input_len > 0);

    if (!EVP_DigestSignFinal(mctx, NULL, &got_len)) {
        t->err = "DIGESTSIGNFINAL_LENGTH_ERROR";
        goto err;
    }
    if (!TEST_ptr(got = OPENSSL_malloc(got_len))) {
        t->err = "TEST_FAILURE";
        goto err;
    }
    if (!EVP_DigestSignFinal(mctx, got, &got_len)
            || !memory_err_compare(t, "TEST_MAC_ERR",
                                   expected->output, expected->output_len,
                                   got, got_len)) {
        t->err = "TEST_MAC_ERR";
        goto err;
    }
    t->err = NULL;
 err:
    EVP_CIPHER_free(cipher);
    EVP_MD_CTX_free(mctx);
    OPENSSL_free(got);
    EVP_PKEY_CTX_free(genctx);
    EVP_PKEY_free(key);
    return 1;
}

static int mac_test_run_mac(EVP_TEST *t)
{
    MAC_DATA *expected = t->data;
    EVP_MAC_CTX *ctx = NULL;
    unsigned char *got = NULL;
    size_t got_len = 0, size = 0;
    size_t size_before_init = 0, size_after_init, size_val = 0;
    int block_size = -1, output_size = -1;
    OSSL_PARAM params[21], sizes[3], *psizes = sizes, *p;
    size_t params_n = 0;
    size_t params_n_allocstart = 0;
    const OSSL_PARAM *defined_params =
        EVP_MAC_settable_ctx_params(expected->mac);
    int xof;
    int reinit = 1;
    size_t input_len, donelen ;

    if (expected->alg == NULL)
        TEST_info("Trying the EVP_MAC %s test", expected->mac_name);
    else
        TEST_info("Trying the EVP_MAC %s test with %s",
                  expected->mac_name, expected->alg);

    if (expected->alg != NULL) {
        int skip = 0;

        /*
         * The underlying algorithm may be a cipher or a digest.
         * We don't know which it is, but we can ask the MAC what it
         * should be and bet on that.
         */
        if (OSSL_PARAM_locate_const(defined_params,
                                    OSSL_MAC_PARAM_CIPHER) != NULL) {
            if (is_cipher_disabled(expected->alg))
                skip = 1;
            else
                params[params_n++] =
                    OSSL_PARAM_construct_utf8_string(OSSL_MAC_PARAM_CIPHER,
                                                     expected->alg, 0);
        } else if (OSSL_PARAM_locate_const(defined_params,
                                           OSSL_MAC_PARAM_DIGEST) != NULL) {
            if (is_digest_disabled(expected->alg))
                skip = 1;
            else
                params[params_n++] =
                    OSSL_PARAM_construct_utf8_string(OSSL_MAC_PARAM_DIGEST,
                                                     expected->alg, 0);
        } else {
            t->err = "MAC_BAD_PARAMS";
            goto err;
        }
        if (skip) {
            TEST_info("skipping, algorithm '%s' is disabled", expected->alg);
            t->skip = 1;
            t->err = NULL;
            goto err;
        }
    }
    if (expected->custom != NULL)
        params[params_n++] =
            OSSL_PARAM_construct_octet_string(OSSL_MAC_PARAM_CUSTOM,
                                              expected->custom,
                                              expected->custom_len);
    if (expected->salt != NULL)
        params[params_n++] =
            OSSL_PARAM_construct_octet_string(OSSL_MAC_PARAM_SALT,
                                              expected->salt,
                                              expected->salt_len);
    if (expected->iv != NULL)
        params[params_n++] =
            OSSL_PARAM_construct_octet_string(OSSL_MAC_PARAM_IV,
                                              expected->iv,
                                              expected->iv_len);

    params_n_allocstart = params_n;
    if (!ctrl2params(t, expected->controls, defined_params,
                     params, OSSL_NELEM(params), &params_n))
        goto err;

    p = OSSL_PARAM_locate(params + params_n_allocstart, "size");
    if (p != NULL) {
        if (!OSSL_PARAM_get_size_t(p, &size_val))
            goto err;
    }

    if ((ctx = EVP_MAC_CTX_new(expected->mac)) == NULL) {
        t->err = "MAC_CREATE_ERROR";
        goto err;
    }
    if (fips_provider_version_gt(libctx, 3, 2, 0)) {
        /* HMAC will put an error on the stack here (digest is not set yet) */
        ERR_set_mark();
        size_before_init = EVP_MAC_CTX_get_mac_size(ctx);
        ERR_pop_to_mark();
    }
    if (!EVP_MAC_init(ctx, expected->key, expected->key_len, params)) {
        t->err = "MAC_INIT_ERROR";
        goto err;
    }
    size_after_init = EVP_MAC_CTX_get_mac_size(ctx);
    if (!TEST_false(size_before_init == 0 && size_after_init == 0)) {
        t->err = "MAC SIZE not set";
        goto err;
    }
    if (size_before_init != 0) {
        /* mac-size not modified by init params */
        if (size_val == 0 && !TEST_size_t_eq(size_before_init, size_after_init)) {
            t->err = "MAC SIZE check failed";
            goto err;
        }
        /* mac-size modified by init params */
        if (size_val != 0 && !TEST_size_t_eq(size_val, size_after_init)) {
            t->err = "MAC SIZE check failed";
            goto err;
        }
    }
    if (expected->output_size >= 0)
        *psizes++ = OSSL_PARAM_construct_int(OSSL_MAC_PARAM_SIZE,
                                             &output_size);
    if (expected->block_size >= 0)
        *psizes++ = OSSL_PARAM_construct_int(OSSL_MAC_PARAM_BLOCK_SIZE,
                                             &block_size);
    if (psizes != sizes) {
        *psizes = OSSL_PARAM_construct_end();
        if (!TEST_true(EVP_MAC_CTX_get_params(ctx, sizes))) {
            t->err = "INTERNAL_ERROR";
            goto err;
        }
        if (expected->output_size >= 0
                && !TEST_int_eq(output_size, expected->output_size)) {
            t->err = "TEST_FAILURE";
            goto err;
        }
        if (expected->block_size >= 0
                && !TEST_int_eq(block_size, expected->block_size)) {
            t->err = "TEST_FAILURE";
            goto err;
        }
    }
 retry:
    input_len = expected->input_len;
    donelen = 0;
    do {
        size_t current_len = (size_t) data_chunk_size;

        if (data_chunk_size == 0 || (size_t) data_chunk_size > input_len)
            current_len = input_len;
        if (!EVP_MAC_update(ctx, expected->input + donelen, current_len)) {
            t->err = "MAC_UPDATE_ERROR";
            goto err;
        }
        donelen += current_len;
        input_len -= current_len;
    } while (input_len > 0);

    xof = expected->xof;
    if (xof) {
        if (!TEST_ptr(got = OPENSSL_malloc(expected->output_len))) {
            t->err = "TEST_FAILURE";
            goto err;
        }
        if (!EVP_MAC_finalXOF(ctx, got, expected->output_len)
            || !memory_err_compare(t, "TEST_MAC_ERR",
                                   expected->output, expected->output_len,
                                   got, expected->output_len)) {
            t->err = "MAC_FINAL_ERROR";
            goto err;
        }
    } else {
        if (!EVP_MAC_final(ctx, NULL, &got_len, 0)) {
            t->err = "MAC_FINAL_LENGTH_ERROR";
            goto err;
        }
        if (!TEST_ptr(got = OPENSSL_malloc(got_len))) {
            t->err = "TEST_FAILURE";
            goto err;
        }
        if (!EVP_MAC_final(ctx, got, &got_len, got_len)
            || !memory_err_compare(t, "TEST_MAC_ERR",
                                   expected->output, expected->output_len,
                                   got, got_len)) {
            t->err = "TEST_MAC_ERR";
            goto err;
        }
        if (!mac_check_fips_approved(ctx, t))
            goto err;
    }
    /* FIPS(3.0.0): can't reinitialise MAC contexts #18100 */
    if (reinit-- && fips_provider_version_gt(libctx, 3, 0, 0)) {
        OSSL_PARAM ivparams[2] = { OSSL_PARAM_END, OSSL_PARAM_END };
        int ret;

        /* If the MAC uses IV, we have to set it again */
        if (expected->iv != NULL) {
            ivparams[0] =
                OSSL_PARAM_construct_octet_string(OSSL_MAC_PARAM_IV,
                                                  expected->iv,
                                                  expected->iv_len);
            ivparams[1] = OSSL_PARAM_construct_end();
        }
        ERR_set_mark();
        ret = EVP_MAC_init(ctx, NULL, 0, ivparams);
        if (expected->no_reinit) {
            if (ret) {
                ERR_clear_last_mark();
                t->err = "MAC_REINIT_SHOULD_FAIL";
                goto err;
            }
        } else if (ret) {
            ERR_clear_last_mark();
            OPENSSL_free(got);
            got = NULL;
            goto retry;
        } else {
            ERR_clear_last_mark();
            t->err = "MAC_REINIT_ERROR";
            goto err;
        }
        /* If reinitialization fails, it is unsupported by the algorithm */
        ERR_pop_to_mark();
    }
    t->err = NULL;

    /* Test the EVP_Q_mac interface as well */
    if (!xof) {
        OPENSSL_cleanse(got, got_len);
        if (!TEST_true(EVP_Q_mac(libctx, expected->mac_name, NULL,
                                 expected->alg, params,
                                 expected->key, expected->key_len,
                                 expected->input, expected->input_len,
                                 got, got_len, &size))
                || !TEST_mem_eq(got, size,
                                expected->output, expected->output_len)) {
            t->err = "EVP_Q_mac failed";
            goto err;
        }
    }
 err:
    ctrl2params_free(params, params_n, params_n_allocstart);
    EVP_MAC_CTX_free(ctx);
    OPENSSL_free(got);
    return 1;
}

static int mac_test_run(EVP_TEST *t)
{
    MAC_DATA *expected = t->data;

    if (expected->mac != NULL)
        return mac_test_run_mac(t);
    return mac_test_run_pkey(t);
}

static const EVP_TEST_METHOD mac_test_method = {
    "MAC",
    mac_test_init,
    mac_test_cleanup,
    mac_test_parse,
    mac_test_run
};

typedef struct kem_data_st {
    /* Context for this operation */
    EVP_PKEY_CTX *ctx;
    const char *op;
    /* Input to decapsulate */
    unsigned char *input;
    size_t inputlen;
    /* Expected secret */
    unsigned char *output;
    size_t outputlen;
    STACK_OF(OPENSSL_STRING) *init_ctrls;
    /* Algorithm name */
    char *algname;
    /* Name of previously generated key */
    char *keyname;
    /* Encoded public key */
    unsigned char *encoded_pub_key;
    size_t encoded_pub_key_len;
    /* Encoded private key */
    unsigned char *encoded_priv_key;
    size_t encoded_priv_key_len;
    /* Entropy for encapsulation */
    unsigned char *entropy;
    size_t entropylen;
    /* Ciphertext */
    unsigned char *ciphertext;
    size_t ciphertext_len;
} KEM_DATA;

static int kem_test_init(EVP_TEST *t, const char *alg)
{
    KEM_DATA *kdata = NULL;
    EVP_PKEY *pkey = NULL;

    if (!TEST_ptr(kdata = OPENSSL_zalloc(sizeof(*kdata)))
        || !TEST_ptr(kdata->algname = OPENSSL_strdup(alg)))
        goto err;

    kdata->init_ctrls = sk_OPENSSL_STRING_new_null();
    t->data = kdata;
    return 1;
err:
    EVP_PKEY_free(pkey);
    OPENSSL_free(kdata);
    return 0;
}

static void kem_test_cleanup(EVP_TEST *t)
{
    KEM_DATA *kdata = t->data;

    ctrlfree(kdata->init_ctrls);
    OPENSSL_free(kdata->input);
    OPENSSL_free(kdata->output);
    OPENSSL_free(kdata->algname);
    OPENSSL_free(kdata->keyname);
    OPENSSL_free(kdata->encoded_pub_key);
    OPENSSL_free(kdata->encoded_priv_key);
    OPENSSL_free(kdata->entropy);
    OPENSSL_free(kdata->ciphertext);
    EVP_PKEY_CTX_free(kdata->ctx);
}

static int kem_test_parse(EVP_TEST *t, const char *keyword, const char *value)
{
    KEM_DATA *kdata = t->data;

    if (strcmp(keyword, "Op") == 0) {
        kdata->op = value;
        return 1;
    }
    if (strcmp(keyword, "CtrlInit") == 0)
        return ctrladd(kdata->init_ctrls, value);
    if (strcmp(keyword, "Input") == 0)
        return parse_bin(value, &kdata->input, &kdata->inputlen);
    if (strcmp(keyword, "Output") == 0)
        return parse_bin(value, &kdata->output, &kdata->outputlen);
    if (strcmp(keyword, "EncodedPublicKey") == 0)
        return parse_bin(value, &kdata->encoded_pub_key,
                         &kdata->encoded_pub_key_len);
    if (strcmp(keyword, "EncodedPrivateKey") == 0)
        return parse_bin(value, &kdata->encoded_priv_key,
                         &kdata->encoded_priv_key_len);
    if (strcmp(keyword, "Entropy") == 0)
        return parse_bin(value, &kdata->entropy, &kdata->entropylen);
    if (strcmp(keyword, "Ciphertext") == 0)
        return parse_bin(value, &kdata->ciphertext, &kdata->ciphertext_len);
    if (strcmp(keyword, "KeyName") == 0)
        return TEST_ptr(kdata->keyname = OPENSSL_strdup(value));
    return 1;
}

static int encapsulate(EVP_TEST *t, EVP_PKEY_CTX *ctx, const char *op,
                       unsigned char **outwrapped, size_t *outwrappedlen,
                       unsigned char **outsecret, size_t *outsecretlen)
{
    int ret = 0;
    KEM_DATA *kdata = t->data;
    unsigned char *wrapped = NULL, *secret = NULL;
    size_t wrappedlen = 0, secretlen = 0;
    OSSL_PARAM params[10];
    size_t params_n = 0;
    /* Reserve space for the terminator and possibly IKME */
    const size_t params_max = OSSL_NELEM(params) - 1 - (kdata->entropy != NULL);

    if (sk_OPENSSL_STRING_num(kdata->init_ctrls) > 0)
        if (!ctrl2params(t, kdata->init_ctrls, NULL, params, params_max,
                         &params_n))
            goto err;

    /* We don't expect very many controls here */
    if (!TEST_size_t_lt(params_n, params_max))
        goto err;

    if (kdata->entropy != NULL)
        /* Input key material a.k.a entropy */
        params[params_n++] =
            OSSL_PARAM_construct_octet_string(OSSL_KEM_PARAM_IKME,
                                              kdata->entropy,
                                              kdata->entropylen);
    params[params_n] = OSSL_PARAM_construct_end();

    if (EVP_PKEY_encapsulate_init(ctx, params) <= 0) {
        t->err = "TEST_ENCAPSULATE_INIT_ERROR";
        goto ok;
    }

    if (op != NULL && EVP_PKEY_CTX_set_kem_op(ctx, op) <= 0) {
        t->err = "TEST_SET_KEM_OP_ERROR";
        goto ok;
    }
    if (EVP_PKEY_encapsulate(ctx, NULL, &wrappedlen, NULL, &secretlen) <= 0) {
        t->err = "TEST_ENCAPSULATE_LEN_ERROR";
        goto ok;
    }
    wrapped = OPENSSL_malloc(wrappedlen);
    secret = OPENSSL_malloc(secretlen);
    if (!TEST_ptr(wrapped) || !TEST_ptr(secret)) {
        ret = 0;
        goto err;
    }
    if (EVP_PKEY_encapsulate(ctx, wrapped, &wrappedlen, secret, &secretlen) <= 0) {
        t->err = "TEST_ENCAPSULATE_ERROR";
        goto ok;
    }
    ret = pkey_check_fips_approved(ctx, t);

    if (kdata->ciphertext != NULL
        && !TEST_mem_eq(wrapped, wrappedlen, kdata->ciphertext, kdata->ciphertext_len)) {
        ret = 0;
        goto err;
    }

    if (kdata->output != NULL
        && !TEST_mem_eq(secret, secretlen, kdata->output, kdata->outputlen)) {
        ret = 0;
        goto err;
    }

    if (ret == 0)
        goto err;

    t->err = NULL;
    *outwrapped = wrapped;
    *outsecret = secret;
    *outwrappedlen = wrappedlen;
    *outsecretlen = secretlen;
ok:
    ret = 1;
err:
    if (ret == 0) {
        OPENSSL_free(wrapped);
        OPENSSL_free(secret);
    }
    if (sk_OPENSSL_STRING_num(kdata->init_ctrls) > 0)
        ctrl2params_free(params, params_n, 0);
    return ret;
}

static int decapsulate(EVP_TEST *t, EVP_PKEY_CTX *ctx, const char *op,
                       const unsigned char *in, size_t inlen,
                       const unsigned char *expected, size_t expectedlen)
{
    int ret = 0;
    KEM_DATA *kdata = t->data;
    size_t outlen = 0;
    unsigned char *out = NULL;
    OSSL_PARAM params[2] = { OSSL_PARAM_END, OSSL_PARAM_END };
    OSSL_PARAM *p = NULL;
    size_t params_n = 0, params_n_allocated = 0;

    if (sk_OPENSSL_STRING_num(kdata->init_ctrls) > 0) {
        if (!ctrl2params(t, kdata->init_ctrls, NULL,
                         params, 2, &params_n))
            goto err;
        p = params;
    }

    if (EVP_PKEY_decapsulate_init(ctx, p) <= 0) {
        t->err = "TEST_DECAPSULATE_INIT_ERROR";
        goto ok;
    }

    if (op != NULL && EVP_PKEY_CTX_set_kem_op(ctx, op) <= 0) {
        t->err = "TEST_SET_KEM_OP_ERROR";
        goto ok;
    }
    if (EVP_PKEY_decapsulate(ctx, NULL, &outlen, in, inlen) <= 0) {
        t->err = "TEST_DECAPSULATE_LEN_ERROR";
        goto ok;
    }
    if (!TEST_ptr(out = OPENSSL_malloc(outlen))) {
        ret = 0;
        goto err;
    }

    if (EVP_PKEY_decapsulate(ctx, out, &outlen, in, inlen) <= 0) {
        t->err = "TEST_DECAPSULATE_ERROR";
        goto err;
    }
    if (!TEST_mem_eq(out, outlen, expected, expectedlen)) {
        t->err = "TEST_SECRET_MISMATCH";
        goto ok;
    }
    t->err = NULL;
ok:
    ret = 1;
err:
    OPENSSL_free(out);
    if (sk_OPENSSL_STRING_num(kdata->init_ctrls) > 0)
        ctrl2params_free(params, params_n, params_n_allocated);
    return ret;
}

static int kem_test_run(EVP_TEST *t)
{
    int ret = 0, found_key = 0;
    EVP_PKEY *pkey = NULL;
    KEM_DATA *kdata = t->data;
    unsigned char *wrapped = NULL, *secret = NULL;

    /* Generate either public or private key based on given params */
    if (kdata->keyname != NULL) {
        /* Previously generated private key */
        found_key = find_key(&pkey, kdata->keyname, private_keys);
        if (found_key == 0 || pkey == NULL) {
            TEST_info("skipping, key '%s' is disabled", kdata->keyname);
            t->skip = 1;
            goto ok;
        }
    } else if (kdata->encoded_pub_key != NULL) {
        /* Encoded public key */
        if ((pkey = EVP_PKEY_new_raw_public_key_ex(libctx, kdata->algname,
                                                   propquery,
                                                   kdata->encoded_pub_key,
                                                   kdata->encoded_pub_key_len)) == NULL) {
            t->err = "TEST_PARSE_PUBLIC_KEY_ERROR";
            goto ok;
        }
    } else if (kdata->encoded_priv_key != NULL) {
        /* Encoded private key */
        if ((pkey = EVP_PKEY_new_raw_private_key_ex(libctx, kdata->algname,
                                                    propquery,
                                                    kdata->encoded_priv_key,
                                                    kdata->encoded_priv_key_len)) == NULL) {
            t->err = "TEST_PARSE_PRIVATE_KEY_ERROR";
            goto ok;
        }
    } else {
        TEST_info("Missing parameters to create key");
        goto err;
    }

    if (!TEST_ptr(kdata->ctx = EVP_PKEY_CTX_new_from_pkey(libctx, pkey, propquery)))
        goto err;

    if (kdata->input == NULL) {
        size_t wrappedlen = 0, secretlen = 0;

        ret = encapsulate(t, kdata->ctx, kdata->op, &wrapped, &wrappedlen,
                          &secret, &secretlen);
        if (ret == 0 || t->err != NULL)
            goto err;

        /* Also attempt to decrypt if we have the private key */
        if (found_key || kdata->encoded_priv_key != NULL)
            ret = decapsulate(t, kdata->ctx, kdata->op, wrapped, wrappedlen,
                              secret, secretlen);
    } else {
        ret = decapsulate(t, kdata->ctx, kdata->op, kdata->input, kdata->inputlen,
                          kdata->output, kdata->outputlen);
    }

ok:
    ret = 1;
err:
    if (!found_key)
        EVP_PKEY_free(pkey);
    OPENSSL_free(wrapped);
    OPENSSL_free(secret);
    return ret;
}

static const EVP_TEST_METHOD pkey_kem_test_method = {
    "Kem",
    kem_test_init,
    kem_test_cleanup,
    kem_test_parse,
    kem_test_run
};

/**
 **  PUBLIC KEY TESTS
 **  These are all very similar and share much common code.
 **/

typedef struct pkey_data_st {
    /* Context for this operation */
    EVP_PKEY_CTX *ctx;
    /* Signature algo for such operations */
    EVP_SIGNATURE *sigalgo;
    /* Key operation to perform */
    int (*keyopinit) (EVP_PKEY_CTX *ctx, const OSSL_PARAM params[]);
    int (*keyopinit_ex2) (EVP_PKEY_CTX *ctx, EVP_SIGNATURE *algo,
                          const OSSL_PARAM params[]);
    int (*keyop) (EVP_PKEY_CTX *ctx,
                  unsigned char *sig, size_t *siglen,
                  const unsigned char *tbs, size_t tbslen);
    /* Input to MAC */
    unsigned char *input;
    size_t input_len;
    /* Expected output */
    unsigned char *output;
    size_t output_len;
    STACK_OF(OPENSSL_STRING) *init_controls; /* collection of controls */
    STACK_OF(OPENSSL_STRING) *controls;      /* collection of controls */
    EVP_PKEY *peer;
    int validate;
} PKEY_DATA;

/*
 * Perform public key operation setup: lookup key, allocated ctx and call
 * the appropriate initialisation function
 */
static int pkey_test_init_keyctx(EVP_TEST *t, const char *keyname,
                                 int use_public)
{
    PKEY_DATA *kdata;
    EVP_PKEY *pkey = NULL;
    int rv = 0;

    if (use_public)
        rv = find_key(&pkey, keyname, public_keys);
    if (rv == 0)
        rv = find_key(&pkey, keyname, private_keys);
    if (rv == 0 || pkey == NULL) {
        TEST_info("skipping, key '%s' is disabled", keyname);
        t->skip = 1;
        return 1;
    }

    if (!TEST_ptr(kdata = OPENSSL_zalloc(sizeof(*kdata)))) {
        EVP_PKEY_free(pkey);
        return 0;
    }
    if (!TEST_ptr(kdata->ctx = EVP_PKEY_CTX_new_from_pkey(libctx, pkey, propquery))) {
        EVP_PKEY_free(pkey);
        OPENSSL_free(kdata);
        return 0;
    }
    t->data = kdata;
    return 1;
}

static int pkey_test_init(EVP_TEST *t, const char *name,
                          int use_public,
                          int (*keyopinit) (EVP_PKEY_CTX *ctx,
                                            const OSSL_PARAM params[]),
                          int (*keyop)(EVP_PKEY_CTX *ctx,
                                       unsigned char *sig, size_t *siglen,
                                       const unsigned char *tbs,
                                       size_t tbslen))
{
    PKEY_DATA *kdata = NULL;
    int rv = 0;

    rv = pkey_test_init_keyctx(t, name, use_public);
    if (t->skip || !rv)
        return rv;
    kdata = t->data;
    kdata->keyopinit = keyopinit;
    kdata->keyop = keyop;
    kdata->init_controls = sk_OPENSSL_STRING_new_null();
    kdata->controls = sk_OPENSSL_STRING_new_null();
    return 1;
}

static int pkey_test_init_ex2(EVP_TEST *t, const char *name,
                              int use_public,
                              int (*keyopinit)(EVP_PKEY_CTX *ctx,
                                               EVP_SIGNATURE *algo,
                                               const OSSL_PARAM param[]),
                              int (*keyop)(EVP_PKEY_CTX *ctx,
                                           unsigned char *sig, size_t *siglen,
                                           const unsigned char *tbs,
                                           size_t tbslen))
{
    PKEY_DATA *kdata = NULL;
    int rv = 0;
    char algoname[OSSL_MAX_NAME_SIZE + 1];
    const char *p;

    if ((p = strchr(name, ':')) == NULL
        || p == name || p[1] == '\0' || p - name > OSSL_MAX_NAME_SIZE) {
        TEST_info("Can't extract algorithm or key name from '%s'", name);
        return 0;
    }
    memcpy(algoname, name, p - name);
    algoname[p - name] = '\0';

    if (is_pkey_disabled(algoname)) {
        t->skip = 1;
        return 1;
    }

    rv = pkey_test_init_keyctx(t, /* keyname */ p + 1, use_public);
    if (t->skip || !rv)
        return rv;
    kdata = t->data;
    kdata->keyopinit_ex2 = keyopinit;
    kdata->keyop = keyop;
    if (!TEST_ptr(kdata->sigalgo
                  = EVP_SIGNATURE_fetch(libctx, algoname, propquery))) {
        TEST_info("algoname = '%s'", algoname);
        return 0;
    }
    kdata->init_controls = sk_OPENSSL_STRING_new_null();
    kdata->controls = sk_OPENSSL_STRING_new_null();
    return 1;
}

static void pkey_test_cleanup(EVP_TEST *t)
{
    PKEY_DATA *kdata = t->data;

    ctrlfree(kdata->init_controls);
    ctrlfree(kdata->controls);
    OPENSSL_free(kdata->input);
    OPENSSL_free(kdata->output);
    EVP_PKEY_CTX_free(kdata->ctx);
    EVP_SIGNATURE_free(kdata->sigalgo);
}

static int pkey_test_ctrl(EVP_TEST *t, EVP_PKEY_CTX *pctx,
                          const char *value)
{
    int rv = 0;
    char *p, *tmpval;

    if (!TEST_ptr(tmpval = OPENSSL_strdup(value)))
        return 0;
    p = strchr(tmpval, ':');
    if (p != NULL) {
        *p++ = '\0';
        rv = EVP_PKEY_CTX_ctrl_str(pctx, tmpval, p);
    }
    if (rv == -2) {
        t->err = "PKEY_CTRL_INVALID";
        rv = 1;
    } else if (p != NULL && rv <= 0) {
        if (is_digest_disabled(p) || is_cipher_disabled(p)) {
            TEST_info("skipping, '%s' is disabled", p);
            t->skip = 1;
            rv = 1;
        } else {
            t->err = "PKEY_CTRL_ERROR";
            rv = 1;
        }
    }
    OPENSSL_free(tmpval);
    return rv > 0;
}

static int pkey_add_control(EVP_TEST *t, STACK_OF(OPENSSL_STRING) *controls,
                            const char *value)
{
    char *p;

    if (controls == NULL)
        return 0;

    p = strchr(value, ':');
    if (p == NULL)
        return 0;
    p++;
    if (is_digest_disabled(p) || is_cipher_disabled(p)) {
        TEST_info("skipping, '%s' is disabled", p);
        t->skip = 1;
        return 1;
    }

    return ctrladd(controls, value) > 0;
}

static int pkey_test_parse(EVP_TEST *t,
                           const char *keyword, const char *value)
{
    PKEY_DATA *kdata = t->data;
    if (strcmp(keyword, "Input") == 0)
        return parse_bin(value, &kdata->input, &kdata->input_len);
    if (strcmp(keyword, "Output") == 0)
        return parse_bin(value, &kdata->output, &kdata->output_len);
    if (strcmp(keyword, "CtrlInit") == 0)
        return ctrladd(kdata->init_controls, value);
    if (strcmp(keyword, "Ctrl") == 0)
        return pkey_add_control(t, kdata->controls, value);
    return 0;
}

static int pkey_test_run_init(EVP_TEST *t)
{
    PKEY_DATA *data = t->data;
    int i, ret = 0;
    OSSL_PARAM params[5] = {
        OSSL_PARAM_END, OSSL_PARAM_END, OSSL_PARAM_END,
        OSSL_PARAM_END, OSSL_PARAM_END
    };
    OSSL_PARAM *p = NULL;
    size_t params_n = 0, params_n_allocstart = 0;

    if (sk_OPENSSL_STRING_num(data->init_controls) > 0) {
        if (!ctrl2params(t, data->init_controls,
                         NULL,
                         params, OSSL_NELEM(params), &params_n))
            goto err;
        p = params;
    }
    if (data->keyopinit != NULL) {
        if (data->keyopinit(data->ctx, p) <= 0) {
            t->err = "KEYOP_INIT_ERROR";
            goto err;
        }
    } else if (data->keyopinit_ex2 != NULL) {
        if (data->keyopinit_ex2(data->ctx, data->sigalgo, p) <= 0) {
            t->err = "KEYOP_INIT_ERROR";
            goto err;
        }
    } else {
        t->err = "KEYOP_INIT_ERROR";
        goto err;
    }

    for (i = 0; i < sk_OPENSSL_STRING_num(data->controls); i++) {
        char *value = sk_OPENSSL_STRING_value(data->controls, i);

        if (!pkey_test_ctrl(t, data->ctx, value) || t->err != NULL)
            goto err;
    }
    ret = 1;
err:
    ctrl2params_free(params, params_n, params_n_allocstart);
    return ret;
}

static int pkey_test_run(EVP_TEST *t)
{
    PKEY_DATA *expected = t->data;
    unsigned char *got = NULL;
    size_t got_len;
    EVP_PKEY_CTX *copy = NULL;

    if (!pkey_test_run_init(t))
        goto err;

    /* Make a copy of the EVP_PKEY context, for repeat use further down */
    if (!TEST_ptr(copy = EVP_PKEY_CTX_dup(expected->ctx))) {
        t->err = "INTERNAL_ERROR";
        goto err;
    }

    if (expected->keyop(expected->ctx, NULL, &got_len,
                        expected->input, expected->input_len) <= 0
            || !TEST_ptr(got = OPENSSL_malloc(got_len))) {
        t->err = "KEYOP_LENGTH_ERROR";
        goto err;
    }
    if (expected->keyop(expected->ctx, got, &got_len,
                        expected->input, expected->input_len) <= 0) {
        t->err = "KEYOP_ERROR";
        goto err;
    }

    if (!memory_err_compare(t, "KEYOP_MISMATCH",
                            expected->output, expected->output_len,
                            got, got_len))
        goto err;

    t->err = NULL;
    OPENSSL_free(got);
    got = NULL;

    /* Repeat the test on the EVP_PKEY context copy. */
    if (expected->keyop(copy, NULL, &got_len, expected->input,
                        expected->input_len) <= 0
            || !TEST_ptr(got = OPENSSL_malloc(got_len))) {
        t->err = "KEYOP_LENGTH_ERROR";
        goto err;
    }
    if (expected->keyop(copy, got, &got_len, expected->input,
                        expected->input_len) <= 0) {
        t->err = "KEYOP_ERROR";
        goto err;
    }
    if (!memory_err_compare(t, "KEYOP_MISMATCH",
                            expected->output, expected->output_len,
                            got, got_len))
        goto err;

    if (pkey_check_fips_approved(expected->ctx, t) <= 0)
        goto err;

 err:
    OPENSSL_free(got);
    EVP_PKEY_CTX_free(copy);
    return 1;
}

static int pkey_fromdata_test_init(EVP_TEST *t, const char *name)
{
    PKEY_DATA *kdata = NULL;

    if (is_pkey_disabled(name)) {
        TEST_info("skipping, '%s' is disabled", name);
        t->skip = 1;
        return 1;
    }

    if (!TEST_ptr(kdata = OPENSSL_zalloc(sizeof(*kdata))))
        return 0;
    kdata->ctx = EVP_PKEY_CTX_new_from_name(libctx, name, "");
    if (kdata->ctx == NULL)
        goto err;
    if (EVP_PKEY_fromdata_init(kdata->ctx) <= 0)
        goto err;
    kdata->controls = sk_OPENSSL_STRING_new_null();
    if (kdata->controls == NULL)
        goto err;
    t->data = kdata;
    return 1;
 err:
    EVP_PKEY_CTX_free(kdata->ctx);
    OPENSSL_free(kdata);
    return 0;
}

static void pkey_fromdata_test_cleanup(EVP_TEST *t)
{
    PKEY_DATA *kdata = t->data;

    ctrlfree(kdata->controls);
    EVP_PKEY_CTX_free(kdata->ctx);
}

static int pkey_fromdata_test_parse(EVP_TEST *t,
                                    const char *keyword, const char *value)
{
    PKEY_DATA *kdata = t->data;

    if (strcmp(keyword, "Ctrl") == 0)
        return pkey_add_control(t, kdata->controls, value);
    return 0;
}

static int pkey_fromdata_test_run(EVP_TEST *t)
{
    EVP_PKEY *key = NULL;
    PKEY_DATA *kdata = t->data;
    int ret = 0;
    static const OSSL_PARAM key_settable_ctx_params[] = {
        OSSL_PARAM_octet_string("priv", NULL, 0),
        OSSL_PARAM_octet_string("pub", NULL, 0),
        OSSL_PARAM_END
    };
    OSSL_PARAM params[5] = {
        OSSL_PARAM_END, OSSL_PARAM_END, OSSL_PARAM_END,
        OSSL_PARAM_END, OSSL_PARAM_END
    };
    OSSL_PARAM *p = NULL;
    size_t params_n = 0, params_n_allocstart = 0;

    if (sk_OPENSSL_STRING_num(kdata->controls) > 0) {
        if (!ctrl2params(t, kdata->controls, key_settable_ctx_params,
                         params, OSSL_NELEM(params), &params_n))
            goto err;
        p = params;
    }

    ret = 1;
    if (EVP_PKEY_fromdata(kdata->ctx, &key, EVP_PKEY_KEYPAIR, p) <= 0) {
        t->err = "KEY_FROMDATA_ERROR";
        goto err;
    }
err:
    ctrl2params_free(params, params_n, params_n_allocstart);
    EVP_PKEY_free(key);
    return ret;
}

static const EVP_TEST_METHOD pkey_fromdata_test_method = {
    "KeyFromData",
    pkey_fromdata_test_init,
    pkey_fromdata_test_cleanup,
    pkey_fromdata_test_parse,
    pkey_fromdata_test_run
};

/*
 * "Sign" implies EVP_PKEY_sign_init_ex2() if the argument is a colon-separated
 * pair, {algorithm}:{key}.  If not, it implies EVP_PKEY_sign_init_ex()
 */
static int sign_test_init(EVP_TEST *t, const char *name)
{
    if (strchr(name, ':') != NULL)
        return pkey_test_init_ex2(t, name, 0,
                                  EVP_PKEY_sign_init_ex2, EVP_PKEY_sign);
    return pkey_test_init(t, name, 0, EVP_PKEY_sign_init_ex, EVP_PKEY_sign);
}

static const EVP_TEST_METHOD psign_test_method = {
    "Sign",
    sign_test_init,
    pkey_test_cleanup,
    pkey_test_parse,
    pkey_test_run
};

/*
 * "Sign-Message" is like "Sign", but uses EVP_PKEY_sign_message_init()
 * The argument must be a colon separated pair, {algorithm}:{key}
 */
static int sign_test_message_init(EVP_TEST *t, const char *name)
{
    return pkey_test_init_ex2(t, name, 0,
                              EVP_PKEY_sign_message_init, EVP_PKEY_sign);
}

static const EVP_TEST_METHOD psign_message_test_method = {
    "Sign-Message",
    sign_test_message_init,
    pkey_test_cleanup,
    pkey_test_parse,
    pkey_test_run
};

/*
 * "VerifyRecover" implies EVP_PKEY_verify_recover_init_ex2() if the argument is a
 * colon-separated pair, {algorithm}:{key}.
 * If not, it implies EVP_PKEY_verify_recover_init_ex()
 */
static int verify_recover_test_init(EVP_TEST *t, const char *name)
{
    if (strchr(name, ':') != NULL)
        return pkey_test_init_ex2(t, name, 1,
                                  EVP_PKEY_verify_recover_init_ex2,
                                  EVP_PKEY_verify_recover);
    return pkey_test_init(t, name, 1, EVP_PKEY_verify_recover_init_ex,
                          EVP_PKEY_verify_recover);
}

static const EVP_TEST_METHOD pverify_recover_test_method = {
    "VerifyRecover",
    verify_recover_test_init,
    pkey_test_cleanup,
    pkey_test_parse,
    pkey_test_run
};

static int decrypt_test_init(EVP_TEST *t, const char *name)
{
    return pkey_test_init(t, name, 0, EVP_PKEY_decrypt_init_ex,
                          EVP_PKEY_decrypt);
}

static const EVP_TEST_METHOD pdecrypt_test_method = {
    "Decrypt",
    decrypt_test_init,
    pkey_test_cleanup,
    pkey_test_parse,
    pkey_test_run
};

/*
 * "Verify" implies EVP_PKEY_verify_init_ex2() if the argument is a
 * colon-separated pair, {algorithm}:{key}.
 * If not, it implies EVP_PKEY_verify_init_ex()
 */
static int verify_test_init(EVP_TEST *t, const char *name)
{
    if (strchr(name, ':') != NULL)
        return pkey_test_init_ex2(t, name, 1,
                                  EVP_PKEY_verify_init_ex2, NULL);
    return pkey_test_init(t, name, 1, EVP_PKEY_verify_init_ex, NULL);
}

static int verify_test_run(EVP_TEST *t)
{
    int ret = 1;
    PKEY_DATA *kdata = t->data;

    if (!pkey_test_run_init(t))
        goto err;
    if (EVP_PKEY_verify(kdata->ctx, kdata->output, kdata->output_len,
                        kdata->input, kdata->input_len) <= 0) {
        t->err = "VERIFY_ERROR";
        goto err;
    }
    if (!pkey_check_fips_approved(kdata->ctx, t))
        ret = 0;
err:
    return ret;
}

static const EVP_TEST_METHOD pverify_test_method = {
    "Verify",
    verify_test_init,
    pkey_test_cleanup,
    pkey_test_parse,
    verify_test_run
};

/*
 * "Verify-Message" is like "Verify", but uses EVP_PKEY_verify_message_init()
 * The argument must be a colon separated pair, {algorithm}:{key}
 */
static int verify_message_test_init(EVP_TEST *t, const char *name)
{
    return pkey_test_init_ex2(t, name, 0,
                              EVP_PKEY_verify_message_init, NULL);
}

static const EVP_TEST_METHOD pverify_message_test_method = {
    "Verify-Message",
    verify_message_test_init,
    pkey_test_cleanup,
    pkey_test_parse,
    verify_test_run
};

/*
 * "Verify-Message-Public" is like "Verify-Message", but uses a public key
 * instead of a private key.
 * The argument must be a colon separated pair, {algorithm}:{key}
 */
static int verify_message_public_test_init(EVP_TEST *t, const char *name)
{
    return pkey_test_init_ex2(t, name, 1,
                              EVP_PKEY_verify_message_init, NULL);
}

static const EVP_TEST_METHOD pverify_message_public_test_method = {
    "Verify-Message-Public",
    verify_message_public_test_init,
    pkey_test_cleanup,
    pkey_test_parse,
    verify_test_run
};

static int pderive_test_init(EVP_TEST *t, const char *name)
{
    return pkey_test_init(t, name, 0, EVP_PKEY_derive_init_ex, 0);
}

static int pderive_test_parse(EVP_TEST *t,
                              const char *keyword, const char *value)
{
    PKEY_DATA *kdata = t->data;
    int validate = 0;

    if (strcmp(keyword, "PeerKeyValidate") == 0)
        validate = 1;

    if (validate || strcmp(keyword, "PeerKey") == 0) {
        EVP_PKEY *peer = NULL;

        kdata->validate = validate;
        if (find_key(&peer, value, public_keys) == 0)
            return -1;
        kdata->peer = peer;
        return 1;
    }
    if (strcmp(keyword, "SharedSecret") == 0)
        return parse_bin(value, &kdata->output, &kdata->output_len);
    if (strcmp(keyword, "Ctrl") == 0)
        return pkey_add_control(t, kdata->controls, value);
    if (strcmp(keyword, "CtrlInit") == 0)
        return ctrladd(kdata->init_controls, value);
    return 0;
}

static int pderive_test_run(EVP_TEST *t)
{
    EVP_PKEY_CTX *dctx = NULL;
    PKEY_DATA *expected = t->data;
    unsigned char *got = NULL;
    size_t got_len;
    int ret = 1;

    if (!pkey_test_run_init(t))
        goto err;

    t->err = NULL;
    if (EVP_PKEY_derive_set_peer_ex(expected->ctx, expected->peer,
                                    expected->validate) <= 0) {
        t->err = "DERIVE_SET_PEER_ERROR";
        goto err;
    }

    if (!TEST_ptr(dctx = EVP_PKEY_CTX_dup(expected->ctx))) {
        t->err = "DERIVE_ERROR";
        goto err;
    }

    if (EVP_PKEY_derive(dctx, NULL, &got_len) <= 0
        || !TEST_size_t_ne(got_len, 0)) {
        t->err = "DERIVE_ERROR";
        goto err;
    }
    if (!TEST_ptr(got = OPENSSL_malloc(got_len))) {
        t->err = "DERIVE_ERROR";
        goto err;
    }
    if (EVP_PKEY_derive(dctx, got, &got_len) <= 0) {
        t->err = "DERIVE_ERROR";
        goto err;
    }
    if (!memory_err_compare(t, "SHARED_SECRET_MISMATCH",
                            expected->output, expected->output_len,
                            got, got_len))
        goto err;

    if (!pkey_check_fips_approved(dctx, t)) {
        ret = 0;
        goto err;
    }
    t->err = NULL;
 err:
    OPENSSL_free(got);
    EVP_PKEY_CTX_free(dctx);
    return ret;
}

static const EVP_TEST_METHOD pderive_test_method = {
    "Derive",
    pderive_test_init,
    pkey_test_cleanup,
    pderive_test_parse,
    pderive_test_run
};

/**
 **  PBE TESTS
 **/

typedef enum pbe_type_enum {
    PBE_TYPE_INVALID = 0,
    PBE_TYPE_SCRYPT, PBE_TYPE_PBKDF2, PBE_TYPE_PKCS12
} PBE_TYPE;

typedef struct pbe_data_st {
    PBE_TYPE pbe_type;
        /* scrypt parameters */
    uint64_t N, r, p, maxmem;
        /* PKCS#12 parameters */
    int id, iter;
    const EVP_MD *md;
        /* password */
    unsigned char *pass;
    size_t pass_len;
        /* salt */
    unsigned char *salt;
    size_t salt_len;
        /* Expected output */
    unsigned char *key;
    size_t key_len;
} PBE_DATA;

#ifndef OPENSSL_NO_SCRYPT
/* Parse unsigned decimal 64 bit integer value */
static int parse_uint64(const char *value, uint64_t *pr)
{
    const char *p = value;

    if (!TEST_true(*p)) {
        TEST_info("Invalid empty integer value");
        return -1;
    }
    for (*pr = 0; *p; ) {
        if (*pr > UINT64_MAX / 10) {
            TEST_error("Integer overflow in string %s", value);
            return -1;
        }
        *pr *= 10;
        if (!TEST_true(isdigit((unsigned char)*p))) {
            TEST_error("Invalid character in string %s", value);
            return -1;
        }
        *pr += *p - '0';
        p++;
    }
    return 1;
}

static int scrypt_test_parse(EVP_TEST *t,
                             const char *keyword, const char *value)
{
    PBE_DATA *pdata = t->data;

    if (strcmp(keyword, "N") == 0)
        return parse_uint64(value, &pdata->N);
    if (strcmp(keyword, "p") == 0)
        return parse_uint64(value, &pdata->p);
    if (strcmp(keyword, "r") == 0)
        return parse_uint64(value, &pdata->r);
    if (strcmp(keyword, "maxmem") == 0)
        return parse_uint64(value, &pdata->maxmem);
    return 0;
}
#endif

static int pbkdf2_test_parse(EVP_TEST *t,
                             const char *keyword, const char *value)
{
    PBE_DATA *pdata = t->data;

    if (strcmp(keyword, "iter") == 0) {
        pdata->iter = atoi(value);
        if (pdata->iter <= 0)
            return -1;
        return 1;
    }
    if (strcmp(keyword, "MD") == 0) {
        pdata->md = EVP_get_digestbyname(value);
        if (pdata->md == NULL)
            return -1;
        return 1;
    }
    return 0;
}

static int pkcs12_test_parse(EVP_TEST *t,
                             const char *keyword, const char *value)
{
    PBE_DATA *pdata = t->data;

    if (strcmp(keyword, "id") == 0) {
        pdata->id = atoi(value);
        if (pdata->id <= 0)
            return -1;
        return 1;
    }
    return pbkdf2_test_parse(t, keyword, value);
}

static int pbe_test_init(EVP_TEST *t, const char *alg)
{
    PBE_DATA *pdat;
    PBE_TYPE pbe_type = PBE_TYPE_INVALID;

    if (is_kdf_disabled(alg)) {
        TEST_info("skipping, '%s' is disabled", alg);
        t->skip = 1;
        return 1;
    }
    if (strcmp(alg, "scrypt") == 0) {
        pbe_type = PBE_TYPE_SCRYPT;
    } else if (strcmp(alg, "pbkdf2") == 0) {
        pbe_type = PBE_TYPE_PBKDF2;
    } else if (strcmp(alg, "pkcs12") == 0) {
        pbe_type = PBE_TYPE_PKCS12;
    } else {
        TEST_error("Unknown pbe algorithm %s", alg);
        return 0;
    }
    if (!TEST_ptr(pdat = OPENSSL_zalloc(sizeof(*pdat))))
        return 0;
    pdat->pbe_type = pbe_type;
    t->data = pdat;
    return 1;
}

static void pbe_test_cleanup(EVP_TEST *t)
{
    PBE_DATA *pdat = t->data;

    OPENSSL_free(pdat->pass);
    OPENSSL_free(pdat->salt);
    OPENSSL_free(pdat->key);
}

static int pbe_test_parse(EVP_TEST *t,
                          const char *keyword, const char *value)
{
    PBE_DATA *pdata = t->data;

    if (strcmp(keyword, "Password") == 0)
        return parse_bin(value, &pdata->pass, &pdata->pass_len);
    if (strcmp(keyword, "Salt") == 0)
        return parse_bin(value, &pdata->salt, &pdata->salt_len);
    if (strcmp(keyword, "Key") == 0)
        return parse_bin(value, &pdata->key, &pdata->key_len);
    if (pdata->pbe_type == PBE_TYPE_PBKDF2)
        return pbkdf2_test_parse(t, keyword, value);
    else if (pdata->pbe_type == PBE_TYPE_PKCS12)
        return pkcs12_test_parse(t, keyword, value);
#ifndef OPENSSL_NO_SCRYPT
    else if (pdata->pbe_type == PBE_TYPE_SCRYPT)
        return scrypt_test_parse(t, keyword, value);
#endif
    return 0;
}

static int pbe_test_run(EVP_TEST *t)
{
    PBE_DATA *expected = t->data;
    unsigned char *key;
    EVP_MD *fetched_digest = NULL;
    OSSL_LIB_CTX *save_libctx;

    save_libctx = OSSL_LIB_CTX_set0_default(libctx);

    if (!TEST_ptr(key = OPENSSL_malloc(expected->key_len))) {
        t->err = "INTERNAL_ERROR";
        goto err;
    }
    if (expected->pbe_type == PBE_TYPE_PBKDF2) {
        if (PKCS5_PBKDF2_HMAC((char *)expected->pass, expected->pass_len,
                              expected->salt, expected->salt_len,
                              expected->iter, expected->md,
                              expected->key_len, key) == 0) {
            t->err = "PBKDF2_ERROR";
            goto err;
        }
#ifndef OPENSSL_NO_SCRYPT
    } else if (expected->pbe_type == PBE_TYPE_SCRYPT) {
        if (EVP_PBE_scrypt((const char *)expected->pass, expected->pass_len,
                            expected->salt, expected->salt_len,
                            expected->N, expected->r, expected->p,
                            expected->maxmem, key, expected->key_len) == 0) {
            t->err = "SCRYPT_ERROR";
            goto err;
        }
#endif
    } else if (expected->pbe_type == PBE_TYPE_PKCS12) {
        fetched_digest = EVP_MD_fetch(libctx, EVP_MD_get0_name(expected->md),
                                      propquery);
        if (fetched_digest == NULL) {
            t->err = "PKCS12_ERROR";
            goto err;
        }
        if (PKCS12_key_gen_uni(expected->pass, expected->pass_len,
                               expected->salt, expected->salt_len,
                               expected->id, expected->iter, expected->key_len,
                               key, fetched_digest) == 0) {
            t->err = "PKCS12_ERROR";
            goto err;
        }
    }
    if (!memory_err_compare(t, "KEY_MISMATCH", expected->key, expected->key_len,
                            key, expected->key_len))
        goto err;

    t->err = NULL;
err:
    EVP_MD_free(fetched_digest);
    OPENSSL_free(key);
    OSSL_LIB_CTX_set0_default(save_libctx);
    return 1;
}

static const EVP_TEST_METHOD pbe_test_method = {
    "PBE",
    pbe_test_init,
    pbe_test_cleanup,
    pbe_test_parse,
    pbe_test_run
};


/**
 **  BASE64 TESTS
 **/

typedef enum {
    BASE64_CANONICAL_ENCODING = 0,
    BASE64_VALID_ENCODING = 1,
    BASE64_INVALID_ENCODING = 2
} base64_encoding_type;

typedef struct encode_data_st {
    /* Input to encoding */
    unsigned char *input;
    size_t input_len;
    /* Expected output */
    unsigned char *output;
    size_t output_len;
    base64_encoding_type encoding;
} ENCODE_DATA;

static int encode_test_init(EVP_TEST *t, const char *encoding)
{
    ENCODE_DATA *edata;

    if (!TEST_ptr(edata = OPENSSL_zalloc(sizeof(*edata))))
        return 0;
    if (strcmp(encoding, "canonical") == 0) {
        edata->encoding = BASE64_CANONICAL_ENCODING;
    } else if (strcmp(encoding, "valid") == 0) {
        edata->encoding = BASE64_VALID_ENCODING;
    } else if (strcmp(encoding, "invalid") == 0) {
        edata->encoding = BASE64_INVALID_ENCODING;
        if (!TEST_ptr(t->expected_err = OPENSSL_strdup("DECODE_ERROR")))
            goto err;
    } else {
        TEST_error("Bad encoding: %s."
                   " Should be one of {canonical, valid, invalid}",
                   encoding);
        goto err;
    }
    t->data = edata;
    return 1;
err:
    OPENSSL_free(edata);
    return 0;
}

static void encode_test_cleanup(EVP_TEST *t)
{
    ENCODE_DATA *edata = t->data;

    OPENSSL_free(edata->input);
    OPENSSL_free(edata->output);
    memset(edata, 0, sizeof(*edata));
}

static int encode_test_parse(EVP_TEST *t,
                             const char *keyword, const char *value)
{
    ENCODE_DATA *edata = t->data;

    if (strcmp(keyword, "Input") == 0)
        return parse_bin(value, &edata->input, &edata->input_len);
    if (strcmp(keyword, "Output") == 0)
        return parse_bin(value, &edata->output, &edata->output_len);
    return 0;
}

static int encode_test_run(EVP_TEST *t)
{
    ENCODE_DATA *expected = t->data;
    unsigned char *encode_out = NULL, *decode_out = NULL;
    int output_len, chunk_len;
    EVP_ENCODE_CTX *decode_ctx = NULL, *encode_ctx = NULL;
    size_t input_len, donelen, decode_length;

    if (!TEST_ptr(decode_ctx = EVP_ENCODE_CTX_new())) {
        t->err = "INTERNAL_ERROR";
        goto err;
    }

    if (expected->encoding == BASE64_CANONICAL_ENCODING) {

        if (!TEST_ptr(encode_ctx = EVP_ENCODE_CTX_new())
                || !TEST_ptr(encode_out =
                        OPENSSL_malloc(EVP_ENCODE_LENGTH(expected->input_len))))
            goto err;

        EVP_EncodeInit(encode_ctx);

        input_len = expected->input_len;
        donelen = 0;
        output_len = 0;
        do {
            size_t current_len = (size_t) data_chunk_size;

            if (data_chunk_size == 0 || (size_t) data_chunk_size > input_len)
                current_len = input_len;
            if (!TEST_true(EVP_EncodeUpdate(encode_ctx, encode_out, &chunk_len,
                                            expected->input + donelen,
                                            current_len)))
                goto err;
            donelen += current_len;
            input_len -= current_len;
            output_len += chunk_len;
        } while (input_len > 0);

        EVP_EncodeFinal(encode_ctx, encode_out + output_len, &chunk_len);
        output_len += chunk_len;

        if (!memory_err_compare(t, "BAD_ENCODING",
                                expected->output, expected->output_len,
                                encode_out, output_len))
            goto err;
    }

    decode_length = EVP_DECODE_LENGTH(expected->output_len);
    if (!TEST_ptr(decode_out = OPENSSL_malloc(decode_length)))
        goto err;
    /*
     * Fill memory with non-zeros
     * to check that decoding does not place redundant zeros.
     */
    memset(decode_out, 0xff, decode_length);

    output_len = 0;
    EVP_DecodeInit(decode_ctx);

    input_len = expected->output_len;
    donelen = 0;
    do {
        size_t current_len = (size_t) data_chunk_size;

        if (data_chunk_size == 0 || (size_t) data_chunk_size > input_len)
            current_len = input_len;
        if (EVP_DecodeUpdate(decode_ctx, decode_out + output_len, &chunk_len,
                                expected->output + donelen, current_len) < 0) {
            t->err = "DECODE_ERROR";
            goto err;
        }
        donelen += current_len;
        input_len -= current_len;
        output_len += chunk_len;
    } while (input_len > 0);

    if (EVP_DecodeFinal(decode_ctx, decode_out + output_len, &chunk_len) != 1) {
        t->err = "DECODE_ERROR";
        goto err;
    }
    output_len += chunk_len;

    if (expected->encoding != BASE64_INVALID_ENCODING
            && !memory_err_compare(t, "BAD_DECODING",
                                   expected->input, expected->input_len,
                                   decode_out, output_len)) {
        t->err = "BAD_DECODING";
        goto err;
    }

    for (; output_len < (int)decode_length; output_len++) {
        if (decode_out[output_len] != 0xff) {
            t->err = "BAD_DECODING";
            goto err;
        }
    }

    t->err = NULL;
 err:
    OPENSSL_free(encode_out);
    OPENSSL_free(decode_out);
    EVP_ENCODE_CTX_free(decode_ctx);
    EVP_ENCODE_CTX_free(encode_ctx);
    return 1;
}

static const EVP_TEST_METHOD encode_test_method = {
    "Encoding",
    encode_test_init,
    encode_test_cleanup,
    encode_test_parse,
    encode_test_run,
};


/**
 **  RAND TESTS
 **/
#define MAX_RAND_REPEATS    15

typedef struct rand_data_pass_st {
    unsigned char *entropy;
    unsigned char *reseed_entropy;
    unsigned char *nonce;
    unsigned char *pers;
    unsigned char *reseed_addin;
    unsigned char *addinA;
    unsigned char *addinB;
    unsigned char *pr_entropyA;
    unsigned char *pr_entropyB;
    unsigned char *output;
    size_t entropy_len, nonce_len, pers_len, addinA_len, addinB_len,
           pr_entropyA_len, pr_entropyB_len, output_len, reseed_entropy_len,
           reseed_addin_len;
} RAND_DATA_PASS;

typedef struct rand_data_st {
    /* Context for this operation */
    EVP_RAND_CTX *ctx;
    EVP_RAND_CTX *parent;
    int n;
    int prediction_resistance;
    int use_df;
    unsigned int generate_bits;
    char *cipher;
    char *digest;
    STACK_OF(OPENSSL_STRING) *init_controls; /* collection of controls */

    /* Expected output */
    RAND_DATA_PASS data[MAX_RAND_REPEATS];
} RAND_DATA;

static int rand_test_init(EVP_TEST *t, const char *name)
{
    RAND_DATA *rdata;
    EVP_RAND *rand;
    OSSL_PARAM params[2] = { OSSL_PARAM_END, OSSL_PARAM_END };
    unsigned int strength = 256;

    if (!TEST_ptr(rdata = OPENSSL_zalloc(sizeof(*rdata))))
        return 0;

    /* TEST-RAND is available in the FIPS provider but not with "fips=yes" */
    rand = EVP_RAND_fetch(libctx, "TEST-RAND", "-fips");
    if (rand == NULL)
        goto err;
    rdata->parent = EVP_RAND_CTX_new(rand, NULL);
    EVP_RAND_free(rand);
    if (rdata->parent == NULL)
        goto err;

    *params = OSSL_PARAM_construct_uint(OSSL_RAND_PARAM_STRENGTH, &strength);
    if (!EVP_RAND_CTX_set_params(rdata->parent, params))
        goto err;

    rand = EVP_RAND_fetch(libctx, name, propquery);
    if (rand == NULL)
        goto err;
    rdata->ctx = EVP_RAND_CTX_new(rand, rdata->parent);
    EVP_RAND_free(rand);
    if (rdata->ctx == NULL)
        goto err;
    rdata->init_controls = sk_OPENSSL_STRING_new_null();

    rdata->n = -1;
    t->data = rdata;
    return 1;
 err:
    EVP_RAND_CTX_free(rdata->parent);
    OPENSSL_free(rdata);
    return 0;
}

static void rand_test_cleanup(EVP_TEST *t)
{
    RAND_DATA *rdata = t->data;
    int i;

    ctrlfree(rdata->init_controls);
    OPENSSL_free(rdata->cipher);
    OPENSSL_free(rdata->digest);

    for (i = 0; i <= rdata->n; i++) {
        OPENSSL_free(rdata->data[i].entropy);
        OPENSSL_free(rdata->data[i].reseed_entropy);
        OPENSSL_free(rdata->data[i].nonce);
        OPENSSL_free(rdata->data[i].pers);
        OPENSSL_free(rdata->data[i].reseed_addin);
        OPENSSL_free(rdata->data[i].addinA);
        OPENSSL_free(rdata->data[i].addinB);
        OPENSSL_free(rdata->data[i].pr_entropyA);
        OPENSSL_free(rdata->data[i].pr_entropyB);
        OPENSSL_free(rdata->data[i].output);
    }
    EVP_RAND_CTX_free(rdata->ctx);
    EVP_RAND_CTX_free(rdata->parent);
}

static int rand_test_parse(EVP_TEST *t,
                          const char *keyword, const char *value)
{
    RAND_DATA *rdata = t->data;
    RAND_DATA_PASS *item;
    const char *p;
    int n;

    if ((p = strchr(keyword, '.')) != NULL) {
        n = atoi(++p);
        if (n >= MAX_RAND_REPEATS)
            return 0;
        if (n > rdata->n)
            rdata->n = n;
        item = rdata->data + n;
        if (HAS_PREFIX(keyword, "Entropy."))
            return parse_bin(value, &item->entropy, &item->entropy_len);
        if (HAS_PREFIX(keyword, "ReseedEntropy."))
            return parse_bin(value, &item->reseed_entropy,
                             &item->reseed_entropy_len);
        if (HAS_PREFIX(keyword, "Nonce."))
            return parse_bin(value, &item->nonce, &item->nonce_len);
        if (HAS_PREFIX(keyword, "PersonalisationString."))
            return parse_bin(value, &item->pers, &item->pers_len);
        if (HAS_PREFIX(keyword, "ReseedAdditionalInput."))
            return parse_bin(value, &item->reseed_addin,
                             &item->reseed_addin_len);
        if (HAS_PREFIX(keyword, "AdditionalInputA."))
            return parse_bin(value, &item->addinA, &item->addinA_len);
        if (HAS_PREFIX(keyword, "AdditionalInputB."))
            return parse_bin(value, &item->addinB, &item->addinB_len);
        if (HAS_PREFIX(keyword, "EntropyPredictionResistanceA."))
            return parse_bin(value, &item->pr_entropyA, &item->pr_entropyA_len);
        if (HAS_PREFIX(keyword, "EntropyPredictionResistanceB."))
            return parse_bin(value, &item->pr_entropyB, &item->pr_entropyB_len);
        if (HAS_PREFIX(keyword, "Output."))
            return parse_bin(value, &item->output, &item->output_len);
    } else {
        if (strcmp(keyword, "Cipher") == 0)
            return TEST_ptr(rdata->cipher = OPENSSL_strdup(value));
        if (strcmp(keyword, "Digest") == 0)
            return TEST_ptr(rdata->digest = OPENSSL_strdup(value));
        if (strcmp(keyword, "DerivationFunction") == 0) {
            rdata->use_df = atoi(value) != 0;
            return 1;
        }
        if (strcmp(keyword, "GenerateBits") == 0) {
            if ((n = atoi(value)) <= 0 || n % 8 != 0)
                return 0;
            rdata->generate_bits = (unsigned int)n;
            return 1;
        }
        if (strcmp(keyword, "PredictionResistance") == 0) {
            rdata->prediction_resistance = atoi(value) != 0;
            return 1;
        }
        if (strcmp(keyword, "CtrlInit") == 0)
            return ctrladd(rdata->init_controls, value);
    }
    return 0;
}

static int rand_test_run(EVP_TEST *t)
{
    RAND_DATA *expected = t->data;
    RAND_DATA_PASS *item;
    unsigned char *got;
    size_t got_len = expected->generate_bits / 8;
    OSSL_PARAM params[8], *p = params;
    int i = -1, ret = 0;
    unsigned int strength;
    unsigned char *z;
    size_t params_n = 0, params_allocated_n = 0;

    if (!TEST_ptr(got = OPENSSL_malloc(got_len)))
        return 0;

    if (sk_OPENSSL_STRING_num(expected->init_controls) > 0) {
        if (!ctrl2params(t, expected->init_controls,
                         NULL,
                         params, OSSL_NELEM(params), &params_n))
            goto err;
    }
    p = params + params_n;

    *p++ = OSSL_PARAM_construct_int(OSSL_DRBG_PARAM_USE_DF, &expected->use_df);
    if (expected->cipher != NULL)
        *p++ = OSSL_PARAM_construct_utf8_string(OSSL_DRBG_PARAM_CIPHER,
                                                expected->cipher, 0);
    if (expected->digest != NULL)
        *p++ = OSSL_PARAM_construct_utf8_string(OSSL_DRBG_PARAM_DIGEST,
                                                expected->digest, 0);
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_DRBG_PARAM_MAC, "HMAC", 0);
    *p = OSSL_PARAM_construct_end();
    if (!EVP_RAND_CTX_set_params(expected->ctx, params)) {
        if (t->expect_unapproved == 0) {
            t->err = "EVP_RAND_CTX_set_params";
            ret = 1;
        }
        goto err;
    }
    ctrl2params_free(params, params_n, params_allocated_n);
    params_n = 0;

    strength = EVP_RAND_get_strength(expected->ctx);
    for (i = 0; i <= expected->n; i++) {
        item = expected->data + i;

        p = params;
        z = item->entropy != NULL ? item->entropy : (unsigned char *)"";
        *p++ = OSSL_PARAM_construct_octet_string(OSSL_RAND_PARAM_TEST_ENTROPY,
                                                 z, item->entropy_len);
        z = item->nonce != NULL ? item->nonce : (unsigned char *)"";
        *p++ = OSSL_PARAM_construct_octet_string(OSSL_RAND_PARAM_TEST_NONCE,
                                                 z, item->nonce_len);
        *p = OSSL_PARAM_construct_end();
        if (!TEST_true(EVP_RAND_instantiate(expected->parent, strength,
                                            0, NULL, 0, params)))
            goto err;

        z = item->pers != NULL ? item->pers : (unsigned char *)"";
        if (!TEST_true(EVP_RAND_instantiate
                           (expected->ctx, strength,
                            expected->prediction_resistance, z,
                            item->pers_len, NULL)))
            goto err;

        if (item->reseed_entropy != NULL) {
            params[0] = OSSL_PARAM_construct_octet_string
                           (OSSL_RAND_PARAM_TEST_ENTROPY, item->reseed_entropy,
                            item->reseed_entropy_len);
            params[1] = OSSL_PARAM_construct_end();
            if (!TEST_true(EVP_RAND_CTX_set_params(expected->parent, params)))
                goto err;

            if (!TEST_true(EVP_RAND_reseed
                               (expected->ctx, expected->prediction_resistance,
                                NULL, 0, item->reseed_addin,
                                item->reseed_addin_len)))
                goto err;
        }
        if (item->pr_entropyA != NULL) {
            params[0] = OSSL_PARAM_construct_octet_string
                           (OSSL_RAND_PARAM_TEST_ENTROPY, item->pr_entropyA,
                            item->pr_entropyA_len);
            params[1] = OSSL_PARAM_construct_end();
            if (!TEST_true(EVP_RAND_CTX_set_params(expected->parent, params)))
                goto err;
        }
        if (!TEST_true(EVP_RAND_generate
                           (expected->ctx, got, got_len,
                            strength, expected->prediction_resistance,
                            item->addinA, item->addinA_len)))
            goto err;

        if (item->pr_entropyB != NULL) {
            params[0] = OSSL_PARAM_construct_octet_string
                           (OSSL_RAND_PARAM_TEST_ENTROPY, item->pr_entropyB,
                            item->pr_entropyB_len);
            params[1] = OSSL_PARAM_construct_end();
            if (!TEST_true(EVP_RAND_CTX_set_params(expected->parent, params)))
                goto err;
        }
        if (!TEST_true(EVP_RAND_generate
                           (expected->ctx, got, got_len,
                            strength, expected->prediction_resistance,
                            item->addinB, item->addinB_len)))
            goto err;
        if (!TEST_mem_eq(got, got_len, item->output, item->output_len))
            goto err;
        if (!rand_check_fips_approved(expected->ctx, t))
            goto err;
        if (!TEST_true(EVP_RAND_uninstantiate(expected->ctx))
                || !TEST_true(EVP_RAND_uninstantiate(expected->parent))
                || !TEST_true(EVP_RAND_verify_zeroization(expected->ctx))
                || !TEST_int_eq(EVP_RAND_get_state(expected->ctx),
                                EVP_RAND_STATE_UNINITIALISED))
            goto err;
    }
    t->err = NULL;
    ret = 1;

 err:
    if (ret == 0 && i >= 0)
        TEST_info("Error in test case %d of %d\n", i, expected->n + 1);
    OPENSSL_free(got);
    ctrl2params_free(params, params_n, params_allocated_n);
    return ret;
}

static const EVP_TEST_METHOD rand_test_method = {
    "RAND",
    rand_test_init,
    rand_test_cleanup,
    rand_test_parse,
    rand_test_run
};

/**
 **  KDF TESTS
 **/
typedef struct kdf_data_st {
    /* Context for this operation */
    EVP_KDF_CTX *ctx;
    /* Expected output */
    unsigned char *output;
    size_t output_len;
    OSSL_PARAM params[20];
    OSSL_PARAM *p;
    STACK_OF(OPENSSL_STRING) *init_controls; /* collection of controls */
} KDF_DATA;

/*
 * Perform public key operation setup: lookup key, allocated ctx and call
 * the appropriate initialisation function
 */
static int kdf_test_init(EVP_TEST *t, const char *name)
{
    KDF_DATA *kdata;
    EVP_KDF *kdf;

    if (is_kdf_disabled(name)) {
        TEST_info("skipping, '%s' is disabled", name);
        t->skip = 1;
        return 1;
    }

    if (!TEST_ptr(kdata = OPENSSL_zalloc(sizeof(*kdata))))
        return 0;
    kdata->p = kdata->params;
    *kdata->p = OSSL_PARAM_construct_end();

    kdf = EVP_KDF_fetch(libctx, name, propquery);
    if (kdf == NULL) {
        OPENSSL_free(kdata);
        return 0;
    }
    kdata->ctx = EVP_KDF_CTX_new(kdf);
    EVP_KDF_free(kdf);
    if (kdata->ctx == NULL) {
        OPENSSL_free(kdata);
        return 0;
    }
    t->data = kdata;
    kdata->init_controls = sk_OPENSSL_STRING_new_null();
    return 1;
}

static void kdf_test_cleanup(EVP_TEST *t)
{
    KDF_DATA *kdata = t->data;
    OSSL_PARAM *p;

    ctrlfree(kdata->init_controls);
    for (p = kdata->params; p->key != NULL; p++)
        OPENSSL_free(p->data);
    OPENSSL_free(kdata->output);
    EVP_KDF_CTX_free(kdata->ctx);
}

static int kdf_test_ctrl(EVP_TEST *t, EVP_KDF_CTX *kctx,
                         const char *value)
{
    KDF_DATA *kdata = t->data;
    int rv;
    char *p, *name;
    const OSSL_PARAM *defs = EVP_KDF_settable_ctx_params(EVP_KDF_CTX_kdf(kctx));

    if (!TEST_ptr(name = OPENSSL_strdup(value)))
        return 0;
    p = strchr(name, ':');
    if (p == NULL)
        p = "";
    else
        *p++ = '\0';

    if (strcmp(name, "r") == 0
        && OSSL_PARAM_locate_const(defs, name) == NULL) {
        TEST_info("skipping, setting 'r' is unsupported");
        t->skip = 1;
        goto end;
    }

    if (strcmp(name, "lanes") == 0
        && OSSL_PARAM_locate_const(defs, name) == NULL) {
        TEST_info("skipping, setting 'lanes' is unsupported");
        t->skip = 1;
        goto end;
    }

    if (strcmp(name, "iter") == 0
        && OSSL_PARAM_locate_const(defs, name) == NULL) {
        TEST_info("skipping, setting 'iter' is unsupported");
        t->skip = 1;
        goto end;
    }

    if (strcmp(name, "memcost") == 0
        && OSSL_PARAM_locate_const(defs, name) == NULL) {
        TEST_info("skipping, setting 'memcost' is unsupported");
        t->skip = 1;
        goto end;
    }

    if (strcmp(name, "secret") == 0
        && OSSL_PARAM_locate_const(defs, name) == NULL) {
        TEST_info("skipping, setting 'secret' is unsupported");
        t->skip = 1;
        goto end;
    }

    if (strcmp(name, "pass") == 0
        && OSSL_PARAM_locate_const(defs, name) == NULL) {
        TEST_info("skipping, setting 'pass' is unsupported");
        t->skip = 1;
        goto end;
    }

    if (strcmp(name, "ad") == 0
        && OSSL_PARAM_locate_const(defs, name) == NULL) {
        TEST_info("skipping, setting 'ad' is unsupported");
        t->skip = 1;
        goto end;
    }

    rv = OSSL_PARAM_allocate_from_text(kdata->p, defs, name, p,
                                       strlen(p), NULL);
    *++kdata->p = OSSL_PARAM_construct_end();
    if (!rv) {
        t->err = "KDF_PARAM_ERROR";
        OPENSSL_free(name);
        return 0;
    }
    if (strcmp(name, "digest") == 0) {
        if (is_digest_disabled(p)) {
            TEST_info("skipping, '%s' is disabled", p);
            t->skip = 1;
        }
        goto end;
    }

    if ((strcmp(name, "cipher") == 0
        || strcmp(name, "cekalg") == 0)
        && is_cipher_disabled(p)) {
        TEST_info("skipping, '%s' is disabled", p);
        t->skip = 1;
        goto end;
    }
    if ((strcmp(name, "mac") == 0)
        && is_mac_disabled(p)) {
        TEST_info("skipping, '%s' is disabled", p);
        t->skip = 1;
    }
 end:
    OPENSSL_free(name);
    return 1;
}

static int kdf_test_parse(EVP_TEST *t,
                          const char *keyword, const char *value)
{
    KDF_DATA *kdata = t->data;

    if (strcmp(keyword, "Output") == 0)
        return parse_bin(value, &kdata->output, &kdata->output_len);
    if (strcmp(keyword, "CtrlInit") == 0)
        return ctrladd(kdata->init_controls, value);
    if (HAS_PREFIX(keyword, "Ctrl"))
        return kdf_test_ctrl(t, kdata->ctx, value);
    return 0;
}

static int kdf_test_run(EVP_TEST *t)
{
    int ret = 1;
    KDF_DATA *expected = t->data;
    unsigned char *got = NULL;
    size_t got_len = expected->output_len;
    EVP_KDF_CTX *ctx;
    OSSL_PARAM params[2] = { OSSL_PARAM_END, OSSL_PARAM_END };
    size_t params_n = 0, params_allocated_n = 0;

    if (sk_OPENSSL_STRING_num(expected->init_controls) > 0) {
        if (!ctrl2params(t, expected->init_controls,
                         NULL,
                         params, OSSL_NELEM(params), &params_n))
            goto err;
        if (!EVP_KDF_CTX_set_params(expected->ctx, params)) {
            t->err = "KDF_CTRL_ERROR";
            goto err;
        }
    }

    if (!EVP_KDF_CTX_set_params(expected->ctx, expected->params)) {
        t->err = "KDF_CTRL_ERROR";
        goto err;
    }
    if (!TEST_ptr(got = OPENSSL_malloc(got_len == 0 ? 1 : got_len))) {
        t->err = "INTERNAL_ERROR";
        goto err;
    }
    /* FIPS(3.0.0): can't dup KDF contexts #17572 */
    if (fips_provider_version_gt(libctx, 3, 0, 0)
            && (ctx = EVP_KDF_CTX_dup(expected->ctx)) != NULL) {
        EVP_KDF_CTX_free(expected->ctx);
        expected->ctx = ctx;
    }
    if (EVP_KDF_derive(expected->ctx, got, got_len, NULL) <= 0) {
        t->err = "KDF_DERIVE_ERROR";
        goto err;
    }
    if (!kdf_check_fips_approved(expected->ctx, t)) {
        ret = 0;
        goto err;
    }
    if (!memory_err_compare(t, "KDF_MISMATCH",
                            expected->output, expected->output_len,
                            got, got_len))
        goto err;

    t->err = NULL;

 err:
    ctrl2params_free(params, params_n, params_allocated_n);
    OPENSSL_free(got);
    return ret;
}

static const EVP_TEST_METHOD kdf_test_method = {
    "KDF",
    kdf_test_init,
    kdf_test_cleanup,
    kdf_test_parse,
    kdf_test_run
};

/**
 **  PKEY KDF TESTS
 **/

typedef struct pkey_kdf_data_st {
    /* Context for this operation */
    EVP_PKEY_CTX *ctx;
    /* Expected output */
    unsigned char *output;
    size_t output_len;
} PKEY_KDF_DATA;

/*
 * Perform public key operation setup: lookup key, allocated ctx and call
 * the appropriate initialisation function
 */
static int pkey_kdf_test_init(EVP_TEST *t, const char *name)
{
    PKEY_KDF_DATA *kdata = NULL;

    if (is_kdf_disabled(name)) {
        TEST_info("skipping, '%s' is disabled", name);
        t->skip = 1;
        return 1;
    }

    if (!TEST_ptr(kdata = OPENSSL_zalloc(sizeof(*kdata))))
        return 0;

    kdata->ctx = EVP_PKEY_CTX_new_from_name(libctx, name, propquery);
    if (kdata->ctx == NULL
        || EVP_PKEY_derive_init(kdata->ctx) <= 0)
        goto err;

    t->data = kdata;
    return 1;
err:
    EVP_PKEY_CTX_free(kdata->ctx);
    OPENSSL_free(kdata);
    return 0;
}

static void pkey_kdf_test_cleanup(EVP_TEST *t)
{
    PKEY_KDF_DATA *kdata = t->data;

    OPENSSL_free(kdata->output);
    EVP_PKEY_CTX_free(kdata->ctx);
}

static int pkey_kdf_test_parse(EVP_TEST *t,
                               const char *keyword, const char *value)
{
    PKEY_KDF_DATA *kdata = t->data;

    if (strcmp(keyword, "Output") == 0)
        return parse_bin(value, &kdata->output, &kdata->output_len);
    if (HAS_PREFIX(keyword, "Ctrl"))
        return pkey_test_ctrl(t, kdata->ctx, value);
    return 0;
}

static int pkey_kdf_test_run(EVP_TEST *t)
{
    int ret = 1;
    PKEY_KDF_DATA *expected = t->data;
    unsigned char *got = NULL;
    size_t got_len = 0;

    if (fips_provider_version_eq(libctx, 3, 0, 0)) {
        /* FIPS(3.0.0): can't deal with oversized output buffers #18533 */
        got_len = expected->output_len;
    } else {
        /* Find out the KDF output size */
        if (EVP_PKEY_derive(expected->ctx, NULL, &got_len) <= 0) {
            t->err = "INTERNAL_ERROR";
            goto err;
        }

        /*
         * We may get an absurd output size, which signals that anything goes.
         * If not, we specify a too big buffer for the output, to test that
         * EVP_PKEY_derive() can cope with it.
         */
        if (got_len == SIZE_MAX || got_len == 0)
            got_len = expected->output_len;
        else
            got_len = expected->output_len * 2;
    }

    if (!TEST_ptr(got = OPENSSL_malloc(got_len == 0 ? 1 : got_len))) {
        t->err = "INTERNAL_ERROR";
        goto err;
    }
    if (EVP_PKEY_derive(expected->ctx, got, &got_len) <= 0) {
        t->err = "KDF_DERIVE_ERROR";
        goto err;
    }
    if (!pkey_check_fips_approved(expected->ctx, t)) {
        ret = 0;
        goto err;
    }
    if (!TEST_mem_eq(expected->output, expected->output_len, got, got_len)) {
        t->err = "KDF_MISMATCH";
        goto err;
    }
    t->err = NULL;

 err:
    OPENSSL_free(got);
    return ret;
}

static const EVP_TEST_METHOD pkey_kdf_test_method = {
    "PKEYKDF",
    pkey_kdf_test_init,
    pkey_kdf_test_cleanup,
    pkey_kdf_test_parse,
    pkey_kdf_test_run
};

/**
 **  KEYPAIR TESTS
 **/

typedef struct keypair_test_data_st {
    EVP_PKEY *privk;
    EVP_PKEY *pubk;
} KEYPAIR_TEST_DATA;

static int keypair_test_init(EVP_TEST *t, const char *pair)
{
    KEYPAIR_TEST_DATA *data;
    int rv = 0;
    EVP_PKEY *pk = NULL, *pubk = NULL;
    char *pub, *priv = NULL;

    /* Split private and public names. */
    if (!TEST_ptr(priv = OPENSSL_strdup(pair))
            || !TEST_ptr(pub = strchr(priv, ':'))) {
        t->err = "PARSING_ERROR";
        goto end;
    }
    *pub++ = '\0';

    if (!TEST_true(find_key(&pk, priv, private_keys))) {
        TEST_info("Can't find private key: %s", priv);
        t->err = "MISSING_PRIVATE_KEY";
        goto end;
    }
    if (!TEST_true(find_key(&pubk, pub, public_keys))) {
        TEST_info("Can't find public key: %s", pub);
        t->err = "MISSING_PUBLIC_KEY";
        goto end;
    }

    if (pk == NULL && pubk == NULL) {
        /* Both keys are listed but unsupported: skip this test */
        t->skip = 1;
        rv = 1;
        goto end;
    }

    if (!TEST_ptr(data = OPENSSL_malloc(sizeof(*data))))
        goto end;
    data->privk = pk;
    data->pubk = pubk;
    t->data = data;
    rv = 1;
    t->err = NULL;

 end:
    OPENSSL_free(priv);
    return rv;
}

static void keypair_test_cleanup(EVP_TEST *t)
{
    OPENSSL_free(t->data);
    t->data = NULL;
}

/*
 * For tests that do not accept any custom keywords.
 */
static int void_test_parse(EVP_TEST *t, const char *keyword, const char *value)
{
    return 0;
}

static int keypair_test_run(EVP_TEST *t)
{
    int rv = 0;
    const KEYPAIR_TEST_DATA *pair = t->data;

    if (pair->privk == NULL || pair->pubk == NULL) {
        /*
         * this can only happen if only one of the keys is not set
         * which means that one of them was unsupported while the
         * other isn't: hence a key type mismatch.
         */
        t->err = "KEYPAIR_TYPE_MISMATCH";
        rv = 1;
        goto end;
    }

    if ((rv = EVP_PKEY_eq(pair->privk, pair->pubk)) != 1) {
        if (0 == rv) {
            t->err = "KEYPAIR_MISMATCH";
        } else if (-1 == rv) {
            t->err = "KEYPAIR_TYPE_MISMATCH";
        } else if (-2 == rv) {
            t->err = "UNSUPPORTED_KEY_COMPARISON";
        } else {
            TEST_error("Unexpected error in key comparison");
            rv = 0;
            goto end;
        }
        rv = 1;
        goto end;
    }

    rv = 1;
    t->err = NULL;

end:
    return rv;
}

static const EVP_TEST_METHOD keypair_test_method = {
    "PrivPubKeyPair",
    keypair_test_init,
    keypair_test_cleanup,
    void_test_parse,
    keypair_test_run
};

/**
 **  KEYGEN TEST
 **/

typedef struct keygen_test_data_st {
    char *keyname; /* Key name to store key or NULL */
    char *paramname;
    char *alg;
    STACK_OF(OPENSSL_STRING) *in_controls; /* Collection of controls */
    STACK_OF(OPENSSL_STRING) *out_controls;
} KEYGEN_TEST_DATA;

static int keygen_test_init(EVP_TEST *t, const char *alg)
{
    KEYGEN_TEST_DATA *data;

    if (is_pkey_disabled(alg)) {
        t->skip = 1;
        return 1;
    }

    if (!TEST_ptr(data = OPENSSL_zalloc(sizeof(*data))))
        return 0;
    data->keyname = NULL;
    data->paramname = NULL;
    data->in_controls = sk_OPENSSL_STRING_new_null();
    data->out_controls = sk_OPENSSL_STRING_new_null();
    data->alg = OPENSSL_strdup(alg);
    t->data = data;
    t->err = NULL;
    return 1;
}

static void keygen_test_cleanup(EVP_TEST *t)
{
    KEYGEN_TEST_DATA *keygen = t->data;

    ctrlfree(keygen->in_controls);
    ctrlfree(keygen->out_controls);
    OPENSSL_free(keygen->alg);
    OPENSSL_free(keygen->keyname);
    OPENSSL_free(keygen->paramname);
    OPENSSL_free(t->data);
    t->data = NULL;
}

static int keygen_test_parse(EVP_TEST *t,
                             const char *keyword, const char *value)
{
    KEYGEN_TEST_DATA *keygen = t->data;

    if (strcmp(keyword, "KeyName") == 0)
        return TEST_ptr(keygen->keyname = OPENSSL_strdup(value));
    if (strcmp(keyword, "KeyParam") == 0)
        return TEST_ptr(keygen->paramname = OPENSSL_strdup(value));
    if (strcmp(keyword, "Ctrl") == 0)
        return ctrladd(keygen->in_controls, value);
    if (strcmp(keyword, "CtrlOut") == 0)
        return ctrladd(keygen->out_controls, value);
    return 0;
}

/* Iterate thru the key's expected values */
static int check_pkey_expected_values(EVP_TEST *t, const EVP_PKEY *pkey,
                                      STACK_OF(OPENSSL_STRING) *out_controls)
{
    int ret = 0;
    OSSL_PARAM out_params[4], *p;
    size_t out_params_n = 0, len;

    if (sk_OPENSSL_STRING_num(out_controls) > 0) {

        if (!ctrl2params(t, out_controls,
                         EVP_PKEY_gettable_params(pkey),
                         out_params, OSSL_NELEM(out_params), &out_params_n))
            goto err;
        for (p = out_params; p->key != NULL; ++p) {
            if (p->data_type == OSSL_PARAM_OCTET_STRING) {
                uint8_t *data = OPENSSL_malloc(p->data_size);

                if (data == NULL)
                    goto err;
                ret = EVP_PKEY_get_octet_string_param(pkey, p->key, data,
                                                      p->data_size, &len)
                    && len == p->data_size
                    && (TEST_mem_eq(p->data, len, data, len) == 1);
                OPENSSL_free(data);
                if (ret == 0) {
                    TEST_error("Expected %s value is incorrect", p->key);
                    goto err;
                }
            }
        }
    }
    ret = 1;
err:
    ctrl2params_free(out_params, out_params_n, 0);
    return ret;
}

static int keygen_test_run(EVP_TEST *t)
{
    KEYGEN_TEST_DATA *keygen = t->data;
    EVP_PKEY *pkey = NULL, *keyparams = NULL;
    EVP_PKEY_CTX *genctx = NULL; /* Keygen context to use */
    int rv = 1;
    OSSL_PARAM_BLD *bld = NULL;
    OSSL_PARAM *params = NULL;
    size_t params_n = 0;
    int key_free = 1;

    if (keygen->paramname != NULL) {
        rv = find_key(&keyparams, keygen->paramname, public_keys);
        if (rv == 0 || keyparams == NULL) {
            TEST_info("skipping, key '%s' is disabled", keygen->paramname);
            t->skip = 1;
            return 1;
        }
        if (!TEST_ptr(genctx = EVP_PKEY_CTX_new_from_pkey(libctx, keyparams,
                                                          propquery)))
            goto err;

    } else {
        if (!TEST_ptr(genctx = EVP_PKEY_CTX_new_from_name(libctx, keygen->alg,
                                                          propquery)))
            goto err;
    }

    if (EVP_PKEY_keygen_init(genctx) <= 0) {
        t->err = "KEYGEN_INIT_ERROR";
        goto err;
    }

    if (sk_OPENSSL_STRING_num(keygen->in_controls) > 0) {
        if ((params = OPENSSL_malloc(sizeof(OSSL_PARAM) * 4)) == NULL)
            goto err;
        if (!ctrl2params(t, keygen->in_controls,
                         EVP_PKEY_CTX_settable_params(genctx),
                         params, 4, &params_n))
            goto err;
        if (!EVP_PKEY_CTX_set_params(genctx, params)) {
            t->err = "PKEY_CTRL_ERROR";
            goto err;
        }
    }

    if (EVP_PKEY_keygen(genctx, &pkey) <= 0) {
        t->err = "KEYGEN_GENERATE_ERROR";
        goto err;
    }

    if (!pkey_check_fips_approved(genctx, t)) {
        rv = 0;
        goto err;
    }

    if (!evp_pkey_is_provided(pkey)) {
        TEST_info("Warning: legacy key generated %s", keygen->keyname);
        goto err;
    }

    if (!check_pkey_expected_values(t, pkey, keygen->out_controls)) {
        t->err = "KEYGEN_PKEY_MISMATCH_ERROR";
        goto err;
    }

    if (keygen->keyname != NULL) {
        KEY_LIST *key;

        rv = 0;
        if (find_key(NULL, keygen->keyname, private_keys)) {
            TEST_info("Duplicate key %s", keygen->keyname);
            goto err;
        }

        if (!TEST_ptr(key = OPENSSL_malloc(sizeof(*key))))
            goto err;
        key->name = keygen->keyname;
        keygen->keyname = NULL;
        key->key = pkey;
        key->next = private_keys;
        private_keys = key;
        rv = 1;
        key_free = 0;
    }

    t->err = NULL;
err:
    if (key_free) {
        EVP_PKEY_free(pkey);
        pkey = NULL;
    }
    EVP_PKEY_CTX_free(genctx);
    if (sk_OPENSSL_STRING_num(keygen->in_controls) > 0)
        ctrl2params_free(params, params_n, 0);
    OSSL_PARAM_free(params);
    OSSL_PARAM_BLD_free(bld);
    return rv;
}

static const EVP_TEST_METHOD keygen_test_method = {
    "KeyGen",
    keygen_test_init,
    keygen_test_cleanup,
    keygen_test_parse,
    keygen_test_run,
};

/**
 **  DIGEST SIGN+VERIFY TESTS
 **/

typedef struct {
    int is_verify; /* Set to 1 if verifying */
    int is_oneshot; /* Set to 1 for one shot operation */
    const EVP_MD *md; /* Digest to use */
    EVP_MD_CTX *ctx; /* Digest context */
    EVP_PKEY_CTX *pctx;
    STACK_OF(EVP_TEST_BUFFER) *input; /* Input data: streaming */
    unsigned char *osin; /* Input data if one shot */
    size_t osin_len; /* Input length data if one shot */
    unsigned char *output; /* Expected output */
    size_t output_len; /* Expected output length */
    int deterministic_noncetype;
    EVP_PKEY *key;
    STACK_OF(OPENSSL_STRING) *init_controls; /* collection of controls */
    STACK_OF(OPENSSL_STRING) *controls; /* Collection of controls */
} DIGESTSIGN_DATA;

static int digestsigver_test_init(EVP_TEST *t, const char *alg, int is_verify,
                                  int is_oneshot)
{
    const EVP_MD *md = NULL;
    DIGESTSIGN_DATA *mdat;

    if (strcmp(alg, "NULL") != 0) {
        if (is_digest_disabled(alg)) {
            t->skip = 1;
            return 1;
        }
        md = EVP_get_digestbyname(alg);
        if (md == NULL)
            return 0;
    }
    if (!TEST_ptr(mdat = OPENSSL_zalloc(sizeof(*mdat))))
        return 0;
    mdat->init_controls = sk_OPENSSL_STRING_new_null();
    mdat->controls = sk_OPENSSL_STRING_new_null();
    mdat->md = md;
    if (!TEST_ptr(mdat->ctx = EVP_MD_CTX_new())) {
        OPENSSL_free(mdat);
        return 0;
    }
    mdat->is_verify = is_verify;
    mdat->is_oneshot = is_oneshot;
    t->data = mdat;
    return 1;
}

static int digestsign_test_init(EVP_TEST *t, const char *alg)
{
    return digestsigver_test_init(t, alg, 0, 0);
}

static void digestsigver_test_cleanup(EVP_TEST *t)
{
    DIGESTSIGN_DATA *mdata = t->data;

    ctrlfree(mdata->init_controls);
    ctrlfree(mdata->controls);
    EVP_MD_CTX_free(mdata->ctx);
    sk_EVP_TEST_BUFFER_pop_free(mdata->input, evp_test_buffer_free);
    OPENSSL_free(mdata->osin);
    OPENSSL_free(mdata->output);
    OPENSSL_free(mdata);
    t->data = NULL;
}

static int digestsigver_test_parse(EVP_TEST *t,
                                   const char *keyword, const char *value)
{
    DIGESTSIGN_DATA *mdata = t->data;

    if (strcmp(keyword, "Key") == 0) {
        EVP_PKEY *pkey = NULL;
        int rv = 0;

        if (mdata->is_verify)
            rv = find_key(&pkey, value, public_keys);
        if (rv == 0)
            rv = find_key(&pkey, value, private_keys);
        if (rv == 0 || pkey == NULL) {
            t->skip = 1;
            return 1;
        }
        mdata->key = pkey;
        return 1;
    }

    if (strcmp(keyword, "Input") == 0) {
        if (mdata->is_oneshot)
            return parse_bin(value, &mdata->osin, &mdata->osin_len);
        return evp_test_buffer_append(value, data_chunk_size, &mdata->input);
    }
    if (strcmp(keyword, "Output") == 0)
        return parse_bin(value, &mdata->output, &mdata->output_len);

    if (!mdata->is_oneshot && data_chunk_size == 0) {
        if (strcmp(keyword, "Count") == 0)
            return evp_test_buffer_set_count(value, mdata->input);
        if (strcmp(keyword, "Ncopy") == 0)
            return evp_test_buffer_ncopy(value, mdata->input);
    }
    if (strcmp(keyword, "Ctrl") == 0)
        return pkey_add_control(t, mdata->controls, value);
    if (strcmp(keyword, "CtrlInit") == 0)
        return ctrladd(mdata->init_controls, value);
    if (strcmp(keyword, "NonceType") == 0) {
        if (strcmp(value, "deterministic") == 0)
            mdata->deterministic_noncetype = 1;
        return 1;
    }
    return 0;
}

static int check_deterministic_noncetype(EVP_TEST *t,
                                         DIGESTSIGN_DATA *mdata)
{
    if (mdata->deterministic_noncetype == 1) {
        OSSL_PARAM params[2];
        unsigned int nonce_type = 1;

        params[0] =
            OSSL_PARAM_construct_uint(OSSL_SIGNATURE_PARAM_NONCE_TYPE,
                                      &nonce_type);
        params[1] = OSSL_PARAM_construct_end();
        if (!EVP_PKEY_CTX_set_params(mdata->pctx, params))
            t->err = "EVP_PKEY_CTX_set_params_ERROR";
        else if (!EVP_PKEY_CTX_get_params(mdata->pctx, params))
            t->err = "EVP_PKEY_CTX_get_params_ERROR";
        else if (!OSSL_PARAM_modified(&params[0]))
            t->err = "nonce_type_not_modified_ERROR";
        else if (nonce_type != 1)
            t->err = "nonce_type_value_ERROR";
    }
    return t->err == NULL;
}

static int signverify_init(EVP_TEST *t, DIGESTSIGN_DATA *data)
{
    const char *name = data->md == NULL ? NULL : EVP_MD_get0_name(data->md);
    OSSL_PARAM params[5];
    OSSL_PARAM *p = NULL;
    int i, ret = 0;
    size_t params_n = 0, params_allocated_n = 0;

    if (sk_OPENSSL_STRING_num(data->init_controls) > 0) {
        if (!ctrl2params(t, data->init_controls,
                         NULL,
                         params, OSSL_NELEM(params), &params_n))
            goto err;
        p = params;
    }

    if (data->is_verify) {
        if (!EVP_DigestVerifyInit_ex(data->ctx, &data->pctx, name, libctx,
                                     NULL, data->key, p)) {
            t->err = "DIGESTVERIFYINIT_ERROR";
            goto err;
        }
    } else {
        if (!EVP_DigestSignInit_ex(data->ctx, &data->pctx, name, libctx, NULL,
                                   data->key, p)) {
            t->err = "DIGESTSIGNINIT_ERROR";
            goto err;
        }
    }

    for (i = 0; i < sk_OPENSSL_STRING_num(data->controls); i++) {
        char *value = sk_OPENSSL_STRING_value(data->controls, i);

        if (!pkey_test_ctrl(t, data->pctx, value) || t->err != NULL)
            goto err;
    }
    ret = 1;
err:
    ctrl2params_free(params, params_n, params_allocated_n);
    return ret;
}

static int digestsign_update_fn(void *ctx, const unsigned char *buf,
                                size_t buflen)
{
    return EVP_DigestSignUpdate(ctx, buf, buflen);
}

static int digestsign_test_run(EVP_TEST *t)
{
    int i;
    DIGESTSIGN_DATA *expected = t->data;
    unsigned char *got = NULL;
    size_t got_len;

    if (!signverify_init(t, expected))
        goto err;
    if (!check_deterministic_noncetype(t, expected))
        goto err;

    for (i = 0; i < sk_OPENSSL_STRING_num(expected->controls); i++) {
        char *value = sk_OPENSSL_STRING_value(expected->controls, i);
        if (!pkey_test_ctrl(t, expected->pctx, value) || t->err != NULL)
            return 0;
    }

    if (!evp_test_buffer_do(expected->input, digestsign_update_fn,
                            expected->ctx)) {
        t->err = "DIGESTUPDATE_ERROR";
        goto err;
    }

    if (!EVP_DigestSignFinal(expected->ctx, NULL, &got_len)) {
        t->err = "DIGESTSIGNFINAL_LENGTH_ERROR";
        goto err;
    }
    if (!TEST_ptr(got = OPENSSL_malloc(got_len))) {
        t->err = "MALLOC_FAILURE";
        goto err;
    }
    got_len *= 2;
    if (!EVP_DigestSignFinal(expected->ctx, got, &got_len)) {
        t->err = "DIGESTSIGNFINAL_ERROR";
        goto err;
    }
    if (!memory_err_compare(t, "SIGNATURE_MISMATCH",
                            expected->output, expected->output_len,
                            got, got_len))
        goto err;

    t->err = NULL;
 err:
    OPENSSL_free(got);
    return 1;
}

static const EVP_TEST_METHOD digestsign_test_method = {
    "DigestSign",
    digestsign_test_init,
    digestsigver_test_cleanup,
    digestsigver_test_parse,
    digestsign_test_run
};

static int digestverify_test_init(EVP_TEST *t, const char *alg)
{
    return digestsigver_test_init(t, alg, 1, 0);
}

static int digestverify_update_fn(void *ctx, const unsigned char *buf,
                                  size_t buflen)
{
    return EVP_DigestVerifyUpdate(ctx, buf, buflen);
}

static int digestverify_test_run(EVP_TEST *t)
{
    DIGESTSIGN_DATA *mdata = t->data;

    if (!signverify_init(t, mdata))
        return 1;

    if (!evp_test_buffer_do(mdata->input, digestverify_update_fn, mdata->ctx)) {
        t->err = "DIGESTUPDATE_ERROR";
        return 1;
    }

    if (EVP_DigestVerifyFinal(mdata->ctx, mdata->output,
                              mdata->output_len) <= 0)
        t->err = "VERIFY_ERROR";
    return 1;
}

static const EVP_TEST_METHOD digestverify_test_method = {
    "DigestVerify",
    digestverify_test_init,
    digestsigver_test_cleanup,
    digestsigver_test_parse,
    digestverify_test_run
};

static int oneshot_digestsign_test_init(EVP_TEST *t, const char *alg)
{
    return digestsigver_test_init(t, alg, 0, 1);
}

static int oneshot_digestsign_test_run(EVP_TEST *t)
{
    DIGESTSIGN_DATA *expected = t->data;
    unsigned char *got = NULL;
    size_t got_len;

    if (!signverify_init(t, expected))
        goto err;

    if (!EVP_DigestSign(expected->ctx, NULL, &got_len,
                        expected->osin, expected->osin_len)) {
        t->err = "DIGESTSIGN_LENGTH_ERROR";
        goto err;
    }
    if (!TEST_ptr(got = OPENSSL_malloc(got_len))) {
        t->err = "MALLOC_FAILURE";
        goto err;
    }
    got_len *= 2;
    if (!EVP_DigestSign(expected->ctx, got, &got_len,
                        expected->osin, expected->osin_len)) {
        t->err = "DIGESTSIGN_ERROR";
        goto err;
    }
    if (!memory_err_compare(t, "SIGNATURE_MISMATCH",
                            expected->output, expected->output_len,
                            got, got_len))
        goto err;

    t->err = NULL;
 err:
    OPENSSL_free(got);
    return 1;
}

static const EVP_TEST_METHOD oneshot_digestsign_test_method = {
    "OneShotDigestSign",
    oneshot_digestsign_test_init,
    digestsigver_test_cleanup,
    digestsigver_test_parse,
    oneshot_digestsign_test_run
};

static int oneshot_digestverify_test_init(EVP_TEST *t, const char *alg)
{
    return digestsigver_test_init(t, alg, 1, 1);
}

static int oneshot_digestverify_test_run(EVP_TEST *t)
{
    DIGESTSIGN_DATA *mdata = t->data;

    if (!signverify_init(t, mdata))
        return 1;

    if (EVP_DigestVerify(mdata->ctx, mdata->output, mdata->output_len,
                         mdata->osin, mdata->osin_len) <= 0)
        t->err = "VERIFY_ERROR";
    return 1;
}

static const EVP_TEST_METHOD oneshot_digestverify_test_method = {
    "OneShotDigestVerify",
    oneshot_digestverify_test_init,
    digestsigver_test_cleanup,
    digestsigver_test_parse,
    oneshot_digestverify_test_run
};


/**
 **  PARSING AND DISPATCH
 **/

static const EVP_TEST_METHOD *evp_test_list[] = {
    &rand_test_method,
    &cipher_test_method,
    &digest_test_method,
    &digestsign_test_method,
    &digestverify_test_method,
    &encode_test_method,
    &kdf_test_method,
    &pkey_kdf_test_method,
    &keypair_test_method,
    &keygen_test_method,
    &mac_test_method,
    &oneshot_digestsign_test_method,
    &oneshot_digestverify_test_method,
    &pbe_test_method,
    &pdecrypt_test_method,
    &pderive_test_method,
    &psign_test_method,
    &psign_message_test_method,
    &pverify_recover_test_method,
    &pverify_test_method,
    &pverify_message_test_method,
    &pverify_message_public_test_method,
    &pkey_kem_test_method,
    &pkey_fromdata_test_method,
    NULL
};

static const EVP_TEST_METHOD *find_test(const char *name)
{
    const EVP_TEST_METHOD **tt;

    for (tt = evp_test_list; *tt; tt++) {
        if (strcmp(name, (*tt)->name) == 0)
            return *tt;
    }
    return NULL;
}

static void clear_test(EVP_TEST *t)
{
    test_clearstanza(&t->s);
    ERR_clear_error();
    if (t->data != NULL) {
        if (t->meth != NULL)
            t->meth->cleanup(t);
        OPENSSL_free(t->data);
        t->data = NULL;
    }
    OPENSSL_free(t->expected_err);
    t->expected_err = NULL;
    OPENSSL_free(t->reason);
    t->reason = NULL;

    /* Text literal. */
    t->err = NULL;
    t->skip = 0;
    t->meth = NULL;
    t->expect_unapproved = 0;

#if !defined(OPENSSL_NO_DEFAULT_THREAD_POOL)
    OSSL_set_max_threads(libctx, 0);
#endif
}

/* Check for errors in the test structure; return 1 if okay, else 0. */
static int check_test_error(EVP_TEST *t)
{
    unsigned long err;
    const char *reason;

    if (t->err == NULL && t->expected_err == NULL)
        return 1;
    if (t->err != NULL && t->expected_err == NULL) {
        if (t->aux_err != NULL) {
            TEST_info("%s:%d: Source of above error (%s); unexpected error %s",
                      t->s.test_file, t->s.start, t->aux_err, t->err);
        } else {
            TEST_info("%s:%d: Source of above error; unexpected error %s",
                      t->s.test_file, t->s.start, t->err);
        }
        return 0;
    }
    if (t->err == NULL && t->expected_err != NULL) {
        TEST_info("%s:%d: Succeeded but was expecting %s",
                  t->s.test_file, t->s.start, t->expected_err);
        return 0;
    }

    if (strcmp(t->err, t->expected_err) != 0) {
        TEST_info("%s:%d: Expected %s got %s",
                  t->s.test_file, t->s.start, t->expected_err, t->err);
        return 0;
    }

    if (t->reason == NULL)
        return 1;

    if (t->reason == NULL) {
        TEST_info("%s:%d: Test is missing function or reason code",
                  t->s.test_file, t->s.start);
        return 0;
    }

    err = ERR_peek_error();
    if (err == 0) {
        TEST_info("%s:%d: Expected error \"%s\" not set",
                  t->s.test_file, t->s.start, t->reason);
        return 0;
    }

    reason = ERR_reason_error_string(err);
    if (reason == NULL) {
        TEST_info("%s:%d: Expected error \"%s\", no strings available."
                  " Assuming ok.",
                  t->s.test_file, t->s.start, t->reason);
        return 1;
    }

    if (strcmp(reason, t->reason) == 0)
        return 1;

    TEST_info("%s:%d: Expected error \"%s\", got \"%s\"",
              t->s.test_file, t->s.start, t->reason, reason);

    return 0;
}

/* Run a parsed test. Log a message and return 0 on error. */
static int run_test(EVP_TEST *t)
{
    if (t->meth == NULL)
        return 1;
    t->s.numtests++;
    if (t->skip) {
        t->s.numskip++;
    } else {
        /* run the test */
        if (t->err == NULL && t->meth->run_test(t) != 1) {
            TEST_info("%s:%d %s error",
                      t->s.test_file, t->s.start, t->meth->name);
            return 0;
        }
        if (!check_test_error(t)) {
            TEST_openssl_errors();
            t->s.errors++;
        }
    }

    /* clean it up */
    return 1;
}

static int find_key(EVP_PKEY **ppk, const char *name, KEY_LIST *lst)
{
    for (; lst != NULL; lst = lst->next) {
        if (strcmp(lst->name, name) == 0) {
            if (ppk != NULL)
                *ppk = lst->key;
            return 1;
        }
    }
    return 0;
}

static void free_key_list(KEY_LIST *lst)
{
    while (lst != NULL) {
        KEY_LIST *next = lst->next;

        EVP_PKEY_free(lst->key);
        OPENSSL_free(lst->name);
        OPENSSL_free(lst);
        lst = next;
    }
}

/*
 * Is the key type an unsupported algorithm?
 */
static int key_unsupported(void)
{
    long err = ERR_peek_last_error();
    int lib = ERR_GET_LIB(err);
    long reason = ERR_GET_REASON(err);

    if ((lib == ERR_LIB_EVP && reason == EVP_R_UNSUPPORTED_ALGORITHM)
        || (lib == ERR_LIB_EVP && reason == EVP_R_DECODE_ERROR)
        || reason == ERR_R_UNSUPPORTED) {
        ERR_clear_error();
        return 1;
    }
#ifndef OPENSSL_NO_EC
    /*
     * If EC support is enabled we should catch also EC_R_UNKNOWN_GROUP as an
     * hint to an unsupported algorithm/curve (e.g. if binary EC support is
     * disabled).
     */
    if (lib == ERR_LIB_EC
        && (reason == EC_R_UNKNOWN_GROUP
            || reason == EC_R_INVALID_CURVE)) {
        ERR_clear_error();
        return 1;
    }
#endif /* OPENSSL_NO_EC */
    return 0;
}

/* NULL out the value from |pp| but return it.  This "steals" a pointer. */
static char *take_value(PAIR *pp)
{
    char *p = pp->value;

    pp->value = NULL;
    return p;
}

#if !defined(OPENSSL_NO_FIPS_SECURITYCHECKS)
static int securitycheck_enabled(void)
{
    static int enabled = -1;

    if (enabled == -1) {
        if (OSSL_PROVIDER_available(libctx, "fips")) {
            OSSL_PARAM params[2];
            OSSL_PROVIDER *prov = NULL;
            int check = 1;

            prov = OSSL_PROVIDER_load(libctx, "fips");
            if (prov != NULL) {
                params[0] =
                    OSSL_PARAM_construct_int(OSSL_PROV_PARAM_SECURITY_CHECKS,
                                             &check);
                params[1] = OSSL_PARAM_construct_end();
                OSSL_PROVIDER_get_params(prov, params);
                OSSL_PROVIDER_unload(prov);
            }
            enabled = check;
            return enabled;
        }
        enabled = 0;
    }
    return enabled;
}
#endif

/*
 * Return 1 if one of the providers named in the string is available.
 * The provider names are separated with whitespace.
 * NOTE: destructive function, it inserts '\0' after each provider name.
 */
static int prov_available(char *providers)
{
    char *p;
    int more = 1;

    while (more) {
        for (; isspace((unsigned char)(*providers)); providers++)
            continue;
        if (*providers == '\0')
            break;               /* End of the road */
        for (p = providers; *p != '\0' && !isspace((unsigned char)(*p)); p++)
            continue;
        if (*p == '\0')
            more = 0;
        else
            *p = '\0';
        if (OSSL_PROVIDER_available(libctx, providers))
            return 1;            /* Found one */
    }
    return 0;
}

/* Read and parse one test.  Return 0 if failure, 1 if okay. */
static int parse(EVP_TEST *t)
{
    KEY_LIST *key, **klist;
    EVP_PKEY *pkey;
    PAIR *pp;
    int i, j, skipped = 0;

    fips_indicator_callback_unapproved_count = 0;
top:
    do {
        if (BIO_eof(t->s.fp))
            return EOF;
        clear_test(t);
        if (!test_readstanza(&t->s))
            return 0;
    } while (t->s.numpairs == 0);
    pp = &t->s.pairs[0];

    /* Are we adding a key? */
    klist = NULL;
    pkey = NULL;
start:
    if (strcmp(pp->key, "PrivateKey") == 0) {
        pkey = PEM_read_bio_PrivateKey_ex(t->s.key, NULL, 0, NULL, libctx, NULL);
        if (pkey == NULL && !key_unsupported()) {
            EVP_PKEY_free(pkey);
            TEST_info("Can't read private key %s", pp->value);
            TEST_openssl_errors();
            return 0;
        }
        klist = &private_keys;
    } else if (strcmp(pp->key, "PublicKey") == 0) {
        pkey = PEM_read_bio_PUBKEY_ex(t->s.key, NULL, 0, NULL, libctx, NULL);
        if (pkey == NULL && !key_unsupported()) {
            EVP_PKEY_free(pkey);
            TEST_info("Can't read public key %s", pp->value);
            TEST_openssl_errors();
            return 0;
        }
        klist = &public_keys;
    } else if (strcmp(pp->key, "ParamKey") == 0) {
        pkey = PEM_read_bio_Parameters_ex(t->s.key, NULL, libctx, NULL);
        if (pkey == NULL && !key_unsupported()) {
            EVP_PKEY_free(pkey);
            TEST_info("Can't read params key %s", pp->value);
            TEST_openssl_errors();
            return 0;
        }
        klist = &public_keys;
    } else if (strcmp(pp->key, "PrivateKeyRaw") == 0
               || strcmp(pp->key, "PublicKeyRaw") == 0) {
        char *strnid = NULL, *keydata = NULL;
        unsigned char *keybin;
        size_t keylen;
        int nid;

        if (strcmp(pp->key, "PrivateKeyRaw") == 0)
            klist = &private_keys;
        else
            klist = &public_keys;

        strnid = strchr(pp->value, ':');
        if (strnid != NULL) {
            *strnid++ = '\0';
            keydata = strchr(strnid, ':');
            if (keydata != NULL)
                *keydata++ = '\0';
        }
        if (keydata == NULL) {
            TEST_info("Failed to parse %s value", pp->key);
            return 0;
        }

        nid = OBJ_txt2nid(strnid);
        if (nid == NID_undef) {
            TEST_info("Unrecognised algorithm NID");
            return 0;
        }
        if (!parse_bin(keydata, &keybin, &keylen)) {
            TEST_info("Failed to create binary key");
            return 0;
        }
        if (klist == &private_keys)
            pkey = EVP_PKEY_new_raw_private_key_ex(libctx, strnid, NULL, keybin,
                                                   keylen);
        else
            pkey = EVP_PKEY_new_raw_public_key_ex(libctx, strnid, NULL, keybin,
                                                  keylen);
        if (pkey == NULL && !key_unsupported()) {
            TEST_info("Can't read %s data", pp->key);
            OPENSSL_free(keybin);
            TEST_openssl_errors();
            return 0;
        }
        OPENSSL_free(keybin);
    } else if (strcmp(pp->key, "Availablein") == 0) {
        if (!prov_available(pp->value)) {
            TEST_info("skipping, '%s' provider not available: %s:%d",
                      pp->value, t->s.test_file, t->s.start);
                t->skip = 1;
                return 0;
        }
        skipped++;
        pp++;
        goto start;
    } else if (strcmp(pp->key, "FIPSversion") == 0) {
        if (prov_available("fips")) {
            j = fips_provider_version_match(libctx, pp->value);
            if (j < 0) {
                TEST_info("Line %d: error matching FIPS versions\n", t->s.curr);
                return 0;
            } else if (j == 0) {
                TEST_info("skipping, FIPS provider incompatible version: %s:%d",
                          t->s.test_file, t->s.start);
                    t->skip = 1;
                    return 0;
            }
        }
        skipped++;
        pp++;
        goto start;
    }

    /* If we have a key add to list */
    if (klist != NULL) {
        if (find_key(NULL, pp->value, *klist)) {
            TEST_info("Duplicate key %s", pp->value);
            return 0;
        }
        if (!TEST_ptr(key = OPENSSL_malloc(sizeof(*key))))
            return 0;
        key->name = take_value(pp);
        key->key = pkey;
        key->next = *klist;
        *klist = key;

        /* Go back and start a new stanza. */
        if ((t->s.numpairs - skipped) != 1)
            TEST_info("Line %d: missing blank line\n", t->s.curr);
        goto top;
    }

    /* Find the test, based on first keyword. */
    if (!TEST_ptr(t->meth = find_test(pp->key)))
        return 0;
    if (!t->meth->init(t, pp->value)) {
        TEST_error("unknown %s: %s\n", pp->key, pp->value);
        return 0;
    }
    if (t->skip == 1) {
        /* TEST_info("skipping %s %s", pp->key, pp->value); */
        return 0;
    }

    for (pp++, i = 1; i < (t->s.numpairs - skipped); pp++, i++) {
        if (strcmp(pp->key, "Securitycheck") == 0) {
#if defined(OPENSSL_NO_FIPS_SECURITYCHECKS)
#else
            if (!securitycheck_enabled())
#endif
            {
                TEST_info("skipping, Securitycheck is disabled: %s:%d",
                          t->s.test_file, t->s.start);
                t->skip = 1;
                return 0;
            }
        } else if (strcmp(pp->key, "Availablein") == 0) {
            TEST_info("Line %d: 'Availablein' should be the first option",
                      t->s.curr);
            return 0;
        } else if (strcmp(pp->key, "Result") == 0) {
            if (t->expected_err != NULL) {
                TEST_info("Line %d: multiple result lines", t->s.curr);
                return 0;
            }
            t->expected_err = take_value(pp);
        } else if (strcmp(pp->key, "Function") == 0) {
            /* Ignore old line. */
        } else if (strcmp(pp->key, "Reason") == 0) {
            if (t->reason != NULL) {
                TEST_info("Line %d: multiple reason lines", t->s.curr);
                return 0;
            }
            t->reason = take_value(pp);
        } else if (strcmp(pp->key, "Threads") == 0) {
            if (OSSL_set_max_threads(libctx, atoi(pp->value)) == 0) {
                TEST_info("skipping, '%s' threads not available: %s:%d",
                          pp->value, t->s.test_file, t->s.start);
                t->skip = 1;
            }
        } else if (strcmp(pp->key, "Unapproved") == 0) {
            t->expect_unapproved = 1;
        } else if (strcmp(pp->key, "Extended-Test") == 0) {
            if (!extended_tests) {
                TEST_info("skipping extended test: %s:%d",
                          t->s.test_file, t->s.start);
                t->skip = 1;
            }
        } else {
            /* Must be test specific line: try to parse it */
            int rv = t->meth->parse(t, pp->key, pp->value);

            if (rv == 0) {
                TEST_info("Line %d: unknown keyword %s", t->s.curr, pp->key);
                return 0;
            }
            if (rv < 0) {
                TEST_info("Line %d: error processing keyword %s = %s\n",
                          t->s.curr, pp->key, pp->value);
                return 0;
            }
            if (t->skip)
                return 0;
        }
    }

    return 1;
}

static int run_file_tests(int i)
{
    EVP_TEST *t;
    const char *testfile = test_get_argument(i);
    int c;

    if (!TEST_ptr(t = OPENSSL_zalloc(sizeof(*t))))
        return 0;
    if (!test_start_file(&t->s, testfile)) {
        OPENSSL_free(t);
        return 0;
    }

    OSSL_INDICATOR_set_callback(libctx, fips_indicator_cb);
    while (!BIO_eof(t->s.fp)) {
        c = parse(t);
        if (t->skip) {
            t->s.numskip++;
            continue;
        }
        if (c == 0 || !run_test(t)) {
            t->s.errors++;
            break;
        }
    }
    test_end_file(&t->s);
    clear_test(t);

    free_key_list(public_keys);
    public_keys = NULL;
    free_key_list(private_keys);
    private_keys = NULL;
    BIO_free(t->s.key);
    c = t->s.errors;
    OPENSSL_free(t);
    return c == 0;
}

const OPTIONS *test_get_options(void)
{
    static const OPTIONS test_options[] = {
        OPT_TEST_OPTIONS_WITH_EXTRA_USAGE("[file...]\n"),
        { "config", OPT_CONFIG_FILE, '<',
          "The configuration file to use for the libctx" },
        { "process", OPT_IN_PLACE, 's',
          "Mode for data processing by cipher tests [in_place/both], both by default"},
        { "provider", OPT_PROVIDER_NAME, 's',
          "The provider to load (when no configuration file, the default value is 'default')" },
        { "propquery", OPT_PROV_PROPQUERY, 's',
          "Property query used when fetching algorithms" },
        { "chunk", OPT_DATA_CHUNK, 'N', "Size of data chunks to be processed, 0 for default size"},
        { OPT_HELP_STR, 1, '-', "file\tFile to run tests on.\n" },
        { NULL }
    };
    return test_options;
}

int setup_tests(void)
{
    size_t n;
    char *config_file = NULL;
    char *provider_name = NULL;

    OPTION_CHOICE o;

    extended_tests = getenv("EVP_TEST_EXTENDED") != NULL;

    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_CONFIG_FILE:
            config_file = opt_arg();
            break;
        case OPT_IN_PLACE:
            if ((process_mode_in_place = evp_test_process_mode(opt_arg())) == -1)
        case OPT_DATA_CHUNK:
            if (!opt_int(opt_arg(), &data_chunk_size))
                return 0;
            break;
        case OPT_PROVIDER_NAME:
            provider_name = opt_arg();
            break;
        case OPT_PROV_PROPQUERY:
            propquery = opt_arg();
            break;
        case OPT_TEST_CASES:
            break;
        default:
        case OPT_ERR:
            return 0;
        }
    }

    /*
     * Load the provider via configuration into the created library context.
     * Load the 'null' provider into the default library context to ensure that
     * the tests do not fallback to using the default provider.
     */
    if (config_file == NULL && provider_name == NULL)
        provider_name = "default";
    if (!test_get_libctx(&libctx, &prov_null, config_file, &libprov, provider_name))
        return 0;

    n = test_get_argument_count();
    if (n == 0)
        return 0;

    ADD_ALL_TESTS(run_file_tests, n);
    return 1;
}

void cleanup_tests(void)
{
    OSSL_PROVIDER_unload(libprov);
    OSSL_PROVIDER_unload(prov_null);
    OSSL_LIB_CTX_free(libctx);
}

static int is_digest_disabled(const char *name)
{
#ifdef OPENSSL_NO_BLAKE2
    if (HAS_CASE_PREFIX(name, "BLAKE"))
        return 1;
#endif
#ifdef OPENSSL_NO_MD2
    if (OPENSSL_strcasecmp(name, "MD2") == 0)
        return 1;
#endif
#ifdef OPENSSL_NO_MDC2
    if (OPENSSL_strcasecmp(name, "MDC2") == 0)
        return 1;
#endif
#ifdef OPENSSL_NO_MD4
    if (OPENSSL_strcasecmp(name, "MD4") == 0)
        return 1;
#endif
#ifdef OPENSSL_NO_MD5
    if (OPENSSL_strcasecmp(name, "MD5") == 0)
        return 1;
#endif
#ifdef OPENSSL_NO_RMD160
    if (OPENSSL_strcasecmp(name, "RIPEMD160") == 0)
        return 1;
#endif
#ifdef OPENSSL_NO_SM3
    if (OPENSSL_strcasecmp(name, "SM3") == 0)
        return 1;
#endif
#ifdef OPENSSL_NO_WHIRLPOOL
    if (OPENSSL_strcasecmp(name, "WHIRLPOOL") == 0)
        return 1;
#endif
    return 0;
}

static int is_pkey_disabled(const char *name)
{
#ifdef OPENSSL_NO_EC
    if (HAS_CASE_PREFIX(name, "EC"))
        return 1;
#endif
#ifdef OPENSSL_NO_DH
    if (HAS_CASE_PREFIX(name, "DH"))
        return 1;
#endif
#ifdef OPENSSL_NO_DSA
    if (HAS_CASE_PREFIX(name, "DSA"))
        return 1;
#endif
#ifdef OPENSSL_NO_SM2
    if (HAS_CASE_PREFIX(name, "SM2"))
        return 1;
#endif

    /* For sigalgs we use, we also check for digest suffixes */
#ifdef OPENSSL_NO_RMD160
    if (HAS_CASE_SUFFIX(name, "-RIPEMD160"))
        return 1;
#endif
#ifdef OPENSSL_NO_SM3
    if (HAS_CASE_SUFFIX(name, "-SM3"))
        return 1;
#endif
#ifdef OPENSSL_NO_ML_DSA
    if (HAS_CASE_PREFIX(name, "ML-DSA"))
        return 1;
#endif
    return 0;
}

static int is_mac_disabled(const char *name)
{
#ifdef OPENSSL_NO_BLAKE2
    if (HAS_CASE_PREFIX(name, "BLAKE2BMAC")
        || HAS_CASE_PREFIX(name, "BLAKE2SMAC"))
        return 1;
#endif
#ifdef OPENSSL_NO_CMAC
    if (HAS_CASE_PREFIX(name, "CMAC"))
        return 1;
#endif
#ifdef OPENSSL_NO_POLY1305
    if (HAS_CASE_PREFIX(name, "Poly1305"))
        return 1;
#endif
#ifdef OPENSSL_NO_SIPHASH
    if (HAS_CASE_PREFIX(name, "SipHash"))
        return 1;
#endif
    return 0;
}
static int is_kdf_disabled(const char *name)
{
#ifdef OPENSSL_NO_SCRYPT
    if (HAS_CASE_SUFFIX(name, "SCRYPT"))
        return 1;
#endif
#ifdef OPENSSL_NO_ARGON2
    if (HAS_CASE_SUFFIX(name, "ARGON2"))
        return 1;
#endif
    return 0;
}

static int is_cipher_disabled(const char *name)
{
#ifdef OPENSSL_NO_ARIA
    if (HAS_CASE_PREFIX(name, "ARIA"))
        return 1;
#endif
#ifdef OPENSSL_NO_BF
    if (HAS_CASE_PREFIX(name, "BF"))
        return 1;
#endif
#ifdef OPENSSL_NO_CAMELLIA
    if (HAS_CASE_PREFIX(name, "CAMELLIA"))
        return 1;
#endif
#ifdef OPENSSL_NO_CAST
    if (HAS_CASE_PREFIX(name, "CAST"))
        return 1;
#endif
#ifdef OPENSSL_NO_CHACHA
    if (HAS_CASE_PREFIX(name, "CHACHA"))
        return 1;
#endif
#ifdef OPENSSL_NO_POLY1305
    if (HAS_CASE_SUFFIX(name, "Poly1305"))
        return 1;
#endif
#ifdef OPENSSL_NO_DES
    if (HAS_CASE_PREFIX(name, "DES"))
        return 1;
    if (HAS_CASE_SUFFIX(name, "3DESwrap"))
        return 1;
#endif
#ifdef OPENSSL_NO_OCB
    if (HAS_CASE_SUFFIX(name, "OCB"))
        return 1;
#endif
#ifdef OPENSSL_NO_IDEA
    if (HAS_CASE_PREFIX(name, "IDEA"))
        return 1;
#endif
#ifdef OPENSSL_NO_RC2
    if (HAS_CASE_PREFIX(name, "RC2"))
        return 1;
#endif
#ifdef OPENSSL_NO_RC4
    if (HAS_CASE_PREFIX(name, "RC4"))
        return 1;
#endif
#ifdef OPENSSL_NO_RC5
    if (HAS_CASE_PREFIX(name, "RC5"))
        return 1;
#endif
#ifdef OPENSSL_NO_SEED
    if (HAS_CASE_PREFIX(name, "SEED"))
        return 1;
#endif
#ifdef OPENSSL_NO_SIV
    if (HAS_CASE_SUFFIX(name, "SIV"))
        return 1;
#endif
#ifdef OPENSSL_NO_SM4
    if (HAS_CASE_PREFIX(name, "SM4"))
        return 1;
#endif
    return 0;
}
