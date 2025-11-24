#! /usr/bin/env perl
# Copyright 2023-2025 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

package OpenSSL::paramnames;

use strict;
use warnings;

require Exporter;
our @ISA = qw(Exporter);
our @EXPORT_OK = qw(generate_public_macros
                    generate_internal_macros
                    produce_decoder);

my $case_sensitive = 1;

my %params = (
# Well known parameter names that core passes to providers
    'PROV_PARAM_CORE_VERSION' =>         "openssl-version",# utf8_ptr
    'PROV_PARAM_CORE_PROV_NAME' =>       "provider-name",  # utf8_ptr
    'PROV_PARAM_CORE_MODULE_FILENAME' => "module-filename",# utf8_ptr

# Well known parameter names that Providers can define
    'PROV_PARAM_NAME' =>               "name",               # utf8_ptr
    'PROV_PARAM_VERSION' =>            "version",            # utf8_ptr
    'PROV_PARAM_BUILDINFO' =>          "buildinfo",          # utf8_ptr
    'PROV_PARAM_STATUS' =>             "status",             # uint
    'PROV_PARAM_SECURITY_CHECKS' =>    "security-checks",    # uint
    'PROV_PARAM_HMAC_KEY_CHECK' =>     "hmac-key-check",     # uint
    'PROV_PARAM_KMAC_KEY_CHECK' =>     "kmac-key-check",     # uint
    'PROV_PARAM_TLS1_PRF_EMS_CHECK' => "tls1-prf-ems-check", # uint
    'PROV_PARAM_NO_SHORT_MAC' =>       "no-short-mac",       # uint
    'PROV_PARAM_DRBG_TRUNC_DIGEST' =>  "drbg-no-trunc-md",   # uint
    'PROV_PARAM_HKDF_DIGEST_CHECK' =>      "hkdf-digest-check",      # uint
    'PROV_PARAM_TLS13_KDF_DIGEST_CHECK' => "tls13-kdf-digest-check", # uint
    'PROV_PARAM_TLS1_PRF_DIGEST_CHECK' =>  "tls1-prf-digest-check",  # uint
    'PROV_PARAM_SSHKDF_DIGEST_CHECK' =>    "sshkdf-digest-check",    # uint
    'PROV_PARAM_SSKDF_DIGEST_CHECK' =>     "sskdf-digest-check",     # uint
    'PROV_PARAM_X963KDF_DIGEST_CHECK' =>   "x963kdf-digest-check",   # uint
    'PROV_PARAM_DSA_SIGN_DISABLED' =>      "dsa-sign-disabled",      # uint
    'PROV_PARAM_TDES_ENCRYPT_DISABLED' =>  "tdes-encrypt-disabled",  # uint
    'PROV_PARAM_RSA_PSS_SALTLEN_CHECK' =>  "rsa-pss-saltlen-check",  # uint
    'PROV_PARAM_RSA_SIGN_X931_PAD_DISABLED' =>  "rsa-sign-x931-pad-disabled",   # uint
    'PROV_PARAM_RSA_PKCS15_PAD_DISABLED' => "rsa-pkcs15-pad-disabled", # uint
    'PROV_PARAM_HKDF_KEY_CHECK' =>         "hkdf-key-check",         # uint
    'PROV_PARAM_KBKDF_KEY_CHECK' =>        "kbkdf-key-check",        # uint
    'PROV_PARAM_TLS13_KDF_KEY_CHECK' =>    "tls13-kdf-key-check",    # uint
    'PROV_PARAM_TLS1_PRF_KEY_CHECK' =>     "tls1-prf-key-check",     # uint
    'PROV_PARAM_SSHKDF_KEY_CHECK' =>       "sshkdf-key-check",       # uint
    'PROV_PARAM_SSKDF_KEY_CHECK' =>        "sskdf-key-check",        # uint
    'PROV_PARAM_X963KDF_KEY_CHECK' =>      "x963kdf-key-check",      # uint
    'PROV_PARAM_X942KDF_KEY_CHECK' =>      "x942kdf-key-check",      # uint
    'PROV_PARAM_PBKDF2_LOWER_BOUND_CHECK' => "pbkdf2-lower-bound-check", # uint
    'PROV_PARAM_ECDH_COFACTOR_CHECK' =>    "ecdh-cofactor-check",    # uint
    'PROV_PARAM_SIGNATURE_DIGEST_CHECK' => "signature-digest-check", # uint

# Self test callback parameters
    'PROV_PARAM_SELF_TEST_PHASE' =>  "st-phase",# utf8_string
    'PROV_PARAM_SELF_TEST_TYPE' =>   "st-type", # utf8_string
    'PROV_PARAM_SELF_TEST_DESC' =>   "st-desc", # utf8_string

# Provider-native object abstractions
#
# These are used when a provider wants to pass object data or an object
# reference back to libcrypto.  This is only useful for provider functions
# that take a callback to which an PARAM array with these parameters
# can be passed.
#
# This set of parameter names is explained in detail in provider-object(7)
# (doc/man7/provider-object.pod)

    'OBJECT_PARAM_TYPE' =>              "type",     # INTEGER
    'OBJECT_PARAM_DATA_TYPE' =>         "data-type",# UTF8_STRING
    'OBJECT_PARAM_DATA_STRUCTURE' =>    "data-structure",# UTF8_STRING
    'OBJECT_PARAM_REFERENCE' =>         "reference",# OCTET_STRING
    'OBJECT_PARAM_DATA' =>              "data",# OCTET_STRING or UTF8_STRING
    'OBJECT_PARAM_DESC' =>              "desc",     # UTF8_STRING
    'OBJECT_PARAM_INPUT_TYPE' =>        "input-type", # UTF8_STRING

# Algorithm parameters
# If "engine",or "properties",are specified, they should always be paired
# with the algorithm type.
# Note these are common names that are shared by many types (such as kdf, mac,
# and pkey) e.g: see MAC_PARAM_DIGEST below.

    'ALG_PARAM_DIGEST' =>       "digest",       # utf8_string
    'ALG_PARAM_CIPHER' =>       "cipher",       # utf8_string
    'ALG_PARAM_ENGINE' =>       "engine",       # utf8_string
    'ALG_PARAM_MAC' =>          "mac",          # utf8_string
    'ALG_PARAM_PROPERTIES' =>   "properties",   # utf8_string
    'ALG_PARAM_FIPS_APPROVED_INDICATOR' => 'fips-indicator',   # int, -1, 0 or 1

    # For any operation that deals with AlgorithmIdentifier, they should
    # implement both of these.
    # ALG_PARAM_ALGORITHM_ID is intended to be gettable, and is the
    # implementation's idea of what its full AlgID should look like.
    # ALG_PARAM_ALGORITHM_ID_PARAMS is intended to be both settable
    # and gettable, to allow the calling application to pass or get
    # AlgID parameters to and from the provided implementation.
    'ALG_PARAM_ALGORITHM_ID' => "algorithm-id", # octet_string (DER)
    'ALG_PARAM_ALGORITHM_ID_PARAMS' =>  "algorithm-id-params", # octet_string

# cipher parameters
    'CIPHER_PARAM_PADDING' =>              "padding",     # uint
    'CIPHER_PARAM_USE_BITS' =>             "use-bits",    # uint
    'CIPHER_PARAM_TLS_VERSION' =>          "tls-version", # uint
    'CIPHER_PARAM_TLS_MAC' =>              "tls-mac",     # octet_ptr
    'CIPHER_PARAM_TLS_MAC_SIZE' =>         "tls-mac-size",# size_t
    'CIPHER_PARAM_MODE' =>                 "mode",        # uint
    'CIPHER_PARAM_BLOCK_SIZE' =>           "blocksize",   # size_t
    'CIPHER_PARAM_AEAD' =>                 "aead",        # int, 0 or 1
    'CIPHER_PARAM_CUSTOM_IV' =>            "custom-iv",   # int, 0 or 1
    'CIPHER_PARAM_CTS' =>                  "cts",         # int, 0 or 1
    'CIPHER_PARAM_TLS1_MULTIBLOCK' =>      "tls-multi",   # int, 0 or 1
    'CIPHER_PARAM_HAS_RAND_KEY' =>         "has-randkey", # int, 0 or 1
    'CIPHER_PARAM_KEYLEN' =>               "keylen",      # size_t
    'CIPHER_PARAM_IVLEN' =>                "ivlen",       # size_t
    'CIPHER_PARAM_IV' =>                   "iv",          # octet_string OR octet_ptr
    'CIPHER_PARAM_UPDATED_IV' =>           "updated-iv",  # octet_string OR octet_ptr
    'CIPHER_PARAM_NUM' =>                  "num",         # uint
    'CIPHER_PARAM_ROUNDS' =>               "rounds",      # uint
    'CIPHER_PARAM_AEAD_TAG' =>             "tag",         # octet_string
    'CIPHER_PARAM_PIPELINE_AEAD_TAG' =>    "pipeline-tag",# octet_ptr
    'CIPHER_PARAM_AEAD_TLS1_AAD' =>        "tlsaad",      # octet_string
    'CIPHER_PARAM_AEAD_TLS1_AAD_PAD' =>    "tlsaadpad",   # size_t
    'CIPHER_PARAM_AEAD_TLS1_IV_FIXED' =>   "tlsivfixed",  # octet_string
    'CIPHER_PARAM_AEAD_TLS1_GET_IV_GEN' => "tlsivgen",    # octet_string
    'CIPHER_PARAM_AEAD_TLS1_SET_IV_INV' => "tlsivinv",    # octet_string
    'CIPHER_PARAM_AEAD_IVLEN' =>           '*CIPHER_PARAM_IVLEN',
    'CIPHER_PARAM_AEAD_IV_GENERATED' => "iv-generated",   # uint
    'CIPHER_PARAM_AEAD_TAGLEN' =>          "taglen",      # size_t
    'CIPHER_PARAM_AEAD_MAC_KEY' =>         "mackey",      # octet_string
    'CIPHER_PARAM_RANDOM_KEY' =>           "randkey",     # octet_string
    'CIPHER_PARAM_RC2_KEYBITS' =>          "keybits",     # size_t
    'CIPHER_PARAM_SPEED' =>                "speed",       # uint
    'CIPHER_PARAM_CTS_MODE' =>             "cts_mode",    # utf8_string
    'CIPHER_PARAM_DECRYPT_ONLY' =>         "decrypt-only",  # int, 0 or 1
    'CIPHER_PARAM_FIPS_ENCRYPT_CHECK' =>   "encrypt-check", # int
    'CIPHER_PARAM_FIPS_APPROVED_INDICATOR' => '*ALG_PARAM_FIPS_APPROVED_INDICATOR',
    'CIPHER_PARAM_ALGORITHM_ID' =>         '*ALG_PARAM_ALGORITHM_ID',
    # Historically, CIPHER_PARAM_ALGORITHM_ID_PARAMS_OLD was used.  For the
    # time being, the old libcrypto functions will use both, so old providers
    # continue to work.
    # New providers are encouraged to use CIPHER_PARAM_ALGORITHM_ID_PARAMS.
    'CIPHER_PARAM_ALGORITHM_ID_PARAMS' =>  '*ALG_PARAM_ALGORITHM_ID_PARAMS',
    'CIPHER_PARAM_ALGORITHM_ID_PARAMS_OLD' => "alg_id_param", # octet_string
    'CIPHER_PARAM_XTS_STANDARD' =>         "xts_standard",# utf8_string

    'CIPHER_PARAM_TLS1_MULTIBLOCK_MAX_SEND_FRAGMENT' =>  "tls1multi_maxsndfrag",# uint
    'CIPHER_PARAM_TLS1_MULTIBLOCK_MAX_BUFSIZE' =>        "tls1multi_maxbufsz",  # size_t
    'CIPHER_PARAM_TLS1_MULTIBLOCK_INTERLEAVE' =>         "tls1multi_interleave",# uint
    'CIPHER_PARAM_TLS1_MULTIBLOCK_AAD' =>                "tls1multi_aad",       # octet_string
    'CIPHER_PARAM_TLS1_MULTIBLOCK_AAD_PACKLEN' =>        "tls1multi_aadpacklen",# uint
    'CIPHER_PARAM_TLS1_MULTIBLOCK_ENC' =>                "tls1multi_enc",       # octet_string
    'CIPHER_PARAM_TLS1_MULTIBLOCK_ENC_IN' =>             "tls1multi_encin",     # octet_string
    'CIPHER_PARAM_TLS1_MULTIBLOCK_ENC_LEN' =>            "tls1multi_enclen",    # size_t

# digest parameters
    'DIGEST_PARAM_XOFLEN' =>       "xoflen",       # size_t
    'DIGEST_PARAM_SSL3_MS' =>      "ssl3-ms",      # octet string
    'DIGEST_PARAM_PAD_TYPE' =>     "pad-type",     # uint
    'DIGEST_PARAM_MICALG' =>       "micalg",       # utf8 string
    'DIGEST_PARAM_BLOCK_SIZE' =>   "blocksize",    # size_t
    'DIGEST_PARAM_SIZE' =>         "size",         # size_t
    'DIGEST_PARAM_XOF' =>          "xof",          # int, 0 or 1
    'DIGEST_PARAM_ALGID_ABSENT' => "algid-absent", # int, 0 or 1

# MAC parameters
    'MAC_PARAM_KEY' =>            "key",           # octet string
    'MAC_PARAM_IV' =>             "iv",            # octet string
    'MAC_PARAM_CUSTOM' =>         "custom",        # utf8 string
    'MAC_PARAM_SALT' =>           "salt",          # octet string
    'MAC_PARAM_XOF' =>            "xof",           # int, 0 or 1
    'MAC_PARAM_DIGEST_NOINIT' =>  "digest-noinit", # int, 0 or 1
    'MAC_PARAM_DIGEST_ONESHOT' => "digest-oneshot",# int, 0 or 1
    'MAC_PARAM_C_ROUNDS' =>       "c-rounds",      # unsigned int
    'MAC_PARAM_D_ROUNDS' =>       "d-rounds",      # unsigned int

# If "engine",or "properties",are specified, they should always be paired
# with "cipher",or "digest".

    'MAC_PARAM_CIPHER' =>           '*ALG_PARAM_CIPHER',        # utf8 string
    'MAC_PARAM_DIGEST' =>           '*ALG_PARAM_DIGEST',        # utf8 string
    'MAC_PARAM_PROPERTIES' =>       '*ALG_PARAM_PROPERTIES',    # utf8 string
    'MAC_PARAM_SIZE' =>             "size",                     # size_t
    'MAC_PARAM_BLOCK_SIZE' =>       "block-size",               # size_t
    'MAC_PARAM_TLS_DATA_SIZE' =>    "tls-data-size",            # size_t
    'MAC_PARAM_FIPS_NO_SHORT_MAC' =>'*PROV_PARAM_NO_SHORT_MAC',
    'MAC_PARAM_FIPS_KEY_CHECK' =>   '*PKEY_PARAM_FIPS_KEY_CHECK',
    'MAC_PARAM_FIPS_APPROVED_INDICATOR' => '*ALG_PARAM_FIPS_APPROVED_INDICATOR',
    'MAC_PARAM_FIPS_NO_SHORT_MAC' => '*PROV_PARAM_NO_SHORT_MAC',

# KDF / PRF parameters
    'KDF_PARAM_SECRET' =>       "secret",                   # octet string
    'KDF_PARAM_KEY' =>          "key",                      # octet string
    'KDF_PARAM_SALT' =>         "salt",                     # octet string
    'KDF_PARAM_PASSWORD' =>     "pass",                     # octet string
    'KDF_PARAM_PREFIX' =>       "prefix",                   # octet string
    'KDF_PARAM_LABEL' =>        "label",                    # octet string
    'KDF_PARAM_DATA' =>         "data",                     # octet string
    'KDF_PARAM_DIGEST' =>       '*ALG_PARAM_DIGEST',        # utf8 string
    'KDF_PARAM_CIPHER' =>       '*ALG_PARAM_CIPHER',        # utf8 string
    'KDF_PARAM_MAC' =>          '*ALG_PARAM_MAC',           # utf8 string
    'KDF_PARAM_MAC_SIZE' =>     "maclen",                   # size_t
    'KDF_PARAM_PROPERTIES' =>   '*ALG_PARAM_PROPERTIES',    # utf8 string
    'KDF_PARAM_ITER' =>         "iter",                     # unsigned int
    'KDF_PARAM_MODE' =>         "mode",                     # utf8 string or int
    'KDF_PARAM_PKCS5' =>        "pkcs5",                    # int
    'KDF_PARAM_UKM' =>          "ukm",                      # octet string
    'KDF_PARAM_CEK_ALG' =>      "cekalg",                   # utf8 string
    'KDF_PARAM_SCRYPT_N' =>     "n",                        # uint64_t
    'KDF_PARAM_SCRYPT_R' =>     "r",                        # uint32_t
    'KDF_PARAM_SCRYPT_P' =>     "p",                        # uint32_t
    'KDF_PARAM_SCRYPT_MAXMEM' => "maxmem_bytes",            # uint64_t
    'KDF_PARAM_INFO' =>         "info",                     # octet string
    'KDF_PARAM_SEED' =>         "seed",                     # octet string
    'KDF_PARAM_SSHKDF_XCGHASH' => "xcghash",                # octet string
    'KDF_PARAM_SSHKDF_SESSION_ID' => "session_id",          # octet string
    'KDF_PARAM_SSHKDF_TYPE' =>  "type",                     # int
    'KDF_PARAM_SIZE' =>         "size",                     # size_t
    'KDF_PARAM_CONSTANT' =>     "constant",                 # octet string
    'KDF_PARAM_PKCS12_ID' =>    "id",                       # int
    'KDF_PARAM_KBKDF_USE_L' =>          "use-l",            # int
    'KDF_PARAM_KBKDF_USE_SEPARATOR' =>  "use-separator",    # int
    'KDF_PARAM_KBKDF_R' =>      "r",                        # int
    'KDF_PARAM_X942_ACVPINFO' =>        "acvp-info",
    'KDF_PARAM_X942_PARTYUINFO' =>      "partyu-info",
    'KDF_PARAM_X942_PARTYVINFO' =>      "partyv-info",
    'KDF_PARAM_X942_SUPP_PUBINFO' =>    "supp-pubinfo",
    'KDF_PARAM_X942_SUPP_PRIVINFO' =>   "supp-privinfo",
    'KDF_PARAM_X942_USE_KEYBITS' =>     "use-keybits",
    'KDF_PARAM_HMACDRBG_ENTROPY' =>     "entropy",
    'KDF_PARAM_HMACDRBG_NONCE' =>       "nonce",
    'KDF_PARAM_THREADS' =>        "threads",                # uint32_t
    'KDF_PARAM_EARLY_CLEAN' =>    "early_clean",            # uint32_t
    'KDF_PARAM_ARGON2_AD' =>      "ad",                     # octet string
    'KDF_PARAM_ARGON2_LANES' =>   "lanes",                  # uint32_t
    'KDF_PARAM_ARGON2_MEMCOST' => "memcost",                # uint32_t
    'KDF_PARAM_ARGON2_VERSION' => "version",                # uint32_t
    'KDF_PARAM_FIPS_EMS_CHECK' => "ems_check",              # int
    'KDF_PARAM_FIPS_DIGEST_CHECK' => '*PKEY_PARAM_FIPS_DIGEST_CHECK',
    'KDF_PARAM_FIPS_KEY_CHECK' => '*PKEY_PARAM_FIPS_KEY_CHECK',
    'KDF_PARAM_FIPS_APPROVED_INDICATOR' => '*ALG_PARAM_FIPS_APPROVED_INDICATOR',

# Known RAND names
    'RAND_PARAM_STATE' =>                   "state",
    'RAND_PARAM_STRENGTH' =>                "strength",
    'RAND_PARAM_MAX_REQUEST' =>             "max_request",
    'RAND_PARAM_TEST_ENTROPY' =>            "test_entropy",
    'RAND_PARAM_TEST_NONCE' =>              "test_nonce",
    'RAND_PARAM_GENERATE' =>                "generate",
    'RAND_PARAM_FIPS_APPROVED_INDICATOR' => '*ALG_PARAM_FIPS_APPROVED_INDICATOR',

# RAND/DRBG names
    'DRBG_PARAM_RESEED_REQUESTS' =>         "reseed_requests",
    'DRBG_PARAM_RESEED_TIME_INTERVAL' =>    "reseed_time_interval",
    'DRBG_PARAM_MIN_ENTROPYLEN' =>          "min_entropylen",
    'DRBG_PARAM_MAX_ENTROPYLEN' =>          "max_entropylen",
    'DRBG_PARAM_MIN_NONCELEN' =>            "min_noncelen",
    'DRBG_PARAM_MAX_NONCELEN' =>            "max_noncelen",
    'DRBG_PARAM_MAX_PERSLEN' =>             "max_perslen",
    'DRBG_PARAM_MAX_ADINLEN' =>             "max_adinlen",
    'DRBG_PARAM_RESEED_COUNTER' =>          "reseed_counter",
    'DRBG_PARAM_RESEED_TIME' =>             "reseed_time",
    'DRBG_PARAM_PROPERTIES' =>              '*ALG_PARAM_PROPERTIES',
    'DRBG_PARAM_DIGEST' =>                  '*ALG_PARAM_DIGEST',
    'DRBG_PARAM_CIPHER' =>                  '*ALG_PARAM_CIPHER',
    'DRBG_PARAM_MAC' =>                     '*ALG_PARAM_MAC',
    'DRBG_PARAM_USE_DF' =>                  "use_derivation_function",
    'DRBG_PARAM_FIPS_DIGEST_CHECK' =>       '*PKEY_PARAM_FIPS_DIGEST_CHECK',
    'DRBG_PARAM_FIPS_APPROVED_INDICATOR' => '*ALG_PARAM_FIPS_APPROVED_INDICATOR',

# DRBG call back parameters
    'DRBG_PARAM_ENTROPY_REQUIRED' =>        "entropy_required",
    'DRBG_PARAM_PREDICTION_RESISTANCE' =>   "prediction_resistance",
    'DRBG_PARAM_MIN_LENGTH' =>              "minium_length",
    'DRBG_PARAM_MAX_LENGTH' =>              "maxium_length",
    'DRBG_PARAM_RANDOM_DATA' =>             "random_data",
    'DRBG_PARAM_SIZE' =>                    "size",

# PKEY parameters
# Common PKEY parameters
    'PKEY_PARAM_BITS' =>                "bits",# integer
    'PKEY_PARAM_MAX_SIZE' =>            "max-size",# integer
    'PKEY_PARAM_SECURITY_BITS' =>       "security-bits",# integer
    'PKEY_PARAM_DIGEST' =>              '*ALG_PARAM_DIGEST',
    'PKEY_PARAM_CIPHER' =>              '*ALG_PARAM_CIPHER', # utf8 string
    'PKEY_PARAM_ENGINE' =>              '*ALG_PARAM_ENGINE', # utf8 string
    'PKEY_PARAM_PROPERTIES' =>          '*ALG_PARAM_PROPERTIES',
    'PKEY_PARAM_DEFAULT_DIGEST' =>      "default-digest",# utf8 string
    'PKEY_PARAM_MANDATORY_DIGEST' =>    "mandatory-digest",# utf8 string
    'PKEY_PARAM_PAD_MODE' =>            "pad-mode",
    'PKEY_PARAM_DIGEST_SIZE' =>         "digest-size",
    'PKEY_PARAM_MASKGENFUNC' =>         "mgf",
    'PKEY_PARAM_MGF1_DIGEST' =>         "mgf1-digest",
    'PKEY_PARAM_MGF1_PROPERTIES' =>     "mgf1-properties",
    'PKEY_PARAM_ENCODED_PUBLIC_KEY' =>  "encoded-pub-key",
    'PKEY_PARAM_GROUP_NAME' =>          "group",
    'PKEY_PARAM_DIST_ID' =>             "distid",
    'PKEY_PARAM_PUB_KEY' =>             "pub",
    'PKEY_PARAM_PRIV_KEY' =>            "priv",
    # PKEY_PARAM_IMPLICIT_REJECTION isn't actually used, or meaningful.  We keep
    # it for API stability, but please use ASYM_CIPHER_PARAM_IMPLICIT_REJECTION
    # instead.
    'PKEY_PARAM_IMPLICIT_REJECTION' =>  "implicit-rejection",
    'PKEY_PARAM_FIPS_DIGEST_CHECK' =>   "digest-check",
    'PKEY_PARAM_FIPS_KEY_CHECK' =>      "key-check",
    'PKEY_PARAM_ALGORITHM_ID' =>        '*ALG_PARAM_ALGORITHM_ID',
    'PKEY_PARAM_ALGORITHM_ID_PARAMS' => '*ALG_PARAM_ALGORITHM_ID_PARAMS',

# Diffie-Hellman/DSA Parameters
    'PKEY_PARAM_FFC_P' =>               "p",
    'PKEY_PARAM_FFC_G' =>               "g",
    'PKEY_PARAM_FFC_Q' =>               "q",
    'PKEY_PARAM_FFC_GINDEX' =>          "gindex",
    'PKEY_PARAM_FFC_PCOUNTER' =>        "pcounter",
    'PKEY_PARAM_FFC_SEED' =>            "seed",
    'PKEY_PARAM_FFC_COFACTOR' =>        "j",
    'PKEY_PARAM_FFC_H' =>               "hindex",
    'PKEY_PARAM_FFC_VALIDATE_PQ' =>     "validate-pq",
    'PKEY_PARAM_FFC_VALIDATE_G' =>      "validate-g",
    'PKEY_PARAM_FFC_VALIDATE_LEGACY' => "validate-legacy",

# Diffie-Hellman params
    'PKEY_PARAM_DH_GENERATOR' =>        "safeprime-generator",
    'PKEY_PARAM_DH_PRIV_LEN' =>         "priv_len",

# Elliptic Curve Domain Parameters
    'PKEY_PARAM_EC_PUB_X' =>     "qx",
    'PKEY_PARAM_EC_PUB_Y' =>     "qy",

# Elliptic Curve Explicit Domain Parameters
    'PKEY_PARAM_EC_FIELD_TYPE' =>                   "field-type",
    'PKEY_PARAM_EC_P' =>                            "p",
    'PKEY_PARAM_EC_A' =>                            "a",
    'PKEY_PARAM_EC_B' =>                            "b",
    'PKEY_PARAM_EC_GENERATOR' =>                    "generator",
    'PKEY_PARAM_EC_ORDER' =>                        "order",
    'PKEY_PARAM_EC_COFACTOR' =>                     "cofactor",
    'PKEY_PARAM_EC_SEED' =>                         "seed",
    'PKEY_PARAM_EC_CHAR2_M' =>                      "m",
    'PKEY_PARAM_EC_CHAR2_TYPE' =>                   "basis-type",
    'PKEY_PARAM_EC_CHAR2_TP_BASIS' =>               "tp",
    'PKEY_PARAM_EC_CHAR2_PP_K1' =>                  "k1",
    'PKEY_PARAM_EC_CHAR2_PP_K2' =>                  "k2",
    'PKEY_PARAM_EC_CHAR2_PP_K3' =>                  "k3",
    'PKEY_PARAM_EC_DECODED_FROM_EXPLICIT_PARAMS' => "decoded-from-explicit",

# Elliptic Curve Key Parameters
    'PKEY_PARAM_USE_COFACTOR_FLAG' => "use-cofactor-flag",
    'PKEY_PARAM_USE_COFACTOR_ECDH' => '*PKEY_PARAM_USE_COFACTOR_FLAG',

# RSA Keys
#
# n, e, d are the usual public and private key components
#
# rsa-num is the number of factors, including p and q
# rsa-factor is used for each factor: p, q, r_i (i = 3, ...)
# rsa-exponent is used for each exponent: dP, dQ, d_i (i = 3, ...)
# rsa-coefficient is used for each coefficient: qInv, t_i (i = 3, ...)
#
# The number of rsa-factor items must be equal to the number of rsa-exponent
# items, and the number of rsa-coefficients must be one less.
# (the base i for the coefficients is 2, not 1, at least as implied by
# RFC 8017)

    'PKEY_PARAM_RSA_N' =>           "n",
    'PKEY_PARAM_RSA_E' =>           "e",
    'PKEY_PARAM_RSA_D' =>           "d",
    'PKEY_PARAM_RSA_FACTOR' =>      "rsa-factor",
    'PKEY_PARAM_RSA_EXPONENT' =>    "rsa-exponent",
    'PKEY_PARAM_RSA_COEFFICIENT' => "rsa-coefficient",
    'PKEY_PARAM_RSA_FACTOR1' =>      "rsa-factor1",
    'PKEY_PARAM_RSA_FACTOR2' =>      "rsa-factor2",
    'PKEY_PARAM_RSA_FACTOR3' =>      "rsa-factor3",
    'PKEY_PARAM_RSA_FACTOR4' =>      "rsa-factor4",
    'PKEY_PARAM_RSA_FACTOR5' =>      "rsa-factor5",
    'PKEY_PARAM_RSA_FACTOR6' =>      "rsa-factor6",
    'PKEY_PARAM_RSA_FACTOR7' =>      "rsa-factor7",
    'PKEY_PARAM_RSA_FACTOR8' =>      "rsa-factor8",
    'PKEY_PARAM_RSA_FACTOR9' =>      "rsa-factor9",
    'PKEY_PARAM_RSA_FACTOR10' =>     "rsa-factor10",
    'PKEY_PARAM_RSA_EXPONENT1' =>    "rsa-exponent1",
    'PKEY_PARAM_RSA_EXPONENT2' =>    "rsa-exponent2",
    'PKEY_PARAM_RSA_EXPONENT3' =>    "rsa-exponent3",
    'PKEY_PARAM_RSA_EXPONENT4' =>    "rsa-exponent4",
    'PKEY_PARAM_RSA_EXPONENT5' =>    "rsa-exponent5",
    'PKEY_PARAM_RSA_EXPONENT6' =>    "rsa-exponent6",
    'PKEY_PARAM_RSA_EXPONENT7' =>    "rsa-exponent7",
    'PKEY_PARAM_RSA_EXPONENT8' =>    "rsa-exponent8",
    'PKEY_PARAM_RSA_EXPONENT9' =>    "rsa-exponent9",
    'PKEY_PARAM_RSA_EXPONENT10' =>   "rsa-exponent10",
    'PKEY_PARAM_RSA_COEFFICIENT1' => "rsa-coefficient1",
    'PKEY_PARAM_RSA_COEFFICIENT2' => "rsa-coefficient2",
    'PKEY_PARAM_RSA_COEFFICIENT3' => "rsa-coefficient3",
    'PKEY_PARAM_RSA_COEFFICIENT4' => "rsa-coefficient4",
    'PKEY_PARAM_RSA_COEFFICIENT5' => "rsa-coefficient5",
    'PKEY_PARAM_RSA_COEFFICIENT6' => "rsa-coefficient6",
    'PKEY_PARAM_RSA_COEFFICIENT7' => "rsa-coefficient7",
    'PKEY_PARAM_RSA_COEFFICIENT8' => "rsa-coefficient8",
    'PKEY_PARAM_RSA_COEFFICIENT9' => "rsa-coefficient9",

# Key generation parameters
    'PKEY_PARAM_RSA_BITS' =>             '*PKEY_PARAM_BITS',
    'PKEY_PARAM_RSA_PRIMES' =>           "primes",
    'PKEY_PARAM_RSA_DIGEST' =>           '*PKEY_PARAM_DIGEST',
    'PKEY_PARAM_RSA_DIGEST_PROPS' =>     '*PKEY_PARAM_PROPERTIES',
    'PKEY_PARAM_RSA_MASKGENFUNC' =>      '*PKEY_PARAM_MASKGENFUNC',
    'PKEY_PARAM_RSA_MGF1_DIGEST' =>      '*PKEY_PARAM_MGF1_DIGEST',
    'PKEY_PARAM_RSA_PSS_SALTLEN' =>      "saltlen",
    'PKEY_PARAM_RSA_DERIVE_FROM_PQ'    =>     "rsa-derive-from-pq",

# EC, X25519 and X448 Key generation parameters
    'PKEY_PARAM_DHKEM_IKM' =>        "dhkem-ikm",

# ML-KEM parameters
    'PKEY_PARAM_ML_KEM_SEED' => "seed",
    'PKEY_PARAM_ML_KEM_PREFER_SEED' => "ml-kem.prefer_seed",
    'PKEY_PARAM_ML_KEM_RETAIN_SEED' => "ml-kem.retain_seed",
    'PKEY_PARAM_ML_KEM_INPUT_FORMATS' => "ml-kem.input_formats",
    'PKEY_PARAM_ML_KEM_OUTPUT_FORMATS' => "ml-kem.output_formats",
    'PKEY_PARAM_ML_KEM_IMPORT_PCT_TYPE' => "ml-kem.import_pct_type",

# Key generation parameters
    'PKEY_PARAM_FFC_TYPE' =>         "type",
    'PKEY_PARAM_FFC_PBITS' =>        "pbits",
    'PKEY_PARAM_FFC_QBITS' =>        "qbits",
    'PKEY_PARAM_FFC_DIGEST' =>       '*PKEY_PARAM_DIGEST',
    'PKEY_PARAM_FFC_DIGEST_PROPS' => '*PKEY_PARAM_PROPERTIES',

    'PKEY_PARAM_EC_ENCODING' =>                "encoding",# utf8_string
    'PKEY_PARAM_EC_POINT_CONVERSION_FORMAT' => "point-format",
    'PKEY_PARAM_EC_GROUP_CHECK_TYPE' =>        "group-check",
    'PKEY_PARAM_EC_INCLUDE_PUBLIC' =>          "include-public",
    'PKEY_PARAM_FIPS_SIGN_CHECK' =>            "sign-check",
    'PKEY_PARAM_FIPS_APPROVED_INDICATOR' => '*ALG_PARAM_FIPS_APPROVED_INDICATOR',

# ML_DSA Key generation parameter
    'PKEY_PARAM_ML_DSA_SEED' =>             "seed",
    'PKEY_PARAM_ML_DSA_RETAIN_SEED' =>      "ml-dsa.retain_seed",
    'PKEY_PARAM_ML_DSA_PREFER_SEED' =>      "ml-dsa.prefer_seed",
    'PKEY_PARAM_ML_DSA_INPUT_FORMATS' =>    "ml-dsa.input_formats",
    'PKEY_PARAM_ML_DSA_OUTPUT_FORMATS' =>   "ml-dsa.output_formats",

# SLH_DSA Key generation parameters
    'PKEY_PARAM_SLH_DSA_SEED' =>              "seed",

# Key Exchange parameters
    'EXCHANGE_PARAM_PAD' =>                   "pad",# uint
    'EXCHANGE_PARAM_EC_ECDH_COFACTOR_MODE' => "ecdh-cofactor-mode",# int
    'EXCHANGE_PARAM_KDF_TYPE' =>              "kdf-type",# utf8_string
    'EXCHANGE_PARAM_KDF_DIGEST' =>            "kdf-digest",# utf8_string
    'EXCHANGE_PARAM_KDF_DIGEST_PROPS' =>      "kdf-digest-props",# utf8_string
    'EXCHANGE_PARAM_KDF_OUTLEN' =>            "kdf-outlen",# size_t
# The following parameter is an octet_string on set and an octet_ptr on get
    'EXCHANGE_PARAM_KDF_UKM' =>               "kdf-ukm",
    'EXCHANGE_PARAM_FIPS_DIGEST_CHECK' =>     '*PKEY_PARAM_FIPS_DIGEST_CHECK',
    'EXCHANGE_PARAM_FIPS_KEY_CHECK' =>        '*PKEY_PARAM_FIPS_KEY_CHECK',
    'EXCHANGE_PARAM_FIPS_ECDH_COFACTOR_CHECK' => '*PROV_PARAM_ECDH_COFACTOR_CHECK',
    'EXCHANGE_PARAM_FIPS_APPROVED_INDICATOR' => '*ALG_PARAM_FIPS_APPROVED_INDICATOR',

# Signature parameters
    'SIGNATURE_PARAM_ALGORITHM_ID' =>         '*PKEY_PARAM_ALGORITHM_ID',
    'SIGNATURE_PARAM_ALGORITHM_ID_PARAMS' =>  '*PKEY_PARAM_ALGORITHM_ID_PARAMS',
    'SIGNATURE_PARAM_PAD_MODE' =>             '*PKEY_PARAM_PAD_MODE',
    'SIGNATURE_PARAM_DIGEST' =>               '*PKEY_PARAM_DIGEST',
    'SIGNATURE_PARAM_PROPERTIES' =>           '*PKEY_PARAM_PROPERTIES',
    'SIGNATURE_PARAM_PSS_SALTLEN' =>          "saltlen",
    'SIGNATURE_PARAM_MGF1_DIGEST' =>          '*PKEY_PARAM_MGF1_DIGEST',
    'SIGNATURE_PARAM_MGF1_PROPERTIES' =>      '*PKEY_PARAM_MGF1_PROPERTIES',
    'SIGNATURE_PARAM_DIGEST_SIZE' =>          '*PKEY_PARAM_DIGEST_SIZE',
    'SIGNATURE_PARAM_NONCE_TYPE' =>           "nonce-type",
    'SIGNATURE_PARAM_INSTANCE' =>             "instance",
    'SIGNATURE_PARAM_CONTEXT_STRING' =>       "context-string",
    'SIGNATURE_PARAM_FIPS_DIGEST_CHECK' =>    '*PKEY_PARAM_FIPS_DIGEST_CHECK',
    'SIGNATURE_PARAM_FIPS_VERIFY_MESSAGE' =>  'verify-message',
    'SIGNATURE_PARAM_FIPS_KEY_CHECK' =>       '*PKEY_PARAM_FIPS_KEY_CHECK',
    'SIGNATURE_PARAM_FIPS_SIGN_CHECK' =>      '*PKEY_PARAM_FIPS_SIGN_CHECK',
    'SIGNATURE_PARAM_FIPS_RSA_PSS_SALTLEN_CHECK' => "rsa-pss-saltlen-check",
    'SIGNATURE_PARAM_FIPS_SIGN_X931_PAD_CHECK' => "sign-x931-pad-check",
    'SIGNATURE_PARAM_FIPS_APPROVED_INDICATOR' => '*ALG_PARAM_FIPS_APPROVED_INDICATOR',
    'SIGNATURE_PARAM_SIGNATURE' =>          "signature",
    'SIGNATURE_PARAM_MESSAGE_ENCODING' =>   "message-encoding",
    'SIGNATURE_PARAM_DETERMINISTIC' =>      "deterministic",
    'SIGNATURE_PARAM_MU' =>                 "mu", # int
    'SIGNATURE_PARAM_TEST_ENTROPY' =>       "test-entropy",
    'SIGNATURE_PARAM_ADD_RANDOM' =>         "additional-random",

# Asym cipher parameters
    'ASYM_CIPHER_PARAM_DIGEST' =>                   '*PKEY_PARAM_DIGEST',
    'ASYM_CIPHER_PARAM_PROPERTIES' =>               '*PKEY_PARAM_PROPERTIES',
    'ASYM_CIPHER_PARAM_ENGINE' =>                   '*PKEY_PARAM_ENGINE',
    'ASYM_CIPHER_PARAM_PAD_MODE' =>                 '*PKEY_PARAM_PAD_MODE',
    'ASYM_CIPHER_PARAM_MGF1_DIGEST' =>              '*PKEY_PARAM_MGF1_DIGEST',
    'ASYM_CIPHER_PARAM_MGF1_DIGEST_PROPS' =>        '*PKEY_PARAM_MGF1_PROPERTIES',
    'ASYM_CIPHER_PARAM_OAEP_DIGEST' =>              '*ALG_PARAM_DIGEST',
    'ASYM_CIPHER_PARAM_OAEP_DIGEST_PROPS' =>        "digest-props",
# The following parameter is an octet_string on set and an octet_ptr on get
    'ASYM_CIPHER_PARAM_OAEP_LABEL' =>               "oaep-label",
    'ASYM_CIPHER_PARAM_TLS_CLIENT_VERSION' =>       "tls-client-version",
    'ASYM_CIPHER_PARAM_TLS_NEGOTIATED_VERSION' =>   "tls-negotiated-version",
    'ASYM_CIPHER_PARAM_IMPLICIT_REJECTION' =>       "implicit-rejection",
    'ASYM_CIPHER_PARAM_FIPS_RSA_PKCS15_PAD_DISABLED' => '*PROV_PARAM_RSA_PKCS15_PAD_DISABLED',
    'ASYM_CIPHER_PARAM_FIPS_KEY_CHECK' =>           '*PKEY_PARAM_FIPS_KEY_CHECK',
    'ASYM_CIPHER_PARAM_FIPS_APPROVED_INDICATOR' =>  '*ALG_PARAM_FIPS_APPROVED_INDICATOR',

# Encoder / decoder parameters

    'ENCODER_PARAM_CIPHER' =>           '*ALG_PARAM_CIPHER',
    'ENCODER_PARAM_PROPERTIES' =>       '*ALG_PARAM_PROPERTIES',
# Currently PVK only, but reusable for others as needed
    'ENCODER_PARAM_ENCRYPT_LEVEL' =>    "encrypt-level",
    'ENCODER_PARAM_SAVE_PARAMETERS' =>  "save-parameters",# integer

    'DECODER_PARAM_PROPERTIES' =>       '*ALG_PARAM_PROPERTIES',

# Passphrase callback parameters
    'PASSPHRASE_PARAM_INFO' =>          "info",

# Keygen callback parameters, from provider to libcrypto
    'GEN_PARAM_POTENTIAL' =>            "potential",# integer
    'GEN_PARAM_ITERATION' =>            "iteration",# integer

# ACVP Test parameters : These should not be used normally
    'PKEY_PARAM_RSA_TEST_XP1' => "xp1",
    'PKEY_PARAM_RSA_TEST_XP2' => "xp2",
    'PKEY_PARAM_RSA_TEST_XP' =>  "xp",
    'PKEY_PARAM_RSA_TEST_XQ1' => "xq1",
    'PKEY_PARAM_RSA_TEST_XQ2' => "xq2",
    'PKEY_PARAM_RSA_TEST_XQ' =>  "xq",
    'PKEY_PARAM_RSA_TEST_P1' =>  "p1",
    'PKEY_PARAM_RSA_TEST_P2' =>  "p2",
    'PKEY_PARAM_RSA_TEST_Q1' =>  "q1",
    'PKEY_PARAM_RSA_TEST_Q2' =>  "q2",
    'SIGNATURE_PARAM_KAT' =>     "kat",

# KEM parameters
    'KEM_PARAM_OPERATION' =>            "operation",
    'KEM_PARAM_IKME' =>                 "ikme",
    'KEM_PARAM_FIPS_KEY_CHECK' =>       '*PKEY_PARAM_FIPS_KEY_CHECK',
    'KEM_PARAM_FIPS_APPROVED_INDICATOR' => '*ALG_PARAM_FIPS_APPROVED_INDICATOR',

# Capabilities

# TLS-GROUP Capability
    'CAPABILITY_TLS_GROUP_NAME' =>              "tls-group-name",
    'CAPABILITY_TLS_GROUP_NAME_INTERNAL' =>     "tls-group-name-internal",
    'CAPABILITY_TLS_GROUP_ID' =>                "tls-group-id",
    'CAPABILITY_TLS_GROUP_ALG' =>               "tls-group-alg",
    'CAPABILITY_TLS_GROUP_SECURITY_BITS' =>     "tls-group-sec-bits",
    'CAPABILITY_TLS_GROUP_IS_KEM' =>            "tls-group-is-kem",
    'CAPABILITY_TLS_GROUP_MIN_TLS' =>           "tls-min-tls",
    'CAPABILITY_TLS_GROUP_MAX_TLS' =>           "tls-max-tls",
    'CAPABILITY_TLS_GROUP_MIN_DTLS' =>          "tls-min-dtls",
    'CAPABILITY_TLS_GROUP_MAX_DTLS' =>          "tls-max-dtls",

# TLS-SIGALG Capability
    'CAPABILITY_TLS_SIGALG_IANA_NAME' =>         "tls-sigalg-iana-name",
    'CAPABILITY_TLS_SIGALG_CODE_POINT' =>        "tls-sigalg-code-point",
    'CAPABILITY_TLS_SIGALG_NAME' =>              "tls-sigalg-name",
    'CAPABILITY_TLS_SIGALG_OID' =>               "tls-sigalg-oid",
    'CAPABILITY_TLS_SIGALG_SIG_NAME' =>          "tls-sigalg-sig-name",
    'CAPABILITY_TLS_SIGALG_SIG_OID' =>           "tls-sigalg-sig-oid",
    'CAPABILITY_TLS_SIGALG_HASH_NAME' =>         "tls-sigalg-hash-name",
    'CAPABILITY_TLS_SIGALG_HASH_OID' =>          "tls-sigalg-hash-oid",
    'CAPABILITY_TLS_SIGALG_KEYTYPE' =>           "tls-sigalg-keytype",
    'CAPABILITY_TLS_SIGALG_KEYTYPE_OID' =>       "tls-sigalg-keytype-oid",
    'CAPABILITY_TLS_SIGALG_SECURITY_BITS' =>     "tls-sigalg-sec-bits",
    'CAPABILITY_TLS_SIGALG_MIN_TLS' =>           "tls-min-tls",
    'CAPABILITY_TLS_SIGALG_MAX_TLS' =>           "tls-max-tls",
    'CAPABILITY_TLS_SIGALG_MIN_DTLS' =>          "tls-min-dtls",
    'CAPABILITY_TLS_SIGALG_MAX_DTLS' =>          "tls-max-dtls",

# storemgmt parameters


# Used by storemgmt_ctx_set_params():
#
# - STORE_PARAM_EXPECT is an INTEGER, and the value is any of the
#   STORE_INFO numbers.  This is used to set the expected type of
#   object loaded.
#
# - STORE_PARAM_SUBJECT, STORE_PARAM_ISSUER,
#   STORE_PARAM_SERIAL, STORE_PARAM_FINGERPRINT,
#   STORE_PARAM_DIGEST, STORE_PARAM_ALIAS
#   are used as search criteria.
#   (STORE_PARAM_DIGEST is used with STORE_PARAM_FINGERPRINT)

    'STORE_PARAM_EXPECT' =>     "expect",       # INTEGER
    'STORE_PARAM_SUBJECT' =>    "subject",      # DER blob => OCTET_STRING
    'STORE_PARAM_ISSUER' =>     "name",         # DER blob => OCTET_STRING
    'STORE_PARAM_SERIAL' =>     "serial",       # INTEGER
    'STORE_PARAM_DIGEST' =>     "digest",       # UTF8_STRING
    'STORE_PARAM_FINGERPRINT' => "fingerprint", # OCTET_STRING
    'STORE_PARAM_ALIAS' =>      "alias",        # UTF8_STRING

# You may want to pass properties for the provider implementation to use
    'STORE_PARAM_PROPERTIES' => "properties",   # utf8_string
# DECODER input type if a decoder is used by the store
    'STORE_PARAM_INPUT_TYPE' => "input-type",   # UTF8_STRING


# Libssl record layer
    'LIBSSL_RECORD_LAYER_PARAM_OPTIONS' =>        "options",
    'LIBSSL_RECORD_LAYER_PARAM_MODE' =>           "mode",
    'LIBSSL_RECORD_LAYER_PARAM_READ_AHEAD' =>     "read_ahead",
    'LIBSSL_RECORD_LAYER_READ_BUFFER_LEN' =>      "read_buffer_len",
    'LIBSSL_RECORD_LAYER_PARAM_USE_ETM' =>        "use_etm",
    'LIBSSL_RECORD_LAYER_PARAM_STREAM_MAC' =>     "stream_mac",
    'LIBSSL_RECORD_LAYER_PARAM_TLSTREE' =>        "tlstree",
    'LIBSSL_RECORD_LAYER_PARAM_MAX_FRAG_LEN' =>   "max_frag_len",
    'LIBSSL_RECORD_LAYER_PARAM_MAX_EARLY_DATA' => "max_early_data",
    'LIBSSL_RECORD_LAYER_PARAM_BLOCK_PADDING' =>  "block_padding",
    'LIBSSL_RECORD_LAYER_PARAM_HS_PADDING' =>     "hs_padding",

# Symmetric Key parametes
    'SKEY_PARAM_RAW_BYTES' => "raw-bytes",
    'SKEY_PARAM_KEY_LENGTH' => "key-length",
);

