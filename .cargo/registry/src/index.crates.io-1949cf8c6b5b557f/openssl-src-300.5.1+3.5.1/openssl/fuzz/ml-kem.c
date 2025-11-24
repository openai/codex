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
 * Test ml-kem operation.
 */
#include <string.h>
#include <openssl/evp.h>
#include <openssl/err.h>
#include <openssl/rand.h>
#include <openssl/byteorder.h>
#include <openssl/ml_kem.h>
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
 * @brief Selects a key type and size from a buffer.
 *
 * This function reads a key size value from the buffer, determines the
 * corresponding key type and length, and updates the buffer pointer
 * accordingly. If `only_valid` is set, it restricts selection to valid
 * key sizes; otherwise, it includes some invalid sizes for testing.
 *
 * @param buf       Pointer to the buffer pointer; updated after reading.
 * @param len       Pointer to the remaining buffer size; updated accordingly.
 * @param keytype   Pointer to store the selected key type string.
 * @param keylen    Pointer to store the selected key length.
 * @param only_valid Flag to restrict selection to valid key sizes.
 *
 * @return 1 if a key type is successfully selected, 0 on failure.
 */
static int select_keytype_and_size(uint8_t **buf, size_t *len,
                                   char **keytype, size_t *keylen,
                                   int only_valid)
{
    uint16_t keysize;
    uint16_t modulus = 6;

    /*
     * Note: We don't really care about endianess here, we just
     * want a random 16 bit value
     */
    *buf = (uint8_t *)OPENSSL_load_u16_le(&keysize, *buf);
    *len -= sizeof(uint16_t);

    if (*buf == NULL)
        return 0;

    /*
     * select from sizes
     * ML-KEM-512, ML-KEM-768, and ML-KEM-1024
     * also select some invalid sizes to trigger
     * error paths
     */
    if (only_valid)
        modulus = 3;

    /*
     * Note, keylens for valid values (cases 0-2)
     * are taken based on input values from our unit tests
     */
    switch (keysize % modulus) {
    case 0:
        *keytype = "ML-KEM-512";
        *keylen = OSSL_ML_KEM_512_PUBLIC_KEY_BYTES;
        break;
    case 1:
        *keytype = "ML-KEM-768";
        *keylen = OSSL_ML_KEM_768_PUBLIC_KEY_BYTES;
        break;
    case 2:
        *keytype = "ML-KEM-1024";
        *keylen = OSSL_ML_KEM_1024_PUBLIC_KEY_BYTES;
        break;
    case 3:
        /* select invalid alg */
        *keytype = "ML-KEM-13";
        *keylen = 13;
        break;
    case 4:
        /* Select valid alg, but bogus size */
        *keytype = "ML-KEM-1024";
        *buf = (uint8_t *)OPENSSL_load_u16_le(&keysize, *buf);
        *len -= sizeof(uint16_t);
        *keylen = (size_t)keysize;
        *keylen %= 1024; /* size to our key buffer */
        break;
    default:
        *keytype = NULL;
        *keylen = 0;
        break;
    }
    return 1;
}

/**
 * @brief Creates an ML-KEM raw key from a buffer.
 *
 * This function selects a key type and size from the buffer, generates
 * a random key of the appropriate length, and creates either a public
 * or private ML-KEM key using OpenSSL's EVP_PKEY interface.
 *
 * @param buf   Pointer to the buffer pointer; updated after reading.
 * @param len   Pointer to the remaining buffer size; updated accordingly.
 * @param key1  Pointer to store the generated EVP_PKEY key (public or private).
 * @param key2  Unused parameter (reserved for future use).
 *
 * @note The generated key is allocated using OpenSSL's EVP_PKEY functions
 *       and should be freed appropriately using `EVP_PKEY_free()`.
 */
static void create_mlkem_raw_key(uint8_t **buf, size_t *len,
                                 void **key1, void **key2)
{
    EVP_PKEY *pubkey;
    char *keytype = NULL;
    size_t keylen = 0;
    uint8_t key[4096];
    int pub = 0;

    if (!select_keytype_and_size(buf, len, &keytype, &keylen, 0))
        return;

    /*
     * Select public or private key creation based on the low order
     * bit of the next buffer value
     * Note that keylen as returned from select_keytype_and_size is
     * a public key length, private keys for ML-KEM are always double
     * the size plus 32, so make that adjustment here
     */
    if ((*buf)[0] & 0x1)
        pub = 1;
    else
        keylen = (keylen * 2) + 32;

    /*
     * libfuzzer provides by default up to 4096 bit input
     * buffers, but its typically much less (between 1 and 100 bytes)
     * so use RAND_bytes here instead
     */
    if (!RAND_bytes(key, keylen))
        return;

    /*
     * Try to generate either a raw public or private key using random data
     * Because the input is completely random, its effectively certain this
     * operation will fail, but it will still exercise the code paths below,
     * which is what we want the fuzzer to do
     */
    if (pub == 1)
        pubkey = EVP_PKEY_new_raw_public_key_ex(NULL, keytype, NULL, key, keylen);
    else
        pubkey = EVP_PKEY_new_raw_private_key_ex(NULL, keytype, NULL, key, keylen);

    *key1 = pubkey;
    return;
}

