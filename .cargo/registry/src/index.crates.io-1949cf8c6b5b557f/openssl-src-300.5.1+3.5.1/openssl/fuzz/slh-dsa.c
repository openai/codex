/*
 * Copyright 2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 * https://www.openssl.org/source/license.html
 * or in the file LICENSE in the source distribution.
 */

/*
 * Test slh-dsa operation.
 */
#include <string.h>
#include <openssl/evp.h>
#include <openssl/err.h>
#include <openssl/rand.h>
#include <openssl/byteorder.h>
#include <openssl/core_names.h>
#include "crypto/slh_dsa.h"
#include "internal/nelem.h"
#include "fuzzer.h"

/**
 * @brief Consumes an 8-bit unsigned integer from a buffer.
 *
 * This function extracts an 8-bit unsigned integer from the provided buffer,
 * updates the buffer pointer, and adjusts the remaining length.
 *
 * @param buf  Pointer to the input buffer.
 * @param len  Pointer to the size of the remaining buffer; updated after consumption.
 * @param val  Pointer to store the extracted 8-bit value.
 *
 * @return Pointer to the updated buffer position after reading the value,
 *         or NULL if the buffer does not contain enough data.
 */
static uint8_t *consume_uint8t(const uint8_t *buf, size_t *len, uint8_t *val)
{
    if (*len < sizeof(uint8_t))
        return NULL;
    *val = *buf;
    *len -= sizeof(uint8_t);
    return (uint8_t *)buf + 1;
}

/**
 * @brief Generates a DSA key pair using OpenSSL EVP API.
 *
 * This function creates a DSA key pair based on the specified key size and
 * parameters. It supports generating keys using explicit parameters if provided.
 *
 * @param name The name of the key type (e.g., "DSA").
 * @param keysize The desired key size in bits.
 * @param params Optional OpenSSL parameters for key generation.
 * @param param_broken A flag indicating if the parameters are broken.
 *                     If true, key generation will fail.
 *
 * @return A pointer to the generated EVP_PKEY structure on success,
 *         or NULL on failure.
 */
static EVP_PKEY *slh_dsa_gen_key(const char *name, uint32_t keysize,
                                 OSSL_PARAM params[], uint8_t *param_broken)
{
    EVP_PKEY_CTX *ctx;
    EVP_PKEY *new = NULL;
    int rc;

    ctx = EVP_PKEY_CTX_new_from_name(NULL, name, NULL);
    OPENSSL_assert(ctx != NULL);
    if (params != NULL) {
        new = EVP_PKEY_new();
        OPENSSL_assert(EVP_PKEY_fromdata_init(ctx));
        if (*param_broken) {
            rc = EVP_PKEY_fromdata(ctx, &new, EVP_PKEY_KEYPAIR, params);
            OPENSSL_assert(rc == 0);
            EVP_PKEY_free(new);
            new = NULL;
        } else {
            OPENSSL_assert(EVP_PKEY_fromdata(ctx, &new, EVP_PKEY_KEYPAIR, params) == 1);
        }
        goto out;
    }

    OPENSSL_assert(EVP_PKEY_keygen_init(ctx));
    OPENSSL_assert(EVP_PKEY_generate(ctx, &new));

out:
    EVP_PKEY_CTX_free(ctx);
    return new;
}

/**
 * @brief Selects a key type and determines the key size.
 *
 * This function maps a selector value to a specific SLH-DSA algorithm
 * using a modulo operation. It then retrieves the corresponding
 * algorithm name and assigns an appropriate key size based on the
 * selected algorithm.
 *
 * @param selector A random selector value used to determine the key type.
 * @param keysize Pointer to a variable where the determined key size
 *                (in bytes) will be stored.
 *
 * @return A pointer to a string containing the long name of the
 *         selected key type, or NULL if invalid.
 */
