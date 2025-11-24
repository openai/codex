/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/core_dispatch.h>
#include <openssl/evp.h>
#include <openssl/pem.h>
#include <openssl/rsa.h>
#include <openssl/x509.h>
#include <openssl/core_names.h>
#include <openssl/params.h>
#include <openssl/param_build.h>
#include <openssl/encoder.h>
#include <openssl/decoder.h>

#include "internal/cryptlib.h"   /* ossl_assert */
#include "crypto/pem.h"          /* For PVK and "blob" PEM headers */
#include "crypto/evp.h"          /* For evp_pkey_is_provided() */

#include "helpers/predefined_dhparams.h"
#include "testutil.h"

#ifdef STATIC_LEGACY
OSSL_provider_init_fn ossl_legacy_provider_init;
#endif

/* Extended test macros to allow passing file & line number */
#define TEST_FL_ptr(a)               test_ptr(file, line, #a, a)
#define TEST_FL_mem_eq(a, m, b, n)   test_mem_eq(file, line, #a, #b, a, m, b, n)
#define TEST_FL_strn_eq(a, b, n)     test_strn_eq(file, line, #a, #b, a, n, b, n)
#define TEST_FL_strn2_eq(a, m, b, n) test_strn_eq(file, line, #a, #b, a, m, b, n)
#define TEST_FL_int_eq(a, b)         test_int_eq(file, line, #a, #b, a, b)
#define TEST_FL_int_ge(a, b)         test_int_ge(file, line, #a, #b, a, b)
#define TEST_FL_int_gt(a, b)         test_int_gt(file, line, #a, #b, a, b)
#define TEST_FL_long_gt(a, b)        test_long_gt(file, line, #a, #b, a, b)
#define TEST_FL_true(a)              test_true(file, line, #a, (a) != 0)

#if defined(OPENSSL_NO_DH) && defined(OPENSSL_NO_DSA) && defined(OPENSSL_NO_EC)
# define OPENSSL_NO_KEYPARAMS
#endif

static int default_libctx = 1;
static int is_fips = 0;
static int is_fips_3_0_0 = 0;
static int is_fips_lt_3_5 = 0;

static OSSL_LIB_CTX *testctx = NULL;
static OSSL_LIB_CTX *keyctx = NULL;
static char *testpropq = NULL;

static OSSL_PROVIDER *nullprov = NULL;
static OSSL_PROVIDER *deflprov = NULL;
static OSSL_PROVIDER *keyprov = NULL;

#ifndef OPENSSL_NO_EC
static BN_CTX *bnctx = NULL;
static OSSL_PARAM_BLD *bld_prime_nc = NULL;
static OSSL_PARAM_BLD *bld_prime = NULL;
static OSSL_PARAM *ec_explicit_prime_params_nc = NULL;
static OSSL_PARAM *ec_explicit_prime_params_explicit = NULL;

# ifndef OPENSSL_NO_EC2M
static OSSL_PARAM_BLD *bld_tri_nc = NULL;
static OSSL_PARAM_BLD *bld_tri = NULL;
static OSSL_PARAM *ec_explicit_tri_params_nc = NULL;
static OSSL_PARAM *ec_explicit_tri_params_explicit = NULL;
# endif
#endif

#ifndef OPENSSL_NO_KEYPARAMS
static EVP_PKEY *make_template(const char *type, OSSL_PARAM *genparams)
{
    EVP_PKEY *pkey = NULL;
    EVP_PKEY_CTX *ctx = NULL;

# ifndef OPENSSL_NO_DH
    /*
     * Use 512-bit DH(X) keys with predetermined parameters for efficiency,
     * for testing only. Use a minimum key size of 2048 for security purposes.
     */
    if (strcmp(type, "DH") == 0)
        return get_dh512(keyctx);

    if (strcmp(type, "X9.42 DH") == 0)
        return get_dhx512(keyctx);
# endif

    /*
     * No real need to check the errors other than for the cascade
     * effect.  |pkey| will simply remain NULL if something goes wrong.
     */
    (void)((ctx = EVP_PKEY_CTX_new_from_name(keyctx, type, testpropq)) != NULL
           && EVP_PKEY_paramgen_init(ctx) > 0
           && (genparams == NULL
               || EVP_PKEY_CTX_set_params(ctx, genparams) > 0)
           && EVP_PKEY_generate(ctx, &pkey) > 0);
    EVP_PKEY_CTX_free(ctx);

    return pkey;
}
#endif

#if !defined(OPENSSL_NO_DH) || \
    !defined(OPENSSL_NO_DSA) || \
    !defined(OPENSSL_NO_EC) || \
    !defined(OPENSSL_NO_ML_DSA) || \
    !defined(OPENSSL_NO_ML_KEM) || \
    !defined(OPENSSL_NO_SLH_DSA)
static EVP_PKEY *make_key(const char *type, EVP_PKEY *template,
                          OSSL_PARAM *genparams)
{
    EVP_PKEY *pkey = NULL;
    EVP_PKEY_CTX *ctx =
        template != NULL
        ? EVP_PKEY_CTX_new_from_pkey(keyctx, template, testpropq)
        : EVP_PKEY_CTX_new_from_name(keyctx, type, testpropq);

    /*
     * No real need to check the errors other than for the cascade
     * effect.  |pkey| will simply remain NULL if something goes wrong.
     */
    (void)(ctx != NULL
           && EVP_PKEY_keygen_init(ctx) > 0
           && (genparams == NULL
               || EVP_PKEY_CTX_set_params(ctx, genparams) > 0)
           && EVP_PKEY_keygen(ctx, &pkey) > 0);
    EVP_PKEY_CTX_free(ctx);
    return pkey;
}
#endif

/* Main test driver */

typedef int (encoder)(const char *file, const int line,
                      void **encoded, long *encoded_len,
                      void *object, int selection,
                      const char *output_type, const char *output_structure,
                      const char *pass, const char *pcipher);
typedef int (decoder)(const char *file, const int line,
                      void **object, void *encoded, long encoded_len,
                      const char *input_type, const char *structure_type,
                      const char *keytype, int selection, const char *pass);
typedef int (tester)(const char *file, const int line,
                     const void *data1, size_t data1_len,
                     const void *data2, size_t data2_len);
typedef int (checker)(const char *file, const int line,
                      const char *type, const void *data, size_t data_len);
typedef void (dumper)(const char *label, const void *data, size_t data_len);

#define FLAG_DECODE_WITH_TYPE   0x0001
#define FLAG_FAIL_IF_FIPS       0x0002

static int test_encode_decode(const char *file, const int line,
                              const char *type, EVP_PKEY *pkey,
                              int selection, const char *output_type,
                              const char *output_structure,
                              const char *pass, const char *pcipher,
                              encoder *encode_cb, decoder *decode_cb,
                              tester *test_cb, checker *check_cb,
                              dumper *dump_cb, int flags)
{
    void *encoded = NULL;
    long encoded_len = 0;
    EVP_PKEY *pkey2 = NULL;
    EVP_PKEY *pkey3 = NULL;
    void *encoded2 = NULL;
    long encoded2_len = 0;
    int ok = 0;

    /*
     * Encode |pkey|, decode the result into |pkey2|, and finish off by
     * encoding |pkey2| as well.  That last encoding is for checking and
     * dumping purposes.
     */
    if (!TEST_true(encode_cb(file, line, &encoded, &encoded_len, pkey, selection,
                             output_type, output_structure, pass, pcipher)))
        goto end;

    if ((flags & FLAG_FAIL_IF_FIPS) != 0 && is_fips && !is_fips_3_0_0) {
        if (TEST_false(decode_cb(file, line, (void **)&pkey2, encoded,
                                  encoded_len, output_type, output_structure,
                                  (flags & FLAG_DECODE_WITH_TYPE ? type : NULL),
                                  selection, pass)))
            ok = 1;
        goto end;
    }

    if (!TEST_true(check_cb(file, line, type, encoded, encoded_len))
        || !TEST_true(decode_cb(file, line, (void **)&pkey2, encoded, encoded_len,
                                output_type, output_structure,
                                (flags & FLAG_DECODE_WITH_TYPE ? type : NULL),
                                selection, pass))
        || ((output_structure == NULL
             || strcmp(output_structure, "type-specific") != 0)
            && !TEST_true(decode_cb(file, line, (void **)&pkey3, encoded, encoded_len,
                                    output_type, output_structure,
                                    (flags & FLAG_DECODE_WITH_TYPE ? type : NULL),
                                    0, pass)))
        || !TEST_true(encode_cb(file, line, &encoded2, &encoded2_len, pkey2, selection,
                                output_type, output_structure, pass, pcipher)))
        goto end;

    if (selection == OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) {
        if (!TEST_int_eq(EVP_PKEY_parameters_eq(pkey, pkey2), 1)
            || (pkey3 != NULL
                && !TEST_int_eq(EVP_PKEY_parameters_eq(pkey, pkey3), 1)))
            goto end;
    } else {
        if (!TEST_int_eq(EVP_PKEY_eq(pkey, pkey2), 1)
            || (pkey3 != NULL
                && !TEST_int_eq(EVP_PKEY_eq(pkey, pkey3), 1)))
            goto end;
    }

    /*
     * Double check the encoding, but only for unprotected keys,
     * as protected keys have a random component, which makes the output
     * differ.
     */
    if ((pass == NULL && pcipher == NULL)
        && !test_cb(file, line, encoded, encoded_len, encoded2, encoded2_len))
        goto end;

    ok = 1;
 end:
    if (!ok) {
        if (encoded != NULL && encoded_len != 0)
            dump_cb("|pkey| encoded", encoded, encoded_len);
        if (encoded2 != NULL && encoded2_len != 0)
            dump_cb("|pkey2| encoded", encoded2, encoded2_len);
    }

    OPENSSL_free(encoded);
    OPENSSL_free(encoded2);
    EVP_PKEY_free(pkey2);
    EVP_PKEY_free(pkey3);
    return ok;
}

