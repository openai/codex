/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2002, Oracle and/or its affiliates. All rights reserved
 * Copyright 2005 Nokia. All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <ctype.h>
#include <openssl/objects.h>
#include <openssl/comp.h>
#include <openssl/engine.h>
#include <openssl/crypto.h>
#include <openssl/conf.h>
#include <openssl/trace.h>
#include "internal/nelem.h"
#include "ssl_local.h"
#include "internal/thread_once.h"
#include "internal/cryptlib.h"
#include "internal/comp.h"
#include "internal/ssl_unwrap.h"

/* NB: make sure indices in these tables match values above */

typedef struct {
    uint32_t mask;
    int nid;
} ssl_cipher_table;

/* Table of NIDs for each cipher */
static const ssl_cipher_table ssl_cipher_table_cipher[SSL_ENC_NUM_IDX] = {
    {SSL_DES, NID_des_cbc},     /* SSL_ENC_DES_IDX 0 */
    {SSL_3DES, NID_des_ede3_cbc}, /* SSL_ENC_3DES_IDX 1 */
    {SSL_RC4, NID_rc4},         /* SSL_ENC_RC4_IDX 2 */
    {SSL_RC2, NID_rc2_cbc},     /* SSL_ENC_RC2_IDX 3 */
    {SSL_IDEA, NID_idea_cbc},   /* SSL_ENC_IDEA_IDX 4 */
    {SSL_eNULL, NID_undef},     /* SSL_ENC_NULL_IDX 5 */
    {SSL_AES128, NID_aes_128_cbc}, /* SSL_ENC_AES128_IDX 6 */
    {SSL_AES256, NID_aes_256_cbc}, /* SSL_ENC_AES256_IDX 7 */
    {SSL_CAMELLIA128, NID_camellia_128_cbc}, /* SSL_ENC_CAMELLIA128_IDX 8 */
    {SSL_CAMELLIA256, NID_camellia_256_cbc}, /* SSL_ENC_CAMELLIA256_IDX 9 */
    {SSL_eGOST2814789CNT, NID_gost89_cnt}, /* SSL_ENC_GOST89_IDX 10 */
    {SSL_SEED, NID_seed_cbc},   /* SSL_ENC_SEED_IDX 11 */
    {SSL_AES128GCM, NID_aes_128_gcm}, /* SSL_ENC_AES128GCM_IDX 12 */
    {SSL_AES256GCM, NID_aes_256_gcm}, /* SSL_ENC_AES256GCM_IDX 13 */
    {SSL_AES128CCM, NID_aes_128_ccm}, /* SSL_ENC_AES128CCM_IDX 14 */
    {SSL_AES256CCM, NID_aes_256_ccm}, /* SSL_ENC_AES256CCM_IDX 15 */
    {SSL_AES128CCM8, NID_aes_128_ccm}, /* SSL_ENC_AES128CCM8_IDX 16 */
    {SSL_AES256CCM8, NID_aes_256_ccm}, /* SSL_ENC_AES256CCM8_IDX 17 */
    {SSL_eGOST2814789CNT12, NID_gost89_cnt_12}, /* SSL_ENC_GOST8912_IDX 18 */
    {SSL_CHACHA20POLY1305, NID_chacha20_poly1305}, /* SSL_ENC_CHACHA_IDX 19 */
    {SSL_ARIA128GCM, NID_aria_128_gcm}, /* SSL_ENC_ARIA128GCM_IDX 20 */
    {SSL_ARIA256GCM, NID_aria_256_gcm}, /* SSL_ENC_ARIA256GCM_IDX 21 */
    {SSL_MAGMA, NID_magma_ctr_acpkm}, /* SSL_ENC_MAGMA_IDX */
    {SSL_KUZNYECHIK, NID_kuznyechik_ctr_acpkm}, /* SSL_ENC_KUZNYECHIK_IDX */
};

/* NB: make sure indices in this table matches values above */
static const ssl_cipher_table ssl_cipher_table_mac[SSL_MD_NUM_IDX] = {
    {SSL_MD5, NID_md5},         /* SSL_MD_MD5_IDX 0 */
    {SSL_SHA1, NID_sha1},       /* SSL_MD_SHA1_IDX 1 */
    {SSL_GOST94, NID_id_GostR3411_94}, /* SSL_MD_GOST94_IDX 2 */
    {SSL_GOST89MAC, NID_id_Gost28147_89_MAC}, /* SSL_MD_GOST89MAC_IDX 3 */
    {SSL_SHA256, NID_sha256},   /* SSL_MD_SHA256_IDX 4 */
    {SSL_SHA384, NID_sha384},   /* SSL_MD_SHA384_IDX 5 */
    {SSL_GOST12_256, NID_id_GostR3411_2012_256}, /* SSL_MD_GOST12_256_IDX 6 */
    {SSL_GOST89MAC12, NID_gost_mac_12}, /* SSL_MD_GOST89MAC12_IDX 7 */
    {SSL_GOST12_512, NID_id_GostR3411_2012_512}, /* SSL_MD_GOST12_512_IDX 8 */
    {0, NID_md5_sha1},          /* SSL_MD_MD5_SHA1_IDX 9 */
    {0, NID_sha224},            /* SSL_MD_SHA224_IDX 10 */
    {0, NID_sha512},            /* SSL_MD_SHA512_IDX 11 */
    {SSL_MAGMAOMAC, NID_magma_mac}, /* sSL_MD_MAGMAOMAC_IDX */
    {SSL_KUZNYECHIKOMAC, NID_kuznyechik_mac} /* SSL_MD_KUZNYECHIKOMAC_IDX */
};

/* *INDENT-OFF* */
static const ssl_cipher_table ssl_cipher_table_kx[] = {
    {SSL_kRSA,      NID_kx_rsa},
    {SSL_kECDHE,    NID_kx_ecdhe},
    {SSL_kDHE,      NID_kx_dhe},
    {SSL_kECDHEPSK, NID_kx_ecdhe_psk},
    {SSL_kDHEPSK,   NID_kx_dhe_psk},
    {SSL_kRSAPSK,   NID_kx_rsa_psk},
    {SSL_kPSK,      NID_kx_psk},
    {SSL_kSRP,      NID_kx_srp},
    {SSL_kGOST,     NID_kx_gost},
    {SSL_kGOST18,   NID_kx_gost18},
    {SSL_kANY,      NID_kx_any}
};

static const ssl_cipher_table ssl_cipher_table_auth[] = {
    {SSL_aRSA,    NID_auth_rsa},
    {SSL_aECDSA,  NID_auth_ecdsa},
    {SSL_aPSK,    NID_auth_psk},
    {SSL_aDSS,    NID_auth_dss},
    {SSL_aGOST01, NID_auth_gost01},
    {SSL_aGOST12, NID_auth_gost12},
    {SSL_aSRP,    NID_auth_srp},
    {SSL_aNULL,   NID_auth_null},
    {SSL_aANY,    NID_auth_any}
};
/* *INDENT-ON* */

/* Utility function for table lookup */
static int ssl_cipher_info_find(const ssl_cipher_table *table,
                                size_t table_cnt, uint32_t mask)
{
    size_t i;
    for (i = 0; i < table_cnt; i++, table++) {
        if (table->mask == mask)
            return (int)i;
    }
    return -1;
}

#define ssl_cipher_info_lookup(table, x) \
    ssl_cipher_info_find(table, OSSL_NELEM(table), x)

/*
 * PKEY_TYPE for GOST89MAC is known in advance, but, because implementation
 * is engine-provided, we'll fill it only if corresponding EVP_PKEY_METHOD is
 * found
 */
static const int default_mac_pkey_id[SSL_MD_NUM_IDX] = {
    /* MD5, SHA, GOST94, MAC89 */
    EVP_PKEY_HMAC, EVP_PKEY_HMAC, EVP_PKEY_HMAC, NID_undef,
    /* SHA256, SHA384, GOST2012_256, MAC89-12 */
    EVP_PKEY_HMAC, EVP_PKEY_HMAC, EVP_PKEY_HMAC, NID_undef,
    /* GOST2012_512 */
    EVP_PKEY_HMAC,
    /* MD5/SHA1, SHA224, SHA512, MAGMAOMAC, KUZNYECHIKOMAC */
    NID_undef, NID_undef, NID_undef, NID_undef, NID_undef
};

#define CIPHER_ADD      1
#define CIPHER_KILL     2
#define CIPHER_DEL      3
#define CIPHER_ORD      4
#define CIPHER_SPECIAL  5
/*
 * Bump the ciphers to the top of the list.
 * This rule isn't currently supported by the public cipherstring API.
 */
#define CIPHER_BUMP     6

typedef struct cipher_order_st {
    const SSL_CIPHER *cipher;
    int active;
    int dead;
    struct cipher_order_st *next, *prev;
} CIPHER_ORDER;