static const char *select_keytype(uint8_t selector, uint32_t *keysize)
{
    unsigned int choice;
    const char *name = NULL;

    *keysize = 0;
    /*
     * There are 12 SLH-DSA algs with registered NIDS at the moment
     * So use our random selector value to get one of them by computing
     * its modulo 12 value and adding the offset of the first NID, 1460
     * Then convert that to a long name
     */
    choice = (selector % 12) + 1460;

    name = OBJ_nid2ln(choice);

    /*
     * Select a keysize, values taken from
     * man7/EVP_PKEY-SLH-DSA.pod
     */
    switch (choice) {
    case NID_SLH_DSA_SHA2_128s:
    case NID_SLH_DSA_SHA2_128f:
    case NID_SLH_DSA_SHAKE_128s:
    case NID_SLH_DSA_SHAKE_128f:
        *keysize = 16;
        break;
    case NID_SLH_DSA_SHA2_192s:
    case NID_SLH_DSA_SHA2_192f:
    case NID_SLH_DSA_SHAKE_192s:
    case NID_SLH_DSA_SHAKE_192f:
        *keysize = 24;
        break;
    case NID_SLH_DSA_SHA2_256s:
    case NID_SLH_DSA_SHA2_256f:
    case NID_SLH_DSA_SHAKE_256s:
    case NID_SLH_DSA_SHAKE_256f:
        *keysize = 32;
        break;
    default:
        fprintf(stderr, "Selecting invalid key size\n");
        *keysize = 0;
        break;
    }
    return name;
}

/**
 * @brief Generates two SLH-DSA key pairs based on consumed selector values.
 *
 * This function extracts two selector values from the provided buffer,
 * determines the corresponding key types and sizes, and generates two
 * SLH-DSA key pairs.
 *
 * @param buf Pointer to a buffer containing selector values. The buffer
 *            pointer is updated as values are consumed.
 * @param len Pointer to the remaining buffer length, updated as values
 *            are consumed.
 * @param out1 Pointer to store the first generated key.
 * @param out2 Pointer to store the second generated key.
 */
static void slh_dsa_gen_keys(uint8_t **buf, size_t *len,
                             void **out1, void **out2)
{
    uint8_t selector = 0;
    const char *keytype = NULL;
    uint32_t keysize;

    *buf = consume_uint8t(*buf, len, &selector);
    keytype = select_keytype(selector, &keysize);
    *out1 = (void *)slh_dsa_gen_key(keytype, keysize, NULL, 0);

    *buf = consume_uint8t(*buf, len, &selector);
    keytype = select_keytype(selector, &keysize);
    *out2 = (void *)slh_dsa_gen_key(keytype, keysize, NULL, 0);
    return;
}

#define PARAM_BUF_SZ 256

/**
 * @brief Generates an SLH-DSA key pair with custom parameters.
 *
 * This function extracts a selector value from the provided buffer,
 * determines the corresponding key type and size, and generates an
 * SLH-DSA key pair using randomly generated public and private key
 * buffers. It also introduces intentional modifications to test
 * invalid parameter handling.
 *
 * @param buf Pointer to a buffer containing the selector value. The
 *            buffer pointer is updated as values are consumed.
 * @param len Pointer to the remaining buffer length, updated as values
 *            are consumed.
 * @param out1 Pointer to store the generated key. Will be NULL if key
 *             generation fails due to invalid parameters.
 * @param out2 Unused output parameter (placeholder for symmetry with
 *             other key generation functions).
 */
static void slh_dsa_gen_key_with_params(uint8_t **buf, size_t *len,
                                        void **out1, void **out2)
{
    uint8_t selector = 0;
    const char *keytype = NULL;
    uint32_t keysize;
    uint8_t pubbuf[PARAM_BUF_SZ]; /* expressly bigger than max key size * 3 */
    uint8_t prvbuf[PARAM_BUF_SZ]; /* expressly bigger than max key size * 3 */
    uint8_t sdbuf[PARAM_BUF_SZ]; /* expressly bigger than max key size * 3 */
    uint8_t *bufptr;
    OSSL_PARAM params[3];
    size_t buflen;
    uint8_t broken = 0;

    *out1 = NULL;

    *buf = consume_uint8t(*buf, len, &selector);
    keytype = select_keytype(selector, &keysize);

    RAND_bytes(pubbuf, PARAM_BUF_SZ);
    RAND_bytes(prvbuf, PARAM_BUF_SZ);
    RAND_bytes(sdbuf, PARAM_BUF_SZ);

    /*
     * select an invalid length if the buffer 0th bit is one
     * make it too big if the 2nd bit is 0, smaller otherwise
     */
    buflen = keysize * 2; /* these params are 2 * the keysize */
    if ((*buf)[0] & 0x1) {
        buflen = ((*buf)[0] & 0x2) ? buflen - 1 : buflen + 1;
        broken = 1;
    }

    /* pass a null buffer if the third bit of the buffer is 1 */
    bufptr = ((*buf)[0] & 0x4) ? NULL : pubbuf;
    if (!broken)
        broken = (bufptr == NULL) ? 1 : 0;

    params[0] = OSSL_PARAM_construct_octet_string(OSSL_PKEY_PARAM_PUB_KEY,
                                                  (char *)bufptr, buflen);

    buflen = keysize * 2;
    /* select an invalid length if the 4th bit is true  */
    if ((*buf)[0] & 0x8) {
        buflen = (*buf[0] & 0x1) ? buflen - 1 : buflen + 1;
        broken = 1;
    }

    /* pass a null buffer if the 5th bit is true */
    bufptr = ((*buf)[0] & 0x10) ? NULL : prvbuf;
    if (!broken)
        broken = (bufptr == NULL) ? 1 : 0;
    params[1] = OSSL_PARAM_construct_octet_string(OSSL_PKEY_PARAM_PRIV_KEY,
                                                  (char *)bufptr, buflen);

    params[2] = OSSL_PARAM_construct_end();

    *out1 = (void *)slh_dsa_gen_key(keytype, keysize, params, &broken);

    if (broken)
        OPENSSL_assert(*out1 == NULL);
    else
        OPENSSL_assert(*out1 != NULL);
    return;
}