/* Encoding and decoding methods */

static int encode_EVP_PKEY_prov(const char *file, const int line,
                                void **encoded, long *encoded_len,
                                void *object, int selection,
                                const char *output_type,
                                const char *output_structure,
                                const char *pass, const char *pcipher)
{
    EVP_PKEY *pkey = object;
    OSSL_ENCODER_CTX *ectx = NULL;
    BIO *mem_ser = NULL;
    BUF_MEM *mem_buf = NULL;
    const unsigned char *upass = (const unsigned char *)pass;
    int ok = 0;

    if (!TEST_FL_ptr(ectx = OSSL_ENCODER_CTX_new_for_pkey(pkey, selection,
                                                       output_type,
                                                       output_structure,
                                                       testpropq))
        || !TEST_FL_int_gt(OSSL_ENCODER_CTX_get_num_encoders(ectx), 0)
        || (pass != NULL
            && !TEST_FL_true(OSSL_ENCODER_CTX_set_passphrase(ectx, upass,
                                                          strlen(pass))))
        || (pcipher != NULL
            && !TEST_FL_true(OSSL_ENCODER_CTX_set_cipher(ectx, pcipher, NULL)))
        || !TEST_FL_ptr(mem_ser = BIO_new(BIO_s_mem()))
        || !TEST_FL_true(OSSL_ENCODER_to_bio(ectx, mem_ser))
        || !TEST_FL_true(BIO_get_mem_ptr(mem_ser, &mem_buf) > 0)
        || !TEST_FL_ptr(*encoded = mem_buf->data)
        || !TEST_FL_long_gt(*encoded_len = mem_buf->length, 0))
        goto end;

    /* Detach the encoded output */
    mem_buf->data = NULL;
    mem_buf->length = 0;
    ok = 1;
 end:
    BIO_free(mem_ser);
    OSSL_ENCODER_CTX_free(ectx);
    return ok;
}

static int decode_EVP_PKEY_prov(const char *file, const int line,
                                void **object, void *encoded, long encoded_len,
                                const char *input_type,
                                const char *structure_type,
                                const char *keytype, int selection,
                                const char *pass)
{
    EVP_PKEY *pkey = NULL, *testpkey = NULL;
    OSSL_DECODER_CTX *dctx = NULL;
    BIO *encoded_bio = NULL;
    const unsigned char *upass = (const unsigned char *)pass;
    int ok = 0;
    int i;
    const char *badtype;

    if (strcmp(input_type, "DER") == 0)
        badtype = "PEM";
    else
        badtype = "DER";

    if (!TEST_FL_ptr(encoded_bio = BIO_new_mem_buf(encoded, encoded_len)))
        goto end;

    /*
     * We attempt the decode 3 times. The first time we provide the expected
     * starting input type. The second time we provide NULL for the starting
     * type. The third time we provide a bad starting input type.
     * The bad starting input type should fail. The other two should succeed
     * and produce the same result.
     */
    for (i = 0; i < 3; i++) {
        const char *testtype = (i == 0) ? input_type
                                        : ((i == 1) ? NULL : badtype);

        if (!TEST_FL_ptr(dctx = OSSL_DECODER_CTX_new_for_pkey(&testpkey,
                                                           testtype,
                                                           structure_type,
                                                           keytype,
                                                           selection,
                                                           testctx, testpropq))
            || (pass != NULL
                && !OSSL_DECODER_CTX_set_passphrase(dctx, upass, strlen(pass)))
            || !TEST_FL_int_gt(BIO_reset(encoded_bio), 0)
               /* We expect to fail when using a bad input type */
            || !TEST_FL_int_eq(OSSL_DECODER_from_bio(dctx, encoded_bio),
                            (i == 2) ? 0 : 1))
            goto end;
        OSSL_DECODER_CTX_free(dctx);
        dctx = NULL;

        if (i == 0) {
            pkey = testpkey;
            testpkey = NULL;
        } else if (i == 1) {
            if (selection == OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) {
                if (!TEST_FL_int_eq(EVP_PKEY_parameters_eq(pkey, testpkey), 1))
                    goto end;
            } else {
                if (!TEST_FL_int_eq(EVP_PKEY_eq(pkey, testpkey), 1))
                    goto end;
            }
        }
    }
    ok = 1;
    *object = pkey;
    pkey = NULL;

 end:
    EVP_PKEY_free(pkey);
    EVP_PKEY_free(testpkey);
    BIO_free(encoded_bio);
    OSSL_DECODER_CTX_free(dctx);
    return ok;
}

static int encode_EVP_PKEY_legacy_PEM(const char *file, const int line,
                                      void **encoded, long *encoded_len,
                                      void *object, ossl_unused int selection,
                                      ossl_unused const char *output_type,
                                      ossl_unused const char *output_structure,
                                      const char *pass, const char *pcipher)
{
    EVP_PKEY *pkey = object;
    EVP_CIPHER *cipher = NULL;
    BIO *mem_ser = NULL;
    BUF_MEM *mem_buf = NULL;
    const unsigned char *upass = (const unsigned char *)pass;
    size_t passlen = 0;
    int ok = 0;

    if (pcipher != NULL && pass != NULL) {
        passlen = strlen(pass);
        if (!TEST_FL_ptr(cipher = EVP_CIPHER_fetch(testctx, pcipher, testpropq)))
            goto end;
    }
    if (!TEST_FL_ptr(mem_ser = BIO_new(BIO_s_mem()))
        || !TEST_FL_true(PEM_write_bio_PrivateKey_traditional(mem_ser, pkey,
                                                           cipher,
                                                           upass, passlen,
                                                           NULL, NULL))
        || !TEST_FL_true(BIO_get_mem_ptr(mem_ser, &mem_buf) > 0)
        || !TEST_FL_ptr(*encoded = mem_buf->data)
        || !TEST_FL_long_gt(*encoded_len = mem_buf->length, 0))
        goto end;

    /* Detach the encoded output */
    mem_buf->data = NULL;
    mem_buf->length = 0;
    ok = 1;
 end:
    BIO_free(mem_ser);
    EVP_CIPHER_free(cipher);
    return ok;
}

static int encode_EVP_PKEY_MSBLOB(const char *file, const int line,
                                  void **encoded, long *encoded_len,
                                  void *object, int selection,
                                  ossl_unused const char *output_type,
                                  ossl_unused const char *output_structure,
                                  ossl_unused const char *pass,
                                  ossl_unused const char *pcipher)
{
    EVP_PKEY *pkey = object;
    BIO *mem_ser = NULL;
    BUF_MEM *mem_buf = NULL;
    int ok = 0;

    if (!TEST_FL_ptr(mem_ser = BIO_new(BIO_s_mem())))
        goto end;

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0) {
        if (!TEST_FL_int_ge(i2b_PrivateKey_bio(mem_ser, pkey), 0))
            goto end;
    } else {
        if (!TEST_FL_int_ge(i2b_PublicKey_bio(mem_ser, pkey), 0))
            goto end;
    }

    if (!TEST_FL_true(BIO_get_mem_ptr(mem_ser, &mem_buf) > 0)
        || !TEST_FL_ptr(*encoded = mem_buf->data)
        || !TEST_FL_long_gt(*encoded_len = mem_buf->length, 0))
        goto end;

    /* Detach the encoded output */
    mem_buf->data = NULL;
    mem_buf->length = 0;
    ok = 1;
 end:
    BIO_free(mem_ser);
    return ok;
}

static pem_password_cb pass_pw;
static int pass_pw(char *buf, int size, int rwflag, void *userdata)
{
    OPENSSL_strlcpy(buf, userdata, size);
    return strlen(userdata);
}