# Generate string based macros for public consumption
sub generate_public_macros {
    my @macros = ();

    foreach my $name (keys %params) {
        my $val = $params{$name};
        my $def = '# define OSSL_' . $name . ' ';

        if (substr($val, 0, 1) eq '*') {
            $def .= 'OSSL_' . substr($val, 1);
        } else {
            $def .= '"' . $val . '"';
        }
        push(@macros, $def)
    }
    return join("\n", sort @macros);
}

# Generate number based macros for internal use
# The numbers are unique per string
sub generate_internal_macros {
    my @macros = ();
    my $count = 0;
    my %reverse;

    # Determine the number for each unique string
    # Sort the names to improve the chance of cache coherency
    foreach my $name (sort keys %params) {
        my $val = $params{$name};

        if (substr($val, 0, 1) ne '*' and not defined $reverse{$val}) {
            $reverse{$val} = $count++;
        }
    }

    # Output the defines
    foreach my $name (keys %params) {
        my $val = $params{$name};
        my $def = '#define PIDX_' . $name . ' ';

        if (substr($val, 0, 1) eq '*') {
            $def .= 'PIDX_' . substr($val, 1);
        } else {
            $def .= $reverse{$val};
        }
        push(@macros, $def)
    }
    return "#define NUM_PIDX $count\n\n" . join("\n", sort @macros);
}