/**
 * @brief Frees allocated SLH-DSA key structures.
 *
 * This function releases memory allocated for SLH-DSA key pairs
 * by freeing the provided EVP_PKEY structures.
 *
 * @param in1 Pointer to the first input key to be freed.
 * @param in2 Pointer to the second input key to be freed.
 * @param out1 Pointer to the first output key to be freed.
 * @param out2 Pointer to the second output key to be freed.
 */
static void slh_dsa_clean_keys(void *in1, void *in2, void *out1, void *out2)
{
    EVP_PKEY_free((EVP_PKEY *)in1);
    EVP_PKEY_free((EVP_PKEY *)in2);
    EVP_PKEY_free((EVP_PKEY *)out1);
    EVP_PKEY_free((EVP_PKEY *)out2);
}

/**
 * @brief Performs SLH-DSA signing and verification on a given message.
 *
 * This function generates an SLH-DSA key, signs a message, and verifies
 * the generated signature. It extracts necessary parameters from the buffer
 * to determine signing options.
 *
 * @param buf Pointer to a buffer containing the selector and message data.
 *            The buffer pointer is updated as values are consumed.
 * @param len Pointer to the remaining buffer length, updated as values
 *            are consumed.
 * @param key1 Unused key parameter (placeholder for function signature consistency).
 * @param key2 Unused key parameter (placeholder for function signature consistency).
 * @param out1 Pointer to store the generated key (for cleanup purposes).
 * @param out2 Unused output parameter (placeholder for consistency).
 */
static void slh_dsa_sign_verify(uint8_t **buf, size_t *len, void *key1,
                                void *key2, void **out1, void **out2)
{
    EVP_PKEY_CTX *ctx = NULL;
    EVP_PKEY *key = NULL;
    EVP_SIGNATURE *sig_alg = NULL;
    const char *keytype;
    uint32_t keylen;
    uint8_t selector = 0;
    unsigned char *msg = NULL;
    size_t msg_len;
    size_t sig_len;
    unsigned char *sig = NULL;
    OSSL_PARAM params[4];
    int paramidx = 0;
    int intval1, intval2;
    int expect_init_rc = 1;

    *buf = consume_uint8t(*buf, len, &selector);
    if (*buf == NULL)
        return;

    keytype = select_keytype(selector, &keylen);

    /*
     * Consume another byte to figure out our params
     */
    *buf = consume_uint8t(*buf, len, &selector);
    if (*buf == NULL)
        return;

    /*
     * Remainder of the buffer is the msg to sign
     */
    msg = (unsigned char *)*buf;
    msg_len = *len;

    /* if msg_len > 255, sign_message_init will fail */
    if (msg_len > 255 && (selector & 0x1) != 0)
        expect_init_rc = 0;

    *len = 0;

    if (selector & 0x1)
        params[paramidx++] = OSSL_PARAM_construct_octet_string(OSSL_SIGNATURE_PARAM_CONTEXT_STRING,
                                                               msg, msg_len);

    if (selector & 0x2) {
        intval1 = selector & 0x4;
        params[paramidx++] = OSSL_PARAM_construct_int(OSSL_SIGNATURE_PARAM_MESSAGE_ENCODING,
                                                      &intval1);
    }

    if (selector & 0x8) {
        intval2 = selector & 0x10;
        params[paramidx++] = OSSL_PARAM_construct_int(OSSL_SIGNATURE_PARAM_DETERMINISTIC,
                                                      &intval2);
    }

    params[paramidx] = OSSL_PARAM_construct_end();

    key = (void *)slh_dsa_gen_key(keytype, keylen, NULL, 0);
    OPENSSL_assert(key != NULL);
    *out1 = key; /* for cleanup */

    ctx = EVP_PKEY_CTX_new_from_pkey(NULL, key, NULL);
    OPENSSL_assert(ctx != NULL);

    sig_alg = EVP_SIGNATURE_fetch(NULL, keytype, NULL);
    OPENSSL_assert(sig_alg != NULL);

    OPENSSL_assert(EVP_PKEY_sign_message_init(ctx, sig_alg, params) == expect_init_rc);
    /*
     * the context_string parameter can be no more than 255 bytes, so if
     * our random input buffer is greater than that, we expect failure above,
     * which we check for.  In that event, theres nothing more we can do here
     * so bail out
     */
    if (expect_init_rc == 0)
        goto out;

    OPENSSL_assert(EVP_PKEY_sign(ctx, NULL, &sig_len, msg, msg_len));
    sig = OPENSSL_zalloc(sig_len);
    OPENSSL_assert(sig != NULL);

    OPENSSL_assert(EVP_PKEY_sign(ctx, sig, &sig_len, msg, msg_len));

    OPENSSL_assert(EVP_PKEY_verify_message_init(ctx, sig_alg, params));
    OPENSSL_assert(EVP_PKEY_verify(ctx, sig, sig_len, msg, msg_len));

out:
    OPENSSL_free(sig);
    EVP_SIGNATURE_free(sig_alg);
    EVP_PKEY_CTX_free(ctx);
}