static const SSL_CIPHER cipher_aliases[] = {
    /* "ALL" doesn't include eNULL (must be specifically enabled) */
    {0, SSL_TXT_ALL, NULL, 0, 0, 0, ~SSL_eNULL},
    /* "COMPLEMENTOFALL" */
    {0, SSL_TXT_CMPALL, NULL, 0, 0, 0, SSL_eNULL},

    /*
     * "COMPLEMENTOFDEFAULT" (does *not* include ciphersuites not found in
     * ALL!)
     */
    {0, SSL_TXT_CMPDEF, NULL, 0, 0, 0, 0, 0, 0, 0, 0, 0, SSL_NOT_DEFAULT},

    /*
     * key exchange aliases (some of those using only a single bit here
     * combine multiple key exchange algs according to the RFCs, e.g. kDHE
     * combines DHE_DSS and DHE_RSA)
     */
    {0, SSL_TXT_kRSA, NULL, 0, SSL_kRSA},

    {0, SSL_TXT_kEDH, NULL, 0, SSL_kDHE},
    {0, SSL_TXT_kDHE, NULL, 0, SSL_kDHE},
    {0, SSL_TXT_DH, NULL, 0, SSL_kDHE},

    {0, SSL_TXT_kEECDH, NULL, 0, SSL_kECDHE},
    {0, SSL_TXT_kECDHE, NULL, 0, SSL_kECDHE},
    {0, SSL_TXT_ECDH, NULL, 0, SSL_kECDHE},

    {0, SSL_TXT_kPSK, NULL, 0, SSL_kPSK},
    {0, SSL_TXT_kRSAPSK, NULL, 0, SSL_kRSAPSK},
    {0, SSL_TXT_kECDHEPSK, NULL, 0, SSL_kECDHEPSK},
    {0, SSL_TXT_kDHEPSK, NULL, 0, SSL_kDHEPSK},
    {0, SSL_TXT_kSRP, NULL, 0, SSL_kSRP},
    {0, SSL_TXT_kGOST, NULL, 0, SSL_kGOST},
    {0, SSL_TXT_kGOST18, NULL, 0, SSL_kGOST18},

    /* server authentication aliases */
    {0, SSL_TXT_aRSA, NULL, 0, 0, SSL_aRSA},
    {0, SSL_TXT_aDSS, NULL, 0, 0, SSL_aDSS},
    {0, SSL_TXT_DSS, NULL, 0, 0, SSL_aDSS},
    {0, SSL_TXT_aNULL, NULL, 0, 0, SSL_aNULL},
    {0, SSL_TXT_aECDSA, NULL, 0, 0, SSL_aECDSA},
    {0, SSL_TXT_ECDSA, NULL, 0, 0, SSL_aECDSA},
    {0, SSL_TXT_aPSK, NULL, 0, 0, SSL_aPSK},
    {0, SSL_TXT_aGOST01, NULL, 0, 0, SSL_aGOST01},
    {0, SSL_TXT_aGOST12, NULL, 0, 0, SSL_aGOST12},
    {0, SSL_TXT_aGOST, NULL, 0, 0, SSL_aGOST01 | SSL_aGOST12},
    {0, SSL_TXT_aSRP, NULL, 0, 0, SSL_aSRP},

    /* aliases combining key exchange and server authentication */
    {0, SSL_TXT_EDH, NULL, 0, SSL_kDHE, ~SSL_aNULL},
    {0, SSL_TXT_DHE, NULL, 0, SSL_kDHE, ~SSL_aNULL},
    {0, SSL_TXT_EECDH, NULL, 0, SSL_kECDHE, ~SSL_aNULL},
    {0, SSL_TXT_ECDHE, NULL, 0, SSL_kECDHE, ~SSL_aNULL},
    {0, SSL_TXT_NULL, NULL, 0, 0, 0, SSL_eNULL},
    {0, SSL_TXT_RSA, NULL, 0, SSL_kRSA, SSL_aRSA},
    {0, SSL_TXT_ADH, NULL, 0, SSL_kDHE, SSL_aNULL},
    {0, SSL_TXT_AECDH, NULL, 0, SSL_kECDHE, SSL_aNULL},
    {0, SSL_TXT_PSK, NULL, 0, SSL_PSK},
    {0, SSL_TXT_SRP, NULL, 0, SSL_kSRP},

    /* symmetric encryption aliases */
    {0, SSL_TXT_3DES, NULL, 0, 0, 0, SSL_3DES},
    {0, SSL_TXT_RC4, NULL, 0, 0, 0, SSL_RC4},
    {0, SSL_TXT_RC2, NULL, 0, 0, 0, SSL_RC2},
    {0, SSL_TXT_IDEA, NULL, 0, 0, 0, SSL_IDEA},
    {0, SSL_TXT_SEED, NULL, 0, 0, 0, SSL_SEED},
    {0, SSL_TXT_eNULL, NULL, 0, 0, 0, SSL_eNULL},
    {0, SSL_TXT_GOST, NULL, 0, 0, 0,
     SSL_eGOST2814789CNT | SSL_eGOST2814789CNT12 | SSL_MAGMA | SSL_KUZNYECHIK},
    {0, SSL_TXT_AES128, NULL, 0, 0, 0,
     SSL_AES128 | SSL_AES128GCM | SSL_AES128CCM | SSL_AES128CCM8},
    {0, SSL_TXT_AES256, NULL, 0, 0, 0,
     SSL_AES256 | SSL_AES256GCM | SSL_AES256CCM | SSL_AES256CCM8},
    {0, SSL_TXT_AES, NULL, 0, 0, 0, SSL_AES},
    {0, SSL_TXT_AES_GCM, NULL, 0, 0, 0, SSL_AES128GCM | SSL_AES256GCM},
    {0, SSL_TXT_AES_CCM, NULL, 0, 0, 0,
     SSL_AES128CCM | SSL_AES256CCM | SSL_AES128CCM8 | SSL_AES256CCM8},
    {0, SSL_TXT_AES_CCM_8, NULL, 0, 0, 0, SSL_AES128CCM8 | SSL_AES256CCM8},
    {0, SSL_TXT_CAMELLIA128, NULL, 0, 0, 0, SSL_CAMELLIA128},
    {0, SSL_TXT_CAMELLIA256, NULL, 0, 0, 0, SSL_CAMELLIA256},
    {0, SSL_TXT_CAMELLIA, NULL, 0, 0, 0, SSL_CAMELLIA},
    {0, SSL_TXT_CHACHA20, NULL, 0, 0, 0, SSL_CHACHA20},
    {0, SSL_TXT_GOST2012_GOST8912_GOST8912, NULL, 0, 0, 0, SSL_eGOST2814789CNT12},

    {0, SSL_TXT_ARIA, NULL, 0, 0, 0, SSL_ARIA},
    {0, SSL_TXT_ARIA_GCM, NULL, 0, 0, 0, SSL_ARIA128GCM | SSL_ARIA256GCM},
    {0, SSL_TXT_ARIA128, NULL, 0, 0, 0, SSL_ARIA128GCM},
    {0, SSL_TXT_ARIA256, NULL, 0, 0, 0, SSL_ARIA256GCM},
    {0, SSL_TXT_CBC, NULL, 0, 0, 0, SSL_CBC},

    /* MAC aliases */
    {0, SSL_TXT_MD5, NULL, 0, 0, 0, 0, SSL_MD5},
    {0, SSL_TXT_SHA1, NULL, 0, 0, 0, 0, SSL_SHA1},
    {0, SSL_TXT_SHA, NULL, 0, 0, 0, 0, SSL_SHA1},
    {0, SSL_TXT_GOST94, NULL, 0, 0, 0, 0, SSL_GOST94},
    {0, SSL_TXT_GOST89MAC, NULL, 0, 0, 0, 0, SSL_GOST89MAC | SSL_GOST89MAC12},
    {0, SSL_TXT_SHA256, NULL, 0, 0, 0, 0, SSL_SHA256},
    {0, SSL_TXT_SHA384, NULL, 0, 0, 0, 0, SSL_SHA384},
    {0, SSL_TXT_GOST12, NULL, 0, 0, 0, 0, SSL_GOST12_256},

    /* protocol version aliases */
    {0, SSL_TXT_SSLV3, NULL, 0, 0, 0, 0, 0, SSL3_VERSION},
    {0, SSL_TXT_TLSV1, NULL, 0, 0, 0, 0, 0, TLS1_VERSION},
    {0, "TLSv1.0", NULL, 0, 0, 0, 0, 0, TLS1_VERSION},
    {0, SSL_TXT_TLSV1_2, NULL, 0, 0, 0, 0, 0, TLS1_2_VERSION},

    /* strength classes */
    {0, SSL_TXT_LOW, NULL, 0, 0, 0, 0, 0, 0, 0, 0, 0, SSL_LOW},
    {0, SSL_TXT_MEDIUM, NULL, 0, 0, 0, 0, 0, 0, 0, 0, 0, SSL_MEDIUM},
    {0, SSL_TXT_HIGH, NULL, 0, 0, 0, 0, 0, 0, 0, 0, 0, SSL_HIGH},
    /* FIPS 140-2 approved ciphersuite */
    {0, SSL_TXT_FIPS, NULL, 0, 0, 0, ~SSL_eNULL, 0, 0, 0, 0, 0, SSL_FIPS},

    /* "EDH-" aliases to "DHE-" labels (for backward compatibility) */
    {0, SSL3_TXT_EDH_DSS_DES_192_CBC3_SHA, NULL, 0,
     SSL_kDHE, SSL_aDSS, SSL_3DES, SSL_SHA1, 0, 0, 0, 0, SSL_HIGH | SSL_FIPS},
    {0, SSL3_TXT_EDH_RSA_DES_192_CBC3_SHA, NULL, 0,
     SSL_kDHE, SSL_aRSA, SSL_3DES, SSL_SHA1, 0, 0, 0, 0, SSL_HIGH | SSL_FIPS},

};

/*
 * Search for public key algorithm with given name and return its pkey_id if
 * it is available. Otherwise return 0
 */
#ifdef OPENSSL_NO_ENGINE

static int get_optional_pkey_id(const char *pkey_name)
{
    const EVP_PKEY_ASN1_METHOD *ameth;
    int pkey_id = 0;
    ameth = EVP_PKEY_asn1_find_str(NULL, pkey_name, -1);
    if (ameth && EVP_PKEY_asn1_get0_info(&pkey_id, NULL, NULL, NULL, NULL,
                                         ameth) > 0)
        return pkey_id;
    return 0;
}

#else

static int get_optional_pkey_id(const char *pkey_name)
{
    const EVP_PKEY_ASN1_METHOD *ameth;
    ENGINE *tmpeng = NULL;
    int pkey_id = 0;
    ameth = EVP_PKEY_asn1_find_str(&tmpeng, pkey_name, -1);
    if (ameth) {
        if (EVP_PKEY_asn1_get0_info(&pkey_id, NULL, NULL, NULL, NULL,
                                    ameth) <= 0)
            pkey_id = 0;
    }
    tls_engine_finish(tmpeng);
    return pkey_id;
}

#endif