static int encode_EVP_PKEY_PVK(const char *file, const int line,
                               void **encoded, long *encoded_len,
                               void *object, int selection,
                               ossl_unused const char *output_type,
                               ossl_unused const char *output_structure,
                               const char *pass,
                               ossl_unused const char *pcipher)
{
    EVP_PKEY *pkey = object;
    BIO *mem_ser = NULL;
    BUF_MEM *mem_buf = NULL;
    int enc = (pass != NULL);
    int ok = 0;

    if (!TEST_FL_true(ossl_assert((selection
                                & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0))
        || !TEST_FL_ptr(mem_ser = BIO_new(BIO_s_mem()))
        || !TEST_FL_int_ge(i2b_PVK_bio_ex(mem_ser, pkey, enc,
                                          pass_pw, (void *)pass, testctx, testpropq), 0)
        || !TEST_FL_true(BIO_get_mem_ptr(mem_ser, &mem_buf) > 0)
        || !TEST_FL_ptr(*encoded = mem_buf->data)
        || !TEST_FL_long_gt(*encoded_len = mem_buf->length, 0))
        goto end;

    /* Detach the encoded output */
    mem_buf->data = NULL;
    mem_buf->length = 0;
    ok = 1;
 end:
    BIO_free(mem_ser);
    return ok;
}

static int test_text(const char *file, const int line,
                     const void *data1, size_t data1_len,
                     const void *data2, size_t data2_len)
{
    return TEST_FL_strn2_eq(data1, data1_len, data2, data2_len);
}

static int test_mem(const char *file, const int line,
                    const void *data1, size_t data1_len,
                    const void *data2, size_t data2_len)
{
    return TEST_FL_mem_eq(data1, data1_len, data2, data2_len);
}

/* Test cases and their dumpers / checkers */

static void collect_name(const char *name, void *arg)
{
    char **namelist = arg;
    char *new_namelist;
    size_t space;

    space = strlen(name);
    if (*namelist != NULL)
        space += strlen(*namelist) + 2 /* for comma and space */;
    space++; /* for terminating null byte */

    new_namelist = OPENSSL_realloc(*namelist, space);
    if (new_namelist == NULL)
        return;
    if (*namelist != NULL) {
        strcat(new_namelist, ", ");
        strcat(new_namelist, name);
    } else {
        strcpy(new_namelist, name);
    }
    *namelist = new_namelist;
}

static void dump_der(const char *label, const void *data, size_t data_len)
{
    test_output_memory(label, data, data_len);
}

static void dump_pem(const char *label, const void *data, size_t data_len)
{
    test_output_string(label, data, data_len - 1);
}

static int check_unprotected_PKCS8_DER(const char *file, const int line,
                                       const char *type,
                                       const void *data, size_t data_len)
{
    const unsigned char *datap = data;
    PKCS8_PRIV_KEY_INFO *p8inf =
        d2i_PKCS8_PRIV_KEY_INFO(NULL, &datap, data_len);
    int ok = 0;

    if (TEST_FL_ptr(p8inf)) {
        EVP_PKEY *pkey = EVP_PKCS82PKEY_ex(p8inf, testctx, testpropq);
        char *namelist = NULL;

        if (TEST_FL_ptr(pkey)) {
            if (!(ok = TEST_FL_true(EVP_PKEY_is_a(pkey, type)))) {
                EVP_PKEY_type_names_do_all(pkey, collect_name, &namelist);
                if (namelist != NULL)
                    TEST_note("%s isn't any of %s", type, namelist);
                OPENSSL_free(namelist);
            }
            ok = ok && TEST_FL_true(evp_pkey_is_provided(pkey));
            EVP_PKEY_free(pkey);
        }
    }
    PKCS8_PRIV_KEY_INFO_free(p8inf);
    return ok;
}

static int test_unprotected_via_DER(const char *type, EVP_PKEY *key, int fips)
{
    return test_encode_decode(__FILE__, __LINE__, type, key,
                              OSSL_KEYMGMT_SELECT_KEYPAIR
                              | OSSL_KEYMGMT_SELECT_ALL_PARAMETERS,
                              "DER", "PrivateKeyInfo", NULL, NULL,
                              encode_EVP_PKEY_prov, decode_EVP_PKEY_prov,
                              test_mem, check_unprotected_PKCS8_DER,
                              dump_der, fips ? 0 : FLAG_FAIL_IF_FIPS);
}

static int check_unprotected_PKCS8_PEM(const char *file, const int line,
                                       const char *type,
                                       const void *data, size_t data_len)
{
    static const char expected_pem_header[] =
        "-----BEGIN " PEM_STRING_PKCS8INF "-----";

    return TEST_FL_strn_eq(data, expected_pem_header,
                        sizeof(expected_pem_header) - 1);
}

static int test_unprotected_via_PEM(const char *type, EVP_PKEY *key, int fips)
{
    return test_encode_decode(__FILE__, __LINE__, type, key,
                              OSSL_KEYMGMT_SELECT_KEYPAIR
                              | OSSL_KEYMGMT_SELECT_ALL_PARAMETERS,
                              "PEM", "PrivateKeyInfo", NULL, NULL,
                              encode_EVP_PKEY_prov, decode_EVP_PKEY_prov,
                              test_text, check_unprotected_PKCS8_PEM,
                              dump_pem, fips ? 0 : FLAG_FAIL_IF_FIPS);
}

#ifndef OPENSSL_NO_KEYPARAMS
static int check_params_DER(const char *file, const int line,
                            const char *type, const void *data, size_t data_len)
{
    const unsigned char *datap = data;
    int ok = 0;
    int itype = NID_undef;
    EVP_PKEY *pkey = NULL;

    if (strcmp(type, "DH") == 0)
        itype = EVP_PKEY_DH;
    else if (strcmp(type, "X9.42 DH") == 0)
        itype = EVP_PKEY_DHX;
    else if (strcmp(type, "DSA") ==  0)
        itype = EVP_PKEY_DSA;
    else if (strcmp(type, "EC") ==  0)
        itype = EVP_PKEY_EC;

    if (itype != NID_undef) {
        pkey = d2i_KeyParams(itype, NULL, &datap, data_len);
        ok = (pkey != NULL);
        EVP_PKEY_free(pkey);
    }

    return ok;
}

static int check_params_PEM(const char *file, const int line,
                            const char *type,
                            const void *data, size_t data_len)
{
    static char expected_pem_header[80];

    return
        TEST_FL_int_gt(BIO_snprintf(expected_pem_header,
                                 sizeof(expected_pem_header),
                                 "-----BEGIN %s PARAMETERS-----", type), 0)
        && TEST_FL_strn_eq(data, expected_pem_header, strlen(expected_pem_header));
}

static int test_params_via_DER(const char *type, EVP_PKEY *key)
{
    return test_encode_decode(__FILE__, __LINE__, type, key, OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS,
                              "DER", "type-specific", NULL, NULL,
                              encode_EVP_PKEY_prov, decode_EVP_PKEY_prov,
                              test_mem, check_params_DER,
                              dump_der, FLAG_DECODE_WITH_TYPE);
}

static int test_params_via_PEM(const char *type, EVP_PKEY *key)
{
    return test_encode_decode(__FILE__, __LINE__, type, key, OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS,
                              "PEM", "type-specific", NULL, NULL,
                              encode_EVP_PKEY_prov, decode_EVP_PKEY_prov,
                              test_text, check_params_PEM,
                              dump_pem, 0);
}
#endif /* !OPENSSL_NO_KEYPARAMS */

static int check_unprotected_legacy_PEM(const char *file, const int line,
                                        const char *type,
                                        const void *data, size_t data_len)
{
    static char expected_pem_header[80];

    return
        TEST_FL_int_gt(BIO_snprintf(expected_pem_header,
                                 sizeof(expected_pem_header),
                                 "-----BEGIN %s PRIVATE KEY-----", type), 0)
        && TEST_FL_strn_eq(data, expected_pem_header, strlen(expected_pem_header));
}

static int test_unprotected_via_legacy_PEM(const char *type, EVP_PKEY *key)
{
    if (!default_libctx || is_fips)
        return TEST_skip("Test not available if using a non-default library context or FIPS provider");

    return test_encode_decode(__FILE__, __LINE__, type, key,
                              OSSL_KEYMGMT_SELECT_KEYPAIR
                              | OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS,
                              "PEM", "type-specific", NULL, NULL,
                              encode_EVP_PKEY_legacy_PEM, decode_EVP_PKEY_prov,
                              test_text, check_unprotected_legacy_PEM,
                              dump_pem, 0);
}

static int check_MSBLOB(const char *file, const int line,
                        const char *type, const void *data, size_t data_len)
{
    const unsigned char *datap = data;
    EVP_PKEY *pkey = b2i_PrivateKey(&datap, data_len);
    int ok = TEST_FL_ptr(pkey);

    EVP_PKEY_free(pkey);
    return ok;
}

static int test_unprotected_via_MSBLOB(const char *type, EVP_PKEY *key)
{
    return test_encode_decode(__FILE__, __LINE__, type, key,
                              OSSL_KEYMGMT_SELECT_KEYPAIR
                              | OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS,
                              "MSBLOB", NULL, NULL, NULL,
                              encode_EVP_PKEY_MSBLOB, decode_EVP_PKEY_prov,
                              test_mem, check_MSBLOB,
                              dump_der, 0);
}

static int check_PVK(const char *file, const int line,
                     const char *type, const void *data, size_t data_len)
{
    const unsigned char *in = data;
    unsigned int saltlen = 0, keylen = 0;
    int isdss = -1;

    return ossl_do_PVK_header(&in, data_len, 0, &isdss, &saltlen, &keylen);
}

static int test_unprotected_via_PVK(const char *type, EVP_PKEY *key)
{
    return test_encode_decode(__FILE__, __LINE__, type, key,
                              OSSL_KEYMGMT_SELECT_KEYPAIR
                              | OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS,
                              "PVK", NULL, NULL, NULL,
                              encode_EVP_PKEY_PVK, decode_EVP_PKEY_prov,
                              test_mem, check_PVK,
                              dump_der, 0);
}

static const char *pass_cipher = "AES-256-CBC";
static const char *pass = "the holy handgrenade of antioch";

static int check_protected_PKCS8_DER(const char *file, const int line,
                                     const char *type,
                                     const void *data, size_t data_len)
{
    const unsigned char *datap = data;
    X509_SIG *p8 = d2i_X509_SIG(NULL, &datap, data_len);
    int ok = TEST_FL_ptr(p8);

    X509_SIG_free(p8);
    return ok;
}

static int test_protected_via_DER(const char *type, EVP_PKEY *key, int fips)
{
    return test_encode_decode(__FILE__, __LINE__, type, key,
                              OSSL_KEYMGMT_SELECT_KEYPAIR
                              | OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS,
                              "DER", "EncryptedPrivateKeyInfo",
                              pass, pass_cipher,
                              encode_EVP_PKEY_prov, decode_EVP_PKEY_prov,
                              test_mem, check_protected_PKCS8_DER,
                              dump_der, fips ? 0 : FLAG_FAIL_IF_FIPS);
}

static int check_protected_PKCS8_PEM(const char *file, const int line,
                                     const char *type,
                                     const void *data, size_t data_len)
{
    static const char expected_pem_header[] =
        "-----BEGIN " PEM_STRING_PKCS8 "-----";

    return TEST_FL_strn_eq(data, expected_pem_header,
                        sizeof(expected_pem_header) - 1);
}

static int test_protected_via_PEM(const char *type, EVP_PKEY *key, int fips)
{
    return test_encode_decode(__FILE__, __LINE__, type, key,
                              OSSL_KEYMGMT_SELECT_KEYPAIR
                              | OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS,
                              "PEM", "EncryptedPrivateKeyInfo",
                              pass, pass_cipher,
                              encode_EVP_PKEY_prov, decode_EVP_PKEY_prov,
                              test_text, check_protected_PKCS8_PEM,
                              dump_pem, fips ? 0 : FLAG_FAIL_IF_FIPS);
}

static int check_protected_legacy_PEM(const char *file, const int line,
                                      const char *type,
                                      const void *data, size_t data_len)
{
    static char expected_pem_header[80];

    return
        TEST_FL_int_gt(BIO_snprintf(expected_pem_header,
                                 sizeof(expected_pem_header),
                                 "-----BEGIN %s PRIVATE KEY-----", type), 0)
        && TEST_FL_strn_eq(data, expected_pem_header, strlen(expected_pem_header))
        && TEST_FL_ptr(strstr(data, "\nDEK-Info: "));
}

static int test_protected_via_legacy_PEM(const char *type, EVP_PKEY *key)
{
    if (!default_libctx || is_fips)
        return TEST_skip("Test not available if using a non-default library context or FIPS provider");

    return test_encode_decode(__FILE__, __LINE__, type, key,
                              OSSL_KEYMGMT_SELECT_KEYPAIR
                              | OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS,
                              "PEM", "type-specific", pass, pass_cipher,
                              encode_EVP_PKEY_legacy_PEM, decode_EVP_PKEY_prov,
                              test_text, check_protected_legacy_PEM,
                              dump_pem, 0);
}

#ifndef OPENSSL_NO_RC4
static int test_protected_via_PVK(const char *type, EVP_PKEY *key)
{
    int ret = 0;
    OSSL_PROVIDER *lgcyprov = OSSL_PROVIDER_load(testctx, "legacy");
    if (lgcyprov == NULL)
        return TEST_skip("Legacy provider not available");

    ret = test_encode_decode(__FILE__, __LINE__, type, key,
                              OSSL_KEYMGMT_SELECT_KEYPAIR
                              | OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS,
                              "PVK", NULL, pass, NULL,
                              encode_EVP_PKEY_PVK, decode_EVP_PKEY_prov,
                              test_mem, check_PVK, dump_der, 0);
    OSSL_PROVIDER_unload(lgcyprov);
    return ret;
}
#endif

static int check_public_DER(const char *file, const int line,
                            const char *type, const void *data, size_t data_len)
{
    const unsigned char *datap = data;
    EVP_PKEY *pkey = d2i_PUBKEY_ex(NULL, &datap, data_len, testctx, testpropq);
    int ok = (TEST_FL_ptr(pkey) && TEST_FL_true(EVP_PKEY_is_a(pkey, type)));

    EVP_PKEY_free(pkey);
    return ok;
}

static int test_public_via_DER(const char *type, EVP_PKEY *key, int fips)
{
    return test_encode_decode(__FILE__, __LINE__, type, key,
                              OSSL_KEYMGMT_SELECT_PUBLIC_KEY
                              | OSSL_KEYMGMT_SELECT_ALL_PARAMETERS,
                              "DER", "SubjectPublicKeyInfo", NULL, NULL,
                              encode_EVP_PKEY_prov, decode_EVP_PKEY_prov,
                              test_mem, check_public_DER, dump_der,
                              fips ? 0 : FLAG_FAIL_IF_FIPS);
}

static int check_public_PEM(const char *file, const int line,
                            const char *type, const void *data, size_t data_len)
{
    static const char expected_pem_header[] =
        "-----BEGIN " PEM_STRING_PUBLIC "-----";

    return
        TEST_FL_strn_eq(data, expected_pem_header,
                     sizeof(expected_pem_header) - 1);
}

static int test_public_via_PEM(const char *type, EVP_PKEY *key, int fips)
{
    return test_encode_decode(__FILE__, __LINE__, type, key,
                              OSSL_KEYMGMT_SELECT_PUBLIC_KEY
                              | OSSL_KEYMGMT_SELECT_ALL_PARAMETERS,
                              "PEM", "SubjectPublicKeyInfo", NULL, NULL,
                              encode_EVP_PKEY_prov, decode_EVP_PKEY_prov,
                              test_text, check_public_PEM, dump_pem,
                              fips ? 0 : FLAG_FAIL_IF_FIPS);
}

static int check_public_MSBLOB(const char *file, const int line,
                               const char *type,
                               const void *data, size_t data_len)
{
    const unsigned char *datap = data;
    EVP_PKEY *pkey = b2i_PublicKey(&datap, data_len);
    int ok = TEST_FL_ptr(pkey);

    EVP_PKEY_free(pkey);
    return ok;
}

static int test_public_via_MSBLOB(const char *type, EVP_PKEY *key)
{
    return test_encode_decode(__FILE__, __LINE__, type, key, OSSL_KEYMGMT_SELECT_PUBLIC_KEY
                              | OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS,
                              "MSBLOB", NULL, NULL, NULL,
                              encode_EVP_PKEY_MSBLOB, decode_EVP_PKEY_prov,
                              test_mem, check_public_MSBLOB, dump_der, 0);
}

#define KEYS(KEYTYPE)                           \
    static EVP_PKEY *key_##KEYTYPE = NULL
#define MAKE_KEYS(KEYTYPE, KEYTYPEstr, params)                          \
    ok = ok                                                             \
        && TEST_ptr(key_##KEYTYPE = make_key(KEYTYPEstr, NULL, params))
#define FREE_KEYS(KEYTYPE)                                              \
    EVP_PKEY_free(key_##KEYTYPE);                                       \

#define DOMAIN_KEYS(KEYTYPE)                    \
    static EVP_PKEY *template_##KEYTYPE = NULL; \
    static EVP_PKEY *key_##KEYTYPE = NULL
#define MAKE_DOMAIN_KEYS(KEYTYPE, KEYTYPEstr, params)                   \
    ok = ok                                                             \
        && TEST_ptr(template_##KEYTYPE =                                \
                    make_template(KEYTYPEstr, params))                  \
        && TEST_ptr(key_##KEYTYPE =                                     \
                    make_key(KEYTYPEstr, template_##KEYTYPE, NULL))
#define FREE_DOMAIN_KEYS(KEYTYPE)                                       \
    EVP_PKEY_free(template_##KEYTYPE);                                  \
    EVP_PKEY_free(key_##KEYTYPE)

#define IMPLEMENT_TEST_SUITE(KEYTYPE, KEYTYPEstr, fips)                 \
    static int test_unprotected_##KEYTYPE##_via_DER(void)               \
    {                                                                   \
        return test_unprotected_via_DER(KEYTYPEstr, key_##KEYTYPE, fips); \
    }                                                                   \
    static int test_unprotected_##KEYTYPE##_via_PEM(void)               \
    {                                                                   \
        return test_unprotected_via_PEM(KEYTYPEstr, key_##KEYTYPE, fips); \
    }                                                                   \
    static int test_protected_##KEYTYPE##_via_DER(void)                 \
    {                                                                   \
        return test_protected_via_DER(KEYTYPEstr, key_##KEYTYPE, fips); \
    }                                                                   \
    static int test_protected_##KEYTYPE##_via_PEM(void)                 \
    {                                                                   \
        return test_protected_via_PEM(KEYTYPEstr, key_##KEYTYPE, fips); \
    }                                                                   \
    static int test_public_##KEYTYPE##_via_DER(void)                    \
    {                                                                   \
        return test_public_via_DER(KEYTYPEstr, key_##KEYTYPE, fips);    \
    }                                                                   \
    static int test_public_##KEYTYPE##_via_PEM(void)                    \
    {                                                                   \
        return test_public_via_PEM(KEYTYPEstr, key_##KEYTYPE, fips);    \
    }

#define ADD_TEST_SUITE(KEYTYPE)                                 \
    ADD_TEST(test_unprotected_##KEYTYPE##_via_DER);             \
    ADD_TEST(test_unprotected_##KEYTYPE##_via_PEM);             \
    ADD_TEST(test_protected_##KEYTYPE##_via_DER);               \
    ADD_TEST(test_protected_##KEYTYPE##_via_PEM);               \
    ADD_TEST(test_public_##KEYTYPE##_via_DER);                  \
    ADD_TEST(test_public_##KEYTYPE##_via_PEM)

#define IMPLEMENT_TEST_SUITE_PARAMS(KEYTYPE, KEYTYPEstr)           \
    static int test_params_##KEYTYPE##_via_DER(void)               \
    {                                                              \
        return test_params_via_DER(KEYTYPEstr, key_##KEYTYPE);     \
    }                                                              \
    static int test_params_##KEYTYPE##_via_PEM(void)               \
    {                                                              \
        return test_params_via_PEM(KEYTYPEstr, key_##KEYTYPE);     \
    }

#define ADD_TEST_SUITE_PARAMS(KEYTYPE)                          \
    ADD_TEST(test_params_##KEYTYPE##_via_DER);                  \
    ADD_TEST(test_params_##KEYTYPE##_via_PEM)

#define IMPLEMENT_TEST_SUITE_LEGACY(KEYTYPE, KEYTYPEstr)                \
    static int test_unprotected_##KEYTYPE##_via_legacy_PEM(void)        \
    {                                                                   \
        return                                                          \
            test_unprotected_via_legacy_PEM(KEYTYPEstr, key_##KEYTYPE); \
    }                                                                   \
    static int test_protected_##KEYTYPE##_via_legacy_PEM(void)          \
    {                                                                   \
        return                                                          \
            test_protected_via_legacy_PEM(KEYTYPEstr, key_##KEYTYPE);   \
    }

#define ADD_TEST_SUITE_LEGACY(KEYTYPE)                                  \
    ADD_TEST(test_unprotected_##KEYTYPE##_via_legacy_PEM);              \
    ADD_TEST(test_protected_##KEYTYPE##_via_legacy_PEM)

#define IMPLEMENT_TEST_SUITE_MSBLOB(KEYTYPE, KEYTYPEstr)                \
    static int test_unprotected_##KEYTYPE##_via_MSBLOB(void)            \
    {                                                                   \
        return test_unprotected_via_MSBLOB(KEYTYPEstr, key_##KEYTYPE);  \
    }                                                                   \
    static int test_public_##KEYTYPE##_via_MSBLOB(void)                 \
    {                                                                   \
        return test_public_via_MSBLOB(KEYTYPEstr, key_##KEYTYPE);       \
    }

#define ADD_TEST_SUITE_MSBLOB(KEYTYPE)                                  \
    ADD_TEST(test_unprotected_##KEYTYPE##_via_MSBLOB);                  \
    ADD_TEST(test_public_##KEYTYPE##_via_MSBLOB)

#define IMPLEMENT_TEST_SUITE_UNPROTECTED_PVK(KEYTYPE, KEYTYPEstr)       \
    static int test_unprotected_##KEYTYPE##_via_PVK(void)               \
    {                                                                   \
        return test_unprotected_via_PVK(KEYTYPEstr, key_##KEYTYPE);     \
    }
# define ADD_TEST_SUITE_UNPROTECTED_PVK(KEYTYPE)                        \
    ADD_TEST(test_unprotected_##KEYTYPE##_via_PVK)
#ifndef OPENSSL_NO_RC4
# define IMPLEMENT_TEST_SUITE_PROTECTED_PVK(KEYTYPE, KEYTYPEstr)        \
    static int test_protected_##KEYTYPE##_via_PVK(void)                 \
    {                                                                   \
        return test_protected_via_PVK(KEYTYPEstr, key_##KEYTYPE);       \
    }
# define ADD_TEST_SUITE_PROTECTED_PVK(KEYTYPE)                          \
    ADD_TEST(test_protected_##KEYTYPE##_via_PVK)
#endif

#ifndef OPENSSL_NO_DH
DOMAIN_KEYS(DH);
IMPLEMENT_TEST_SUITE(DH, "DH", 1)
IMPLEMENT_TEST_SUITE_PARAMS(DH, "DH")
DOMAIN_KEYS(DHX);
IMPLEMENT_TEST_SUITE(DHX, "X9.42 DH", 1)
IMPLEMENT_TEST_SUITE_PARAMS(DHX, "X9.42 DH")
/*
 * DH has no support for PEM_write_bio_PrivateKey_traditional(),
 * so no legacy tests.
 */
#endif
#ifndef OPENSSL_NO_DSA
DOMAIN_KEYS(DSA);
IMPLEMENT_TEST_SUITE(DSA, "DSA", 1)
IMPLEMENT_TEST_SUITE_PARAMS(DSA, "DSA")
IMPLEMENT_TEST_SUITE_LEGACY(DSA, "DSA")
IMPLEMENT_TEST_SUITE_MSBLOB(DSA, "DSA")
IMPLEMENT_TEST_SUITE_UNPROTECTED_PVK(DSA, "DSA")
# ifndef OPENSSL_NO_RC4
IMPLEMENT_TEST_SUITE_PROTECTED_PVK(DSA, "DSA")
# endif
#endif
#ifndef OPENSSL_NO_EC
DOMAIN_KEYS(EC);
IMPLEMENT_TEST_SUITE(EC, "EC", 1)
IMPLEMENT_TEST_SUITE_PARAMS(EC, "EC")
IMPLEMENT_TEST_SUITE_LEGACY(EC, "EC")
DOMAIN_KEYS(ECExplicitPrimeNamedCurve);
IMPLEMENT_TEST_SUITE(ECExplicitPrimeNamedCurve, "EC", 1)
IMPLEMENT_TEST_SUITE_LEGACY(ECExplicitPrimeNamedCurve, "EC")
DOMAIN_KEYS(ECExplicitPrime2G);
IMPLEMENT_TEST_SUITE(ECExplicitPrime2G, "EC", 0)
IMPLEMENT_TEST_SUITE_LEGACY(ECExplicitPrime2G, "EC")
# ifndef OPENSSL_NO_EC2M
DOMAIN_KEYS(ECExplicitTriNamedCurve);
IMPLEMENT_TEST_SUITE(ECExplicitTriNamedCurve, "EC", 1)
IMPLEMENT_TEST_SUITE_LEGACY(ECExplicitTriNamedCurve, "EC")
DOMAIN_KEYS(ECExplicitTri2G);
IMPLEMENT_TEST_SUITE(ECExplicitTri2G, "EC", 0)
IMPLEMENT_TEST_SUITE_LEGACY(ECExplicitTri2G, "EC")
# endif
# ifndef OPENSSL_NO_SM2
KEYS(SM2);
IMPLEMENT_TEST_SUITE(SM2, "SM2", 0)
# endif
#endif
#ifndef OPENSSL_NO_ECX
/*
 * ED25519, ED448, X25519 and X448 have no support for
 * PEM_write_bio_PrivateKey_traditional(), so no legacy tests.
 */
KEYS(ED25519);
IMPLEMENT_TEST_SUITE(ED25519, "ED25519", 1)
KEYS(ED448);
IMPLEMENT_TEST_SUITE(ED448, "ED448", 1)
KEYS(X25519);
IMPLEMENT_TEST_SUITE(X25519, "X25519", 1)
KEYS(X448);
IMPLEMENT_TEST_SUITE(X448, "X448", 1)
#endif
#ifndef OPENSSL_NO_ML_KEM
/*
 * ML-KEM has no support for PEM_write_bio_PrivateKey_traditional(), so no
 * legacy tests.
 */
KEYS(ML_KEM_512);
IMPLEMENT_TEST_SUITE(ML_KEM_512, "ML-KEM-512", 1)
KEYS(ML_KEM_768);
IMPLEMENT_TEST_SUITE(ML_KEM_768, "ML-KEM-768", 1)
KEYS(ML_KEM_1024);
IMPLEMENT_TEST_SUITE(ML_KEM_1024, "ML-KEM-1024", 1)
#endif
#ifndef OPENSSL_NO_SLH_DSA
KEYS(SLH_DSA_SHA2_128s);
KEYS(SLH_DSA_SHA2_128f);
KEYS(SLH_DSA_SHA2_192s);
KEYS(SLH_DSA_SHA2_192f);
KEYS(SLH_DSA_SHA2_256s);
KEYS(SLH_DSA_SHA2_256f);
KEYS(SLH_DSA_SHAKE_128s);
KEYS(SLH_DSA_SHAKE_128f);
KEYS(SLH_DSA_SHAKE_192s);
KEYS(SLH_DSA_SHAKE_192f);
KEYS(SLH_DSA_SHAKE_256s);
KEYS(SLH_DSA_SHAKE_256f);
IMPLEMENT_TEST_SUITE(SLH_DSA_SHA2_128s, "SLH-DSA-SHA2-128s", 1)
IMPLEMENT_TEST_SUITE(SLH_DSA_SHA2_128f, "SLH-DSA-SHA2-128f", 1)
IMPLEMENT_TEST_SUITE(SLH_DSA_SHA2_192s, "SLH-DSA-SHA2-192s", 1)
IMPLEMENT_TEST_SUITE(SLH_DSA_SHA2_192f, "SLH-DSA-SHA2-192f", 1)
IMPLEMENT_TEST_SUITE(SLH_DSA_SHA2_256s, "SLH-DSA-SHA2-256s", 1)
IMPLEMENT_TEST_SUITE(SLH_DSA_SHA2_256f, "SLH-DSA-SHA2-256f", 1)
IMPLEMENT_TEST_SUITE(SLH_DSA_SHAKE_128s, "SLH-DSA-SHAKE-128s", 1)
IMPLEMENT_TEST_SUITE(SLH_DSA_SHAKE_128f, "SLH-DSA-SHAKE-128f", 1)
IMPLEMENT_TEST_SUITE(SLH_DSA_SHAKE_192s, "SLH-DSA-SHAKE-192s", 1)
IMPLEMENT_TEST_SUITE(SLH_DSA_SHAKE_192f, "SLH-DSA-SHAKE-192f", 1)
IMPLEMENT_TEST_SUITE(SLH_DSA_SHAKE_256s, "SLH-DSA-SHAKE-256s", 1)
IMPLEMENT_TEST_SUITE(SLH_DSA_SHAKE_256f, "SLH-DSA-SHAKE-256f", 1)
#endif /* OPENSSL_NO_SLH_DSA */
KEYS(RSA);
IMPLEMENT_TEST_SUITE(RSA, "RSA", 1)
IMPLEMENT_TEST_SUITE_LEGACY(RSA, "RSA")
KEYS(RSA_PSS);
IMPLEMENT_TEST_SUITE(RSA_PSS, "RSA-PSS", 1)
/*
 * RSA-PSS has no support for PEM_write_bio_PrivateKey_traditional(),
 * so no legacy tests.
 */
IMPLEMENT_TEST_SUITE_MSBLOB(RSA, "RSA")
IMPLEMENT_TEST_SUITE_UNPROTECTED_PVK(RSA, "RSA")
#ifndef OPENSSL_NO_RC4
IMPLEMENT_TEST_SUITE_PROTECTED_PVK(RSA, "RSA")
#endif

#ifndef OPENSSL_NO_ML_DSA
KEYS(ML_DSA_44);
KEYS(ML_DSA_65);
KEYS(ML_DSA_87);
IMPLEMENT_TEST_SUITE(ML_DSA_44, "ML-DSA-44", 1)
IMPLEMENT_TEST_SUITE(ML_DSA_65, "ML-DSA-65", 1)
IMPLEMENT_TEST_SUITE(ML_DSA_87, "ML-DSA-87", 1)
#endif /*  OPENSSL_NO_ML_DSA */

#ifndef OPENSSL_NO_EC
/* Explicit parameters that match a named curve */
static int do_create_ec_explicit_prime_params(OSSL_PARAM_BLD *bld,
                                              const unsigned char *gen,
                                              size_t gen_len)
{
    BIGNUM *a, *b, *prime, *order;

    /* Curve prime256v1 */
    static const unsigned char prime_data[] = {
        0x00, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00,
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff
    };
    static const unsigned char a_data[] = {
        0x00, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00,
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xfc
    };
    static const unsigned char b_data[] = {
        0x5a, 0xc6, 0x35, 0xd8, 0xaa, 0x3a, 0x93, 0xe7,
        0xb3, 0xeb, 0xbd, 0x55, 0x76, 0x98, 0x86, 0xbc,
        0x65, 0x1d, 0x06, 0xb0, 0xcc, 0x53, 0xb0, 0xf6,
        0x3b, 0xce, 0x3c, 0x3e, 0x27, 0xd2, 0x60, 0x4b
    };
    static const unsigned char seed[] = {
        0xc4, 0x9d, 0x36, 0x08, 0x86, 0xe7, 0x04, 0x93,
        0x6a, 0x66, 0x78, 0xe1, 0x13, 0x9d, 0x26, 0xb7,
        0x81, 0x9f, 0x7e, 0x90
    };
    static const unsigned char order_data[] = {
        0x00, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00,
        0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xbc, 0xe6, 0xfa, 0xad, 0xa7, 0x17, 0x9e,
        0x84, 0xf3, 0xb9, 0xca, 0xc2, 0xfc, 0x63, 0x25, 0x51
    };
    return TEST_ptr(a = BN_CTX_get(bnctx))
           && TEST_ptr(b = BN_CTX_get(bnctx))
           && TEST_ptr(prime = BN_CTX_get(bnctx))
           && TEST_ptr(order = BN_CTX_get(bnctx))
           && TEST_ptr(BN_bin2bn(prime_data, sizeof(prime_data), prime))
           && TEST_ptr(BN_bin2bn(a_data, sizeof(a_data), a))
           && TEST_ptr(BN_bin2bn(b_data, sizeof(b_data), b))
           && TEST_ptr(BN_bin2bn(order_data, sizeof(order_data), order))
           && TEST_true(OSSL_PARAM_BLD_push_utf8_string(bld,
                            OSSL_PKEY_PARAM_EC_FIELD_TYPE, SN_X9_62_prime_field,
                            0))
           && TEST_true(OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_EC_P, prime))
           && TEST_true(OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_EC_A, a))
           && TEST_true(OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_EC_B, b))
           && TEST_true(OSSL_PARAM_BLD_push_BN(bld,
                            OSSL_PKEY_PARAM_EC_ORDER, order))
           && TEST_true(OSSL_PARAM_BLD_push_octet_string(bld,
                            OSSL_PKEY_PARAM_EC_GENERATOR, gen, gen_len))
           && TEST_true(OSSL_PARAM_BLD_push_octet_string(bld,
                            OSSL_PKEY_PARAM_EC_SEED, seed, sizeof(seed)))
           && TEST_true(OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_EC_COFACTOR,
                                               BN_value_one()));
}

static int create_ec_explicit_prime_params_namedcurve(OSSL_PARAM_BLD *bld)
{
    static const unsigned char prime256v1_gen[] = {
        0x04,
        0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47,
        0xf8, 0xbc, 0xe6, 0xe5, 0x63, 0xa4, 0x40, 0xf2,
        0x77, 0x03, 0x7d, 0x81, 0x2d, 0xeb, 0x33, 0xa0,
        0xf4, 0xa1, 0x39, 0x45, 0xd8, 0x98, 0xc2, 0x96,
        0x4f, 0xe3, 0x42, 0xe2, 0xfe, 0x1a, 0x7f, 0x9b,
        0x8e, 0xe7, 0xeb, 0x4a, 0x7c, 0x0f, 0x9e, 0x16,
        0x2b, 0xce, 0x33, 0x57, 0x6b, 0x31, 0x5e, 0xce,
        0xcb, 0xb6, 0x40, 0x68, 0x37, 0xbf, 0x51, 0xf5
    };
    return do_create_ec_explicit_prime_params(bld, prime256v1_gen,
                                              sizeof(prime256v1_gen));
}

static int create_ec_explicit_prime_params(OSSL_PARAM_BLD *bld)
{
    /* 2G */
    static const unsigned char prime256v1_gen2[] = {
        0x04,
        0xe4, 0x97, 0x08, 0xbe, 0x7d, 0xfa, 0xa2, 0x9a,
        0xa3, 0x12, 0x6f, 0xe4, 0xe7, 0xd0, 0x25, 0xe3,
        0x4a, 0xc1, 0x03, 0x15, 0x8c, 0xd9, 0x33, 0xc6,
        0x97, 0x42, 0xf5, 0xdc, 0x97, 0xb9, 0xd7, 0x31,
        0xe9, 0x7d, 0x74, 0x3d, 0x67, 0x6a, 0x3b, 0x21,
        0x08, 0x9c, 0x31, 0x73, 0xf8, 0xc1, 0x27, 0xc9,
        0xd2, 0xa0, 0xa0, 0x83, 0x66, 0xe0, 0xc9, 0xda,
        0xa8, 0xc6, 0x56, 0x2b, 0x94, 0xb1, 0xae, 0x55
    };
    return do_create_ec_explicit_prime_params(bld, prime256v1_gen2,
                                              sizeof(prime256v1_gen2));
}

# ifndef OPENSSL_NO_EC2M
static int do_create_ec_explicit_trinomial_params(OSSL_PARAM_BLD *bld,
                                                  const unsigned char *gen,
                                                  size_t gen_len)
{
    BIGNUM *a, *b, *poly, *order, *cofactor;
    /* sect233k1 characteristic-two-field tpBasis */
    static const unsigned char poly_data[] = {
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
    };
    static const unsigned char a_data[] = {
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00
    };
    static const unsigned char b_data[] = {
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x01
    };
    static const unsigned char order_data[] = {
        0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x06, 0x9D, 0x5B, 0xB9, 0x15, 0xBC, 0xD4, 0x6E, 0xFB,
        0x1A, 0xD5, 0xF1, 0x73, 0xAB, 0xDF
    };
    static const unsigned char cofactor_data[]= {
        0x4
    };
    return TEST_ptr(a = BN_CTX_get(bnctx))
           && TEST_ptr(b = BN_CTX_get(bnctx))
           && TEST_ptr(poly = BN_CTX_get(bnctx))
           && TEST_ptr(order = BN_CTX_get(bnctx))
           && TEST_ptr(cofactor = BN_CTX_get(bnctx))
           && TEST_ptr(BN_bin2bn(poly_data, sizeof(poly_data), poly))
           && TEST_ptr(BN_bin2bn(a_data, sizeof(a_data), a))
           && TEST_ptr(BN_bin2bn(b_data, sizeof(b_data), b))
           && TEST_ptr(BN_bin2bn(order_data, sizeof(order_data), order))
           && TEST_ptr(BN_bin2bn(cofactor_data, sizeof(cofactor_data), cofactor))
           && TEST_true(OSSL_PARAM_BLD_push_utf8_string(bld,
                            OSSL_PKEY_PARAM_EC_FIELD_TYPE,
                            SN_X9_62_characteristic_two_field, 0))
           && TEST_true(OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_EC_P, poly))
           && TEST_true(OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_EC_A, a))
           && TEST_true(OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_EC_B, b))
           && TEST_true(OSSL_PARAM_BLD_push_BN(bld,
                            OSSL_PKEY_PARAM_EC_ORDER, order))
           && TEST_true(OSSL_PARAM_BLD_push_octet_string(bld,
                            OSSL_PKEY_PARAM_EC_GENERATOR, gen, gen_len))
           && TEST_true(OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_EC_COFACTOR,
                                               cofactor));
}

static int create_ec_explicit_trinomial_params_namedcurve(OSSL_PARAM_BLD *bld)
{
    static const unsigned char gen[] = {
        0x04,
        0x01, 0x72, 0x32, 0xBA, 0x85, 0x3A, 0x7E, 0x73, 0x1A, 0xF1, 0x29, 0xF2,
        0x2F, 0xF4, 0x14, 0x95, 0x63, 0xA4, 0x19, 0xC2, 0x6B, 0xF5, 0x0A, 0x4C,
        0x9D, 0x6E, 0xEF, 0xAD, 0x61, 0x26,
        0x01, 0xDB, 0x53, 0x7D, 0xEC, 0xE8, 0x19, 0xB7, 0xF7, 0x0F, 0x55, 0x5A,
        0x67, 0xC4, 0x27, 0xA8, 0xCD, 0x9B, 0xF1, 0x8A, 0xEB, 0x9B, 0x56, 0xE0,
        0xC1, 0x10, 0x56, 0xFA, 0xE6, 0xA3
    };
    return do_create_ec_explicit_trinomial_params(bld, gen, sizeof(gen));
}

static int create_ec_explicit_trinomial_params(OSSL_PARAM_BLD *bld)
{
    static const unsigned char gen2[] = {
        0x04,
        0x00, 0xd7, 0xba, 0xd0, 0x26, 0x6c, 0x31, 0x6a, 0x78, 0x76, 0x01, 0xd1,
        0x32, 0x4b, 0x8f, 0x30, 0x29, 0x2d, 0x78, 0x30, 0xca, 0x43, 0xaa, 0xf0,
        0xa2, 0x5a, 0xd4, 0x0f, 0xb3, 0xf4,
        0x00, 0x85, 0x4b, 0x1b, 0x8d, 0x50, 0x10, 0xa5, 0x1c, 0x80, 0xf7, 0x86,
        0x40, 0x62, 0x4c, 0x87, 0xd1, 0x26, 0x7a, 0x9c, 0x5c, 0xe9, 0x82, 0x29,
        0xd1, 0x67, 0x70, 0x41, 0xea, 0xcb
    };
    return do_create_ec_explicit_trinomial_params(bld, gen2, sizeof(gen2));
}
# endif /* OPENSSL_NO_EC2M */

/*
 * Test that multiple calls to OSSL_ENCODER_to_data() do not cause side effects
 */
static int ec_encode_to_data_multi(void)
{
    int ret;
    OSSL_ENCODER_CTX *ectx = NULL;
    EVP_PKEY *key = NULL;
    uint8_t *enc = NULL;
    size_t enc_len = 0;

    ret = TEST_ptr(key = EVP_PKEY_Q_keygen(testctx, "", "EC", "P-256"))
        && TEST_ptr(ectx = OSSL_ENCODER_CTX_new_for_pkey(key, EVP_PKEY_KEYPAIR,
                                                         "DER", NULL, NULL))
        && TEST_int_eq(OSSL_ENCODER_to_data(ectx, NULL, &enc_len), 1)
        && TEST_int_eq(OSSL_ENCODER_to_data(ectx, &enc, &enc_len), 1);
    OPENSSL_free(enc);
    EVP_PKEY_free(key);
    OSSL_ENCODER_CTX_free(ectx);
    return ret;
}
#endif /* OPENSSL_NO_EC */

typedef enum OPTION_choice {
    OPT_ERR = -1,
    OPT_EOF = 0,
    OPT_CONTEXT,
    OPT_RSA_FILE,
    OPT_RSA_PSS_FILE,
    OPT_CONFIG_FILE,
    OPT_PROVIDER_NAME,
    OPT_TEST_ENUM
} OPTION_CHOICE;

const OPTIONS *test_get_options(void)
{
    static const OPTIONS options[] = {
        OPT_TEST_OPTIONS_DEFAULT_USAGE,
        { "context", OPT_CONTEXT, '-',
          "Explicitly use a non-default library context" },
        { "rsa", OPT_RSA_FILE, '<',
          "PEM format RSA key file to encode/decode" },
        { "pss", OPT_RSA_PSS_FILE, '<',
          "PEM format RSA-PSS key file to encode/decode" },
        { "config", OPT_CONFIG_FILE, '<',
          "The configuration file to use for the library context" },
        { "provider", OPT_PROVIDER_NAME, 's',
          "The provider to load (The default value is 'default')" },
        { NULL }
    };
    return options;
}

int setup_tests(void)
{
    const char *rsa_file = NULL;
    const char *rsa_pss_file = NULL;
    const char *prov_name = "default";
    char *config_file = NULL;
    int ok = 1;

#ifndef OPENSSL_NO_DSA
    static size_t qbits = 160;  /* PVK only tolerates 160 Q bits */
    static size_t pbits = 1024; /* With 160 Q bits, we MUST use 1024 P bits */
    OSSL_PARAM DSA_params[] = {
        OSSL_PARAM_size_t("pbits", &pbits),
        OSSL_PARAM_size_t("qbits", &qbits),
        OSSL_PARAM_END
    };
#endif

#ifndef OPENSSL_NO_EC
    static char groupname[] = "prime256v1";
    OSSL_PARAM EC_params[] = {
        OSSL_PARAM_utf8_string("group", groupname, sizeof(groupname) - 1),
        OSSL_PARAM_END
    };
#endif

    OPTION_CHOICE o;

    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_CONTEXT:
            default_libctx = 0;
            break;
        case OPT_PROVIDER_NAME:
            prov_name = opt_arg();
            break;
        case OPT_CONFIG_FILE:
            config_file = opt_arg();
            break;
        case OPT_RSA_FILE:
            rsa_file = opt_arg();
            break;
        case OPT_RSA_PSS_FILE:
            rsa_pss_file = opt_arg();
            break;
        case OPT_TEST_CASES:
            break;
        default:
            return 0;
        }
    }

    if (strcmp(prov_name, "fips") == 0)
        is_fips = 1;

    if (default_libctx) {
        if (!test_get_libctx(NULL, NULL, config_file, &deflprov, prov_name))
            return 0;
    } else {
        if (!test_get_libctx(&testctx, &nullprov, config_file, &deflprov, prov_name))
            return 0;
    }

    /* FIPS(3.0.0): provider imports explicit params but they won't work #17998 */
    is_fips_3_0_0 = is_fips && fips_provider_version_eq(testctx, 3, 0, 0);
    /* FIPS(3.5.0) is the first to support ML-DSA, ML-KEM and SLH-DSA */
    is_fips_lt_3_5 = is_fips && fips_provider_version_lt(testctx, 3, 5, 0);

#ifdef STATIC_LEGACY
    /*
     * This test is always statically linked against libcrypto. We must not
     * attempt to load legacy.so that might be dynamically linked against
     * libcrypto. Instead we use a built-in version of the legacy provider.
     */
    if (!OSSL_PROVIDER_add_builtin(testctx, "legacy", ossl_legacy_provider_init))
        return 0;
#endif

    /* Separate provider/ctx for generating the test data */
    if (!TEST_ptr(keyctx = OSSL_LIB_CTX_new()))
        return 0;
    if (!TEST_ptr(keyprov = OSSL_PROVIDER_load(keyctx, "default")))
        return 0;

#ifndef OPENSSL_NO_EC
    if (!TEST_ptr(bnctx = BN_CTX_new_ex(testctx))
        || !TEST_ptr(bld_prime_nc = OSSL_PARAM_BLD_new())
        || !TEST_ptr(bld_prime = OSSL_PARAM_BLD_new())
        || !create_ec_explicit_prime_params_namedcurve(bld_prime_nc)
        || !create_ec_explicit_prime_params(bld_prime)
        || !TEST_ptr(ec_explicit_prime_params_nc = OSSL_PARAM_BLD_to_param(bld_prime_nc))
        || !TEST_ptr(ec_explicit_prime_params_explicit = OSSL_PARAM_BLD_to_param(bld_prime))
# ifndef OPENSSL_NO_EC2M
        || !TEST_ptr(bld_tri_nc = OSSL_PARAM_BLD_new())
        || !TEST_ptr(bld_tri = OSSL_PARAM_BLD_new())
        || !create_ec_explicit_trinomial_params_namedcurve(bld_tri_nc)
        || !create_ec_explicit_trinomial_params(bld_tri)
        || !TEST_ptr(ec_explicit_tri_params_nc = OSSL_PARAM_BLD_to_param(bld_tri_nc))
        || !TEST_ptr(ec_explicit_tri_params_explicit = OSSL_PARAM_BLD_to_param(bld_tri))
# endif
        )
        return 0;
#endif

    TEST_info("Generating keys...");

#ifndef OPENSSL_NO_DH
    TEST_info("Generating DH keys...");
    MAKE_DOMAIN_KEYS(DH, "DH", NULL);
    MAKE_DOMAIN_KEYS(DHX, "X9.42 DH", NULL);
#endif
#ifndef OPENSSL_NO_DSA
    TEST_info("Generating DSA keys...");
    MAKE_DOMAIN_KEYS(DSA, "DSA", DSA_params);
#endif
#ifndef OPENSSL_NO_EC
    TEST_info("Generating EC keys...");
    MAKE_DOMAIN_KEYS(EC, "EC", EC_params);
    MAKE_DOMAIN_KEYS(ECExplicitPrimeNamedCurve, "EC", ec_explicit_prime_params_nc);
    MAKE_DOMAIN_KEYS(ECExplicitPrime2G, "EC", ec_explicit_prime_params_explicit);
# ifndef OPENSSL_NO_EC2M
    MAKE_DOMAIN_KEYS(ECExplicitTriNamedCurve, "EC", ec_explicit_tri_params_nc);
    MAKE_DOMAIN_KEYS(ECExplicitTri2G, "EC", ec_explicit_tri_params_explicit);
# endif
# ifndef OPENSSL_NO_SM2
    MAKE_KEYS(SM2, "SM2", NULL);
# endif
#endif
#ifndef OPENSSL_NO_ECX
    MAKE_KEYS(ED25519, "ED25519", NULL);
    MAKE_KEYS(ED448, "ED448", NULL);
    MAKE_KEYS(X25519, "X25519", NULL);
    MAKE_KEYS(X448, "X448", NULL);
#endif
#ifndef OPENSSL_NO_ML_DSA
    if (!is_fips_lt_3_5) {
        MAKE_KEYS(ML_DSA_44, "ML-DSA-44", NULL);
        MAKE_KEYS(ML_DSA_65, "ML-DSA-65", NULL);
        MAKE_KEYS(ML_DSA_87, "ML-DSA-87", NULL);
    }
#endif /* OPENSSL_NO_ML_DSA */
#ifndef OPENSSL_NO_ML_KEM
    if (!is_fips_lt_3_5) {
        MAKE_KEYS(ML_KEM_512, "ML-KEM-512", NULL);
        MAKE_KEYS(ML_KEM_768, "ML-KEM-768", NULL);
        MAKE_KEYS(ML_KEM_1024, "ML-KEM-1024", NULL);
    }
#endif
#ifndef OPENSSL_NO_SLH_DSA
    if (!is_fips_lt_3_5) {
        MAKE_KEYS(SLH_DSA_SHA2_128s, "SLH-DSA-SHA2-128s", NULL);
        MAKE_KEYS(SLH_DSA_SHA2_128f, "SLH-DSA-SHA2-128f", NULL);
        MAKE_KEYS(SLH_DSA_SHA2_192s, "SLH-DSA-SHA2-192s", NULL);
        MAKE_KEYS(SLH_DSA_SHA2_192f, "SLH-DSA-SHA2-192f", NULL);
        MAKE_KEYS(SLH_DSA_SHA2_256s, "SLH-DSA-SHA2-256s", NULL);
        MAKE_KEYS(SLH_DSA_SHA2_256f, "SLH-DSA-SHA2-256f", NULL);
        MAKE_KEYS(SLH_DSA_SHAKE_128s, "SLH-DSA-SHAKE-128s", NULL);
        MAKE_KEYS(SLH_DSA_SHAKE_128f, "SLH-DSA-SHAKE-128f", NULL);
        MAKE_KEYS(SLH_DSA_SHAKE_192s, "SLH-DSA-SHAKE-192s", NULL);
        MAKE_KEYS(SLH_DSA_SHAKE_192f, "SLH-DSA-SHAKE-192f", NULL);
        MAKE_KEYS(SLH_DSA_SHAKE_256s, "SLH-DSA-SHAKE-256s", NULL);
        MAKE_KEYS(SLH_DSA_SHAKE_256f, "SLH-DSA-SHAKE-256f", NULL);
    }
#endif /* OPENSSL_NO_SLH_DSA */

    TEST_info("Loading RSA key...");
    ok = ok && TEST_ptr(key_RSA = load_pkey_pem(rsa_file, keyctx));
    TEST_info("Loading RSA_PSS key...");
    ok = ok && TEST_ptr(key_RSA_PSS = load_pkey_pem(rsa_pss_file, keyctx));
    TEST_info("Generating keys done");

    if (ok) {
#ifndef OPENSSL_NO_DH
        ADD_TEST_SUITE(DH);
        ADD_TEST_SUITE_PARAMS(DH);
        ADD_TEST_SUITE(DHX);
        ADD_TEST_SUITE_PARAMS(DHX);
        /*
         * DH has no support for PEM_write_bio_PrivateKey_traditional(),
         * so no legacy tests.
         */
#endif
#ifndef OPENSSL_NO_DSA
        ADD_TEST_SUITE(DSA);
        ADD_TEST_SUITE_PARAMS(DSA);
        ADD_TEST_SUITE_LEGACY(DSA);
        ADD_TEST_SUITE_MSBLOB(DSA);
        ADD_TEST_SUITE_UNPROTECTED_PVK(DSA);
# ifndef OPENSSL_NO_RC4
        ADD_TEST_SUITE_PROTECTED_PVK(DSA);
# endif
#endif
#ifndef OPENSSL_NO_EC
        ADD_TEST(ec_encode_to_data_multi);
        ADD_TEST_SUITE(EC);
        ADD_TEST_SUITE_PARAMS(EC);
        ADD_TEST_SUITE_LEGACY(EC);
        ADD_TEST_SUITE(ECExplicitPrimeNamedCurve);
        ADD_TEST_SUITE_LEGACY(ECExplicitPrimeNamedCurve);
        ADD_TEST_SUITE(ECExplicitPrime2G);
        ADD_TEST_SUITE_LEGACY(ECExplicitPrime2G);
# ifndef OPENSSL_NO_EC2M
        ADD_TEST_SUITE(ECExplicitTriNamedCurve);
        ADD_TEST_SUITE_LEGACY(ECExplicitTriNamedCurve);
        ADD_TEST_SUITE(ECExplicitTri2G);
        ADD_TEST_SUITE_LEGACY(ECExplicitTri2G);
# endif
# ifndef OPENSSL_NO_SM2
        if (!is_fips_3_0_0) {
            /* 3.0.0 FIPS provider imports explicit EC params and then fails. */
            ADD_TEST_SUITE(SM2);
        }
# endif
#endif
#ifndef OPENSSL_NO_ECX
        ADD_TEST_SUITE(ED25519);
        ADD_TEST_SUITE(ED448);
        ADD_TEST_SUITE(X25519);
        ADD_TEST_SUITE(X448);
        /*
         * ED25519, ED448, X25519 and X448 have no support for
         * PEM_write_bio_PrivateKey_traditional(), so no legacy tests.
         */
#endif
#ifndef OPENSSL_NO_ML_KEM
        if (!is_fips_lt_3_5) {
            ADD_TEST_SUITE(ML_KEM_512);
            ADD_TEST_SUITE(ML_KEM_768);
            ADD_TEST_SUITE(ML_KEM_1024);
        }
#endif
        ADD_TEST_SUITE(RSA);
        ADD_TEST_SUITE_LEGACY(RSA);
        ADD_TEST_SUITE(RSA_PSS);
        /*
         * RSA-PSS has no support for PEM_write_bio_PrivateKey_traditional(),
         * so no legacy tests.
         */
        ADD_TEST_SUITE_MSBLOB(RSA);
        ADD_TEST_SUITE_UNPROTECTED_PVK(RSA);
# ifndef OPENSSL_NO_RC4
        ADD_TEST_SUITE_PROTECTED_PVK(RSA);
# endif

#ifndef OPENSSL_NO_ML_DSA
        if (!is_fips_lt_3_5) {
            ADD_TEST_SUITE(ML_DSA_44);
            ADD_TEST_SUITE(ML_DSA_65);
            ADD_TEST_SUITE(ML_DSA_87);
        }
#endif /* OPENSSL_NO_ML_DSA */

#ifndef OPENSSL_NO_SLH_DSA
        if (!is_fips_lt_3_5) {
            ADD_TEST_SUITE(SLH_DSA_SHA2_128s);
            ADD_TEST_SUITE(SLH_DSA_SHA2_128f);
            ADD_TEST_SUITE(SLH_DSA_SHA2_192s);
            ADD_TEST_SUITE(SLH_DSA_SHA2_192f);
            ADD_TEST_SUITE(SLH_DSA_SHA2_256s);
            ADD_TEST_SUITE(SLH_DSA_SHA2_256f);
            ADD_TEST_SUITE(SLH_DSA_SHAKE_128s);
            ADD_TEST_SUITE(SLH_DSA_SHAKE_128f);
            ADD_TEST_SUITE(SLH_DSA_SHAKE_192s);
            ADD_TEST_SUITE(SLH_DSA_SHAKE_192f);
            ADD_TEST_SUITE(SLH_DSA_SHAKE_256s);
            ADD_TEST_SUITE(SLH_DSA_SHAKE_256f);
        }
#endif /* OPENSSL_NO_SLH_DSA */
    }

    return 1;
}

void cleanup_tests(void)
{
#ifndef OPENSSL_NO_EC
    OSSL_PARAM_free(ec_explicit_prime_params_nc);
    OSSL_PARAM_free(ec_explicit_prime_params_explicit);
    OSSL_PARAM_BLD_free(bld_prime_nc);
    OSSL_PARAM_BLD_free(bld_prime);
# ifndef OPENSSL_NO_EC2M
    OSSL_PARAM_free(ec_explicit_tri_params_nc);
    OSSL_PARAM_free(ec_explicit_tri_params_explicit);
    OSSL_PARAM_BLD_free(bld_tri_nc);
    OSSL_PARAM_BLD_free(bld_tri);
# endif
    BN_CTX_free(bnctx);
#endif /* OPENSSL_NO_EC */

#ifndef OPENSSL_NO_DH
    FREE_DOMAIN_KEYS(DH);
    FREE_DOMAIN_KEYS(DHX);
#endif
#ifndef OPENSSL_NO_DSA
    FREE_DOMAIN_KEYS(DSA);
#endif
#ifndef OPENSSL_NO_EC
    FREE_DOMAIN_KEYS(EC);
    FREE_DOMAIN_KEYS(ECExplicitPrimeNamedCurve);
    FREE_DOMAIN_KEYS(ECExplicitPrime2G);
# ifndef OPENSSL_NO_EC2M
    FREE_DOMAIN_KEYS(ECExplicitTriNamedCurve);
    FREE_DOMAIN_KEYS(ECExplicitTri2G);
# endif
# ifndef OPENSSL_NO_SM2
    FREE_KEYS(SM2);
# endif
#endif
#ifndef OPENSSL_NO_ECX
    FREE_KEYS(ED25519);
    FREE_KEYS(ED448);
    FREE_KEYS(X25519);
    FREE_KEYS(X448);
#endif
#ifndef OPENSSL_NO_ML_KEM
    if (!is_fips_lt_3_5) {
        FREE_KEYS(ML_KEM_512);
        FREE_KEYS(ML_KEM_768);
        FREE_KEYS(ML_KEM_1024);
    }
#endif
    FREE_KEYS(RSA);
    FREE_KEYS(RSA_PSS);

#ifndef OPENSSL_NO_ML_DSA
    if (!is_fips_lt_3_5) {
        FREE_KEYS(ML_DSA_44);
        FREE_KEYS(ML_DSA_65);
        FREE_KEYS(ML_DSA_87);
    }
#endif /* OPENSSL_NO_ML_DSA */

#ifndef OPENSSL_NO_SLH_DSA
    if (!is_fips_lt_3_5) {
        FREE_KEYS(SLH_DSA_SHA2_128s);
        FREE_KEYS(SLH_DSA_SHA2_128f);
        FREE_KEYS(SLH_DSA_SHA2_192s);
        FREE_KEYS(SLH_DSA_SHA2_192f);
        FREE_KEYS(SLH_DSA_SHA2_256s);
        FREE_KEYS(SLH_DSA_SHA2_256f);
        FREE_KEYS(SLH_DSA_SHAKE_128s);
        FREE_KEYS(SLH_DSA_SHAKE_128f);
        FREE_KEYS(SLH_DSA_SHAKE_192s);
        FREE_KEYS(SLH_DSA_SHAKE_192f);
        FREE_KEYS(SLH_DSA_SHAKE_256s);
        FREE_KEYS(SLH_DSA_SHAKE_256f);
    }
#endif /* OPENSSL_NO_SLH_DSA */

    OSSL_PROVIDER_unload(nullprov);
    OSSL_PROVIDER_unload(deflprov);
    OSSL_PROVIDER_unload(keyprov);
    OSSL_LIB_CTX_free(testctx);
    OSSL_LIB_CTX_free(keyctx);
}