/**
 * @brief Exports and imports SLH-DSA key pairs, verifying equivalence.
 *
 * This function extracts key data from two given SLH-DSA keys (`alice` and `bob`),
 * reconstructs new keys from the extracted data, and verifies that the imported
 * keys are equivalent to the originals. It ensures that key export/import
 * functionality is working correctly.
 *
 * @param buf Unused buffer parameter (placeholder for function signature consistency).
 * @param len Unused length parameter (placeholder for function signature consistency).
 * @param key1 Pointer to the first key (`alice`) to be exported and imported.
 * @param key2 Pointer to the second key (`bob`) to be exported and imported.
 * @param out1 Unused output parameter (placeholder for consistency).
 * @param out2 Unused output parameter (placeholder for consistency).
 */
static void slh_dsa_export_import(uint8_t **buf, size_t *len, void *key1,
                                  void *key2, void **out1, void **out2)
{
    int rc;
    EVP_PKEY *alice = (EVP_PKEY *)key1;
    EVP_PKEY *bob = (EVP_PKEY *)key2;
    EVP_PKEY *new = NULL;
    EVP_PKEY_CTX *ctx = NULL;
    OSSL_PARAM *params = NULL;

    OPENSSL_assert(EVP_PKEY_todata(alice, EVP_PKEY_KEYPAIR, &params) == 1);

    ctx = EVP_PKEY_CTX_new_from_pkey(NULL, alice, NULL);
    OPENSSL_assert(ctx != NULL);

    OPENSSL_assert(EVP_PKEY_fromdata_init(ctx));

    new = EVP_PKEY_new();
    OPENSSL_assert(new != NULL);
    OPENSSL_assert(EVP_PKEY_fromdata(ctx, &new, EVP_PKEY_KEYPAIR, params) == 1);

    /*
     * EVP_PKEY returns:
     * 1 if the keys are equivalent
     * 0 if the keys are not equivalent
     * -1 if the key types are differnt
     * -2 if the operation is not supported
     */
    OPENSSL_assert(EVP_PKEY_eq(alice, new) == 1);
    EVP_PKEY_free(new);
    EVP_PKEY_CTX_free(ctx);
    OSSL_PARAM_free(params);
    params = NULL;
    ctx = NULL;
    new = NULL;

    OPENSSL_assert(EVP_PKEY_todata(bob, EVP_PKEY_KEYPAIR, &params) == 1);

    ctx = EVP_PKEY_CTX_new_from_pkey(NULL, bob, NULL);
    OPENSSL_assert(ctx != NULL);

    OPENSSL_assert(EVP_PKEY_fromdata_init(ctx));

    new = EVP_PKEY_new();
    OPENSSL_assert(new != NULL);
    OPENSSL_assert(EVP_PKEY_fromdata(ctx, &new, EVP_PKEY_KEYPAIR, params) == 1);

    OPENSSL_assert(EVP_PKEY_eq(bob, new) == 1);

    /*
     * Depending on the types of eys that get generated
     * we might get a simple non-equivalence or a type mismatch here
     */
    rc = EVP_PKEY_eq(alice, new);
    OPENSSL_assert(rc == 0 || rc == -1);

    EVP_PKEY_CTX_free(ctx);
    EVP_PKEY_free(new);
    OSSL_PARAM_free(params);
}

