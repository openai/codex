/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core_dispatch.h>
#include <openssl/pem.h>
#include <openssl/encoder.h>

/*
 * Selectors, named according to the ASN.1 names used throughout libcrypto.
 *
 * Note that these are not absolutely mandatory, they are rather a wishlist
 * of sorts.  The provider implementations are free to make choices that
 * make sense for them, based on these selectors.
 * For example, the EC backend is likely to really just output the private
 * key to a PKCS#8 structure, even thought PEM_SELECTION_PrivateKey specifies
 * the public key as well.  This is fine, as long as the corresponding
 * decoding operation can return an object that contains what libcrypto
 * expects.
 */
# define PEM_SELECTION_PUBKEY           EVP_PKEY_PUBLIC_KEY
# define PEM_SELECTION_PrivateKey       EVP_PKEY_KEYPAIR
# define PEM_SELECTION_Parameters       EVP_PKEY_KEY_PARAMETERS

/*
 * Properties, named according to the ASN.1 names used throughout libcrypto.
 */
# define PEM_STRUCTURE_PUBKEY "SubjectPublicKeyInfo"
# define PEM_STRUCTURE_PrivateKey "PrivateKeyInfo"
# define PEM_STRUCTURE_Parameters "type-specific"

# define PEM_STRUCTURE_RSAPrivateKey "type-specific"
# define PEM_STRUCTURE_RSAPublicKey "type-specific"

/* Alternative IMPLEMENT macros for provided encoders */

# define IMPLEMENT_PEM_provided_write_body_vars(type, asn1, pq)         \
    int ret = 0;                                                        \
    OSSL_ENCODER_CTX *ctx =                                             \
        OSSL_ENCODER_CTX_new_for_##type(x, PEM_SELECTION_##asn1,        \
                                       "PEM", PEM_STRUCTURE_##asn1,     \
                                       (pq));                           \
                                                                        \
    if (OSSL_ENCODER_CTX_get_num_encoders(ctx) == 0) {                  \
        OSSL_ENCODER_CTX_free(ctx);                                     \
        goto legacy;                                                    \
    }
# define IMPLEMENT_PEM_provided_write_body_pass()                       \
    ret = 1;                                                            \
    if (kstr == NULL && cb == NULL) {                                   \
        if (u != NULL) {                                                \
            kstr = u;                                                   \
            klen = strlen(u);                                           \
        } else {                                                        \
            cb = PEM_def_callback;                                      \
        }                                                               \
    }                                                                   \
    if (enc != NULL) {                                                  \
        ret = 0;                                                        \
        if (OSSL_ENCODER_CTX_set_cipher(ctx, EVP_CIPHER_get0_name(enc), \
                                        NULL)) {                        \
            ret = 1;                                                    \
            if (kstr != NULL                                            \
                && !OSSL_ENCODER_CTX_set_passphrase(ctx, kstr, klen))   \
                ret = 0;                                                \
            else if (cb != NULL                                         \
                     && !OSSL_ENCODER_CTX_set_pem_password_cb(ctx,      \
                                                              cb, u))   \
                ret = 0;                                                \
        }                                                               \
    }                                                                   \
    if (!ret) {                                                         \
        OSSL_ENCODER_CTX_free(ctx);                                     \
        return 0;                                                       \
    }
# define IMPLEMENT_PEM_provided_write_body_main(type, outtype)          \
    ret = OSSL_ENCODER_to_##outtype(ctx, out);                          \
    OSSL_ENCODER_CTX_free(ctx);                                         \
    return ret
# define IMPLEMENT_PEM_provided_write_body_fallback(str, asn1,          \
                                                    writename)          \
    legacy:                                                             \
    return PEM_ASN1_##writename((i2d_of_void *)i2d_##asn1, str, out,    \
                                x, NULL, NULL, 0, NULL, NULL)
# define IMPLEMENT_PEM_provided_write_body_fallback_cb(str, asn1,       \
                                                       writename)       \
    legacy:                                                             \
    return PEM_ASN1_##writename##((i2d_of_void *)i2d_##asn1, str, out,  \
                                  x, enc, kstr, klen, cb, u)

