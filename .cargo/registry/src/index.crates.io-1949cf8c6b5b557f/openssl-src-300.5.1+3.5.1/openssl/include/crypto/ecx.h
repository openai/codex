/*
 * Copyright 2020-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Internal EC functions for other submodules: not for application use */

#ifndef OSSL_CRYPTO_ECX_H
# define OSSL_CRYPTO_ECX_H
# pragma once

# include <openssl/opensslconf.h>

# ifndef OPENSSL_NO_ECX

#  include <openssl/core.h>
#  include <openssl/e_os2.h>
#  include <openssl/crypto.h>
#  include "internal/refcount.h"
#  include "crypto/types.h"

#  define X25519_KEYLEN         32
#  define X448_KEYLEN           56
#  define ED25519_KEYLEN        32
#  define ED448_KEYLEN          57

#  define MAX_KEYLEN  ED448_KEYLEN

#  define X25519_BITS           253
#  define X25519_SECURITY_BITS  128

#  define X448_BITS             448
#  define X448_SECURITY_BITS    224

#  define ED25519_BITS          256
/* RFC8032 Section 8.5 */
#  define ED25519_SECURITY_BITS 128
#  define ED25519_SIGSIZE       64

#  define ED448_BITS            456
/* RFC8032 Section 8.5 */
#  define ED448_SECURITY_BITS   224
#  define ED448_SIGSIZE         114


typedef enum {
    ECX_KEY_TYPE_X25519,
    ECX_KEY_TYPE_X448,
    ECX_KEY_TYPE_ED25519,
    ECX_KEY_TYPE_ED448
} ECX_KEY_TYPE;

#define KEYTYPE2NID(type) \
    ((type) == ECX_KEY_TYPE_X25519 \
     ?  EVP_PKEY_X25519 \
     : ((type) == ECX_KEY_TYPE_X448 \
        ? EVP_PKEY_X448 \
        : ((type) == ECX_KEY_TYPE_ED25519 \
           ? EVP_PKEY_ED25519 \
           : EVP_PKEY_ED448)))

struct ecx_key_st {
    OSSL_LIB_CTX *libctx;
    char *propq;
    unsigned int haspubkey:1;
    unsigned char pubkey[MAX_KEYLEN];
    unsigned char *privkey;
    size_t keylen;
    ECX_KEY_TYPE type;
    CRYPTO_REF_COUNT references;
};

size_t ossl_ecx_key_length(ECX_KEY_TYPE type);
ECX_KEY *ossl_ecx_key_new(OSSL_LIB_CTX *libctx, ECX_KEY_TYPE type,
                          int haspubkey, const char *propq);
void ossl_ecx_key_set0_libctx(ECX_KEY *key, OSSL_LIB_CTX *libctx);
unsigned char *ossl_ecx_key_allocate_privkey(ECX_KEY *key);
void ossl_ecx_key_free(ECX_KEY *key);
int ossl_ecx_key_up_ref(ECX_KEY *key);
ECX_KEY *ossl_ecx_key_dup(const ECX_KEY *key, int selection);
int ossl_ecx_compute_key(ECX_KEY *peer, ECX_KEY *priv, size_t keylen,
                         unsigned char *secret, size_t *secretlen,
                         size_t outlen);

int ossl_x25519(uint8_t out_shared_key[32], const uint8_t private_key[32],
                const uint8_t peer_public_value[32]);
void ossl_x25519_public_from_private(uint8_t out_public_value[32],
                                     const uint8_t private_key[32]);

int
ossl_ed25519_public_from_private(OSSL_LIB_CTX *ctx, uint8_t out_public_key[32],
                                 const uint8_t private_key[32],
                                 const char *propq);
int
ossl_ed25519_sign(uint8_t *out_sig, const uint8_t *tbs, size_t tbs_len,
                  const uint8_t public_key[32], const uint8_t private_key[32],
                  const uint8_t dom2flag, const uint8_t phflag, const uint8_t csflag,
                  const uint8_t *context, size_t context_len,
                  OSSL_LIB_CTX *libctx, const char *propq);
int
ossl_ed25519_verify(const uint8_t *tbs, size_t tbs_len,
                    const uint8_t signature[64], const uint8_t public_key[32],
                    const uint8_t dom2flag, const uint8_t phflag, const uint8_t csflag,
                    const uint8_t *context, size_t context_len,
                    OSSL_LIB_CTX *libctx, const char *propq);
int
ossl_ed25519_pubkey_verify(const uint8_t *pub, size_t pub_len);
int
ossl_ed448_public_from_private(OSSL_LIB_CTX *ctx, uint8_t out_public_key[57],
                               const uint8_t private_key[57], const char *propq);
int
ossl_ed448_sign(OSSL_LIB_CTX *ctx, uint8_t *out_sig,
                const uint8_t *message, size_t message_len,
                const uint8_t public_key[57], const uint8_t private_key[57],
                const uint8_t *context, size_t context_len,
                const uint8_t phflag, const char *propq);

int
ossl_ed448_verify(OSSL_LIB_CTX *ctx,
                  const uint8_t *message, size_t message_len,
                  const uint8_t signature[114], const uint8_t public_key[57],
                  const uint8_t *context, size_t context_len,
                  const uint8_t phflag, const char *propq);

int
ossl_ed448_pubkey_verify(const uint8_t *pub, size_t pub_len);

int
ossl_x448(uint8_t out_shared_key[56], const uint8_t private_key[56],
          const uint8_t peer_public_value[56]);
void
ossl_x448_public_from_private(uint8_t out_public_value[56],
                              const uint8_t private_key[56]);


/* Backend support */
typedef enum {
    KEY_OP_PUBLIC,
    KEY_OP_PRIVATE,
    KEY_OP_KEYGEN
} ecx_key_op_t;

ECX_KEY *ossl_ecx_key_op(const X509_ALGOR *palg,
                         const unsigned char *p, int plen,
                         int pkey_id, ecx_key_op_t op,
                         OSSL_LIB_CTX *libctx, const char *propq);

int ossl_ecx_public_from_private(ECX_KEY *key);
int ossl_ecx_key_fromdata(ECX_KEY *ecx, const OSSL_PARAM params[],
                          int include_private);
ECX_KEY *ossl_ecx_key_from_pkcs8(const PKCS8_PRIV_KEY_INFO *p8inf,
                                 OSSL_LIB_CTX *libctx, const char *propq);

ECX_KEY *ossl_evp_pkey_get1_X25519(EVP_PKEY *pkey);
ECX_KEY *ossl_evp_pkey_get1_X448(EVP_PKEY *pkey);
ECX_KEY *ossl_evp_pkey_get1_ED25519(EVP_PKEY *pkey);
ECX_KEY *ossl_evp_pkey_get1_ED448(EVP_PKEY *pkey);
# endif /* OPENSSL_NO_ECX */
#endif