int ssl_load_ciphers(SSL_CTX *ctx)
{
    size_t i;
    const ssl_cipher_table *t;
    EVP_KEYEXCH *kex = NULL;
    EVP_SIGNATURE *sig = NULL;

    ctx->disabled_enc_mask = 0;
    for (i = 0, t = ssl_cipher_table_cipher; i < SSL_ENC_NUM_IDX; i++, t++) {
        if (t->nid != NID_undef) {
            const EVP_CIPHER *cipher
                = ssl_evp_cipher_fetch(ctx->libctx, t->nid, ctx->propq);

            ctx->ssl_cipher_methods[i] = cipher;
            if (cipher == NULL)
                ctx->disabled_enc_mask |= t->mask;
        }
    }
    ctx->disabled_mac_mask = 0;
    for (i = 0, t = ssl_cipher_table_mac; i < SSL_MD_NUM_IDX; i++, t++) {
        const EVP_MD *md
            = ssl_evp_md_fetch(ctx->libctx, t->nid, ctx->propq);

        ctx->ssl_digest_methods[i] = md;
        if (md == NULL) {
            ctx->disabled_mac_mask |= t->mask;
        } else {
            int tmpsize = EVP_MD_get_size(md);

            if (!ossl_assert(tmpsize > 0))
                return 0;
            ctx->ssl_mac_secret_size[i] = tmpsize;
        }
    }

    ctx->disabled_mkey_mask = 0;
    ctx->disabled_auth_mask = 0;

    /*
     * We ignore any errors from the fetches below. They are expected to fail
     * if these algorithms are not available.
     */
    ERR_set_mark();
    sig = EVP_SIGNATURE_fetch(ctx->libctx, "DSA", ctx->propq);
    if (sig == NULL)
        ctx->disabled_auth_mask |= SSL_aDSS;
    else
        EVP_SIGNATURE_free(sig);
    kex = EVP_KEYEXCH_fetch(ctx->libctx, "DH", ctx->propq);
    if (kex == NULL)
        ctx->disabled_mkey_mask |= SSL_kDHE | SSL_kDHEPSK;
    else
        EVP_KEYEXCH_free(kex);
    kex = EVP_KEYEXCH_fetch(ctx->libctx, "ECDH", ctx->propq);
    if (kex == NULL)
        ctx->disabled_mkey_mask |= SSL_kECDHE | SSL_kECDHEPSK;
    else
        EVP_KEYEXCH_free(kex);
    sig = EVP_SIGNATURE_fetch(ctx->libctx, "ECDSA", ctx->propq);
    if (sig == NULL)
        ctx->disabled_auth_mask |= SSL_aECDSA;
    else
        EVP_SIGNATURE_free(sig);
    ERR_pop_to_mark();

#ifdef OPENSSL_NO_PSK
    ctx->disabled_mkey_mask |= SSL_PSK;
    ctx->disabled_auth_mask |= SSL_aPSK;
#endif
#ifdef OPENSSL_NO_SRP
    ctx->disabled_mkey_mask |= SSL_kSRP;
#endif

    /*
     * Check for presence of GOST 34.10 algorithms, and if they are not
     * present, disable appropriate auth and key exchange
     */
    memcpy(ctx->ssl_mac_pkey_id, default_mac_pkey_id,
           sizeof(ctx->ssl_mac_pkey_id));

    ctx->ssl_mac_pkey_id[SSL_MD_GOST89MAC_IDX] =
        get_optional_pkey_id(SN_id_Gost28147_89_MAC);
    if (ctx->ssl_mac_pkey_id[SSL_MD_GOST89MAC_IDX])
        ctx->ssl_mac_secret_size[SSL_MD_GOST89MAC_IDX] = 32;
    else
        ctx->disabled_mac_mask |= SSL_GOST89MAC;

    ctx->ssl_mac_pkey_id[SSL_MD_GOST89MAC12_IDX] =
        get_optional_pkey_id(SN_gost_mac_12);
    if (ctx->ssl_mac_pkey_id[SSL_MD_GOST89MAC12_IDX])
        ctx->ssl_mac_secret_size[SSL_MD_GOST89MAC12_IDX] = 32;
    else
        ctx->disabled_mac_mask |= SSL_GOST89MAC12;

    ctx->ssl_mac_pkey_id[SSL_MD_MAGMAOMAC_IDX] =
        get_optional_pkey_id(SN_magma_mac);
    if (ctx->ssl_mac_pkey_id[SSL_MD_MAGMAOMAC_IDX])
        ctx->ssl_mac_secret_size[SSL_MD_MAGMAOMAC_IDX] = 32;
    else
        ctx->disabled_mac_mask |= SSL_MAGMAOMAC;

    ctx->ssl_mac_pkey_id[SSL_MD_KUZNYECHIKOMAC_IDX] =
        get_optional_pkey_id(SN_kuznyechik_mac);
    if (ctx->ssl_mac_pkey_id[SSL_MD_KUZNYECHIKOMAC_IDX])
        ctx->ssl_mac_secret_size[SSL_MD_KUZNYECHIKOMAC_IDX] = 32;
    else
        ctx->disabled_mac_mask |= SSL_KUZNYECHIKOMAC;

    if (!get_optional_pkey_id(SN_id_GostR3410_2001))
        ctx->disabled_auth_mask |= SSL_aGOST01 | SSL_aGOST12;
    if (!get_optional_pkey_id(SN_id_GostR3410_2012_256))
        ctx->disabled_auth_mask |= SSL_aGOST12;
    if (!get_optional_pkey_id(SN_id_GostR3410_2012_512))
        ctx->disabled_auth_mask |= SSL_aGOST12;
    /*
     * Disable GOST key exchange if no GOST signature algs are available *
     */
    if ((ctx->disabled_auth_mask & (SSL_aGOST01 | SSL_aGOST12)) ==
        (SSL_aGOST01 | SSL_aGOST12))
        ctx->disabled_mkey_mask |= SSL_kGOST;

    if ((ctx->disabled_auth_mask & SSL_aGOST12) ==  SSL_aGOST12)
        ctx->disabled_mkey_mask |= SSL_kGOST18;

    return 1;
}

int ssl_cipher_get_evp_cipher(SSL_CTX *ctx, const SSL_CIPHER *sslc,
                              const EVP_CIPHER **enc)
{
    int i = ssl_cipher_info_lookup(ssl_cipher_table_cipher,
                                   sslc->algorithm_enc);

    if (i == -1) {
        *enc = NULL;
    } else {
        if (i == SSL_ENC_NULL_IDX) {
            /*
             * We assume we don't care about this coming from an ENGINE so
             * just do a normal EVP_CIPHER_fetch instead of
             * ssl_evp_cipher_fetch()
             */
            *enc = EVP_CIPHER_fetch(ctx->libctx, "NULL", ctx->propq);
            if (*enc == NULL)
                return 0;
        } else {
            const EVP_CIPHER *cipher = ctx->ssl_cipher_methods[i];

            if (cipher == NULL
                    || !ssl_evp_cipher_up_ref(cipher))
                return 0;
            *enc = ctx->ssl_cipher_methods[i];
        }
    }
    return 1;
}

int ssl_cipher_get_evp_md_mac(SSL_CTX *ctx, const SSL_CIPHER *sslc,
                              const EVP_MD **md,
                              int *mac_pkey_type, size_t *mac_secret_size)
{
    int i = ssl_cipher_info_lookup(ssl_cipher_table_mac, sslc->algorithm_mac);

    if (i == -1) {
        *md = NULL;
        if (mac_pkey_type != NULL)
            *mac_pkey_type = NID_undef;
        if (mac_secret_size != NULL)
            *mac_secret_size = 0;
    } else {
        const EVP_MD *digest = ctx->ssl_digest_methods[i];

        if (digest == NULL || !ssl_evp_md_up_ref(digest)) 
            return 0;

        *md = digest;
        if (mac_pkey_type != NULL)
            *mac_pkey_type = ctx->ssl_mac_pkey_id[i];
        if (mac_secret_size != NULL)
            *mac_secret_size = ctx->ssl_mac_secret_size[i];
    }
    return 1;
}

int ssl_cipher_get_evp(SSL_CTX *ctx, const SSL_SESSION *s,
                       const EVP_CIPHER **enc, const EVP_MD **md,
                       int *mac_pkey_type, size_t *mac_secret_size,
                       SSL_COMP **comp, int use_etm)
{
    int i;
    const SSL_CIPHER *c;

    c = s->cipher;
    if (c == NULL)
        return 0;
    if (comp != NULL) {
        SSL_COMP ctmp;
        STACK_OF(SSL_COMP) *comp_methods;

        *comp = NULL;
        ctmp.id = s->compress_meth;
        comp_methods = SSL_COMP_get_compression_methods();
        if (comp_methods != NULL) {
            i = sk_SSL_COMP_find(comp_methods, &ctmp);
            if (i >= 0)
                *comp = sk_SSL_COMP_value(comp_methods, i);
        }
        /* If were only interested in comp then return success */
        if ((enc == NULL) && (md == NULL))
            return 1;
    }

    if ((enc == NULL) || (md == NULL))
        return 0;

    if (!ssl_cipher_get_evp_cipher(ctx, c, enc))
        return 0;

    if (!ssl_cipher_get_evp_md_mac(ctx, c, md, mac_pkey_type,
                                   mac_secret_size)) {
        ssl_evp_cipher_free(*enc);
        return 0;
    }

    if ((*enc != NULL)
        && (*md != NULL
            || (EVP_CIPHER_get_flags(*enc) & EVP_CIPH_FLAG_AEAD_CIPHER))
        && (c->algorithm_mac == SSL_AEAD
            || mac_pkey_type == NULL || *mac_pkey_type != NID_undef)) {
        const EVP_CIPHER *evp = NULL;

        if (use_etm
                || s->ssl_version >> 8 != TLS1_VERSION_MAJOR
                || s->ssl_version < TLS1_VERSION)
            return 1;

        if (c->algorithm_enc == SSL_RC4
                && c->algorithm_mac == SSL_MD5)
            evp = ssl_evp_cipher_fetch(ctx->libctx, NID_rc4_hmac_md5,
                                       ctx->propq);
        else if (c->algorithm_enc == SSL_AES128
                    && c->algorithm_mac == SSL_SHA1)
            evp = ssl_evp_cipher_fetch(ctx->libctx,
                                       NID_aes_128_cbc_hmac_sha1,
                                       ctx->propq);
        else if (c->algorithm_enc == SSL_AES256
                    && c->algorithm_mac == SSL_SHA1)
             evp = ssl_evp_cipher_fetch(ctx->libctx,
                                        NID_aes_256_cbc_hmac_sha1,
                                        ctx->propq);
        else if (c->algorithm_enc == SSL_AES128
                    && c->algorithm_mac == SSL_SHA256)
            evp = ssl_evp_cipher_fetch(ctx->libctx,
                                       NID_aes_128_cbc_hmac_sha256,
                                       ctx->propq);
        else if (c->algorithm_enc == SSL_AES256
                    && c->algorithm_mac == SSL_SHA256)
            evp = ssl_evp_cipher_fetch(ctx->libctx,
                                       NID_aes_256_cbc_hmac_sha256,
                                       ctx->propq);

        if (evp != NULL) {
            ssl_evp_cipher_free(*enc);
            ssl_evp_md_free(*md);
            *enc = evp;
            *md = NULL;
        }
        return 1;
    }

    return 0;
}

const EVP_MD *ssl_md(SSL_CTX *ctx, int idx)
{
    idx &= SSL_HANDSHAKE_MAC_MASK;
    if (idx < 0 || idx >= SSL_MD_NUM_IDX)
        return NULL;
    return ctx->ssl_digest_methods[idx];
}

const EVP_MD *ssl_handshake_md(SSL_CONNECTION *s)
{
    return ssl_md(SSL_CONNECTION_GET_CTX(s), ssl_get_algorithm2(s));
}

const EVP_MD *ssl_prf_md(SSL_CONNECTION *s)
{
    return ssl_md(SSL_CONNECTION_GET_CTX(s),
                  ssl_get_algorithm2(s) >> TLS1_PRF_DGST_SHIFT);
}


#define ITEM_SEP(a) \
        (((a) == ':') || ((a) == ' ') || ((a) == ';') || ((a) == ','))

static void ll_append_tail(CIPHER_ORDER **head, CIPHER_ORDER *curr,
                           CIPHER_ORDER **tail)
{
    if (curr == *tail)
        return;
    if (curr == *head)
        *head = curr->next;
    if (curr->prev != NULL)
        curr->prev->next = curr->next;
    if (curr->next != NULL)
        curr->next->prev = curr->prev;
    (*tail)->next = curr;
    curr->prev = *tail;
    curr->next = NULL;
    *tail = curr;
}