/**
 * @brief Generates a valid ML-KEM key using OpenSSL.
 *
 * This function selects a valid ML-KEM key type and size from the buffer,
 * initializes an OpenSSL EVP_PKEY context, and generates a cryptographic
 * key accordingly.
 *
 * @param buf    Pointer to the buffer pointer; updated after reading.
 * @param len    Pointer to the remaining buffer size; updated accordingly.
 * @param key1   Pointer to store the generated EVP_PKEY key.
 * @param unused Unused parameter (reserved for future use).
 *
 * @note The generated key is allocated using OpenSSL's EVP_PKEY functions
 *       and should be freed using `EVP_PKEY_free()`.
 */
static void keygen_mlkem_real_key(uint8_t **buf, size_t *len,
                                  void **key1, void **key2)
{
    char *keytype = NULL;
    size_t keylen = 0;
    EVP_PKEY_CTX *ctx = NULL;
    EVP_PKEY **key;

    *key1 = *key2 = NULL;

    key = (EVP_PKEY **)key1;

again:
    /*
     * Only generate valid key types and lengths
     * Note, no adjustment is made to keylen here, as
     * the provider is responsible for selecting the keys and sizes
     * for us during the EVP_PKEY_keygen call
     */
    if (!select_keytype_and_size(buf, len, &keytype, &keylen, 1))
        return;

    ctx = EVP_PKEY_CTX_new_from_name(NULL, keytype, NULL);
    if (!ctx) {
        fprintf(stderr, "Failed to generate ctx\n");
        return;
    }

    if (!EVP_PKEY_keygen_init(ctx)) {
        fprintf(stderr, "Failed to init keygen ctx\n");
        goto err;
    }

    *key = EVP_PKEY_new();
    if (*key == NULL)
        goto err;

    if (!EVP_PKEY_generate(ctx, key)) {
        fprintf(stderr, "Failed to generate new real key\n");
        goto err;
    }

    if (key == (EVP_PKEY **)key1) {
        EVP_PKEY_CTX_free(ctx);
        key = (EVP_PKEY **)key2;
        goto again;
    }

err:
    EVP_PKEY_CTX_free(ctx);
    return;
}

/**
 * @brief Performs key encapsulation and decapsulation using an EVP_PKEY.
 *
 * This function generates a random key, encapsulates it using the provided
 * public key, then decapsulates it to retrieve the original key. It makes
 * use of OpenSSL's EVP_PKEY API for encryption and decryption.
 *
 * @param[out] buf   Unused output buffer (reserved for future use).
 * @param[out] len   Unused length parameter (reserved for future use).
 * @param[in]  key1  Pointer to an EVP_PKEY structure used for key operations.
 * @param[in]  in2   Unused input parameter (reserved for future use).
 * @param[out] out1  Unused output parameter (reserved for future use).
 * @param[out] out2  Unused output parameter (reserved for future use).
 */
static void mlkem_encap_decap(uint8_t **buf, size_t *len, void *key1, void *in2,
                              void **out1, void **out2)
{
    EVP_PKEY *key = (EVP_PKEY *)key1;
    EVP_PKEY_CTX *ctx;
    unsigned char genkey[32];
    size_t genkey_len = 32;
    unsigned char unwrappedkey[32];
    size_t unwrappedkey_len = 32;
    unsigned char wrapkey[1568];
    size_t wrapkey_len = 1568;

    ctx = EVP_PKEY_CTX_new_from_pkey(NULL, key, NULL);
    if (ctx == NULL) {
        fprintf(stderr, "Failed to allocate ctx\n");
        goto err;
    }

    if (!EVP_PKEY_encapsulate_init(ctx, NULL)) {
        fprintf(stderr, "Failed to init encap context\n");
        goto err;
    }

    if (!RAND_bytes(genkey, genkey_len))
        goto err;

    if (EVP_PKEY_encapsulate(ctx, wrapkey, &wrapkey_len, genkey, &genkey_len) <= 0) {
        fprintf(stderr, "Failed to encapsulate key\n");
        goto err;
    }

    EVP_PKEY_CTX_free(ctx);
    ctx = EVP_PKEY_CTX_new_from_pkey(NULL, key, NULL);
    if (ctx == NULL) {
        fprintf(stderr, "Failed to create context\n");
        goto err;
    }

    if (!EVP_PKEY_decapsulate_init(ctx, NULL)) {
        fprintf(stderr, "Failed to init decap\n");
        goto err;
    }

    if (EVP_PKEY_decapsulate(ctx, unwrappedkey, &unwrappedkey_len,
                             wrapkey, wrapkey_len) <= 0) {
        fprintf(stderr, "Failed to decap key\n");
        goto err;
    }

    if (memcmp(unwrappedkey, genkey, genkey_len))
        fprintf(stderr, "mismatch on secret comparison\n");
err:
    EVP_PKEY_CTX_free(ctx);
    return;
}