# define IMPLEMENT_PEM_provided_write_to(name, TYPE, type, str, asn1,   \
                                         OUTTYPE, outtype, writename)   \
    PEM_write_fnsig(name, TYPE, OUTTYPE, writename)                     \
    {                                                                   \
        IMPLEMENT_PEM_provided_write_body_vars(type, asn1, NULL);       \
        IMPLEMENT_PEM_provided_write_body_main(type, outtype);          \
        IMPLEMENT_PEM_provided_write_body_fallback(str, asn1,           \
                                                   writename);          \
    }                                                                   \
    PEM_write_ex_fnsig(name, TYPE, OUTTYPE, writename)                  \
    {                                                                   \
        IMPLEMENT_PEM_provided_write_body_vars(type, asn1, propq);      \
        IMPLEMENT_PEM_provided_write_body_main(type, outtype);          \
        IMPLEMENT_PEM_provided_write_body_fallback(str, asn1,           \
                                                   writename);          \
    }


# define IMPLEMENT_PEM_provided_write_cb_to(name, TYPE, type, str, asn1, \
                                            OUTTYPE, outtype, writename) \
    PEM_write_cb_fnsig(name, TYPE, OUTTYPE, writename)                  \
    {                                                                   \
        IMPLEMENT_PEM_provided_write_body_vars(type, asn1, NULL);       \
        IMPLEMENT_PEM_provided_write_body_pass();                       \
        IMPLEMENT_PEM_provided_write_body_main(type, outtype);          \
        IMPLEMENT_PEM_provided_write_body_fallback_cb(str, asn1,        \
                                                      writename);       \
    }                                                                   \
    PEM_write_ex_cb_fnsig(name, TYPE, OUTTYPE, writename)               \
    {                                                                   \
        IMPLEMENT_PEM_provided_write_body_vars(type, asn1, propq);      \
        IMPLEMENT_PEM_provided_write_body_pass();                       \
        IMPLEMENT_PEM_provided_write_body_main(type, outtype);          \
        IMPLEMENT_PEM_provided_write_body_fallback(str, asn1,           \
                                                   writename);          \
    }

# ifdef OPENSSL_NO_STDIO

#  define IMPLEMENT_PEM_provided_write_fp(name, TYPE, type, str, asn1)
#  define IMPLEMENT_PEM_provided_write_cb_fp(name, TYPE, type, str, asn1)

# else

#  define IMPLEMENT_PEM_provided_write_fp(name, TYPE, type, str, asn1)    \
    IMPLEMENT_PEM_provided_write_to(name, TYPE, type, str, asn1, FILE, fp, write)
#  define IMPLEMENT_PEM_provided_write_cb_fp(name, TYPE, type, str, asn1) \
    IMPLEMENT_PEM_provided_write_cb_to(name, TYPE, type, str, asn1, FILE, fp, write)

# endif

# define IMPLEMENT_PEM_provided_write_bio(name, TYPE, type, str, asn1)    \
    IMPLEMENT_PEM_provided_write_to(name, TYPE, type, str, asn1, BIO, bio, write_bio)
# define IMPLEMENT_PEM_provided_write_cb_bio(name, TYPE, type, str, asn1) \
    IMPLEMENT_PEM_provided_write_cb_to(name, TYPE, type, str, asn1, BIO, bio, write_bio)

# define IMPLEMENT_PEM_provided_write(name, TYPE, type, str, asn1)        \
    IMPLEMENT_PEM_provided_write_bio(name, TYPE, type, str, asn1)         \
    IMPLEMENT_PEM_provided_write_fp(name, TYPE, type, str, asn1)

# define IMPLEMENT_PEM_provided_write_cb(name, TYPE, type, str, asn1)     \
    IMPLEMENT_PEM_provided_write_cb_bio(name, TYPE, type, str, asn1)      \
    IMPLEMENT_PEM_provided_write_cb_fp(name, TYPE, type, str, asn1)

# define IMPLEMENT_PEM_provided_rw(name, TYPE, type, str, asn1)           \
    IMPLEMENT_PEM_read(name, TYPE, str, asn1)                             \
    IMPLEMENT_PEM_provided_write(name, TYPE, type, str, asn1)

# define IMPLEMENT_PEM_provided_rw_cb(name, TYPE, type, str, asn1)        \
    IMPLEMENT_PEM_read(name, TYPE, str, asn1)                             \
    IMPLEMENT_PEM_provided_write_cb(name, TYPE, type, str, asn1)