static void ll_append_head(CIPHER_ORDER **head, CIPHER_ORDER *curr,
                           CIPHER_ORDER **tail)
{
    if (curr == *head)
        return;
    if (curr == *tail)
        *tail = curr->prev;
    if (curr->next != NULL)
        curr->next->prev = curr->prev;
    if (curr->prev != NULL)
        curr->prev->next = curr->next;
    (*head)->prev = curr;
    curr->next = *head;
    curr->prev = NULL;
    *head = curr;
}

static void ssl_cipher_collect_ciphers(const SSL_METHOD *ssl_method,
                                       int num_of_ciphers,
                                       uint32_t disabled_mkey,
                                       uint32_t disabled_auth,
                                       uint32_t disabled_enc,
                                       uint32_t disabled_mac,
                                       CIPHER_ORDER *co_list,
                                       CIPHER_ORDER **head_p,
                                       CIPHER_ORDER **tail_p)
{
    int i, co_list_num;
    const SSL_CIPHER *c;

    /*
     * We have num_of_ciphers descriptions compiled in, depending on the
     * method selected (SSLv3, TLSv1 etc).
     * These will later be sorted in a linked list with at most num
     * entries.
     */

    /* Get the initial list of ciphers */
    co_list_num = 0;            /* actual count of ciphers */
    for (i = 0; i < num_of_ciphers; i++) {
        c = ssl_method->get_cipher(i);
        /* drop those that use any of that is not available */
        if (c == NULL || !c->valid)
            continue;
        if ((c->algorithm_mkey & disabled_mkey) ||
            (c->algorithm_auth & disabled_auth) ||
            (c->algorithm_enc & disabled_enc) ||
            (c->algorithm_mac & disabled_mac))
            continue;
        if (((ssl_method->ssl3_enc->enc_flags & SSL_ENC_FLAG_DTLS) == 0) &&
            c->min_tls == 0)
            continue;
        if (((ssl_method->ssl3_enc->enc_flags & SSL_ENC_FLAG_DTLS) != 0) &&
            c->min_dtls == 0)
            continue;

        co_list[co_list_num].cipher = c;
        co_list[co_list_num].next = NULL;
        co_list[co_list_num].prev = NULL;
        co_list[co_list_num].active = 0;
        co_list_num++;
    }

    /*
     * Prepare linked list from list entries
     */
    if (co_list_num > 0) {
        co_list[0].prev = NULL;

        if (co_list_num > 1) {
            co_list[0].next = &co_list[1];

            for (i = 1; i < co_list_num - 1; i++) {
                co_list[i].prev = &co_list[i - 1];
                co_list[i].next = &co_list[i + 1];
            }

            co_list[co_list_num - 1].prev = &co_list[co_list_num - 2];
        }

        co_list[co_list_num - 1].next = NULL;

        *head_p = &co_list[0];
        *tail_p = &co_list[co_list_num - 1];
    }
}

static void ssl_cipher_collect_aliases(const SSL_CIPHER **ca_list,
                                       int num_of_group_aliases,
                                       uint32_t disabled_mkey,
                                       uint32_t disabled_auth,
                                       uint32_t disabled_enc,
                                       uint32_t disabled_mac,
                                       CIPHER_ORDER *head)
{
    CIPHER_ORDER *ciph_curr;
    const SSL_CIPHER **ca_curr;
    int i;
    uint32_t mask_mkey = ~disabled_mkey;
    uint32_t mask_auth = ~disabled_auth;
    uint32_t mask_enc = ~disabled_enc;
    uint32_t mask_mac = ~disabled_mac;

    /*
     * First, add the real ciphers as already collected
     */
    ciph_curr = head;
    ca_curr = ca_list;
    while (ciph_curr != NULL) {
        *ca_curr = ciph_curr->cipher;
        ca_curr++;
        ciph_curr = ciph_curr->next;
    }

    /*
     * Now we add the available ones from the cipher_aliases[] table.
     * They represent either one or more algorithms, some of which
     * in any affected category must be supported (set in enabled_mask),
     * or represent a cipher strength value (will be added in any case because algorithms=0).
     */
    for (i = 0; i < num_of_group_aliases; i++) {
        uint32_t algorithm_mkey = cipher_aliases[i].algorithm_mkey;
        uint32_t algorithm_auth = cipher_aliases[i].algorithm_auth;
        uint32_t algorithm_enc = cipher_aliases[i].algorithm_enc;
        uint32_t algorithm_mac = cipher_aliases[i].algorithm_mac;

        if (algorithm_mkey)
            if ((algorithm_mkey & mask_mkey) == 0)
                continue;

        if (algorithm_auth)
            if ((algorithm_auth & mask_auth) == 0)
                continue;

        if (algorithm_enc)
            if ((algorithm_enc & mask_enc) == 0)
                continue;

        if (algorithm_mac)
            if ((algorithm_mac & mask_mac) == 0)
                continue;

        *ca_curr = (SSL_CIPHER *)(cipher_aliases + i);
        ca_curr++;
    }

    *ca_curr = NULL;            /* end of list */
}

static void ssl_cipher_apply_rule(uint32_t cipher_id, uint32_t alg_mkey,
                                  uint32_t alg_auth, uint32_t alg_enc,
                                  uint32_t alg_mac, int min_tls,
                                  uint32_t algo_strength, int rule,
                                  int32_t strength_bits, CIPHER_ORDER **head_p,
                                  CIPHER_ORDER **tail_p)
{
    CIPHER_ORDER *head, *tail, *curr, *next, *last;
    const SSL_CIPHER *cp;
    int reverse = 0;

    OSSL_TRACE_BEGIN(TLS_CIPHER) {
        BIO_printf(trc_out,
                   "Applying rule %d with %08x/%08x/%08x/%08x/%08x %08x (%d)\n",
                   rule, (unsigned int)alg_mkey, (unsigned int)alg_auth,
                   (unsigned int)alg_enc, (unsigned int)alg_mac, min_tls,
                   (unsigned int)algo_strength, (int)strength_bits);
    }

    if (rule == CIPHER_DEL || rule == CIPHER_BUMP)
        reverse = 1;            /* needed to maintain sorting between currently
                                 * deleted ciphers */

    head = *head_p;
    tail = *tail_p;

    if (reverse) {
        next = tail;
        last = head;
    } else {
        next = head;
        last = tail;
    }

    curr = NULL;
    for (;;) {
        if (curr == last)
            break;

        curr = next;

        if (curr == NULL)
            break;

        next = reverse ? curr->prev : curr->next;

        cp = curr->cipher;

        /*
         * Selection criteria is either the value of strength_bits
         * or the algorithms used.
         */
        if (strength_bits >= 0) {
            if (strength_bits != cp->strength_bits)
                continue;
        } else {
            if (trc_out != NULL) {
                BIO_printf(trc_out,
                           "\nName: %s:"
                           "\nAlgo = %08x/%08x/%08x/%08x/%08x Algo_strength = %08x\n",
                           cp->name,
                           (unsigned int)cp->algorithm_mkey,
                           (unsigned int)cp->algorithm_auth,
                           (unsigned int)cp->algorithm_enc,
                           (unsigned int)cp->algorithm_mac,
                           cp->min_tls,
                           (unsigned int)cp->algo_strength);
            }
            if (cipher_id != 0 && (cipher_id != cp->id))
                continue;
            if (alg_mkey && !(alg_mkey & cp->algorithm_mkey))
                continue;
            if (alg_auth && !(alg_auth & cp->algorithm_auth))
                continue;
            if (alg_enc && !(alg_enc & cp->algorithm_enc))
                continue;
            if (alg_mac && !(alg_mac & cp->algorithm_mac))
                continue;
            if (min_tls && (min_tls != cp->min_tls))
                continue;
            if ((algo_strength & SSL_STRONG_MASK)
                && !(algo_strength & SSL_STRONG_MASK & cp->algo_strength))
                continue;
            if ((algo_strength & SSL_DEFAULT_MASK)
                && !(algo_strength & SSL_DEFAULT_MASK & cp->algo_strength))
                continue;
        }

        if (trc_out != NULL)
            BIO_printf(trc_out, "Action = %d\n", rule);

        /* add the cipher if it has not been added yet. */
        if (rule == CIPHER_ADD) {
            /* reverse == 0 */
            if (!curr->active) {
                ll_append_tail(&head, curr, &tail);
                curr->active = 1;
            }
        }
        /* Move the added cipher to this location */
        else if (rule == CIPHER_ORD) {
            /* reverse == 0 */
            if (curr->active) {
                ll_append_tail(&head, curr, &tail);
            }
        } else if (rule == CIPHER_DEL) {
            /* reverse == 1 */
            if (curr->active) {
                /*
                 * most recently deleted ciphersuites get best positions for
                 * any future CIPHER_ADD (note that the CIPHER_DEL loop works
                 * in reverse to maintain the order)
                 */
                ll_append_head(&head, curr, &tail);
                curr->active = 0;
            }
        } else if (rule == CIPHER_BUMP) {
            if (curr->active)
                ll_append_head(&head, curr, &tail);
        } else if (rule == CIPHER_KILL) {
            /* reverse == 0 */
            if (head == curr)
                head = curr->next;
            else
                curr->prev->next = curr->next;
            if (tail == curr)
                tail = curr->prev;
            curr->active = 0;
            if (curr->next != NULL)
                curr->next->prev = curr->prev;
            if (curr->prev != NULL)
                curr->prev->next = curr->next;
            curr->next = NULL;
            curr->prev = NULL;
        }
    }

    *head_p = head;
    *tail_p = tail;

    OSSL_TRACE_END(TLS_CIPHER);
}

static int ssl_cipher_strength_sort(CIPHER_ORDER **head_p,
                                    CIPHER_ORDER **tail_p)
{
    int32_t max_strength_bits;
    int i, *number_uses;
    CIPHER_ORDER *curr;

    /*
     * This routine sorts the ciphers with descending strength. The sorting
     * must keep the pre-sorted sequence, so we apply the normal sorting
     * routine as '+' movement to the end of the list.
     */
    max_strength_bits = 0;
    curr = *head_p;
    while (curr != NULL) {
        if (curr->active && (curr->cipher->strength_bits > max_strength_bits))
            max_strength_bits = curr->cipher->strength_bits;
        curr = curr->next;
    }

    number_uses = OPENSSL_zalloc(sizeof(int) * (max_strength_bits + 1));
    if (number_uses == NULL)
        return 0;

    /*
     * Now find the strength_bits values actually used
     */
    curr = *head_p;
    while (curr != NULL) {
        if (curr->active)
            number_uses[curr->cipher->strength_bits]++;
        curr = curr->next;
    }
    /*
     * Go through the list of used strength_bits values in descending
     * order.
     */
    for (i = max_strength_bits; i >= 0; i--)
        if (number_uses[i] > 0)
            ssl_cipher_apply_rule(0, 0, 0, 0, 0, 0, 0, CIPHER_ORD, i, head_p,
                                  tail_p);

    OPENSSL_free(number_uses);
    return 1;
}