/**
 * @brief Derives a shared secret using the provided key and peer key.
 *
 * This function performs a key derivation operation using the given
 * private key and peer public key. The resulting shared secret is
 * allocated dynamically and must be freed by the caller.
 *
 * @param[in] key The private key used for derivation.
 * @param[in] peer The peer's public key.
 * @param[out] shared Pointer to the derived shared secret (allocated).
 * @param[out] shared_len Length of the derived shared secret.
 *
 * @note The caller is responsible for freeing the memory allocated
 *       for `shared` using `OPENSSL_free()`.
 */
static void do_derive(EVP_PKEY *key, EVP_PKEY *peer, uint8_t **shared, size_t *shared_len)
{
    EVP_PKEY_CTX *ctx = NULL;

    *shared = NULL;
    *shared_len = 0;

    ctx = EVP_PKEY_CTX_new_from_pkey(NULL, key, NULL);
    if (ctx == NULL) {
        fprintf(stderr, "failed to create keygen context\n");
        goto err;
    }

    if (!EVP_PKEY_derive_init(ctx)) {
        fprintf(stderr, "failed to init derive context\n");
        goto err;
    }

    if (!EVP_PKEY_derive_set_peer(ctx, peer)) {
        fprintf(stderr, "failed to set peer\n");
        goto err;
    }

    if (!EVP_PKEY_derive(ctx, NULL, shared_len)) {
        fprintf(stderr, "Derive failed 1\n");
        goto err;
    }

    if (*shared_len == 0)
        goto err;

    *shared = OPENSSL_zalloc(*shared_len);
    if (*shared == NULL) {
        fprintf(stderr, "Failed to alloc\n");
        goto err;
    }
    if (!EVP_PKEY_derive(ctx, *shared, shared_len)) {
        fprintf(stderr, "Derive failed 2\n");
        OPENSSL_free(*shared);
        *shared = NULL;
        *shared_len = 0;
        goto err;
    }
err:
    EVP_PKEY_CTX_free(ctx);
}

/**
 * @brief Performs a key exchange using ML-KEM.
 *
 * This function derives shared secrets using the provided key pairs.
 * It calls `do_derive()` to compute shared secrets for both participants
 * and frees the allocated memory for the shared secrets.
 *
 * @param[out] buf Unused output buffer (reserved for future use).
 * @param[out] len Unused output length (reserved for future use).
 * @param[in] key1 First key (typically Alice's key).
 * @param[in] key2 Second key (typically Bob's key).
 * @param[out] out1 Unused output parameter (reserved for future use).
 * @param[out] out2 Unused output parameter (reserved for future use).
 *
 * @note Currently, this function does not validate whether the derived
 *       shared secrets match. A check should be added when ML-KEM
 *       supports this.
 */
static void mlkem_kex(uint8_t **buf, size_t *len, void *key1, void *key2,
                      void **out1, void **out2)
{
    EVP_PKEY *alice = (EVP_PKEY *)key1;
    EVP_PKEY *bob = (EVP_PKEY *)key2;
    size_t boblen, alicelen;
    uint8_t *bobshare = NULL;
    uint8_t *aliceshare = NULL;

    do_derive(alice, bob, &aliceshare, &alicelen);
    do_derive(bob, alice, &bobshare, &boblen);

    /*
     * TODO add check of shared secrets here when ML-KEM supports this
     */
    OPENSSL_free(bobshare);
    OPENSSL_free(aliceshare);
}

/**
 * @brief Exports and imports an ML-KEM key.
 *
 * This function extracts key material from the given key (`key1`),
 * exports it as parameters, and then attempts to reconstruct a new
 * key from those parameters. It uses OpenSSL's `EVP_PKEY_todata()`
 * and `EVP_PKEY_fromdata()` functions for this process.
 *
 * @param[out] buf Unused output buffer (reserved for future use).
 * @param[out] len Unused output length (reserved for future use).
 * @param[in] key1 The key to be exported and imported.
 * @param[in] key2 Unused input key (reserved for future use).
 * @param[out] out1 Unused output parameter (reserved for future use).
 * @param[out] out2 Unused output parameter (reserved for future use).
 *
 * @note If any step in the export-import process fails, the function
 *       logs an error and cleans up allocated resources.
 */