/**
 * @brief Represents an operation table entry for cryptographic operations.
 *
 * This structure defines a table entry containing function pointers for
 * setting up, executing, and cleaning up cryptographic operations, along
 * with associated metadata such as a name and description.
 *
 * @struct op_table_entry
 */
struct op_table_entry {
    /** Name of the operation. */
    char *name;

    /**
     * @brief Function pointer for setting up the operation.
     *
     * @param buf   Pointer to the buffer pointer; may be updated.
     * @param len   Pointer to the remaining buffer size; may be updated.
     * @param out1  Pointer to store the first output of the setup function.
     * @param out2  Pointer to store the second output of the setup function.
     */
    void (*setup)(uint8_t **buf, size_t *len, void **out1, void **out2);

    /**
     * @brief Function pointer for executing the operation.
     *
     * @param buf   Pointer to the buffer pointer; may be updated.
     * @param len   Pointer to the remaining buffer size; may be updated.
     * @param in1   First input parameter for the operation.
     * @param in2   Second input parameter for the operation.
     * @param out1  Pointer to store the first output of the operation.
     * @param out2  Pointer to store the second output of the operation.
     */
    void (*doit)(uint8_t **buf, size_t *len, void *in1, void *in2,
                 void **out1, void **out2);

    /**
     * @brief Function pointer for cleaning up after the operation.
     *
     * @param in1   First input parameter to be cleaned up.
     * @param in2   Second input parameter to be cleaned up.
     * @param out1  First output parameter to be cleaned up.
     * @param out2  Second output parameter to be cleaned up.
     */
    void (*cleanup)(void *in1, void *in2, void *out1, void *out2);
};

static struct op_table_entry ops[] = {
    {
        "Generate SLH-DSA keys",
        slh_dsa_gen_keys,
        NULL,
        slh_dsa_clean_keys
    }, {
        "Generate SLH-DSA keys with params",
        slh_dsa_gen_key_with_params,
        NULL,
        slh_dsa_clean_keys
    }, {
        "SLH-DSA Export/Import",
        slh_dsa_gen_keys,
        slh_dsa_export_import,
        slh_dsa_clean_keys
    }, {
        "SLH-DSA sign and verify",
        NULL,
        slh_dsa_sign_verify,
        slh_dsa_clean_keys
    }
};

int FuzzerInitialize(int *argc, char ***argv)
{
    return 0;
}

/**
 * @brief Processes a fuzzing input by selecting and executing an operation.
 *
 * This function interprets the first byte of the input buffer to determine
 * an operation to execute. It then follows a setup, execution, and cleanup
 * sequence based on the selected operation.
 *
 * @param buf Pointer to the input buffer.
 * @param len Length of the input buffer.
 *
 * @return 0 on successful execution, -1 if the input is too short.
 *
 * @note The function requires at least 32 bytes in the buffer to proceed.
 *       It utilizes the `ops` operation table to dynamically determine and
 *       execute the selected operation.
 */
int FuzzerTestOneInput(const uint8_t *buf, size_t len)
{
    uint8_t operation;
    uint8_t *buffer_cursor;
    void *in1 = NULL, *in2 = NULL;
    void *out1 = NULL, *out2 = NULL;

    if (len < 32)
        return -1;
    /*
     * Get the first byte of the buffer to tell us what operation
     * to preform
     */
    buffer_cursor = consume_uint8t(buf, &len, &operation);
    if (buffer_cursor == NULL)
        return -1;

    /*
     * Adjust for operational array size
     */
    operation %= OSSL_NELEM(ops);

    /*
     * And run our setup/doit/cleanup sequence
     */
    if (ops[operation].setup != NULL)
        ops[operation].setup(&buffer_cursor, &len, &in1, &in2);
    if (ops[operation].doit != NULL)
        ops[operation].doit(&buffer_cursor, &len, in1, in2, &out1, &out2);
    if (ops[operation].cleanup != NULL)
        ops[operation].cleanup(in1, in2, out1, out2);

    return 0;
}

void FuzzerCleanup(void)
{
    OPENSSL_cleanup();
}