static int ssl_cipher_process_rulestr(const char *rule_str,
                                      CIPHER_ORDER **head_p,
                                      CIPHER_ORDER **tail_p,
                                      const SSL_CIPHER **ca_list, CERT *c)
{
    uint32_t alg_mkey, alg_auth, alg_enc, alg_mac, algo_strength;
    int min_tls;
    const char *l, *buf;
    int j, multi, found, rule, retval, ok, buflen;
    uint32_t cipher_id = 0;
    char ch;

    retval = 1;
    l = rule_str;
    for (;;) {
        ch = *l;

        if (ch == '\0')
            break;              /* done */
        if (ch == '-') {
            rule = CIPHER_DEL;
            l++;
        } else if (ch == '+') {
            rule = CIPHER_ORD;
            l++;
        } else if (ch == '!') {
            rule = CIPHER_KILL;
            l++;
        } else if (ch == '@') {
            rule = CIPHER_SPECIAL;
            l++;
        } else {
            rule = CIPHER_ADD;
        }

        if (ITEM_SEP(ch)) {
            l++;
            continue;
        }

        alg_mkey = 0;
        alg_auth = 0;
        alg_enc = 0;
        alg_mac = 0;
        min_tls = 0;
        algo_strength = 0;

        for (;;) {
            ch = *l;
            buf = l;
            buflen = 0;
#ifndef CHARSET_EBCDIC
            while (((ch >= 'A') && (ch <= 'Z')) ||
                   ((ch >= '0') && (ch <= '9')) ||
                   ((ch >= 'a') && (ch <= 'z')) ||
                   (ch == '-') || (ch == '_') || (ch == '.') || (ch == '='))
#else
            while (isalnum((unsigned char)ch) || (ch == '-') || (ch == '_') || (ch == '.')
                   || (ch == '='))
#endif
            {
                ch = *(++l);
                buflen++;
            }

            if (buflen == 0) {
                /*
                 * We hit something we cannot deal with,
                 * it is no command or separator nor
                 * alphanumeric, so we call this an error.
                 */
                ERR_raise(ERR_LIB_SSL, SSL_R_INVALID_COMMAND);
                return 0;
            }

            if (rule == CIPHER_SPECIAL) {
                found = 0;      /* unused -- avoid compiler warning */
                break;          /* special treatment */
            }

            /* check for multi-part specification */
            if (ch == '+') {
                multi = 1;
                l++;
            } else {
                multi = 0;
            }

            /*
             * Now search for the cipher alias in the ca_list. Be careful
             * with the strncmp, because the "buflen" limitation
             * will make the rule "ADH:SOME" and the cipher
             * "ADH-MY-CIPHER" look like a match for buflen=3.
             * So additionally check whether the cipher name found
             * has the correct length. We can save a strlen() call:
             * just checking for the '\0' at the right place is
             * sufficient, we have to strncmp() anyway. (We cannot
             * use strcmp(), because buf is not '\0' terminated.)
             */
            j = found = 0;
            cipher_id = 0;
            while (ca_list[j]) {
                if (strncmp(buf, ca_list[j]->name, buflen) == 0
                    && (ca_list[j]->name[buflen] == '\0')) {
                    found = 1;
                    break;
                } else if (ca_list[j]->stdname != NULL
                           && strncmp(buf, ca_list[j]->stdname, buflen) == 0
                           && ca_list[j]->stdname[buflen] == '\0') {
                    found = 1;
                    break;
                } else
                    j++;
            }

            if (!found)
                break;          /* ignore this entry */

            if (ca_list[j]->algorithm_mkey) {
                if (alg_mkey) {
                    alg_mkey &= ca_list[j]->algorithm_mkey;
                    if (!alg_mkey) {
                        found = 0;
                        break;
                    }
                } else {
                    alg_mkey = ca_list[j]->algorithm_mkey;
                }
            }

            if (ca_list[j]->algorithm_auth) {
                if (alg_auth) {
                    alg_auth &= ca_list[j]->algorithm_auth;
                    if (!alg_auth) {
                        found = 0;
                        break;
                    }
                } else {
                    alg_auth = ca_list[j]->algorithm_auth;
                }
            }

            if (ca_list[j]->algorithm_enc) {
                if (alg_enc) {
                    alg_enc &= ca_list[j]->algorithm_enc;
                    if (!alg_enc) {
                        found = 0;
                        break;
                    }
                } else {
                    alg_enc = ca_list[j]->algorithm_enc;
                }
            }

            if (ca_list[j]->algorithm_mac) {
                if (alg_mac) {
                    alg_mac &= ca_list[j]->algorithm_mac;
                    if (!alg_mac) {
                        found = 0;
                        break;
                    }
                } else {
                    alg_mac = ca_list[j]->algorithm_mac;
                }
            }

            if (ca_list[j]->algo_strength & SSL_STRONG_MASK) {
                if (algo_strength & SSL_STRONG_MASK) {
                    algo_strength &=
                        (ca_list[j]->algo_strength & SSL_STRONG_MASK) |
                        ~SSL_STRONG_MASK;
                    if (!(algo_strength & SSL_STRONG_MASK)) {
                        found = 0;
                        break;
                    }
                } else {
                    algo_strength = ca_list[j]->algo_strength & SSL_STRONG_MASK;
                }
            }

            if (ca_list[j]->algo_strength & SSL_DEFAULT_MASK) {
                if (algo_strength & SSL_DEFAULT_MASK) {
                    algo_strength &=
                        (ca_list[j]->algo_strength & SSL_DEFAULT_MASK) |
                        ~SSL_DEFAULT_MASK;
                    if (!(algo_strength & SSL_DEFAULT_MASK)) {
                        found = 0;
                        break;
                    }
                } else {
                    algo_strength |=
                        ca_list[j]->algo_strength & SSL_DEFAULT_MASK;
                }
            }

            if (ca_list[j]->valid) {
                /*
                 * explicit ciphersuite found; its protocol version does not
                 * become part of the search pattern!
                 */

                cipher_id = ca_list[j]->id;
            } else {
                /*
                 * not an explicit ciphersuite; only in this case, the
                 * protocol version is considered part of the search pattern
                 */

                if (ca_list[j]->min_tls) {
                    if (min_tls != 0 && min_tls != ca_list[j]->min_tls) {
                        found = 0;
                        break;
                    } else {
                        min_tls = ca_list[j]->min_tls;
                    }
                }
            }

            if (!multi)
                break;
        }

        /*
         * Ok, we have the rule, now apply it
         */
        if (rule == CIPHER_SPECIAL) { /* special command */
            ok = 0;
            if ((buflen == 8) && HAS_PREFIX(buf, "STRENGTH")) {
                ok = ssl_cipher_strength_sort(head_p, tail_p);
            } else if (buflen == 10 && CHECK_AND_SKIP_PREFIX(buf, "SECLEVEL=")) {
                int level = *buf - '0';
                if (level < 0 || level > 5) {
                    ERR_raise(ERR_LIB_SSL, SSL_R_INVALID_COMMAND);
                } else {
                    c->sec_level = level;
                    ok = 1;
                }
            } else {
                ERR_raise(ERR_LIB_SSL, SSL_R_INVALID_COMMAND);
            }
            if (ok == 0)
                retval = 0;
            /*
             * We do not support any "multi" options
             * together with "@", so throw away the
             * rest of the command, if any left, until
             * end or ':' is found.
             */
            while ((*l != '\0') && !ITEM_SEP(*l))
                l++;
        } else if (found) {
            ssl_cipher_apply_rule(cipher_id,
                                  alg_mkey, alg_auth, alg_enc, alg_mac,
                                  min_tls, algo_strength, rule, -1, head_p,
                                  tail_p);
        } else {
            while ((*l != '\0') && !ITEM_SEP(*l))
                l++;
        }
        if (*l == '\0')
            break;              /* done */
    }

    return retval;
}

static int check_suiteb_cipher_list(const SSL_METHOD *meth, CERT *c,
                                    const char **prule_str)
{
    unsigned int suiteb_flags = 0, suiteb_comb2 = 0;
    if (HAS_PREFIX(*prule_str, "SUITEB128ONLY")) {
        suiteb_flags = SSL_CERT_FLAG_SUITEB_128_LOS_ONLY;
    } else if (HAS_PREFIX(*prule_str, "SUITEB128C2")) {
        suiteb_comb2 = 1;
        suiteb_flags = SSL_CERT_FLAG_SUITEB_128_LOS;
    } else if (HAS_PREFIX(*prule_str, "SUITEB128")) {
        suiteb_flags = SSL_CERT_FLAG_SUITEB_128_LOS;
    } else if (HAS_PREFIX(*prule_str, "SUITEB192")) {
        suiteb_flags = SSL_CERT_FLAG_SUITEB_192_LOS;
    }

    if (suiteb_flags) {
        c->cert_flags &= ~SSL_CERT_FLAG_SUITEB_128_LOS;
        c->cert_flags |= suiteb_flags;
    } else {
        suiteb_flags = c->cert_flags & SSL_CERT_FLAG_SUITEB_128_LOS;
    }

    if (!suiteb_flags)
        return 1;
    /* Check version: if TLS 1.2 ciphers allowed we can use Suite B */

    if (!(meth->ssl3_enc->enc_flags & SSL_ENC_FLAG_TLS1_2_CIPHERS)) {
        ERR_raise(ERR_LIB_SSL, SSL_R_AT_LEAST_TLS_1_2_NEEDED_IN_SUITEB_MODE);
        return 0;
    }

    switch (suiteb_flags) {
    case SSL_CERT_FLAG_SUITEB_128_LOS:
        if (suiteb_comb2)
            *prule_str = "ECDHE-ECDSA-AES256-GCM-SHA384";
        else
            *prule_str =
                "ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384";
        break;
    case SSL_CERT_FLAG_SUITEB_128_LOS_ONLY:
        *prule_str = "ECDHE-ECDSA-AES128-GCM-SHA256";
        break;
    case SSL_CERT_FLAG_SUITEB_192_LOS:
        *prule_str = "ECDHE-ECDSA-AES256-GCM-SHA384";
        break;
    }
    return 1;
}