sub generate_trie {
    my %trie;
    my $nodes = 0;
    my $chars = 0;

    foreach my $name (sort keys %params) {
        my $val = $params{$name};
        if (substr($val, 0, 1) ne '*') {
            my $cursor = \%trie;

            $chars += length($val);
            for my $i (0 .. length($val) - 1) {
                my $c = substr($val, $i, 1);

                if (not $case_sensitive) {
                    $c = '_' if $c eq '-';
                    $c = lc $c;
                }

                if (not defined $$cursor{$c}) {
                    $cursor->{$c} = {};
                    $nodes++;
                }
                $cursor = $cursor->{$c};
            }
            $cursor->{'val'} = $name;
        }
    }
    #print "\n\n/* $nodes nodes for $chars letters*/\n\n";
    return %trie;
}

sub generate_code_from_trie {
    my $n = shift;
    my $trieref = shift;
    my $idt = "    ";
    my $indent0 = $idt x ($n + 1);
    my $indent1 = $indent0 . $idt;
    my $strcmp = $case_sensitive ? 'strcmp' : 'strcasecmp';

    print "int ossl_param_find_pidx(const char *s)\n{\n" if $n == 0;

    if ($trieref->{'suffix'}) {
        my $suf = $trieref->{'suffix'};

        printf "%sif ($strcmp(\"$suf\", s + $n) == 0", $indent0;
        if (not $case_sensitive) {
            $suf =~ tr/_/-/;
            print " || $strcmp(\"$suf\", s + $n) == 0"
                if ($suf ne $trieref->{'suffix'});
        }
        printf ")\n%sreturn PIDX_%s;\n", $indent1, $trieref->{'name'};
        #printf "%sbreak;\n", $indent0;
        return;
    }

    printf "%sswitch(s\[%d\]) {\n", $indent0, $n;
    printf "%sdefault:\n", $indent0;
    for my $l (sort keys %$trieref) {
        if ($l eq 'val') {
            printf "%sbreak;\n", $indent1;
            printf "%scase '\\0':\n", $indent0;
            printf "%sreturn PIDX_%s;\n", $indent1, $trieref->{'val'};
        } else {
            printf "%sbreak;\n", $indent1;
            printf "%scase '%s':", $indent0, $l;
            if (not $case_sensitive) {
                print "   case '-':" if ($l eq '_');
                printf "   case '%s':", uc $l if ($l =~ /[a-z]/);
            }
            print "\n";
            generate_code_from_trie($n + 1, $trieref->{$l});
        }
    }
    printf "%s}\n", $indent0;
    print "    return -1;\n}\n" if $n == 0;
    return "";
}

# Find long endings and cache what they resolve to
sub locate_long_endings {
    my $trieref = shift;
    my @names = keys %$trieref;
    my $num = @names;

    return (1, '', $trieref->{$names[0]}) if ($num == 1 and $names[0] eq 'val');

    if ($num == 1) {
        my ($res, $suffix, $name) = locate_long_endings($trieref->{$names[0]});
        my $e = $names[0] . $suffix;
        if ($res) {
            $trieref->{'suffix'} = $e;
            $trieref->{'name'} = $name;
        }
        return $res, $e, $name;
    }

    for my $l (@names) {
        if ($l ne 'val') {
            my ($res, $suffix, $name) = locate_long_endings($trieref->{$l});
        }
    }
    return 0, '';
}

sub produce_decoder {
    my %t = generate_trie();
    my $s;

    locate_long_endings(\%t);

    open local *STDOUT, '>', \$s;
    generate_code_from_trie(0, \%t);
    return $s;
}
