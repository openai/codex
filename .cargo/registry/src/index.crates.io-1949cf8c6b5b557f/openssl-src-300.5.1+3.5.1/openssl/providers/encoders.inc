/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef ENCODER_PROVIDER
# error Macro ENCODER_PROVIDER undefined
#endif

#define ENCODER_STRUCTURE_type_specific_keypair         "type-specific"
#define ENCODER_STRUCTURE_type_specific_params          "type-specific"
#define ENCODER_STRUCTURE_type_specific                 "type-specific"
#define ENCODER_STRUCTURE_type_specific_no_pub          "type-specific"
#define ENCODER_STRUCTURE_EncryptedPrivateKeyInfo       "EncryptedPrivateKeyInfo"
#define ENCODER_STRUCTURE_PrivateKeyInfo                "PrivateKeyInfo"
#define ENCODER_STRUCTURE_SubjectPublicKeyInfo          "SubjectPublicKeyInfo"
#define ENCODER_STRUCTURE_DH                            "dh"
#define ENCODER_STRUCTURE_DHX                           "dhx"
#define ENCODER_STRUCTURE_DSA                           "dsa"
#define ENCODER_STRUCTURE_EC                            "ec"
#define ENCODER_STRUCTURE_RSA                           "rsa"
#define ENCODER_STRUCTURE_PKCS1                         "pkcs1"
#define ENCODER_STRUCTURE_PKCS3                         "pkcs3"
#define ENCODER_STRUCTURE_X9_42                         "X9.42"
#define ENCODER_STRUCTURE_X9_62                         "X9.62"