static int ciphersuite_cb(const char *elem, int len, void *arg)
{
    STACK_OF(SSL_CIPHER) *ciphersuites = (STACK_OF(SSL_CIPHER) *)arg;
    const SSL_CIPHER *cipher;
    /* Arbitrary sized temp buffer for the cipher name. Should be big enough */
    char name[80];

    if (len > (int)(sizeof(name) - 1))
        /* Anyway return 1 so we can parse rest of the list */
        return 1;

    memcpy(name, elem, len);
    name[len] = '\0';

    cipher = ssl3_get_cipher_by_std_name(name);
    if (cipher == NULL)
        /* Ciphersuite not found but return 1 to parse rest of the list */
        return 1;

    if (!sk_SSL_CIPHER_push(ciphersuites, cipher)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    return 1;
}

static __owur int set_ciphersuites(STACK_OF(SSL_CIPHER) **currciphers, const char *str)
{
    STACK_OF(SSL_CIPHER) *newciphers = sk_SSL_CIPHER_new_null();

    if (newciphers == NULL)
        return 0;

    /* Parse the list. We explicitly allow an empty list */
    if (*str != '\0'
            && (CONF_parse_list(str, ':', 1, ciphersuite_cb, newciphers) <= 0
                || sk_SSL_CIPHER_num(newciphers) == 0)) {
        ERR_raise(ERR_LIB_SSL, SSL_R_NO_CIPHER_MATCH);
        sk_SSL_CIPHER_free(newciphers);
        return 0;
    }
    sk_SSL_CIPHER_free(*currciphers);
    *currciphers = newciphers;

    return 1;
}

static int update_cipher_list_by_id(STACK_OF(SSL_CIPHER) **cipher_list_by_id,
                                    STACK_OF(SSL_CIPHER) *cipherstack)
{
    STACK_OF(SSL_CIPHER) *tmp_cipher_list = sk_SSL_CIPHER_dup(cipherstack);

    if (tmp_cipher_list == NULL) {
        return 0;
    }

    sk_SSL_CIPHER_free(*cipher_list_by_id);
    *cipher_list_by_id = tmp_cipher_list;

    (void)sk_SSL_CIPHER_set_cmp_func(*cipher_list_by_id, ssl_cipher_ptr_id_cmp);
    sk_SSL_CIPHER_sort(*cipher_list_by_id);

    return 1;
}

static int update_cipher_list(SSL_CTX *ctx,
                              STACK_OF(SSL_CIPHER) **cipher_list,
                              STACK_OF(SSL_CIPHER) **cipher_list_by_id,
                              STACK_OF(SSL_CIPHER) *tls13_ciphersuites)
{
    int i;
    STACK_OF(SSL_CIPHER) *tmp_cipher_list = sk_SSL_CIPHER_dup(*cipher_list);

    if (tmp_cipher_list == NULL)
        return 0;

    /*
     * Delete any existing TLSv1.3 ciphersuites. These are always first in the
     * list.
     */
    while (sk_SSL_CIPHER_num(tmp_cipher_list) > 0
           && sk_SSL_CIPHER_value(tmp_cipher_list, 0)->min_tls
              == TLS1_3_VERSION)
        (void)sk_SSL_CIPHER_delete(tmp_cipher_list, 0);

    /* Insert the new TLSv1.3 ciphersuites */
    for (i = sk_SSL_CIPHER_num(tls13_ciphersuites) - 1; i >= 0; i--) {
        const SSL_CIPHER *sslc = sk_SSL_CIPHER_value(tls13_ciphersuites, i);

        /* Don't include any TLSv1.3 ciphersuites that are disabled */
        if ((sslc->algorithm_enc & ctx->disabled_enc_mask) == 0
                && (ssl_cipher_table_mac[sslc->algorithm2
                                         & SSL_HANDSHAKE_MAC_MASK].mask
                    & ctx->disabled_mac_mask) == 0) {
            sk_SSL_CIPHER_unshift(tmp_cipher_list, sslc);
        }
    }

    if (!update_cipher_list_by_id(cipher_list_by_id, tmp_cipher_list)) {
        sk_SSL_CIPHER_free(tmp_cipher_list);
        return 0;
    }

    sk_SSL_CIPHER_free(*cipher_list);
    *cipher_list = tmp_cipher_list;

    return 1;
}

int SSL_CTX_set_ciphersuites(SSL_CTX *ctx, const char *str)
{
    int ret = set_ciphersuites(&(ctx->tls13_ciphersuites), str);

    if (ret && ctx->cipher_list != NULL)
        return update_cipher_list(ctx, &ctx->cipher_list, &ctx->cipher_list_by_id,
                                  ctx->tls13_ciphersuites);

    return ret;
}

int SSL_set_ciphersuites(SSL *s, const char *str)
{
    STACK_OF(SSL_CIPHER) *cipher_list;
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL(s);
    int ret;

    if (sc == NULL)
        return 0;

    ret = set_ciphersuites(&(sc->tls13_ciphersuites), str);

    if (sc->cipher_list == NULL) {
        if ((cipher_list = SSL_get_ciphers(s)) != NULL)
            sc->cipher_list = sk_SSL_CIPHER_dup(cipher_list);
    }
    if (ret && sc->cipher_list != NULL)
        return update_cipher_list(s->ctx, &sc->cipher_list,
                                  &sc->cipher_list_by_id,
                                  sc->tls13_ciphersuites);

    return ret;
}

STACK_OF(SSL_CIPHER) *ssl_create_cipher_list(SSL_CTX *ctx,
                                             STACK_OF(SSL_CIPHER) *tls13_ciphersuites,
                                             STACK_OF(SSL_CIPHER) **cipher_list,
                                             STACK_OF(SSL_CIPHER) **cipher_list_by_id,
                                             const char *rule_str,
                                             CERT *c)
{
    int ok, num_of_ciphers, num_of_alias_max, num_of_group_aliases, i;
    uint32_t disabled_mkey, disabled_auth, disabled_enc, disabled_mac;
    STACK_OF(SSL_CIPHER) *cipherstack;
    const char *rule_p;
    CIPHER_ORDER *co_list = NULL, *head = NULL, *tail = NULL, *curr;
    const SSL_CIPHER **ca_list = NULL;
    const SSL_METHOD *ssl_method = ctx->method;

    /*
     * Return with error if nothing to do.
     */
    if (rule_str == NULL || cipher_list == NULL || cipher_list_by_id == NULL)
        return NULL;

    if (!check_suiteb_cipher_list(ssl_method, c, &rule_str))
        return NULL;

    /*
     * To reduce the work to do we only want to process the compiled
     * in algorithms, so we first get the mask of disabled ciphers.
     */

    disabled_mkey = ctx->disabled_mkey_mask;
    disabled_auth = ctx->disabled_auth_mask;
    disabled_enc = ctx->disabled_enc_mask;
    disabled_mac = ctx->disabled_mac_mask;

    /*
     * Now we have to collect the available ciphers from the compiled
     * in ciphers. We cannot get more than the number compiled in, so
     * it is used for allocation.
     */
    num_of_ciphers = ssl_method->num_ciphers();

    if (num_of_ciphers > 0) {
        co_list = OPENSSL_malloc(sizeof(*co_list) * num_of_ciphers);
        if (co_list == NULL)
            return NULL;          /* Failure */
    }

    ssl_cipher_collect_ciphers(ssl_method, num_of_ciphers,
                               disabled_mkey, disabled_auth, disabled_enc,
                               disabled_mac, co_list, &head, &tail);

    /* Now arrange all ciphers by preference. */

    /*
     * Everything else being equal, prefer ephemeral ECDH over other key
     * exchange mechanisms.
     * For consistency, prefer ECDSA over RSA (though this only matters if the
     * server has both certificates, and is using the DEFAULT, or a client
     * preference).
     */
    ssl_cipher_apply_rule(0, SSL_kECDHE, SSL_aECDSA, 0, 0, 0, 0, CIPHER_ADD,
                          -1, &head, &tail);
    ssl_cipher_apply_rule(0, SSL_kECDHE, 0, 0, 0, 0, 0, CIPHER_ADD, -1, &head,
                          &tail);
    ssl_cipher_apply_rule(0, SSL_kECDHE, 0, 0, 0, 0, 0, CIPHER_DEL, -1, &head,
                          &tail);

    /* Within each strength group, we prefer GCM over CHACHA... */
    ssl_cipher_apply_rule(0, 0, 0, SSL_AESGCM, 0, 0, 0, CIPHER_ADD, -1,
                          &head, &tail);
    ssl_cipher_apply_rule(0, 0, 0, SSL_CHACHA20, 0, 0, 0, CIPHER_ADD, -1,
                          &head, &tail);

    /*
     * ...and generally, our preferred cipher is AES.
     * Note that AEADs will be bumped to take preference after sorting by
     * strength.
     */
    ssl_cipher_apply_rule(0, 0, 0, SSL_AES ^ SSL_AESGCM, 0, 0, 0, CIPHER_ADD,
                          -1, &head, &tail);

    /* Temporarily enable everything else for sorting */
    ssl_cipher_apply_rule(0, 0, 0, 0, 0, 0, 0, CIPHER_ADD, -1, &head, &tail);

    /* Low priority for MD5 */
    ssl_cipher_apply_rule(0, 0, 0, 0, SSL_MD5, 0, 0, CIPHER_ORD, -1, &head,
                          &tail);

    /*
     * Move anonymous ciphers to the end.  Usually, these will remain
     * disabled. (For applications that allow them, they aren't too bad, but
     * we prefer authenticated ciphers.)
     */
    ssl_cipher_apply_rule(0, 0, SSL_aNULL, 0, 0, 0, 0, CIPHER_ORD, -1, &head,
                          &tail);

    ssl_cipher_apply_rule(0, SSL_kRSA, 0, 0, 0, 0, 0, CIPHER_ORD, -1, &head,
                          &tail);
    ssl_cipher_apply_rule(0, SSL_kPSK, 0, 0, 0, 0, 0, CIPHER_ORD, -1, &head,
                          &tail);

    /* RC4 is sort-of broken -- move to the end */
    ssl_cipher_apply_rule(0, 0, 0, SSL_RC4, 0, 0, 0, CIPHER_ORD, -1, &head,
                          &tail);

    /*
     * Now sort by symmetric encryption strength.  The above ordering remains
     * in force within each class
     */
    if (!ssl_cipher_strength_sort(&head, &tail)) {
        OPENSSL_free(co_list);
        return NULL;
    }

    /*
     * Partially overrule strength sort to prefer TLS 1.2 ciphers/PRFs.
     */
    ssl_cipher_apply_rule(0, 0, 0, 0, 0, TLS1_2_VERSION, 0, CIPHER_BUMP, -1,
                          &head, &tail);

    /*
     * Irrespective of strength, enforce the following order:
     * (EC)DHE + AEAD > (EC)DHE > rest of AEAD > rest.
     * Within each group, ciphers remain sorted by strength and previous
     * preference, i.e.,
     * 1) ECDHE > DHE
     * 2) GCM > CHACHA
     * 3) AES > rest
     * 4) TLS 1.2 > legacy
     *
     * Because we now bump ciphers to the top of the list, we proceed in
     * reverse order of preference.
     */
    ssl_cipher_apply_rule(0, 0, 0, 0, SSL_AEAD, 0, 0, CIPHER_BUMP, -1,
                          &head, &tail);
    ssl_cipher_apply_rule(0, SSL_kDHE | SSL_kECDHE, 0, 0, 0, 0, 0,
                          CIPHER_BUMP, -1, &head, &tail);
    ssl_cipher_apply_rule(0, SSL_kDHE | SSL_kECDHE, 0, 0, SSL_AEAD, 0, 0,
                          CIPHER_BUMP, -1, &head, &tail);

    /* Now disable everything (maintaining the ordering!) */
    ssl_cipher_apply_rule(0, 0, 0, 0, 0, 0, 0, CIPHER_DEL, -1, &head, &tail);

    /*
     * We also need cipher aliases for selecting based on the rule_str.
     * There might be two types of entries in the rule_str: 1) names
     * of ciphers themselves 2) aliases for groups of ciphers.
     * For 1) we need the available ciphers and for 2) the cipher
     * groups of cipher_aliases added together in one list (otherwise
     * we would be happy with just the cipher_aliases table).
     */
    num_of_group_aliases = OSSL_NELEM(cipher_aliases);
    num_of_alias_max = num_of_ciphers + num_of_group_aliases + 1;
    ca_list = OPENSSL_malloc(sizeof(*ca_list) * num_of_alias_max);
    if (ca_list == NULL) {
        OPENSSL_free(co_list);
        return NULL;          /* Failure */
    }
    ssl_cipher_collect_aliases(ca_list, num_of_group_aliases,
                               disabled_mkey, disabled_auth, disabled_enc,
                               disabled_mac, head);

    /*
     * If the rule_string begins with DEFAULT, apply the default rule
     * before using the (possibly available) additional rules.
     */
    ok = 1;
    rule_p = rule_str;
    if (HAS_PREFIX(rule_str, "DEFAULT")) {
        ok = ssl_cipher_process_rulestr(OSSL_default_cipher_list(),
                                        &head, &tail, ca_list, c);
        rule_p += 7;
        if (*rule_p == ':')
            rule_p++;
    }

    if (ok && (rule_p[0] != '\0'))
        ok = ssl_cipher_process_rulestr(rule_p, &head, &tail, ca_list, c);

    OPENSSL_free(ca_list);      /* Not needed anymore */

    if (!ok) {                  /* Rule processing failure */
        OPENSSL_free(co_list);
        return NULL;
    }

    /*
     * Allocate new "cipherstack" for the result, return with error
     * if we cannot get one.
     */
    if ((cipherstack = sk_SSL_CIPHER_new_null()) == NULL) {
        OPENSSL_free(co_list);
        return NULL;
    }

    /* Add TLSv1.3 ciphers first - we always prefer those if possible */
    for (i = 0; i < sk_SSL_CIPHER_num(tls13_ciphersuites); i++) {
        const SSL_CIPHER *sslc = sk_SSL_CIPHER_value(tls13_ciphersuites, i);

        /* Don't include any TLSv1.3 ciphers that are disabled */
        if ((sslc->algorithm_enc & disabled_enc) != 0
                || (ssl_cipher_table_mac[sslc->algorithm2
                                         & SSL_HANDSHAKE_MAC_MASK].mask
                    & ctx->disabled_mac_mask) != 0) {
            sk_SSL_CIPHER_delete(tls13_ciphersuites, i);
            i--;
            continue;
        }

        if (!sk_SSL_CIPHER_push(cipherstack, sslc)) {
            OPENSSL_free(co_list);
            sk_SSL_CIPHER_free(cipherstack);
            return NULL;
        }
    }

    OSSL_TRACE_BEGIN(TLS_CIPHER) {
        BIO_printf(trc_out, "cipher selection:\n");
    }
    /*
     * The cipher selection for the list is done. The ciphers are added
     * to the resulting precedence to the STACK_OF(SSL_CIPHER).
     */
    for (curr = head; curr != NULL; curr = curr->next) {
        if (curr->active) {
            if (!sk_SSL_CIPHER_push(cipherstack, curr->cipher)) {
                OPENSSL_free(co_list);
                sk_SSL_CIPHER_free(cipherstack);
                OSSL_TRACE_CANCEL(TLS_CIPHER);
                return NULL;
            }
            if (trc_out != NULL)
                BIO_printf(trc_out, "<%s>\n", curr->cipher->name);
        }
    }
    OPENSSL_free(co_list);      /* Not needed any longer */
    OSSL_TRACE_END(TLS_CIPHER);

    if (!update_cipher_list_by_id(cipher_list_by_id, cipherstack)) {
        sk_SSL_CIPHER_free(cipherstack);
        return NULL;
    }
    sk_SSL_CIPHER_free(*cipher_list);
    *cipher_list = cipherstack;

    return cipherstack;
}

char *SSL_CIPHER_description(const SSL_CIPHER *cipher, char *buf, int len)
{
    const char *ver;
    const char *kx, *au, *enc, *mac;
    uint32_t alg_mkey, alg_auth, alg_enc, alg_mac;
    static const char *const format = "%-30s %-7s Kx=%-8s Au=%-5s Enc=%-22s Mac=%-4s\n";

    if (buf == NULL) {
        len = 128;
        if ((buf = OPENSSL_malloc(len)) == NULL)
            return NULL;
    } else if (len < 128) {
        return NULL;
    }

    alg_mkey = cipher->algorithm_mkey;
    alg_auth = cipher->algorithm_auth;
    alg_enc = cipher->algorithm_enc;
    alg_mac = cipher->algorithm_mac;

    ver = ssl_protocol_to_string(cipher->min_tls);

    switch (alg_mkey) {
    case SSL_kRSA:
        kx = "RSA";
        break;
    case SSL_kDHE:
        kx = "DH";
        break;
    case SSL_kECDHE:
        kx = "ECDH";
        break;
    case SSL_kPSK:
        kx = "PSK";
        break;
    case SSL_kRSAPSK:
        kx = "RSAPSK";
        break;
    case SSL_kECDHEPSK:
        kx = "ECDHEPSK";
        break;
    case SSL_kDHEPSK:
        kx = "DHEPSK";
        break;
    case SSL_kSRP:
        kx = "SRP";
        break;
    case SSL_kGOST:
        kx = "GOST";
        break;
    case SSL_kGOST18:
        kx = "GOST18";
        break;
    case SSL_kANY:
        kx = "any";
        break;
    default:
        kx = "unknown";
    }

    switch (alg_auth) {
    case SSL_aRSA:
        au = "RSA";
        break;
    case SSL_aDSS:
        au = "DSS";
        break;
    case SSL_aNULL:
        au = "None";
        break;
    case SSL_aECDSA:
        au = "ECDSA";
        break;
    case SSL_aPSK:
        au = "PSK";
        break;
    case SSL_aSRP:
        au = "SRP";
        break;
    case SSL_aGOST01:
        au = "GOST01";
        break;
    /* New GOST ciphersuites have both SSL_aGOST12 and SSL_aGOST01 bits */
    case (SSL_aGOST12 | SSL_aGOST01):
        au = "GOST12";
        break;
    case SSL_aANY:
        au = "any";
        break;
    default:
        au = "unknown";
        break;
    }

    switch (alg_enc) {
    case SSL_DES:
        enc = "DES(56)";
        break;
    case SSL_3DES:
        enc = "3DES(168)";
        break;
    case SSL_RC4:
        enc = "RC4(128)";
        break;
    case SSL_RC2:
        enc = "RC2(128)";
        break;
    case SSL_IDEA:
        enc = "IDEA(128)";
        break;
    case SSL_eNULL:
        enc = "None";
        break;
    case SSL_AES128:
        enc = "AES(128)";
        break;
    case SSL_AES256:
        enc = "AES(256)";
        break;
    case SSL_AES128GCM:
        enc = "AESGCM(128)";
        break;
    case SSL_AES256GCM:
        enc = "AESGCM(256)";
        break;
    case SSL_AES128CCM:
        enc = "AESCCM(128)";
        break;
    case SSL_AES256CCM:
        enc = "AESCCM(256)";
        break;
    case SSL_AES128CCM8:
        enc = "AESCCM8(128)";
        break;
    case SSL_AES256CCM8:
        enc = "AESCCM8(256)";
        break;
    case SSL_CAMELLIA128:
        enc = "Camellia(128)";
        break;
    case SSL_CAMELLIA256:
        enc = "Camellia(256)";
        break;
    case SSL_ARIA128GCM:
        enc = "ARIAGCM(128)";
        break;
    case SSL_ARIA256GCM:
        enc = "ARIAGCM(256)";
        break;
    case SSL_SEED:
        enc = "SEED(128)";
        break;
    case SSL_eGOST2814789CNT:
    case SSL_eGOST2814789CNT12:
        enc = "GOST89(256)";
        break;
    case SSL_MAGMA:
        enc = "MAGMA";
        break;
    case SSL_KUZNYECHIK:
        enc = "KUZNYECHIK";
        break;
    case SSL_CHACHA20POLY1305:
        enc = "CHACHA20/POLY1305(256)";
        break;
    default:
        enc = "unknown";
        break;
    }

    switch (alg_mac) {
    case SSL_MD5:
        mac = "MD5";
        break;
    case SSL_SHA1:
        mac = "SHA1";
        break;
    case SSL_SHA256:
        mac = "SHA256";
        break;
    case SSL_SHA384:
        mac = "SHA384";
        break;
    case SSL_AEAD:
        mac = "AEAD";
        break;
    case SSL_GOST89MAC:
    case SSL_GOST89MAC12:
        mac = "GOST89";
        break;
    case SSL_GOST94:
        mac = "GOST94";
        break;
    case SSL_GOST12_256:
    case SSL_GOST12_512:
        mac = "GOST2012";
        break;
    default:
        mac = "unknown";
        break;
    }

    BIO_snprintf(buf, len, format, cipher->name, ver, kx, au, enc, mac);

    return buf;
}

const char *SSL_CIPHER_get_version(const SSL_CIPHER *c)
{
    if (c == NULL)
        return "(NONE)";

    /*
     * Backwards-compatibility crutch.  In almost all contexts we report TLS
     * 1.0 as "TLSv1", but for ciphers we report "TLSv1.0".
     */
    if (c->min_tls == TLS1_VERSION)
        return "TLSv1.0";
    return ssl_protocol_to_string(c->min_tls);
}

/* return the actual cipher being used */
const char *SSL_CIPHER_get_name(const SSL_CIPHER *c)
{
    if (c != NULL)
        return c->name;
    return "(NONE)";
}

/* return the actual cipher being used in RFC standard name */
const char *SSL_CIPHER_standard_name(const SSL_CIPHER *c)
{
    if (c != NULL)
        return c->stdname;
    return "(NONE)";
}

/* return the OpenSSL name based on given RFC standard name */
const char *OPENSSL_cipher_name(const char *stdname)
{
    const SSL_CIPHER *c;

    if (stdname == NULL)
        return "(NONE)";
    c = ssl3_get_cipher_by_std_name(stdname);
    return SSL_CIPHER_get_name(c);
}

/* number of bits for symmetric cipher */
int SSL_CIPHER_get_bits(const SSL_CIPHER *c, int *alg_bits)
{
    int ret = 0;

    if (c != NULL) {
        if (alg_bits != NULL)
            *alg_bits = (int)c->alg_bits;
        ret = (int)c->strength_bits;
    }
    return ret;
}

uint32_t SSL_CIPHER_get_id(const SSL_CIPHER *c)
{
    return c->id;
}

uint16_t SSL_CIPHER_get_protocol_id(const SSL_CIPHER *c)
{
    return c->id & 0xFFFF;
}

SSL_COMP *ssl3_comp_find(STACK_OF(SSL_COMP) *sk, int n)
{
    SSL_COMP *ctmp;
    SSL_COMP srch_key;
    int i;

    if ((n == 0) || (sk == NULL))
        return NULL;
    srch_key.id = n;
    i = sk_SSL_COMP_find(sk, &srch_key);
    if (i >= 0)
        ctmp = sk_SSL_COMP_value(sk, i);
    else
        ctmp = NULL;

    return ctmp;
}

#ifdef OPENSSL_NO_COMP
STACK_OF(SSL_COMP) *SSL_COMP_get_compression_methods(void)
{
    return NULL;
}

STACK_OF(SSL_COMP) *SSL_COMP_set0_compression_methods(STACK_OF(SSL_COMP)
                                                      *meths)
{
    return meths;
}

int SSL_COMP_add_compression_method(int id, COMP_METHOD *cm)
{
    return 1;
}

#else
STACK_OF(SSL_COMP) *SSL_COMP_get_compression_methods(void)
{
    STACK_OF(SSL_COMP) **rv;

    rv = (STACK_OF(SSL_COMP) **)OSSL_LIB_CTX_get_data(NULL,
                                     OSSL_LIB_CTX_COMP_METHODS);
    if (rv != NULL)
        return *rv;
    else
        return NULL;
}

STACK_OF(SSL_COMP) *SSL_COMP_set0_compression_methods(STACK_OF(SSL_COMP)
                                                      *meths)
{
    STACK_OF(SSL_COMP) **comp_methods;
    STACK_OF(SSL_COMP) *old_meths;

    comp_methods = (STACK_OF(SSL_COMP) **)OSSL_LIB_CTX_get_data(NULL,
                                              OSSL_LIB_CTX_COMP_METHODS);
    if (comp_methods == NULL) {
        old_meths = meths;
    } else {
        old_meths = *comp_methods;
        *comp_methods = meths;
    }

    return old_meths;
}

int SSL_COMP_add_compression_method(int id, COMP_METHOD *cm)
{
    STACK_OF(SSL_COMP) *comp_methods;
    SSL_COMP *comp;

    comp_methods = SSL_COMP_get_compression_methods();

    if (comp_methods == NULL)
        return 1;

    if (cm == NULL || COMP_get_type(cm) == NID_undef)
        return 1;

    /*-
     * According to draft-ietf-tls-compression-04.txt, the
     * compression number ranges should be the following:
     *
     *   0 to  63:  methods defined by the IETF
     *  64 to 192:  external party methods assigned by IANA
     * 193 to 255:  reserved for private use
     */
    if (id < 193 || id > 255) {
        ERR_raise(ERR_LIB_SSL, SSL_R_COMPRESSION_ID_NOT_WITHIN_PRIVATE_RANGE);
        return 1;
    }

    comp = OPENSSL_malloc(sizeof(*comp));
    if (comp == NULL)
        return 1;

    comp->id = id;
    if (sk_SSL_COMP_find(comp_methods, comp) >= 0) {
        OPENSSL_free(comp);
        ERR_raise(ERR_LIB_SSL, SSL_R_DUPLICATE_COMPRESSION_ID);
        return 1;
    }
    if (!sk_SSL_COMP_push(comp_methods, comp)) {
        OPENSSL_free(comp);
        ERR_raise(ERR_LIB_SSL, ERR_R_CRYPTO_LIB);
        return 1;
    }

    return 0;
}
#endif

const char *SSL_COMP_get_name(const COMP_METHOD *comp)
{
#ifndef OPENSSL_NO_COMP
    return comp ? COMP_get_name(comp) : NULL;
#else
    return NULL;
#endif
}

const char *SSL_COMP_get0_name(const SSL_COMP *comp)
{
#ifndef OPENSSL_NO_COMP
    return comp->name;
#else
    return NULL;
#endif
}

int SSL_COMP_get_id(const SSL_COMP *comp)
{
#ifndef OPENSSL_NO_COMP
    return comp->id;
#else
    return -1;
#endif
}

const SSL_CIPHER *ssl_get_cipher_by_char(SSL_CONNECTION *s,
                                         const unsigned char *ptr,
                                         int all)
{
    const SSL_CIPHER *c = SSL_CONNECTION_GET_SSL(s)->method->get_cipher_by_char(ptr);

    if (c == NULL || (!all && c->valid == 0))
        return NULL;
    return c;
}

const SSL_CIPHER *SSL_CIPHER_find(SSL *ssl, const unsigned char *ptr)
{
    return ssl->method->get_cipher_by_char(ptr);
}

int SSL_CIPHER_get_cipher_nid(const SSL_CIPHER *c)
{
    int i;
    if (c == NULL)
        return NID_undef;
    i = ssl_cipher_info_lookup(ssl_cipher_table_cipher, c->algorithm_enc);
    if (i == -1)
        return NID_undef;
    return ssl_cipher_table_cipher[i].nid;
}

int SSL_CIPHER_get_digest_nid(const SSL_CIPHER *c)
{
    int i = ssl_cipher_info_lookup(ssl_cipher_table_mac, c->algorithm_mac);

    if (i == -1)
        return NID_undef;
    return ssl_cipher_table_mac[i].nid;
}

int SSL_CIPHER_get_kx_nid(const SSL_CIPHER *c)
{
    int i = ssl_cipher_info_lookup(ssl_cipher_table_kx, c->algorithm_mkey);

    if (i == -1)
        return NID_undef;
    return ssl_cipher_table_kx[i].nid;
}

int SSL_CIPHER_get_auth_nid(const SSL_CIPHER *c)
{
    int i = ssl_cipher_info_lookup(ssl_cipher_table_auth, c->algorithm_auth);

    if (i == -1)
        return NID_undef;
    return ssl_cipher_table_auth[i].nid;
}

int ssl_get_md_idx(int md_nid) {
    int i;

    for(i = 0; i < SSL_MD_NUM_IDX; i++) {
        if (md_nid == ssl_cipher_table_mac[i].nid)
            return i;
    }
    return -1;
}

const EVP_MD *SSL_CIPHER_get_handshake_digest(const SSL_CIPHER *c)
{
    int idx = c->algorithm2 & SSL_HANDSHAKE_MAC_MASK;

    if (idx < 0 || idx >= SSL_MD_NUM_IDX)
        return NULL;
    return EVP_get_digestbynid(ssl_cipher_table_mac[idx].nid);
}

int SSL_CIPHER_is_aead(const SSL_CIPHER *c)
{
    return (c->algorithm_mac & SSL_AEAD) ? 1 : 0;
}

int ssl_cipher_get_overhead(const SSL_CIPHER *c, size_t *mac_overhead,
                            size_t *int_overhead, size_t *blocksize,
                            size_t *ext_overhead)
{
    int mac = 0, in = 0, blk = 0, out = 0;

    /* Some hard-coded numbers for the CCM/Poly1305 MAC overhead
     * because there are no handy #defines for those. */
    if (c->algorithm_enc & (SSL_AESGCM | SSL_ARIAGCM)) {
        out = EVP_GCM_TLS_EXPLICIT_IV_LEN + EVP_GCM_TLS_TAG_LEN;
    } else if (c->algorithm_enc & (SSL_AES128CCM | SSL_AES256CCM)) {
        out = EVP_CCM_TLS_EXPLICIT_IV_LEN + 16;
    } else if (c->algorithm_enc & (SSL_AES128CCM8 | SSL_AES256CCM8)) {
        out = EVP_CCM_TLS_EXPLICIT_IV_LEN + 8;
    } else if (c->algorithm_enc & SSL_CHACHA20POLY1305) {
        out = 16;
    } else if (c->algorithm_mac & SSL_AEAD) {
        /* We're supposed to have handled all the AEAD modes above */
        return 0;
    } else {
        /* Non-AEAD modes. Calculate MAC/cipher overhead separately */
        int digest_nid = SSL_CIPHER_get_digest_nid(c);
        const EVP_MD *e_md = EVP_get_digestbynid(digest_nid);

        if (e_md == NULL)
            return 0;

        mac = EVP_MD_get_size(e_md);
        if (mac <= 0)
            return 0;
        if (c->algorithm_enc != SSL_eNULL) {
            int cipher_nid = SSL_CIPHER_get_cipher_nid(c);
            const EVP_CIPHER *e_ciph = EVP_get_cipherbynid(cipher_nid);

            /* If it wasn't AEAD or SSL_eNULL, we expect it to be a
               known CBC cipher. */
            if (e_ciph == NULL ||
                EVP_CIPHER_get_mode(e_ciph) != EVP_CIPH_CBC_MODE)
                return 0;

            in = 1; /* padding length byte */
            out = EVP_CIPHER_get_iv_length(e_ciph);
            if (out < 0)
                return 0;
            blk = EVP_CIPHER_get_block_size(e_ciph);
            if (blk <= 0)
                return 0;
        }
    }

    *mac_overhead = (size_t)mac;
    *int_overhead = (size_t)in;
    *blocksize = (size_t)blk;
    *ext_overhead = (size_t)out;

    return 1;
}

int ssl_cert_is_disabled(SSL_CTX *ctx, size_t idx)
{
    const SSL_CERT_LOOKUP *cl;

    /* A provider-loaded key type is always enabled */
    if (idx >= SSL_PKEY_NUM)
        return 0;

    cl = ssl_cert_lookup_by_idx(idx, ctx);
    if (cl == NULL || (cl->amask & ctx->disabled_auth_mask) != 0)
        return 1;
    return 0;
}

/*
 * Default list of TLSv1.2 (and earlier) ciphers
 * SSL_DEFAULT_CIPHER_LIST deprecated in 3.0.0
 * Update both macro and function simultaneously
 */
const char *OSSL_default_cipher_list(void)
{
    return "ALL:!COMPLEMENTOFDEFAULT:!eNULL";
}

/*
 * Default list of TLSv1.3 (and later) ciphers
 * TLS_DEFAULT_CIPHERSUITES deprecated in 3.0.0
 * Update both macro and function simultaneously
 */
const char *OSSL_default_ciphersuites(void)
{
    return "TLS_AES_256_GCM_SHA384:"
           "TLS_CHACHA20_POLY1305_SHA256:"
           "TLS_AES_128_GCM_SHA256";
}