static void mlkem_export_import(uint8_t **buf, size_t *len, void *key1,
                                void *key2, void **out1, void **out2)
{
    EVP_PKEY *alice = (EVP_PKEY *)key1;
    EVP_PKEY *new = NULL;
    EVP_PKEY_CTX *ctx = NULL;
    OSSL_PARAM *params = NULL;

    if (!EVP_PKEY_todata(alice, EVP_PKEY_KEYPAIR, &params)) {
        fprintf(stderr, "Failed todata\n");
        goto err;
    }

    ctx = EVP_PKEY_CTX_new_from_pkey(NULL, alice, NULL);
    if (ctx == NULL) {
        fprintf(stderr, "Failed new ctx\n");
        goto err;
    }

    if (!EVP_PKEY_fromdata(ctx, &new, EVP_PKEY_KEYPAIR, params)) {
        fprintf(stderr, "Failed fromdata\n");
        goto err;
    }

err:
    EVP_PKEY_CTX_free(ctx);
    EVP_PKEY_free(new);
    OSSL_PARAM_free(params);
}

/**
 * @brief Compares two cryptographic keys and performs equality checks.
 *
 * This function takes in two cryptographic keys, casts them to `EVP_PKEY`
 * structures, and checks their equality using `EVP_PKEY_eq()`. The purpose
 * of `buf`, `len`, `out1`, and `out2` parameters is not clear from the
 * function's current implementation.
 *
 * @param buf   Unused parameter (purpose unclear).
 * @param len   Unused parameter (purpose unclear).
 * @param key1  First key, expected to be an `EVP_PKEY *`.
 * @param key2  Second key, expected to be an `EVP_PKEY *`.
 * @param out1  Unused parameter (purpose unclear).
 * @param out2  Unused parameter (purpose unclear).
 */
static void mlkem_compare(uint8_t **buf, size_t *len, void *key1,
                          void *key2, void **out1, void **out2)
{
    EVP_PKEY *alice = (EVP_PKEY *)key1;
    EVP_PKEY *bob = (EVP_PKEY *)key2;

    EVP_PKEY_eq(alice, alice);
    EVP_PKEY_eq(alice, bob);
}

/**
 * @brief Frees allocated ML-KEM keys.
 *
 * This function releases memory associated with up to four EVP_PKEY
 * objects by calling `EVP_PKEY_free()` on each provided key.
 *
 * @param key1 Pointer to the first key to be freed.
 * @param key2 Pointer to the second key to be freed.
 * @param key3 Pointer to the third key to be freed.
 * @param key4 Pointer to the fourth key to be freed.
 *
 * @note This function assumes that each key is either a valid EVP_PKEY
 *       object or NULL. Passing NULL is safe and has no effect.
 */
static void cleanup_mlkem_keys(void *key1, void *key2,
                               void *key3, void *key4)
{
    EVP_PKEY_free((EVP_PKEY *)key1);
    EVP_PKEY_free((EVP_PKEY *)key2);
    EVP_PKEY_free((EVP_PKEY *)key3);
    EVP_PKEY_free((EVP_PKEY *)key4);
    return;
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

    /** Description of the operation. */
    char *desc;

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
        "Generate ML-KEM raw key",
        "Try generate a raw keypair using random data. Usually fails",
        create_mlkem_raw_key,
        NULL,
        cleanup_mlkem_keys
    }, {
        "Generate ML-KEM keypair, using EVP_PKEY_keygen",
        "Generates a real ML-KEM keypair, should always work",
        keygen_mlkem_real_key,
        NULL,
        cleanup_mlkem_keys
    }, {
        "Do a key encap/decap operation on a key",
        "Generate key, encap it, decap it and compare, should work",
        keygen_mlkem_real_key,
        mlkem_encap_decap,
        cleanup_mlkem_keys
    }, {
        "Do a key exchange operation on two keys",
        "Gen keys, do a key exchange both ways and compare",
        keygen_mlkem_real_key,
        mlkem_kex,
        cleanup_mlkem_keys
    }, {
        "Do an export/import of key data",
        "Exercise EVP_PKEY_todata/fromdata",
        keygen_mlkem_real_key,
        mlkem_export_import,
        cleanup_mlkem_keys
    }, {
        "Compare keys for equality",
        "Compare key1/key1 and key1/key2 for equality",
        keygen_mlkem_real_key,
        mlkem_compare,
        cleanup_mlkem_keys
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