/* Arguments are prefixed with '_' to avoid build breaks on certain platforms */
#define ENCODER_TEXT(_name, _sym, _fips)                                \
    { _name,                                                            \
      "provider=" ENCODER_PROVIDER ",fips=" #_fips ",output=text",      \
      (ossl_##_sym##_to_text_encoder_functions) }
#define ENCODER(_name, _sym, _fips, _output)                            \
    { _name,                                                            \
      "provider=" ENCODER_PROVIDER ",fips=" #_fips ",output=" #_output, \
      (ossl_##_sym##_to_##_output##_encoder_functions) }
#define ENCODER_w_structure(_name, _sym, _fips, _output, _structure)    \
    { _name,                                                            \
      "provider=" ENCODER_PROVIDER ",fips=" #_fips ",output=" #_output  \
      ",structure=" ENCODER_STRUCTURE_##_structure,                     \
      (ossl_##_sym##_to_##_structure##_##_output##_encoder_functions) }

/*
 * Entries for human text "encoders"
 */
ENCODER_TEXT("RSA", rsa, yes),
ENCODER_TEXT("RSA-PSS", rsapss, yes),
#ifndef OPENSSL_NO_DH
ENCODER_TEXT("DH", dh, yes),
ENCODER_TEXT("DHX", dhx, yes),
#endif
#ifndef OPENSSL_NO_DSA
ENCODER_TEXT("DSA", dsa, yes),
#endif
#ifndef OPENSSL_NO_EC
ENCODER_TEXT("EC", ec, yes),
# ifndef OPENSSL_NO_ECX
ENCODER_TEXT("ED25519", ed25519, yes),
ENCODER_TEXT("ED448", ed448, yes),
ENCODER_TEXT("X25519", x25519, yes),
ENCODER_TEXT("X448", x448, yes),
# endif
# ifndef OPENSSL_NO_SM2
ENCODER_TEXT("SM2", sm2, no),
# endif
#endif
#ifndef OPENSSL_NO_ML_KEM
ENCODER_TEXT("ML-KEM-512", ml_kem_512, yes),
ENCODER_TEXT("ML-KEM-768", ml_kem_768, yes),
ENCODER_TEXT("ML-KEM-1024", ml_kem_1024, yes),
#endif
#ifndef OPENSSL_NO_ML_DSA
ENCODER_TEXT("ML-DSA-44", ml_dsa_44, yes),
ENCODER_TEXT("ML-DSA-65", ml_dsa_65, yes),
ENCODER_TEXT("ML-DSA-87", ml_dsa_87, yes),
#endif
#ifndef OPENSSL_NO_SLH_DSA
ENCODER_TEXT("SLH-DSA-SHA2-128s", slh_dsa_sha2_128s, yes),
ENCODER_TEXT("SLH-DSA-SHA2-128f", slh_dsa_sha2_128f, yes),
ENCODER_TEXT("SLH-DSA-SHA2-192s", slh_dsa_sha2_192s, yes),
ENCODER_TEXT("SLH-DSA-SHA2-192f", slh_dsa_sha2_192f, yes),
ENCODER_TEXT("SLH-DSA-SHA2-256s", slh_dsa_sha2_256s, yes),
ENCODER_TEXT("SLH-DSA-SHA2-256f", slh_dsa_sha2_256f, yes),
ENCODER_TEXT("SLH-DSA-SHAKE-128s", slh_dsa_shake_128s, yes),
ENCODER_TEXT("SLH-DSA-SHAKE-128f", slh_dsa_shake_128f, yes),
ENCODER_TEXT("SLH-DSA-SHAKE-192s", slh_dsa_shake_192s, yes),
ENCODER_TEXT("SLH-DSA-SHAKE-192f", slh_dsa_shake_192f, yes),
ENCODER_TEXT("SLH-DSA-SHAKE-256s", slh_dsa_shake_256s, yes),
ENCODER_TEXT("SLH-DSA-SHAKE-256f", slh_dsa_shake_256f, yes),
#endif

/*
 * Entries for key type specific output formats.  The structure name on these
 * is the same as the key type name.  This allows us to say something like:
 *
 * To replace i2d_{TYPE}PrivateKey(), i2d_{TYPE}PublicKey() and
 * i2d_{TYPE}Params(), use OSSL_ENCODER functions with an OSSL_ENCODER_CTX
 * created like this:
 *
 * OSSL_ENCODER_CTX *ctx =
 *     OSSL_ENCODER_CTX_new_for_pkey(pkey, selection, "DER", "type-specific",
 *                                   NULL, NULL);
 *
 * To replace PEM_write_bio_{TYPE}PrivateKey(), PEM_write_bio_{TYPE}PublicKey()
 * and PEM_write_bio_{TYPE}Params(), use OSSL_ENCODER functions with an
 * OSSL_ENCODER_CTX created like this:
 *
 * OSSL_ENCODER_CTX *ctx =
 *     OSSL_ENCODER_CTX_new_for_pkey(pkey, selection, "PEM", "type-specific",
 *                                   NULL, NULL);
 *
 * We only implement those for which there are current i2d_ and PEM_write_bio
 * implementations.
 */

/* The RSA encoders only support private key and public key output */
ENCODER_w_structure("RSA", rsa, yes, der, type_specific_keypair),
ENCODER_w_structure("RSA", rsa, yes, pem, type_specific_keypair),
#ifndef OPENSSL_NO_DH
/* DH and X9.42 DH only support key parameters output. */
ENCODER_w_structure("DH", dh, yes, der, type_specific_params),
ENCODER_w_structure("DH", dh, yes, pem, type_specific_params),
ENCODER_w_structure("DHX", dhx, yes, der, type_specific_params),
ENCODER_w_structure("DHX", dhx, yes, pem, type_specific_params),
#endif
#ifndef OPENSSL_NO_DSA
ENCODER_w_structure("DSA", dsa, yes, der, type_specific),
ENCODER_w_structure("DSA", dsa, yes, pem, type_specific),
#endif
#ifndef OPENSSL_NO_EC
/* EC only supports keypair and parameters DER and PEM output. */
ENCODER_w_structure("EC", ec, yes, der, type_specific_no_pub),
ENCODER_w_structure("EC", ec, yes, pem, type_specific_no_pub),
/* EC supports blob output for the public key */
ENCODER("EC", ec, yes, blob),
# ifndef OPENSSL_NO_SM2
ENCODER_w_structure("SM2", sm2, no, der, type_specific_no_pub),
ENCODER_w_structure("SM2", sm2, no, pem, type_specific_no_pub),
ENCODER("SM2", sm2, no, blob),
# endif
#endif

/*
 * Entries for the output formats MSBLOB and PVK
 */
ENCODER("RSA", rsa, yes, msblob),
ENCODER("RSA", rsa, yes, pvk),
#ifndef OPENSSL_NO_DSA
ENCODER("DSA", dsa, yes, msblob),
ENCODER("DSA", dsa, yes, pvk),
#endif

/*
 * Entries for encrypted PKCS#8 (EncryptedPrivateKeyInfo), unencrypted PKCS#8
 * (PrivateKeyInfo) and SubjectPublicKeyInfo.  The "der" ones are added
 * convenience for any user that wants to use OSSL_ENCODER directly.
 * The "pem" ones also support PEM_write_bio_PrivateKey() and
 * PEM_write_bio_PUBKEY().
 */
ENCODER_w_structure("RSA", rsa, yes, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure("RSA", rsa, yes, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure("RSA", rsa, yes, der, PrivateKeyInfo),
ENCODER_w_structure("RSA", rsa, yes, pem, PrivateKeyInfo),
ENCODER_w_structure("RSA", rsa, yes, der, SubjectPublicKeyInfo),
ENCODER_w_structure("RSA", rsa, yes, pem, SubjectPublicKeyInfo),

ENCODER_w_structure("RSA-PSS", rsapss, yes, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure("RSA-PSS", rsapss, yes, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure("RSA-PSS", rsapss, yes, der, PrivateKeyInfo),
ENCODER_w_structure("RSA-PSS", rsapss, yes, pem, PrivateKeyInfo),
ENCODER_w_structure("RSA-PSS", rsapss, yes, der, SubjectPublicKeyInfo),
ENCODER_w_structure("RSA-PSS", rsapss, yes, pem, SubjectPublicKeyInfo),

#ifndef OPENSSL_NO_DH
ENCODER_w_structure("DH", dh, yes, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure("DH", dh, yes, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure("DH", dh, yes, der, PrivateKeyInfo),
ENCODER_w_structure("DH", dh, yes, pem, PrivateKeyInfo),
ENCODER_w_structure("DH", dh, yes, der, SubjectPublicKeyInfo),
ENCODER_w_structure("DH", dh, yes, pem, SubjectPublicKeyInfo),

ENCODER_w_structure("DHX", dhx, yes, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure("DHX", dhx, yes, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure("DHX", dhx, yes, der, PrivateKeyInfo),
ENCODER_w_structure("DHX", dhx, yes, pem, PrivateKeyInfo),
ENCODER_w_structure("DHX", dhx, yes, der, SubjectPublicKeyInfo),
ENCODER_w_structure("DHX", dhx, yes, pem, SubjectPublicKeyInfo),
#endif

#ifndef OPENSSL_NO_DSA
ENCODER_w_structure("DSA", dsa, yes, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure("DSA", dsa, yes, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure("DSA", dsa, yes, der, PrivateKeyInfo),
ENCODER_w_structure("DSA", dsa, yes, pem, PrivateKeyInfo),
ENCODER_w_structure("DSA", dsa, yes, der, SubjectPublicKeyInfo),
ENCODER_w_structure("DSA", dsa, yes, pem, SubjectPublicKeyInfo),
#endif

#ifndef OPENSSL_NO_EC
ENCODER_w_structure("EC", ec, yes, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure("EC", ec, yes, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure("EC", ec, yes, der, PrivateKeyInfo),
ENCODER_w_structure("EC", ec, yes, pem, PrivateKeyInfo),
ENCODER_w_structure("EC", ec, yes, der, SubjectPublicKeyInfo),
ENCODER_w_structure("EC", ec, yes, pem, SubjectPublicKeyInfo),

# ifndef OPENSSL_NO_SM2
ENCODER_w_structure("SM2", sm2, no, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure("SM2", sm2, no, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure("SM2", sm2, no, der, PrivateKeyInfo),
ENCODER_w_structure("SM2", sm2, no, pem, PrivateKeyInfo),
ENCODER_w_structure("SM2", sm2, no, der, SubjectPublicKeyInfo),
ENCODER_w_structure("SM2", sm2, no, pem, SubjectPublicKeyInfo),
# endif
#endif

#ifndef OPENSSL_NO_ECX
ENCODER_w_structure("X25519", x25519, yes, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure("X25519", x25519, yes, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure("X25519", x25519, yes, der, PrivateKeyInfo),
ENCODER_w_structure("X25519", x25519, yes, pem, PrivateKeyInfo),
ENCODER_w_structure("X25519", x25519, yes, der, SubjectPublicKeyInfo),
ENCODER_w_structure("X25519", x25519, yes, pem, SubjectPublicKeyInfo),

ENCODER_w_structure("X448", x448, yes, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure("X448", x448, yes, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure("X448", x448, yes, der, PrivateKeyInfo),
ENCODER_w_structure("X448", x448, yes, pem, PrivateKeyInfo),
ENCODER_w_structure("X448", x448, yes, der, SubjectPublicKeyInfo),
ENCODER_w_structure("X448", x448, yes, pem, SubjectPublicKeyInfo),

ENCODER_w_structure("ED25519", ed25519, yes, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure("ED25519", ed25519, yes, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure("ED25519", ed25519, yes, der, PrivateKeyInfo),
ENCODER_w_structure("ED25519", ed25519, yes, pem, PrivateKeyInfo),
ENCODER_w_structure("ED25519", ed25519, yes, der, SubjectPublicKeyInfo),
ENCODER_w_structure("ED25519", ed25519, yes, pem, SubjectPublicKeyInfo),

ENCODER_w_structure("ED448", ed448, yes, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure("ED448", ed448, yes, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure("ED448", ed448, yes, der, PrivateKeyInfo),
ENCODER_w_structure("ED448", ed448, yes, pem, PrivateKeyInfo),
ENCODER_w_structure("ED448", ed448, yes, der, SubjectPublicKeyInfo),
ENCODER_w_structure("ED448", ed448, yes, pem, SubjectPublicKeyInfo),
#endif
#ifndef OPENSSL_NO_SLH_DSA
ENCODER_w_structure( "SLH-DSA-SHA2-128s", slh_dsa_sha2_128s, yes, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-128f", slh_dsa_sha2_128f, yes, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-192s", slh_dsa_sha2_192s, yes, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-192f", slh_dsa_sha2_192f, yes, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-256s", slh_dsa_sha2_256s, yes, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-256f", slh_dsa_sha2_256f, yes, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-128s", slh_dsa_shake_128s, yes, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-128f", slh_dsa_shake_128f, yes, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-192s", slh_dsa_shake_192s, yes, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-192f", slh_dsa_shake_192f, yes, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-256s", slh_dsa_shake_256s, yes, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-256f", slh_dsa_shake_256f, yes, der, EncryptedPrivateKeyInfo),

ENCODER_w_structure( "SLH-DSA-SHA2-128s", slh_dsa_sha2_128s, yes, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-128f", slh_dsa_sha2_128f, yes, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-192s", slh_dsa_sha2_192s, yes, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-192f", slh_dsa_sha2_192f, yes, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-256s", slh_dsa_sha2_256s, yes, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-256f", slh_dsa_sha2_256f, yes, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-128s", slh_dsa_shake_128s, yes, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-128f", slh_dsa_shake_128f, yes, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-192s", slh_dsa_shake_192s, yes, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-192f", slh_dsa_shake_192f, yes, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-256s", slh_dsa_shake_256s, yes, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-256f", slh_dsa_shake_256f, yes, pem, EncryptedPrivateKeyInfo),

ENCODER_w_structure( "SLH-DSA-SHA2-128s", slh_dsa_sha2_128s, yes, der, PrivateKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-128f", slh_dsa_sha2_128f, yes, der, PrivateKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-192s", slh_dsa_sha2_192s, yes, der, PrivateKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-192f", slh_dsa_sha2_192f, yes, der, PrivateKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-256s", slh_dsa_sha2_256s, yes, der, PrivateKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-256f", slh_dsa_sha2_256f, yes, der, PrivateKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-128s", slh_dsa_shake_128s, yes, der, PrivateKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-128f", slh_dsa_shake_128f, yes, der, PrivateKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-192s", slh_dsa_shake_192s, yes, der, PrivateKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-192f", slh_dsa_shake_192f, yes, der, PrivateKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-256s", slh_dsa_shake_256s, yes, der, PrivateKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-256f", slh_dsa_shake_256f, yes, der, PrivateKeyInfo),

ENCODER_w_structure( "SLH-DSA-SHA2-128s", slh_dsa_sha2_128s, yes, pem, PrivateKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-128f", slh_dsa_sha2_128f, yes, pem, PrivateKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-192s", slh_dsa_sha2_192s, yes, pem, PrivateKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-192f", slh_dsa_sha2_192f, yes, pem, PrivateKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-256s", slh_dsa_sha2_256s, yes, pem, PrivateKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-256f", slh_dsa_sha2_256f, yes, pem, PrivateKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-128s", slh_dsa_shake_128s, yes, pem, PrivateKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-128f", slh_dsa_shake_128f, yes, pem, PrivateKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-192s", slh_dsa_shake_192s, yes, pem, PrivateKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-192f", slh_dsa_shake_192f, yes, pem, PrivateKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-256s", slh_dsa_shake_256s, yes, pem, PrivateKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-256f", slh_dsa_shake_256f, yes, pem, PrivateKeyInfo),

ENCODER_w_structure( "SLH-DSA-SHA2-128s", slh_dsa_sha2_128s, yes, der, SubjectPublicKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-128f", slh_dsa_sha2_128f, yes, der, SubjectPublicKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-192s", slh_dsa_sha2_192s, yes, der, SubjectPublicKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-192f", slh_dsa_sha2_192f, yes, der, SubjectPublicKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-256s", slh_dsa_sha2_256s, yes, der, SubjectPublicKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-256f", slh_dsa_sha2_256f, yes, der, SubjectPublicKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-128s", slh_dsa_shake_128s, yes, der, SubjectPublicKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-128f", slh_dsa_shake_128f, yes, der, SubjectPublicKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-192s", slh_dsa_shake_192s, yes, der, SubjectPublicKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-192f", slh_dsa_shake_192f, yes, der, SubjectPublicKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-256s", slh_dsa_shake_256s, yes, der, SubjectPublicKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-256f", slh_dsa_shake_256f, yes, der, SubjectPublicKeyInfo),

ENCODER_w_structure( "SLH-DSA-SHA2-128s", slh_dsa_sha2_128s, yes, pem, SubjectPublicKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-128f", slh_dsa_sha2_128f, yes, pem, SubjectPublicKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-192s", slh_dsa_sha2_192s, yes, pem, SubjectPublicKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-192f", slh_dsa_sha2_192f, yes, pem, SubjectPublicKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-256s", slh_dsa_sha2_256s, yes, pem, SubjectPublicKeyInfo),
ENCODER_w_structure( "SLH-DSA-SHA2-256f", slh_dsa_sha2_256f, yes, pem, SubjectPublicKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-128s", slh_dsa_shake_128s, yes, pem, SubjectPublicKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-128f", slh_dsa_shake_128f, yes, pem, SubjectPublicKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-192s", slh_dsa_shake_192s, yes, pem, SubjectPublicKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-192f", slh_dsa_shake_192f, yes, pem, SubjectPublicKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-256s", slh_dsa_shake_256s, yes, pem, SubjectPublicKeyInfo),
ENCODER_w_structure("SLH-DSA-SHAKE-256f", slh_dsa_shake_256f, yes, pem, SubjectPublicKeyInfo),
#endif

#ifndef OPENSSL_NO_ML_KEM
ENCODER_w_structure("ML-KEM-512", ml_kem_512, yes, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure("ML-KEM-512", ml_kem_512, yes, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure("ML-KEM-512", ml_kem_512, yes, der, PrivateKeyInfo),
ENCODER_w_structure("ML-KEM-512", ml_kem_512, yes, pem, PrivateKeyInfo),
ENCODER_w_structure("ML-KEM-512", ml_kem_512, yes, der, SubjectPublicKeyInfo),
ENCODER_w_structure("ML-KEM-512", ml_kem_512, yes, pem, SubjectPublicKeyInfo),

ENCODER_w_structure("ML-KEM-768", ml_kem_768, yes, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure("ML-KEM-768", ml_kem_768, yes, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure("ML-KEM-768", ml_kem_768, yes, der, PrivateKeyInfo),
ENCODER_w_structure("ML-KEM-768", ml_kem_768, yes, pem, PrivateKeyInfo),
ENCODER_w_structure("ML-KEM-768", ml_kem_768, yes, der, SubjectPublicKeyInfo),
ENCODER_w_structure("ML-KEM-768", ml_kem_768, yes, pem, SubjectPublicKeyInfo),

ENCODER_w_structure("ML-KEM-1024", ml_kem_1024, yes, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure("ML-KEM-1024", ml_kem_1024, yes, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure("ML-KEM-1024", ml_kem_1024, yes, der, PrivateKeyInfo),
ENCODER_w_structure("ML-KEM-1024", ml_kem_1024, yes, pem, PrivateKeyInfo),
ENCODER_w_structure("ML-KEM-1024", ml_kem_1024, yes, der, SubjectPublicKeyInfo),
ENCODER_w_structure("ML-KEM-1024", ml_kem_1024, yes, pem, SubjectPublicKeyInfo),
#endif

# ifndef OPENSSL_NO_ML_DSA
ENCODER_w_structure("ML-DSA-44", ml_dsa_44, yes, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure("ML-DSA-44", ml_dsa_44, yes, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure("ML-DSA-44", ml_dsa_44, yes, der, PrivateKeyInfo),
ENCODER_w_structure("ML-DSA-44", ml_dsa_44, yes, pem, PrivateKeyInfo),
ENCODER_w_structure("ML-DSA-44", ml_dsa_44, yes, der, SubjectPublicKeyInfo),
ENCODER_w_structure("ML-DSA-44", ml_dsa_44, yes, pem, SubjectPublicKeyInfo),

ENCODER_w_structure("ML-DSA-65", ml_dsa_65, yes, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure("ML-DSA-65", ml_dsa_65, yes, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure("ML-DSA-65", ml_dsa_65, yes, der, PrivateKeyInfo),
ENCODER_w_structure("ML-DSA-65", ml_dsa_65, yes, pem, PrivateKeyInfo),
ENCODER_w_structure("ML-DSA-65", ml_dsa_65, yes, der, SubjectPublicKeyInfo),
ENCODER_w_structure("ML-DSA-65", ml_dsa_65, yes, pem, SubjectPublicKeyInfo),

ENCODER_w_structure("ML-DSA-87", ml_dsa_87, yes, der, EncryptedPrivateKeyInfo),
ENCODER_w_structure("ML-DSA-87", ml_dsa_87, yes, pem, EncryptedPrivateKeyInfo),
ENCODER_w_structure("ML-DSA-87", ml_dsa_87, yes, der, PrivateKeyInfo),
ENCODER_w_structure("ML-DSA-87", ml_dsa_87, yes, pem, PrivateKeyInfo),
ENCODER_w_structure("ML-DSA-87", ml_dsa_87, yes, der, SubjectPublicKeyInfo),
ENCODER_w_structure("ML-DSA-87", ml_dsa_87, yes, pem, SubjectPublicKeyInfo),
# endif /* OPENSSL_NO_ML_DSA */

/*
 * Entries for key type specific output formats.  These are exactly the
 * same as the type specific above, except that they use the key type
 * name as structure name instead of "type-specific", in the call on
 * OSSL_ENCODER_CTX_new_for_pkey().
 */

/* The RSA encoders only support private key and public key output */
ENCODER_w_structure("RSA", rsa, yes, der, RSA),
ENCODER_w_structure("RSA", rsa, yes, pem, RSA),
#ifndef OPENSSL_NO_DH
/* DH and X9.42 DH only support key parameters output. */
ENCODER_w_structure("DH", dh, yes, der, DH),
ENCODER_w_structure("DH", dh, yes, pem, DH),
ENCODER_w_structure("DHX", dhx, yes, der, DHX),
ENCODER_w_structure("DHX", dhx, yes, pem, DHX),
#endif
#ifndef OPENSSL_NO_DSA
ENCODER_w_structure("DSA", dsa, yes, der, DSA),
ENCODER_w_structure("DSA", dsa, yes, pem, DSA),
#endif
#ifndef OPENSSL_NO_EC
ENCODER_w_structure("EC", ec, yes, der, EC),
ENCODER_w_structure("EC", ec, yes, pem, EC),
#endif

/*
 * Additional entries with structure names being the standard name.
 * This is entirely for the convenience of the user that wants to use
 * OSSL_ENCODER directly with names they may fancy.  These do not impact
 * on libcrypto functionality in any way.
 */
/* PKCS#1 is a well known for plain RSA keys, so we add that too */
ENCODER_w_structure("RSA", rsa, yes, der, PKCS1),
ENCODER_w_structure("RSA", rsa, yes, pem, PKCS1),
ENCODER_w_structure("RSA-PSS", rsapss, yes, der, PKCS1),
ENCODER_w_structure("RSA-PSS", rsapss, yes, pem, PKCS1),
#ifndef OPENSSL_NO_DH
/* PKCS#3 defines the format for DH parameters */
ENCODER_w_structure("DH", dh, yes, der, PKCS3),
ENCODER_w_structure("DH", dh, yes, pem, PKCS3),
/* X9.42 defines the format for DHX parameters */
ENCODER_w_structure("DHX", dhx, yes, der, X9_42),
ENCODER_w_structure("DHX", dhx, yes, pem, X9_42),
#endif
#ifndef OPENSSL_NO_EC
/* RFC 5915 defines the format for EC keys and parameters */
ENCODER_w_structure("EC", ec, yes, der, X9_62),
ENCODER_w_structure("EC", ec, yes, pem, X9_62),
#endif
