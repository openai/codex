/*
 * Copyright 2022-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/evp.h>
#include <openssl/core_names.h>
#include <openssl/rand.h>
#include <openssl/hpke.h>
#include "testutil.h"

/* a size to use for stack buffers */
#define OSSL_HPKE_TSTSIZE 512

static OSSL_LIB_CTX *testctx = NULL;
static OSSL_PROVIDER *nullprov = NULL;
static OSSL_PROVIDER *deflprov = NULL;
static char *testpropq = "provider=default";
static int verbose = 0;

typedef struct {
    int mode;
    OSSL_HPKE_SUITE suite;
    const unsigned char *ikmE;
    size_t ikmElen;
    const unsigned char *expected_pkEm;
    size_t expected_pkEmlen;
    const unsigned char *ikmR;
    size_t ikmRlen;
    const unsigned char *expected_pkRm;
    size_t expected_pkRmlen;
    const unsigned char *expected_skRm;
    size_t expected_skRmlen;
    const unsigned char *expected_secret;
    size_t expected_secretlen;
    const unsigned char *ksinfo;
    size_t ksinfolen;
    const unsigned char *ikmAuth;
    size_t ikmAuthlen;
    const unsigned char *psk;
    size_t psklen;
    const char *pskid; /* want terminating NUL here */
} TEST_BASEDATA;

typedef struct {
    int seq;
    const unsigned char *pt;
    size_t ptlen;
    const unsigned char *aad;
    size_t aadlen;
    const unsigned char *expected_ct;
    size_t expected_ctlen;
} TEST_AEADDATA;

typedef struct {
    const unsigned char *context;
    size_t contextlen;
    const unsigned char *expected_secret;
    size_t expected_secretlen;
} TEST_EXPORTDATA;

/**
 * @brief Test that an EVP_PKEY encoded public key matches the supplied buffer
 * @param pkey is the EVP_PKEY we want to check
 * @param pub is the expected public key buffer
 * @param publen is the length of the above
 * @return 1 for good, 0 for bad
 */
static int cmpkey(const EVP_PKEY *pkey,
                  const unsigned char *pub, size_t publen)
{
    unsigned char pubbuf[256];
    size_t pubbuflen = 0;
    int erv = 0;

    if (!TEST_true(publen <= sizeof(pubbuf)))
        return 0;
    erv = EVP_PKEY_get_octet_string_param(pkey,
                                          OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY,
                                          pubbuf, sizeof(pubbuf), &pubbuflen);
    if (!TEST_true(erv))
        return 0;
    if (pub != NULL && !TEST_mem_eq(pubbuf, pubbuflen, pub, publen))
        return 0;
    return 1;
}

static int do_testhpke(const TEST_BASEDATA *base,
                       const TEST_AEADDATA *aead, size_t aeadsz,
                       const TEST_EXPORTDATA *export, size_t exportsz)
{
    OSSL_LIB_CTX *libctx = testctx;
    const char *propq = testpropq;
    OSSL_HPKE_CTX *sealctx = NULL, *openctx = NULL;
    unsigned char ct[256];
    unsigned char enc[256];
    unsigned char ptout[256];
    size_t ptoutlen = sizeof(ptout);
    size_t enclen = sizeof(enc);
    size_t ctlen = sizeof(ct);
    unsigned char pub[OSSL_HPKE_TSTSIZE];
    size_t publen = sizeof(pub);
    EVP_PKEY *privE = NULL;
    unsigned char authpub[OSSL_HPKE_TSTSIZE];
    size_t authpublen = sizeof(authpub);
    EVP_PKEY *authpriv = NULL;
    unsigned char rpub[OSSL_HPKE_TSTSIZE];
    size_t rpublen = sizeof(pub);
    EVP_PKEY *privR = NULL;
    int ret = 0;
    size_t i;
    uint64_t lastseq = 0;

    if (!TEST_true(OSSL_HPKE_keygen(base->suite, pub, &publen, &privE,
                                    base->ikmE, base->ikmElen, libctx, propq)))
        goto end;
    if (!TEST_true(cmpkey(privE, base->expected_pkEm, base->expected_pkEmlen)))
        goto end;
    if (!TEST_ptr(sealctx = OSSL_HPKE_CTX_new(base->mode, base->suite,
                                              OSSL_HPKE_ROLE_SENDER,
                                              libctx, propq)))
        goto end;
    if (!TEST_true(OSSL_HPKE_CTX_set1_ikme(sealctx, base->ikmE, base->ikmElen)))
        goto end;
    if (base->mode == OSSL_HPKE_MODE_AUTH
        || base->mode == OSSL_HPKE_MODE_PSKAUTH) {
        if (!TEST_true(base->ikmAuth != NULL && base->ikmAuthlen > 0))
            goto end;
        if (!TEST_true(OSSL_HPKE_keygen(base->suite,
                                        authpub, &authpublen, &authpriv,
                                        base->ikmAuth, base->ikmAuthlen,
                                        libctx, propq)))
            goto end;
        if (!TEST_true(OSSL_HPKE_CTX_set1_authpriv(sealctx, authpriv)))
            goto end;
    }
    if (!TEST_true(OSSL_HPKE_keygen(base->suite, rpub, &rpublen, &privR,
                                    base->ikmR, base->ikmRlen, libctx, propq)))
        goto end;
    if (!TEST_true(cmpkey(privR, base->expected_pkRm, base->expected_pkRmlen)))
        goto end;
    if (base->mode == OSSL_HPKE_MODE_PSK
        || base->mode == OSSL_HPKE_MODE_PSKAUTH) {
        if (!TEST_true(OSSL_HPKE_CTX_set1_psk(sealctx, base->pskid,
                                              base->psk, base->psklen)))
            goto end;
    }
    if (!TEST_true(OSSL_HPKE_encap(sealctx, enc, &enclen,
                                   rpub, rpublen,
                                   base->ksinfo, base->ksinfolen)))
        goto end;
    if (!TEST_true(cmpkey(privE, enc, enclen)))
        goto end;
    for (i = 0; i < aeadsz; ++i) {
        ctlen = sizeof(ct);
        memset(ct, 0, ctlen);
        if (!TEST_true(OSSL_HPKE_seal(sealctx, ct, &ctlen,
                                      aead[i].aad, aead[i].aadlen,
                                      aead[i].pt, aead[i].ptlen)))
            goto end;
        if (!TEST_mem_eq(ct, ctlen, aead[i].expected_ct,
                         aead[i].expected_ctlen))
            goto end;
        if (!TEST_true(OSSL_HPKE_CTX_get_seq(sealctx, &lastseq)))
            goto end;
        if (lastseq != (uint64_t)(i + 1))
            goto end;
    }
    if (!TEST_ptr(openctx = OSSL_HPKE_CTX_new(base->mode, base->suite,
                                              OSSL_HPKE_ROLE_RECEIVER,
                                              libctx, propq)))
        goto end;
    if (base->mode == OSSL_HPKE_MODE_PSK
        || base->mode == OSSL_HPKE_MODE_PSKAUTH) {
        if (!TEST_true(base->pskid != NULL && base->psk != NULL
                       && base->psklen > 0))
            goto end;
        if (!TEST_true(OSSL_HPKE_CTX_set1_psk(openctx, base->pskid,
                                              base->psk, base->psklen)))
            goto end;
    }
    if (base->mode == OSSL_HPKE_MODE_AUTH
        || base->mode == OSSL_HPKE_MODE_PSKAUTH) {
        if (!TEST_true(OSSL_HPKE_CTX_set1_authpub(openctx,
                                                  authpub, authpublen)))
            goto end;
    }
    if (!TEST_true(OSSL_HPKE_decap(openctx, enc, enclen, privR,
                                   base->ksinfo, base->ksinfolen)))
        goto end;
    for (i = 0; i < aeadsz; ++i) {
        ptoutlen = sizeof(ptout);
        memset(ptout, 0, ptoutlen);
        if (!TEST_true(OSSL_HPKE_open(openctx, ptout, &ptoutlen,
                                      aead[i].aad, aead[i].aadlen,
                                      aead[i].expected_ct,
                                      aead[i].expected_ctlen)))
            goto end;
        if (!TEST_mem_eq(aead[i].pt, aead[i].ptlen, ptout, ptoutlen))
            goto end;
        /* check the sequence is being incremented as expected */
        if (!TEST_true(OSSL_HPKE_CTX_get_seq(openctx, &lastseq)))
            goto end;
        if (lastseq != (uint64_t)(i + 1))
            goto end;
    }
    /* check exporters */
    for (i = 0; i < exportsz; ++i) {
        size_t len = export[i].expected_secretlen;
        unsigned char eval[OSSL_HPKE_TSTSIZE];

        if (len > sizeof(eval))
            goto end;
        /* export with too long label should fail */
        if (!TEST_false(OSSL_HPKE_export(sealctx, eval, len,
                                         export[i].context, -1)))
            goto end;
        /* good export call */
        if (!TEST_true(OSSL_HPKE_export(sealctx, eval, len,
                                        export[i].context,
                                        export[i].contextlen)))
            goto end;
        if (!TEST_mem_eq(eval, len, export[i].expected_secret,
                         export[i].expected_secretlen))
            goto end;

        /* check seal fails if export only mode */
        if (aeadsz == 0) {

            if (!TEST_false(OSSL_HPKE_seal(sealctx, ct, &ctlen,
                                           NULL, 0, ptout, ptoutlen)))
                goto end;
        }
    }
    ret = 1;
end:
    OSSL_HPKE_CTX_free(sealctx);
    OSSL_HPKE_CTX_free(openctx);
    EVP_PKEY_free(privE);
    EVP_PKEY_free(privR);
    EVP_PKEY_free(authpriv);
    return ret;
}

static const unsigned char pt[] = {
    0x42, 0x65, 0x61, 0x75, 0x74, 0x79, 0x20, 0x69,
    0x73, 0x20, 0x74, 0x72, 0x75, 0x74, 0x68, 0x2c,
    0x20, 0x74, 0x72, 0x75, 0x74, 0x68, 0x20, 0x62,
    0x65, 0x61, 0x75, 0x74, 0x79
};
static const unsigned char ksinfo[] = {
    0x4f, 0x64, 0x65, 0x20, 0x6f, 0x6e, 0x20, 0x61,
    0x20, 0x47, 0x72, 0x65, 0x63, 0x69, 0x61, 0x6e,
    0x20, 0x55, 0x72, 0x6e
};
#ifndef OPENSSL_NO_ECX
/*
 * static const char *pskid = "Ennyn Durin aran Moria";
 */
static const unsigned char pskid[] = {
    0x45, 0x6e, 0x6e, 0x79, 0x6e, 0x20, 0x44, 0x75,
    0x72, 0x69, 0x6e, 0x20, 0x61, 0x72, 0x61, 0x6e,
    0x20, 0x4d, 0x6f, 0x72, 0x69, 0x61, 0x00
};
static const unsigned char psk[] = {
    0x02, 0x47, 0xfd, 0x33, 0xb9, 0x13, 0x76, 0x0f,
    0xa1, 0xfa, 0x51, 0xe1, 0x89, 0x2d, 0x9f, 0x30,
    0x7f, 0xbe, 0x65, 0xeb, 0x17, 0x1e, 0x81, 0x32,
    0xc2, 0xaf, 0x18, 0x55, 0x5a, 0x73, 0x8b, 0x82
};

/* these need to be "outside" the function below to keep check-ansi CI happy */
static const unsigned char first_ikme[] = {
    0x78, 0x62, 0x8c, 0x35, 0x4e, 0x46, 0xf3, 0xe1,
    0x69, 0xbd, 0x23, 0x1b, 0xe7, 0xb2, 0xff, 0x1c,
    0x77, 0xaa, 0x30, 0x24, 0x60, 0xa2, 0x6d, 0xbf,
    0xa1, 0x55, 0x15, 0x68, 0x4c, 0x00, 0x13, 0x0b
};
static const unsigned char first_ikmr[] = {
    0xd4, 0xa0, 0x9d, 0x09, 0xf5, 0x75, 0xfe, 0xf4,
    0x25, 0x90, 0x5d, 0x2a, 0xb3, 0x96, 0xc1, 0x44,
    0x91, 0x41, 0x46, 0x3f, 0x69, 0x8f, 0x8e, 0xfd,
    0xb7, 0xac, 0xcf, 0xaf, 0xf8, 0x99, 0x50, 0x98
};
static const unsigned char first_ikmepub[] = {
    0x0a, 0xd0, 0x95, 0x0d, 0x9f, 0xb9, 0x58, 0x8e,
    0x59, 0x69, 0x0b, 0x74, 0xf1, 0x23, 0x7e, 0xcd,
    0xf1, 0xd7, 0x75, 0xcd, 0x60, 0xbe, 0x2e, 0xca,
    0x57, 0xaf, 0x5a, 0x4b, 0x04, 0x71, 0xc9, 0x1b,
};
static const unsigned char first_ikmrpub[] = {
    0x9f, 0xed, 0x7e, 0x8c, 0x17, 0x38, 0x75, 0x60,
    0xe9, 0x2c, 0xc6, 0x46, 0x2a, 0x68, 0x04, 0x96,
    0x57, 0x24, 0x6a, 0x09, 0xbf, 0xa8, 0xad, 0xe7,
    0xae, 0xfe, 0x58, 0x96, 0x72, 0x01, 0x63, 0x66
};
static const unsigned char first_ikmrpriv[] = {
    0xc5, 0xeb, 0x01, 0xeb, 0x45, 0x7f, 0xe6, 0xc6,
    0xf5, 0x75, 0x77, 0xc5, 0x41, 0x3b, 0x93, 0x15,
    0x50, 0xa1, 0x62, 0xc7, 0x1a, 0x03, 0xac, 0x8d,
    0x19, 0x6b, 0xab, 0xbd, 0x4e, 0x5c, 0xe0, 0xfd
};
static const unsigned char first_expected_shared_secret[] = {
    0x72, 0x76, 0x99, 0xf0, 0x09, 0xff, 0xe3, 0xc0,
    0x76, 0x31, 0x50, 0x19, 0xc6, 0x96, 0x48, 0x36,
    0x6b, 0x69, 0x17, 0x14, 0x39, 0xbd, 0x7d, 0xd0,
    0x80, 0x77, 0x43, 0xbd, 0xe7, 0x69, 0x86, 0xcd
};
static const unsigned char first_aad0[] = {
    0x43, 0x6f, 0x75, 0x6e, 0x74, 0x2d, 0x30
};
static const unsigned char first_ct0[] = {
    0xe5, 0x2c, 0x6f, 0xed, 0x7f, 0x75, 0x8d, 0x0c,
    0xf7, 0x14, 0x56, 0x89, 0xf2, 0x1b, 0xc1, 0xbe,
    0x6e, 0xc9, 0xea, 0x09, 0x7f, 0xef, 0x4e, 0x95,
    0x94, 0x40, 0x01, 0x2f, 0x4f, 0xeb, 0x73, 0xfb,
    0x61, 0x1b, 0x94, 0x61, 0x99, 0xe6, 0x81, 0xf4,
    0xcf, 0xc3, 0x4d, 0xb8, 0xea
};
static const unsigned char first_aad1[] = {
    0x43, 0x6f, 0x75, 0x6e, 0x74, 0x2d, 0x31
};
static const unsigned char first_ct1[] = {
    0x49, 0xf3, 0xb1, 0x9b, 0x28, 0xa9, 0xea, 0x9f,
    0x43, 0xe8, 0xc7, 0x12, 0x04, 0xc0, 0x0d, 0x4a,
    0x49, 0x0e, 0xe7, 0xf6, 0x13, 0x87, 0xb6, 0x71,
    0x9d, 0xb7, 0x65, 0xe9, 0x48, 0x12, 0x3b, 0x45,
    0xb6, 0x16, 0x33, 0xef, 0x05, 0x9b, 0xa2, 0x2c,
    0xd6, 0x24, 0x37, 0xc8, 0xba
};
static const unsigned char first_aad2[] = {
    0x43, 0x6f, 0x75, 0x6e, 0x74, 0x2d, 0x32
};
static const unsigned char first_ct2[] = {
    0x25, 0x7c, 0xa6, 0xa0, 0x84, 0x73, 0xdc, 0x85,
    0x1f, 0xde, 0x45, 0xaf, 0xd5, 0x98, 0xcc, 0x83,
    0xe3, 0x26, 0xdd, 0xd0, 0xab, 0xe1, 0xef, 0x23,
    0xba, 0xa3, 0xba, 0xa4, 0xdd, 0x8c, 0xde, 0x99,
    0xfc, 0xe2, 0xc1, 0xe8, 0xce, 0x68, 0x7b, 0x0b,
    0x47, 0xea, 0xd1, 0xad, 0xc9
};
static const unsigned char first_export1[] = {
    0xdf, 0xf1, 0x7a, 0xf3, 0x54, 0xc8, 0xb4, 0x16,
    0x73, 0x56, 0x7d, 0xb6, 0x25, 0x9f, 0xd6, 0x02,
    0x99, 0x67, 0xb4, 0xe1, 0xaa, 0xd1, 0x30, 0x23,
    0xc2, 0xae, 0x5d, 0xf8, 0xf4, 0xf4, 0x3b, 0xf6
};
static const unsigned char first_context2[] = { 0x00 };
static const unsigned char first_export2[] = {
    0x6a, 0x84, 0x72, 0x61, 0xd8, 0x20, 0x7f, 0xe5,
    0x96, 0xbe, 0xfb, 0x52, 0x92, 0x84, 0x63, 0x88,
    0x1a, 0xb4, 0x93, 0xda, 0x34, 0x5b, 0x10, 0xe1,
    0xdc, 0xc6, 0x45, 0xe3, 0xb9, 0x4e, 0x2d, 0x95
};
static const unsigned char first_context3[] = {
    0x54, 0x65, 0x73, 0x74, 0x43, 0x6f, 0x6e, 0x74,
    0x65, 0x78, 0x74
};
static const unsigned char first_export3[] = {
    0x8a, 0xff, 0x52, 0xb4, 0x5a, 0x1b, 0xe3, 0xa7,
    0x34, 0xbc, 0x7a, 0x41, 0xe2, 0x0b, 0x4e, 0x05,
    0x5a, 0xd4, 0xc4, 0xd2, 0x21, 0x04, 0xb0, 0xc2,
    0x02, 0x85, 0xa7, 0xc4, 0x30, 0x24, 0x01, 0xcd
};

static int x25519kdfsha256_hkdfsha256_aes128gcm_psk_test(void)
{
    const TEST_BASEDATA pskdata = {
        /* "X25519", NULL, "SHA256", "SHA256", "AES-128-GCM", */
        OSSL_HPKE_MODE_PSK,
        {
            OSSL_HPKE_KEM_ID_X25519,
            OSSL_HPKE_KDF_ID_HKDF_SHA256,
            OSSL_HPKE_AEAD_ID_AES_GCM_128
        },
        first_ikme, sizeof(first_ikme),
        first_ikmepub, sizeof(first_ikmepub),
        first_ikmr, sizeof(first_ikmr),
        first_ikmrpub, sizeof(first_ikmrpub),
        first_ikmrpriv, sizeof(first_ikmrpriv),
        first_expected_shared_secret, sizeof(first_expected_shared_secret),
        ksinfo, sizeof(ksinfo),
        NULL, 0,    /* No Auth */
        psk, sizeof(psk), (char *) pskid
    };
    const TEST_AEADDATA aeaddata[] = {
        {
            0,
            pt, sizeof(pt),
            first_aad0, sizeof(first_aad0),
            first_ct0, sizeof(first_ct0)
        },
        {
            1,
            pt, sizeof(pt),
            first_aad1, sizeof(first_aad1),
            first_ct1, sizeof(first_ct1)
        },
        {
            2,
            pt, sizeof(pt),
            first_aad2, sizeof(first_aad2),
            first_ct2, sizeof(first_ct2)
        }
    };
    const TEST_EXPORTDATA exportdata[] = {
        { NULL, 0, first_export1, sizeof(first_export1) },
        { first_context2, sizeof(first_context2),
          first_export2, sizeof(first_export2) },
        { first_context3, sizeof(first_context3),
          first_export3, sizeof(first_export3) },
    };
    return do_testhpke(&pskdata, aeaddata, OSSL_NELEM(aeaddata),
                       exportdata, OSSL_NELEM(exportdata));
}

static const unsigned char second_ikme[] = {
    0x72, 0x68, 0x60, 0x0d, 0x40, 0x3f, 0xce, 0x43,
    0x15, 0x61, 0xae, 0xf5, 0x83, 0xee, 0x16, 0x13,
    0x52, 0x7c, 0xff, 0x65, 0x5c, 0x13, 0x43, 0xf2,
    0x98, 0x12, 0xe6, 0x67, 0x06, 0xdf, 0x32, 0x34
};
static const unsigned char second_ikmepub[] = {
    0x37, 0xfd, 0xa3, 0x56, 0x7b, 0xdb, 0xd6, 0x28,
    0xe8, 0x86, 0x68, 0xc3, 0xc8, 0xd7, 0xe9, 0x7d,
    0x1d, 0x12, 0x53, 0xb6, 0xd4, 0xea, 0x6d, 0x44,
    0xc1, 0x50, 0xf7, 0x41, 0xf1, 0xbf, 0x44, 0x31,
};
static const unsigned char second_ikmr[] = {
    0x6d, 0xb9, 0xdf, 0x30, 0xaa, 0x07, 0xdd, 0x42,
    0xee, 0x5e, 0x81, 0x81, 0xaf, 0xdb, 0x97, 0x7e,
    0x53, 0x8f, 0x5e, 0x1f, 0xec, 0x8a, 0x06, 0x22,
    0x3f, 0x33, 0xf7, 0x01, 0x3e, 0x52, 0x50, 0x37
};
static const unsigned char second_ikmrpub[] = {
    0x39, 0x48, 0xcf, 0xe0, 0xad, 0x1d, 0xdb, 0x69,
    0x5d, 0x78, 0x0e, 0x59, 0x07, 0x71, 0x95, 0xda,
    0x6c, 0x56, 0x50, 0x6b, 0x02, 0x73, 0x29, 0x79,
    0x4a, 0xb0, 0x2b, 0xca, 0x80, 0x81, 0x5c, 0x4d
};
static const unsigned char second_ikmrpriv[] = {
    0x46, 0x12, 0xc5, 0x50, 0x26, 0x3f, 0xc8, 0xad,
    0x58, 0x37, 0x5d, 0xf3, 0xf5, 0x57, 0xaa, 0xc5,
    0x31, 0xd2, 0x68, 0x50, 0x90, 0x3e, 0x55, 0xa9,
    0xf2, 0x3f, 0x21, 0xd8, 0x53, 0x4e, 0x8a, 0xc8
};
static const unsigned char second_expected_shared_secret[] = {
    0xfe, 0x0e, 0x18, 0xc9, 0xf0, 0x24, 0xce, 0x43,
    0x79, 0x9a, 0xe3, 0x93, 0xc7, 0xe8, 0xfe, 0x8f,
    0xce, 0x9d, 0x21, 0x88, 0x75, 0xe8, 0x22, 0x7b,
    0x01, 0x87, 0xc0, 0x4e, 0x7d, 0x2e, 0xa1, 0xfc
};
static const unsigned char second_aead0[] = {
    0x43, 0x6f, 0x75, 0x6e, 0x74, 0x2d, 0x30
};
static const unsigned char second_ct0[] = {
    0xf9, 0x38, 0x55, 0x8b, 0x5d, 0x72, 0xf1, 0xa2,
    0x38, 0x10, 0xb4, 0xbe, 0x2a, 0xb4, 0xf8, 0x43,
    0x31, 0xac, 0xc0, 0x2f, 0xc9, 0x7b, 0xab, 0xc5,
    0x3a, 0x52, 0xae, 0x82, 0x18, 0xa3, 0x55, 0xa9,
    0x6d, 0x87, 0x70, 0xac, 0x83, 0xd0, 0x7b, 0xea,
    0x87, 0xe1, 0x3c, 0x51, 0x2a
};
static const unsigned char second_aead1[] = {
    0x43, 0x6f, 0x75, 0x6e, 0x74, 0x2d, 0x31
};
static const unsigned char second_ct1[] = {
    0xaf, 0x2d, 0x7e, 0x9a, 0xc9, 0xae, 0x7e, 0x27,
    0x0f, 0x46, 0xba, 0x1f, 0x97, 0x5b, 0xe5, 0x3c,
    0x09, 0xf8, 0xd8, 0x75, 0xbd, 0xc8, 0x53, 0x54,
    0x58, 0xc2, 0x49, 0x4e, 0x8a, 0x6e, 0xab, 0x25,
    0x1c, 0x03, 0xd0, 0xc2, 0x2a, 0x56, 0xb8, 0xca,
    0x42, 0xc2, 0x06, 0x3b, 0x84
};
static const unsigned char second_export1[] = {
    0x38, 0x53, 0xfe, 0x2b, 0x40, 0x35, 0x19, 0x5a,
    0x57, 0x3f, 0xfc, 0x53, 0x85, 0x6e, 0x77, 0x05,
    0x8e, 0x15, 0xd9, 0xea, 0x06, 0x4d, 0xe3, 0xe5,
    0x9f, 0x49, 0x61, 0xd0, 0x09, 0x52, 0x50, 0xee
};
static const unsigned char second_context2[] = { 0x00 };
static const unsigned char second_export2[] = {
    0x2e, 0x8f, 0x0b, 0x54, 0x67, 0x3c, 0x70, 0x29,
    0x64, 0x9d, 0x4e, 0xb9, 0xd5, 0xe3, 0x3b, 0xf1,
    0x87, 0x2c, 0xf7, 0x6d, 0x62, 0x3f, 0xf1, 0x64,
    0xac, 0x18, 0x5d, 0xa9, 0xe8, 0x8c, 0x21, 0xa5
};
static const unsigned char second_context3[] = {
    0x54, 0x65, 0x73, 0x74, 0x43, 0x6f, 0x6e, 0x74,
    0x65, 0x78, 0x74
};
static const unsigned char second_export3[] = {
    0xe9, 0xe4, 0x30, 0x65, 0x10, 0x2c, 0x38, 0x36,
    0x40, 0x1b, 0xed, 0x8c, 0x3c, 0x3c, 0x75, 0xae,
    0x46, 0xbe, 0x16, 0x39, 0x86, 0x93, 0x91, 0xd6,
    0x2c, 0x61, 0xf1, 0xec, 0x7a, 0xf5, 0x49, 0x31
};

static int x25519kdfsha256_hkdfsha256_aes128gcm_base_test(void)
{
    const TEST_BASEDATA basedata = {
        OSSL_HPKE_MODE_BASE,
        {
            OSSL_HPKE_KEM_ID_X25519,
            OSSL_HPKE_KDF_ID_HKDF_SHA256,
            OSSL_HPKE_AEAD_ID_AES_GCM_128
        },
        second_ikme, sizeof(second_ikme),
        second_ikmepub, sizeof(second_ikmepub),
        second_ikmr, sizeof(second_ikmr),
        second_ikmrpub, sizeof(second_ikmrpub),
        second_ikmrpriv, sizeof(second_ikmrpriv),
        second_expected_shared_secret, sizeof(second_expected_shared_secret),
        ksinfo, sizeof(ksinfo),
        NULL, 0, /* no auth ikm */
        NULL, 0, NULL /* no psk */
    };
    const TEST_AEADDATA aeaddata[] = {
        {
            0,
            pt, sizeof(pt),
            second_aead0, sizeof(second_aead0),
            second_ct0, sizeof(second_ct0)
        },
        {
            1,
            pt, sizeof(pt),
            second_aead1, sizeof(second_aead1),
            second_ct1, sizeof(second_ct1)
        }
    };
    const TEST_EXPORTDATA exportdata[] = {
        { NULL, 0, second_export1, sizeof(second_export1) },
        { second_context2, sizeof(second_context2),
          second_export2, sizeof(second_export2) },
        { second_context3, sizeof(second_context3),
          second_export3, sizeof(second_export3) },
    };
    return do_testhpke(&basedata, aeaddata, OSSL_NELEM(aeaddata),
                       exportdata, OSSL_NELEM(exportdata));
}
#endif

static const unsigned char third_ikme[] = {
    0x42, 0x70, 0xe5, 0x4f, 0xfd, 0x08, 0xd7, 0x9d,
    0x59, 0x28, 0x02, 0x0a, 0xf4, 0x68, 0x6d, 0x8f,
    0x6b, 0x7d, 0x35, 0xdb, 0xe4, 0x70, 0x26, 0x5f,
    0x1f, 0x5a, 0xa2, 0x28, 0x16, 0xce, 0x86, 0x0e
};
static const unsigned char third_ikmepub[] = {
    0x04, 0xa9, 0x27, 0x19, 0xc6, 0x19, 0x5d, 0x50,
    0x85, 0x10, 0x4f, 0x46, 0x9a, 0x8b, 0x98, 0x14,
    0xd5, 0x83, 0x8f, 0xf7, 0x2b, 0x60, 0x50, 0x1e,
    0x2c, 0x44, 0x66, 0xe5, 0xe6, 0x7b, 0x32, 0x5a,
    0xc9, 0x85, 0x36, 0xd7, 0xb6, 0x1a, 0x1a, 0xf4,
    0xb7, 0x8e, 0x5b, 0x7f, 0x95, 0x1c, 0x09, 0x00,
    0xbe, 0x86, 0x3c, 0x40, 0x3c, 0xe6, 0x5c, 0x9b,
    0xfc, 0xb9, 0x38, 0x26, 0x57, 0x22, 0x2d, 0x18,
    0xc4,
};
static const unsigned char third_ikmr[] = {
    0x66, 0x8b, 0x37, 0x17, 0x1f, 0x10, 0x72, 0xf3,
    0xcf, 0x12, 0xea, 0x8a, 0x23, 0x6a, 0x45, 0xdf,
    0x23, 0xfc, 0x13, 0xb8, 0x2a, 0xf3, 0x60, 0x9a,
    0xd1, 0xe3, 0x54, 0xf6, 0xef, 0x81, 0x75, 0x50
};
static const unsigned char third_ikmrpub[] = {
    0x04, 0xfe, 0x8c, 0x19, 0xce, 0x09, 0x05, 0x19,
    0x1e, 0xbc, 0x29, 0x8a, 0x92, 0x45, 0x79, 0x25,
    0x31, 0xf2, 0x6f, 0x0c, 0xec, 0xe2, 0x46, 0x06,
    0x39, 0xe8, 0xbc, 0x39, 0xcb, 0x7f, 0x70, 0x6a,
    0x82, 0x6a, 0x77, 0x9b, 0x4c, 0xf9, 0x69, 0xb8,
    0xa0, 0xe5, 0x39, 0xc7, 0xf6, 0x2f, 0xb3, 0xd3,
    0x0a, 0xd6, 0xaa, 0x8f, 0x80, 0xe3, 0x0f, 0x1d,
    0x12, 0x8a, 0xaf, 0xd6, 0x8a, 0x2c, 0xe7, 0x2e,
    0xa0
};
static const unsigned char third_ikmrpriv[] = {
    0xf3, 0xce, 0x7f, 0xda, 0xe5, 0x7e, 0x1a, 0x31,
    0x0d, 0x87, 0xf1, 0xeb, 0xbd, 0xe6, 0xf3, 0x28,
    0xbe, 0x0a, 0x99, 0xcd, 0xbc, 0xad, 0xf4, 0xd6,
    0x58, 0x9c, 0xf2, 0x9d, 0xe4, 0xb8, 0xff, 0xd2
};
static const unsigned char third_expected_shared_secret[] = {
    0xc0, 0xd2, 0x6a, 0xea, 0xb5, 0x36, 0x60, 0x9a,
    0x57, 0x2b, 0x07, 0x69, 0x5d, 0x93, 0x3b, 0x58,
    0x9d, 0xcf, 0x36, 0x3f, 0xf9, 0xd9, 0x3c, 0x93,
    0xad, 0xea, 0x53, 0x7a, 0xea, 0xbb, 0x8c, 0xb8
};
static const unsigned char third_aead0[] = {
    0x43, 0x6f, 0x75, 0x6e, 0x74, 0x2d, 0x30
};
static const unsigned char third_ct0[] = {
    0x5a, 0xd5, 0x90, 0xbb, 0x8b, 0xaa, 0x57, 0x7f,
    0x86, 0x19, 0xdb, 0x35, 0xa3, 0x63, 0x11, 0x22,
    0x6a, 0x89, 0x6e, 0x73, 0x42, 0xa6, 0xd8, 0x36,
    0xd8, 0xb7, 0xbc, 0xd2, 0xf2, 0x0b, 0x6c, 0x7f,
    0x90, 0x76, 0xac, 0x23, 0x2e, 0x3a, 0xb2, 0x52,
    0x3f, 0x39, 0x51, 0x34, 0x34
};
static const unsigned char third_aead1[] = {
    0x43, 0x6f, 0x75, 0x6e, 0x74, 0x2d, 0x31
};
static const unsigned char third_ct1[] = {
    0xfa, 0x6f, 0x03, 0x7b, 0x47, 0xfc, 0x21, 0x82,
    0x6b, 0x61, 0x01, 0x72, 0xca, 0x96, 0x37, 0xe8,
    0x2d, 0x6e, 0x58, 0x01, 0xeb, 0x31, 0xcb, 0xd3,
    0x74, 0x82, 0x71, 0xaf, 0xfd, 0x4e, 0xcb, 0x06,
    0x64, 0x6e, 0x03, 0x29, 0xcb, 0xdf, 0x3c, 0x3c,
    0xd6, 0x55, 0xb2, 0x8e, 0x82
};
static const unsigned char third_export1[] = {
    0x5e, 0x9b, 0xc3, 0xd2, 0x36, 0xe1, 0x91, 0x1d,
    0x95, 0xe6, 0x5b, 0x57, 0x6a, 0x8a, 0x86, 0xd4,
    0x78, 0xfb, 0x82, 0x7e, 0x8b, 0xdf, 0xe7, 0x7b,
    0x74, 0x1b, 0x28, 0x98, 0x90, 0x49, 0x0d, 0x4d
};
static const unsigned char third_context2[] = { 0x00 };
static const unsigned char third_export2[] = {
    0x6c, 0xff, 0x87, 0x65, 0x89, 0x31, 0xbd, 0xa8,
    0x3d, 0xc8, 0x57, 0xe6, 0x35, 0x3e, 0xfe, 0x49,
    0x87, 0xa2, 0x01, 0xb8, 0x49, 0x65, 0x8d, 0x9b,
    0x04, 0x7a, 0xab, 0x4c, 0xf2, 0x16, 0xe7, 0x96
};
static const unsigned char third_context3[] = {
    0x54, 0x65, 0x73, 0x74, 0x43, 0x6f, 0x6e, 0x74,
    0x65, 0x78, 0x74
};
static const unsigned char third_export3[] = {
    0xd8, 0xf1, 0xea, 0x79, 0x42, 0xad, 0xbb, 0xa7,
    0x41, 0x2c, 0x6d, 0x43, 0x1c, 0x62, 0xd0, 0x13,
    0x71, 0xea, 0x47, 0x6b, 0x82, 0x3e, 0xb6, 0x97,
    0xe1, 0xf6, 0xe6, 0xca, 0xe1, 0xda, 0xb8, 0x5a
};

static int P256kdfsha256_hkdfsha256_aes128gcm_base_test(void)
{
    const TEST_BASEDATA basedata = {
        OSSL_HPKE_MODE_BASE,
        {
            OSSL_HPKE_KEM_ID_P256,
            OSSL_HPKE_KDF_ID_HKDF_SHA256,
            OSSL_HPKE_AEAD_ID_AES_GCM_128
        },
        third_ikme, sizeof(third_ikme),
        third_ikmepub, sizeof(third_ikmepub),
        third_ikmr, sizeof(third_ikmr),
        third_ikmrpub, sizeof(third_ikmrpub),
        third_ikmrpriv, sizeof(third_ikmrpriv),
        third_expected_shared_secret, sizeof(third_expected_shared_secret),
        ksinfo, sizeof(ksinfo),
        NULL, 0, /* no auth */
        NULL, 0, NULL /* PSK stuff */
    };
    const TEST_AEADDATA aeaddata[] = {
        {
            0,
            pt, sizeof(pt),
            third_aead0, sizeof(third_aead0),
            third_ct0, sizeof(third_ct0)
        },
        {
            1,
            pt, sizeof(pt),
            third_aead1, sizeof(third_aead1),
            third_ct1, sizeof(third_ct1)
        }
    };
    const TEST_EXPORTDATA exportdata[] = {
        { NULL, 0, third_export1, sizeof(third_export1) },
        { third_context2, sizeof(third_context2),
          third_export2, sizeof(third_export2) },
        { third_context3, sizeof(third_context3),
          third_export3, sizeof(third_export3) },
    };
    return do_testhpke(&basedata, aeaddata, OSSL_NELEM(aeaddata),
                       exportdata, OSSL_NELEM(exportdata));
}

#ifndef OPENSSL_NO_ECX
static const unsigned char fourth_ikme[] = {
    0x55, 0xbc, 0x24, 0x5e, 0xe4, 0xef, 0xda, 0x25,
    0xd3, 0x8f, 0x2d, 0x54, 0xd5, 0xbb, 0x66, 0x65,
    0x29, 0x1b, 0x99, 0xf8, 0x10, 0x8a, 0x8c, 0x4b,
    0x68, 0x6c, 0x2b, 0x14, 0x89, 0x3e, 0xa5, 0xd9
};
static const unsigned char fourth_ikmepub[] = {
    0xe5, 0xe8, 0xf9, 0xbf, 0xff, 0x6c, 0x2f, 0x29,
    0x79, 0x1f, 0xc3, 0x51, 0xd2, 0xc2, 0x5c, 0xe1,
    0x29, 0x9a, 0xa5, 0xea, 0xca, 0x78, 0xa7, 0x57,
    0xc0, 0xb4, 0xfb, 0x4b, 0xcd, 0x83, 0x09, 0x18
};
static const unsigned char fourth_ikmr[] = {
    0x68, 0x3a, 0xe0, 0xda, 0x1d, 0x22, 0x18, 0x1e,
    0x74, 0xed, 0x2e, 0x50, 0x3e, 0xbf, 0x82, 0x84,
    0x0d, 0xeb, 0x1d, 0x5e, 0x87, 0x2c, 0xad, 0xe2,
    0x0f, 0x4b, 0x45, 0x8d, 0x99, 0x78, 0x3e, 0x31
};
static const unsigned char fourth_ikmrpub[] = {
    0x19, 0x41, 0x41, 0xca, 0x6c, 0x3c, 0x3b, 0xeb,
    0x47, 0x92, 0xcd, 0x97, 0xba, 0x0e, 0xa1, 0xfa,
    0xff, 0x09, 0xd9, 0x84, 0x35, 0x01, 0x23, 0x45,
    0x76, 0x6e, 0xe3, 0x3a, 0xae, 0x2d, 0x76, 0x64
};
static const unsigned char fourth_ikmrpriv[] = {
    0x33, 0xd1, 0x96, 0xc8, 0x30, 0xa1, 0x2f, 0x9a,
    0xc6, 0x5d, 0x6e, 0x56, 0x5a, 0x59, 0x0d, 0x80,
    0xf0, 0x4e, 0xe9, 0xb1, 0x9c, 0x83, 0xc8, 0x7f,
    0x2c, 0x17, 0x0d, 0x97, 0x2a, 0x81, 0x28, 0x48
};
static const unsigned char fourth_expected_shared_secret[] = {
    0xe8, 0x17, 0x16, 0xce, 0x8f, 0x73, 0x14, 0x1d,
    0x4f, 0x25, 0xee, 0x90, 0x98, 0xef, 0xc9, 0x68,
    0xc9, 0x1e, 0x5b, 0x8c, 0xe5, 0x2f, 0xff, 0xf5,
    0x9d, 0x64, 0x03, 0x9e, 0x82, 0x91, 0x8b, 0x66
};
static const unsigned char fourth_export1[] = {
    0x7a, 0x36, 0x22, 0x1b, 0xd5, 0x6d, 0x50, 0xfb,
    0x51, 0xee, 0x65, 0xed, 0xfd, 0x98, 0xd0, 0x6a,
    0x23, 0xc4, 0xdc, 0x87, 0x08, 0x5a, 0xa5, 0x86,
    0x6c, 0xb7, 0x08, 0x72, 0x44, 0xbd, 0x2a, 0x36
};
static const unsigned char fourth_context2[] = { 0x00 };
static const unsigned char fourth_export2[] = {
    0xd5, 0x53, 0x5b, 0x87, 0x09, 0x9c, 0x6c, 0x3c,
    0xe8, 0x0d, 0xc1, 0x12, 0xa2, 0x67, 0x1c, 0x6e,
    0xc8, 0xe8, 0x11, 0xa2, 0xf2, 0x84, 0xf9, 0x48,
    0xce, 0xc6, 0xdd, 0x17, 0x08, 0xee, 0x33, 0xf0
};
static const unsigned char fourth_context3[] = {
    0x54, 0x65, 0x73, 0x74, 0x43, 0x6f, 0x6e, 0x74,
    0x65, 0x78, 0x74
};
static const unsigned char fourth_export3[] = {
    0xff, 0xaa, 0xbc, 0x85, 0xa7, 0x76, 0x13, 0x6c,
    0xa0, 0xc3, 0x78, 0xe5, 0xd0, 0x84, 0xc9, 0x14,
    0x0a, 0xb5, 0x52, 0xb7, 0x8f, 0x03, 0x9d, 0x2e,
    0x87, 0x75, 0xf2, 0x6e, 0xff, 0xf4, 0xc7, 0x0e
};

static int export_only_test(void)
{
    /* based on RFC9180 A.7 */
    const TEST_BASEDATA basedata = {
        OSSL_HPKE_MODE_BASE,
        {
            OSSL_HPKE_KEM_ID_X25519,
            OSSL_HPKE_KDF_ID_HKDF_SHA256,
            OSSL_HPKE_AEAD_ID_EXPORTONLY
        },
        fourth_ikme, sizeof(fourth_ikme),
        fourth_ikmepub, sizeof(fourth_ikmepub),
        fourth_ikmr, sizeof(fourth_ikmr),
        fourth_ikmrpub, sizeof(fourth_ikmrpub),
        fourth_ikmrpriv, sizeof(fourth_ikmrpriv),
        fourth_expected_shared_secret, sizeof(fourth_expected_shared_secret),
        ksinfo, sizeof(ksinfo),
        NULL, 0, /* no auth */
        NULL, 0, NULL /* PSK stuff */
    };
    const TEST_EXPORTDATA exportdata[] = {
        { NULL, 0, fourth_export1, sizeof(fourth_export1) },
        { fourth_context2, sizeof(fourth_context2),
          fourth_export2, sizeof(fourth_export2) },
        { fourth_context3, sizeof(fourth_context3),
          fourth_export3, sizeof(fourth_export3) },
    };
    return do_testhpke(&basedata, NULL, 0,
                       exportdata, OSSL_NELEM(exportdata));
}
#endif

/*
 * Randomly toss a coin
 */
#define COIN_IS_HEADS (test_random() % 2)

/* tables of HPKE modes and suite values */
static int hpke_mode_list[] = {
    OSSL_HPKE_MODE_BASE,
    OSSL_HPKE_MODE_PSK,
    OSSL_HPKE_MODE_AUTH,
    OSSL_HPKE_MODE_PSKAUTH
};
static uint16_t hpke_kem_list[] = {
    OSSL_HPKE_KEM_ID_P256,
    OSSL_HPKE_KEM_ID_P384,
    OSSL_HPKE_KEM_ID_P521,
#ifndef OPENSSL_NO_ECX
    OSSL_HPKE_KEM_ID_X25519,
    OSSL_HPKE_KEM_ID_X448
#endif
};
static uint16_t hpke_kdf_list[] = {
    OSSL_HPKE_KDF_ID_HKDF_SHA256,
    OSSL_HPKE_KDF_ID_HKDF_SHA384,
    OSSL_HPKE_KDF_ID_HKDF_SHA512
};
static uint16_t hpke_aead_list[] = {
    OSSL_HPKE_AEAD_ID_AES_GCM_128,
    OSSL_HPKE_AEAD_ID_AES_GCM_256,
#if !defined(OPENSSL_NO_CHACHA) && !defined(OPENSSL_NO_POLY1305)
    OSSL_HPKE_AEAD_ID_CHACHA_POLY1305
#endif
};

/*
 * Strings that can be used with names or IANA codepoints.
 * Note that the initial entries from these lists should
 * match the lists above, i.e. kem_str_list[0] and
 * hpke_kem_list[0] should refer to the same KEM. We use
 * that for verbose output via TEST_note() below.
 * Subsequent entries are only used for tests of
 * OSSL_HPKE_str2suite()
 */
static const char *mode_str_list[] = {
    "base", "psk", "auth", "pskauth"
};
static const char *kem_str_list[] = {
#ifndef OPENSSL_NO_ECX
    "P-256", "P-384", "P-521", "x25519", "x448",
    "0x10", "0x11", "0x12", "0x20", "0x21",
    "16", "17", "18", "32", "33"
#else
    "P-256", "P-384", "P-521",
    "0x10", "0x11", "0x12",
    "16", "17", "18"
#endif
};
static const char *kdf_str_list[] = {
    "hkdf-sha256", "hkdf-sha384", "hkdf-sha512",
    "0x1", "0x01", "0x2", "0x02", "0x3", "0x03",
    "1", "2", "3"
};
static const char *aead_str_list[] = {
    "aes-128-gcm", "aes-256-gcm", "chacha20-poly1305", "exporter",
    "0x1", "0x01", "0x2", "0x02", "0x3", "0x03",
    "1", "2", "3",
    "0xff", "255"
};
/* table of bogus strings that better not work */
static const char *bogus_suite_strs[] = {
    "3,33,3",
    "bogus,bogus,bogus",
    "bogus,33,3,1,bogus",
    "bogus,33,3,1",
    "bogus,bogus",
    "bogus",
    /* one bad token */
    "0x10,0x01,bogus",
    "0x10,bogus,0x01",
    "bogus,0x02,0x01",
    /* in reverse order */
    "aes-256-gcm,hkdf-sha512,x25519",
    /* surplus separators */
    ",,0x10,0x01,0x02",
    "0x10,,0x01,0x02",
    "0x10,0x01,,0x02",
    /* embedded NUL chars */
    "0x10,\00x01,,0x02",
    "0x10,\0""0x01,0x02",
    "0x10\0,0x01,0x02",
    "0x10,0x01\0,0x02",
    "0x10,0x01,\0""0x02",
    /* embedded whitespace */
    " aes-256-gcm,hkdf-sha512,x25519",
    "aes-256-gcm, hkdf-sha512,x25519",
    "aes-256-gcm ,hkdf-sha512,x25519",
    "aes-256-gcm,hkdf-sha512, x25519",
    "aes-256-gcm,hkdf-sha512 ,x25519",
    "aes-256-gcm,hkdf-sha512,x25519 ",
    /* good value followed by extra stuff */
    "0x10,0x01,0x02,",
    "0x10,0x01,0x02,,,",
    "0x10,0x01,0x01,0x02",
    "0x10,0x01,0x01,blah",
    "0x10,0x01,0x01 0x02",
    /* too few but good tokens */
    "0x10,0x01",
    "0x10",
    /* empty things */
    NULL,
    "",
    ",",
    ",,"
};

/**
 * @brief round-trips, generating keys, encrypt and decrypt
 *
 * This iterates over all mode and ciphersuite options trying
 * a key gen, encrypt and decrypt for each. The aad, info, and
 * seq inputs are randomly set or omitted each time. EVP and
 * non-EVP key generation are randomly selected.
 *
 * @return 1 for success, other otherwise
 */
static int test_hpke_modes_suites(void)
{
    int overallresult = 1;
    size_t mind = 0; /* index into hpke_mode_list */
    size_t kemind = 0; /* index into hpke_kem_list */
    size_t kdfind = 0; /* index into hpke_kdf_list */
    size_t aeadind = 0; /* index into hpke_aead_list */

    /* iterate over the different modes */
    for (mind = 0; mind < OSSL_NELEM(hpke_mode_list); mind++) {
        int hpke_mode = hpke_mode_list[mind];
        size_t aadlen = OSSL_HPKE_TSTSIZE;
        unsigned char aad[OSSL_HPKE_TSTSIZE];
        unsigned char *aadp = NULL;
        size_t infolen = 32;
        unsigned char info[32];
        unsigned char *infop = NULL;
        unsigned char lpsk[32];
        unsigned char *pskp = NULL;
        char lpskid[32];
        size_t psklen = 32;
        char *pskidp = NULL;
        EVP_PKEY *privp = NULL;
        OSSL_HPKE_SUITE hpke_suite = OSSL_HPKE_SUITE_DEFAULT;
        size_t plainlen = OSSL_HPKE_TSTSIZE;
        unsigned char plain[OSSL_HPKE_TSTSIZE];
        OSSL_HPKE_CTX *rctx = NULL;
        OSSL_HPKE_CTX *ctx = NULL;

        memset(plain, 0x00, OSSL_HPKE_TSTSIZE);
        strcpy((char *)plain, "a message not in a bottle");
        plainlen = strlen((char *)plain);
        /*
         * Randomly try with/without info, aad, seq. Given mode and suite
         * combos, and this being run even a few times, we'll exercise many
         * code paths fairly quickly. We don't really care what the values
         * are but it'll be easier to debug if they're known, so we set 'em.
         */
        if (COIN_IS_HEADS) {
            aadp = aad;
            memset(aad, 'a', aadlen);
        } else {
            aadlen = 0;
        }
        if (COIN_IS_HEADS) {
            infop = info;
            memset(info, 'i', infolen);
        } else {
            infolen = 0;
        }
        if (hpke_mode == OSSL_HPKE_MODE_PSK
            || hpke_mode == OSSL_HPKE_MODE_PSKAUTH) {
            pskp = lpsk;
            memset(lpsk, 'P', psklen);
            pskidp = lpskid;
            memset(lpskid, 'I', psklen - 1);
            lpskid[psklen - 1] = '\0';
        } else {
            psklen = 0;
        }
        for (kemind = 0; /* iterate over the kems, kdfs and aeads */
             overallresult == 1 && kemind < OSSL_NELEM(hpke_kem_list);
             kemind++) {
            uint16_t kem_id = hpke_kem_list[kemind];
            size_t authpublen = OSSL_HPKE_TSTSIZE;
            unsigned char authpub[OSSL_HPKE_TSTSIZE];
            unsigned char *authpubp = NULL;
            EVP_PKEY *authpriv = NULL;

            hpke_suite.kem_id = kem_id;
            if (hpke_mode == OSSL_HPKE_MODE_AUTH
                || hpke_mode == OSSL_HPKE_MODE_PSKAUTH) {
                if (TEST_true(OSSL_HPKE_keygen(hpke_suite, authpub, &authpublen,
                                               &authpriv, NULL, 0,
                                               testctx, NULL)) != 1) {
                    overallresult = 0;
                }
                authpubp = authpub;
            } else {
                authpublen = 0;
            }
            for (kdfind = 0;
                 overallresult == 1 && kdfind < OSSL_NELEM(hpke_kdf_list);
                 kdfind++) {
                uint16_t kdf_id = hpke_kdf_list[kdfind];

                hpke_suite.kdf_id = kdf_id;
                for (aeadind = 0;
                     overallresult == 1
                     && aeadind < OSSL_NELEM(hpke_aead_list);
                     aeadind++) {
                    uint16_t aead_id = hpke_aead_list[aeadind];
                    size_t publen = OSSL_HPKE_TSTSIZE;
                    unsigned char pub[OSSL_HPKE_TSTSIZE];
                    size_t senderpublen = OSSL_HPKE_TSTSIZE;
                    unsigned char senderpub[OSSL_HPKE_TSTSIZE];
                    size_t cipherlen = OSSL_HPKE_TSTSIZE;
                    unsigned char cipher[OSSL_HPKE_TSTSIZE];
                    size_t clearlen = OSSL_HPKE_TSTSIZE;
                    unsigned char clear[OSSL_HPKE_TSTSIZE];

                    hpke_suite.aead_id = aead_id;
                    if (!TEST_true(OSSL_HPKE_keygen(hpke_suite,
                                                    pub, &publen, &privp,
                                                    NULL, 0, testctx, NULL)))
                        overallresult = 0;
                    if (!TEST_ptr(ctx = OSSL_HPKE_CTX_new(hpke_mode, hpke_suite,
                                                          OSSL_HPKE_ROLE_SENDER,
                                                          testctx, NULL)))
                        overallresult = 0;
                    if (hpke_mode == OSSL_HPKE_MODE_PSK
                        || hpke_mode == OSSL_HPKE_MODE_PSKAUTH) {
                        if (!TEST_true(OSSL_HPKE_CTX_set1_psk(ctx, pskidp,
                                                              pskp, psklen)))
                            overallresult = 0;
                    }
                    if (hpke_mode == OSSL_HPKE_MODE_AUTH
                        || hpke_mode == OSSL_HPKE_MODE_PSKAUTH) {
                        if (!TEST_true(OSSL_HPKE_CTX_set1_authpriv(ctx,
                                                                   authpriv)))
                            overallresult = 0;
                    }
                    if (!TEST_true(OSSL_HPKE_encap(ctx, senderpub,
                                                   &senderpublen,
                                                   pub, publen,
                                                   infop, infolen)))
                        overallresult = 0;
                    /* throw in a call with a too-short cipherlen */
                    cipherlen = 15;
                    if (!TEST_false(OSSL_HPKE_seal(ctx, cipher, &cipherlen,
                                                   aadp, aadlen,
                                                   plain, plainlen)))
                        overallresult = 0;
                    /* fix back real cipherlen */
                    cipherlen = OSSL_HPKE_TSTSIZE;
                    if (!TEST_true(OSSL_HPKE_seal(ctx, cipher, &cipherlen,
                                                  aadp, aadlen,
                                                  plain, plainlen)))
                        overallresult = 0;
                    OSSL_HPKE_CTX_free(ctx);
                    memset(clear, 0, clearlen);
                    rctx = OSSL_HPKE_CTX_new(hpke_mode, hpke_suite,
                                             OSSL_HPKE_ROLE_RECEIVER,
                                             testctx, NULL);
                    if (!TEST_ptr(rctx))
                        overallresult = 0;
                    if (hpke_mode == OSSL_HPKE_MODE_PSK
                        || hpke_mode == OSSL_HPKE_MODE_PSKAUTH) {
                        if (!TEST_true(OSSL_HPKE_CTX_set1_psk(rctx, pskidp,
                                                              pskp, psklen)))
                            overallresult = 0;
                    }
                    if (hpke_mode == OSSL_HPKE_MODE_AUTH
                        || hpke_mode == OSSL_HPKE_MODE_PSKAUTH) {
                        /* check a borked p256 key */
                        if (hpke_suite.kem_id == OSSL_HPKE_KEM_ID_P256) {
                            /* set to fail decode of authpub this time */
                            if (!TEST_false(OSSL_HPKE_CTX_set1_authpub(rctx,
                                                                       authpub,
                                                                       10
                                                                       )))
                                overallresult = 0;
                        }
                        if (!TEST_true(OSSL_HPKE_CTX_set1_authpub(rctx,
                                                                  authpubp,
                                                                  authpublen)))
                            overallresult = 0;
                    }
                    if (!TEST_true(OSSL_HPKE_decap(rctx, senderpub,
                                                   senderpublen, privp,
                                                   infop, infolen)))
                        overallresult = 0;
                    /* throw in a call with a too-short clearlen */
                    clearlen = 15;
                    if (!TEST_false(OSSL_HPKE_open(rctx, clear, &clearlen,
                                                   aadp, aadlen, cipher,
                                                   cipherlen)))
                        overallresult = 0;
                    /* fix up real clearlen again */
                    clearlen = OSSL_HPKE_TSTSIZE;
                    if (!TEST_true(OSSL_HPKE_open(rctx, clear, &clearlen,
                                                  aadp, aadlen, cipher,
                                                  cipherlen)))
                        overallresult = 0;
                    OSSL_HPKE_CTX_free(rctx);
                    EVP_PKEY_free(privp);
                    privp = NULL;
                    /* check output */
                    if (!TEST_mem_eq(clear, clearlen, plain, plainlen)) {
                        overallresult = 0;
                    }
                    if (verbose || overallresult != 1) {
                        const char *res = NULL;

                        res = (overallresult == 1 ? "worked" : "failed");
                        TEST_note("HPKE %s for mode: %s/0x%02x, "\
                                  "kem: %s/0x%02x, kdf: %s/0x%02x, "\
                                  "aead: %s/0x%02x", res,
                                  mode_str_list[mind], (int) mind,
                                  kem_str_list[kemind], kem_id,
                                  kdf_str_list[kdfind], kdf_id,
                                  aead_str_list[aeadind], aead_id);
                    }
                }
            }
            EVP_PKEY_free(authpriv);
        }
    }
    return overallresult;
}

/**
 * @brief check roundtrip for export
 * @return 1 for success, other otherwise
 */
static int test_hpke_export(void)
{
    int erv = 0;
    EVP_PKEY *privp = NULL;
    unsigned char pub[OSSL_HPKE_TSTSIZE];
    size_t publen = sizeof(pub);
    int hpke_mode = OSSL_HPKE_MODE_BASE;
    OSSL_HPKE_SUITE hpke_suite = OSSL_HPKE_SUITE_DEFAULT;
    OSSL_HPKE_CTX *ctx = NULL;
    OSSL_HPKE_CTX *rctx = NULL;
    unsigned char exp[32];
    unsigned char exp2[32];
    unsigned char rexp[32];
    unsigned char rexp2[32];
    unsigned char plain[] = "quick brown fox";
    size_t plainlen = sizeof(plain);
    unsigned char enc[OSSL_HPKE_TSTSIZE];
    size_t enclen = sizeof(enc);
    unsigned char cipher[OSSL_HPKE_TSTSIZE];
    size_t cipherlen = sizeof(cipher);
    unsigned char clear[OSSL_HPKE_TSTSIZE];
    size_t clearlen = sizeof(clear);
    char *estr = "foo";

    if (!TEST_true(OSSL_HPKE_keygen(hpke_suite, pub, &publen, &privp,
                                    NULL, 0, testctx, NULL)))
        goto end;
    if (!TEST_ptr(ctx = OSSL_HPKE_CTX_new(hpke_mode, hpke_suite,
                                          OSSL_HPKE_ROLE_SENDER,
                                          testctx, NULL)))
        goto end;
    /* a few error cases 1st */
    if (!TEST_false(OSSL_HPKE_export(NULL, exp, sizeof(exp),
                                     (unsigned char *)estr, strlen(estr))))
        goto end;
    /* ctx before encap should fail too */
    if (!TEST_false(OSSL_HPKE_export(ctx, exp, sizeof(exp),
                                     (unsigned char *)estr, strlen(estr))))
        goto end;
    if (!TEST_true(OSSL_HPKE_encap(ctx, enc, &enclen, pub, publen, NULL, 0)))
        goto end;
    if (!TEST_true(OSSL_HPKE_seal(ctx, cipher, &cipherlen, NULL, 0,
                                  plain, plainlen)))
        goto end;
    /* now for real */
    if (!TEST_true(OSSL_HPKE_export(ctx, exp, sizeof(exp),
                                    (unsigned char *)estr, strlen(estr))))
        goto end;
    /* check a 2nd call with same input gives same output */
    if (!TEST_true(OSSL_HPKE_export(ctx, exp2, sizeof(exp2),
                                    (unsigned char *)estr, strlen(estr))))
        goto end;
    if (!TEST_mem_eq(exp, sizeof(exp), exp2, sizeof(exp2)))
        goto end;
    if (!TEST_ptr(rctx = OSSL_HPKE_CTX_new(hpke_mode, hpke_suite,
                                           OSSL_HPKE_ROLE_RECEIVER,
                                           testctx, NULL)))
        goto end;
    if (!TEST_true(OSSL_HPKE_decap(rctx, enc, enclen, privp, NULL, 0)))
        goto end;
    if (!TEST_true(OSSL_HPKE_open(rctx, clear, &clearlen, NULL, 0,
                                  cipher, cipherlen)))
        goto end;
    if (!TEST_true(OSSL_HPKE_export(rctx, rexp, sizeof(rexp),
                                    (unsigned char *)estr, strlen(estr))))
        goto end;
    /* check a 2nd call with same input gives same output */
    if (!TEST_true(OSSL_HPKE_export(rctx, rexp2, sizeof(rexp2),
                                    (unsigned char *)estr, strlen(estr))))
        goto end;
    if (!TEST_mem_eq(rexp, sizeof(rexp), rexp2, sizeof(rexp2)))
        goto end;
    if (!TEST_mem_eq(exp, sizeof(exp), rexp, sizeof(rexp)))
        goto end;
    erv = 1;
end:
    OSSL_HPKE_CTX_free(ctx);
    OSSL_HPKE_CTX_free(rctx);
    EVP_PKEY_free(privp);
    return erv;
}

/**
 * @brief Check mapping from strings to HPKE suites
 * @return 1 for success, other otherwise
 */
static int test_hpke_suite_strs(void)
{
    int overallresult = 1;
    int kemind = 0;
    int kdfind = 0;
    int aeadind = 0;
    int sind = 0;
    char sstr[128];
    OSSL_HPKE_SUITE stirred;
    char giant[2048];

    for (kemind = 0; kemind != OSSL_NELEM(kem_str_list); kemind++) {
        for (kdfind = 0; kdfind != OSSL_NELEM(kdf_str_list); kdfind++) {
            for (aeadind = 0; aeadind != OSSL_NELEM(aead_str_list); aeadind++) {
                BIO_snprintf(sstr, 128, "%s,%s,%s", kem_str_list[kemind],
                             kdf_str_list[kdfind], aead_str_list[aeadind]);
                if (TEST_true(OSSL_HPKE_str2suite(sstr, &stirred)) != 1) {
                    if (verbose)
                        TEST_note("Unexpected str2suite fail for :%s",
                                  bogus_suite_strs[sind]);
                    overallresult = 0;
                }
            }
        }
    }
    for (sind = 0; sind != OSSL_NELEM(bogus_suite_strs); sind++) {
        if (TEST_false(OSSL_HPKE_str2suite(bogus_suite_strs[sind],
                                           &stirred)) != 1) {
            if (verbose)
                TEST_note("OSSL_HPKE_str2suite didn't fail for bogus[%d]:%s",
                          sind, bogus_suite_strs[sind]);
            overallresult = 0;
        }
    }
    /* check a few errors */
    if (!TEST_false(OSSL_HPKE_str2suite("", &stirred)))
        overallresult = 0;
    if (!TEST_false(OSSL_HPKE_str2suite(NULL, &stirred)))
        overallresult = 0;
    if (!TEST_false(OSSL_HPKE_str2suite("", NULL)))
        overallresult = 0;
    memset(giant, 'A', sizeof(giant) - 1);
    giant[sizeof(giant) - 1] = '\0';
    if (!TEST_false(OSSL_HPKE_str2suite(giant, &stirred)))
        overallresult = 0;

    return overallresult;
}

/**
 * @brief try the various GREASEy APIs
 * @return 1 for success, other otherwise
 */
static int test_hpke_grease(void)
{
    int overallresult = 1;
    OSSL_HPKE_SUITE g_suite;
    unsigned char g_pub[OSSL_HPKE_TSTSIZE];
    size_t g_pub_len = OSSL_HPKE_TSTSIZE;
    unsigned char g_cipher[OSSL_HPKE_TSTSIZE];
    size_t g_cipher_len = 266;
    size_t clearlen = 128;
    size_t expanded = 0;
    size_t enclen = 0;
    size_t ikmelen = 0;

    memset(&g_suite, 0, sizeof(OSSL_HPKE_SUITE));
    /* GREASEing */
    /* check too short for public value */
    g_pub_len = 10;
    if (TEST_false(OSSL_HPKE_get_grease_value(NULL, &g_suite,
                                              g_pub, &g_pub_len,
                                              g_cipher, g_cipher_len,
                                              testctx, NULL)) != 1) {
        overallresult = 0;
    }
    /* reset to work */
    g_pub_len = OSSL_HPKE_TSTSIZE;
    if (TEST_true(OSSL_HPKE_get_grease_value(NULL, &g_suite,
                                             g_pub, &g_pub_len,
                                             g_cipher, g_cipher_len,
                                             testctx, NULL)) != 1) {
        overallresult = 0;
    }
    /* expansion */
    expanded = OSSL_HPKE_get_ciphertext_size(g_suite, clearlen);
    if (!TEST_size_t_gt(expanded, clearlen)) {
        overallresult = 0;
    }
    enclen = OSSL_HPKE_get_public_encap_size(g_suite);
    if (!TEST_size_t_ne(enclen, 0))
        overallresult = 0;
    /* not really GREASE but we'll check ikmelen thing */
    ikmelen = OSSL_HPKE_get_recommended_ikmelen(g_suite);
    if (!TEST_size_t_ne(ikmelen, 0))
        overallresult = 0;

    return overallresult;
}

/*
 * Make a set of calls with odd parameters
 */
static int test_hpke_oddcalls(void)
{
    int erv = 0;
    EVP_PKEY *privp = NULL;
    unsigned char pub[OSSL_HPKE_TSTSIZE];
    size_t publen = sizeof(pub);
    int hpke_mode = OSSL_HPKE_MODE_BASE;
    int bad_mode = 0xbad;
    OSSL_HPKE_SUITE hpke_suite = OSSL_HPKE_SUITE_DEFAULT;
    OSSL_HPKE_SUITE bad_suite = { 0xbad, 0xbad, 0xbad };
    OSSL_HPKE_CTX *ctx = NULL;
    OSSL_HPKE_CTX *rctx = NULL;
    unsigned char plain[] = "quick brown fox";
    size_t plainlen = sizeof(plain);
    unsigned char enc[OSSL_HPKE_TSTSIZE], smallenc[10];
    size_t enclen = sizeof(enc), smallenclen = sizeof(smallenc);
    unsigned char cipher[OSSL_HPKE_TSTSIZE];
    size_t cipherlen = sizeof(cipher);
    unsigned char clear[OSSL_HPKE_TSTSIZE];
    size_t clearlen = sizeof(clear);
    unsigned char fake_ikm[OSSL_HPKE_TSTSIZE];
    char *badpropq = "yeah, this won't work";
    uint64_t lseq = 0;
    char giant_pskid[OSSL_HPKE_MAX_PARMLEN + 10];
    unsigned char info[OSSL_HPKE_TSTSIZE];

    /* many of the calls below are designed to get better test coverage */

    /* NULL ctx calls */
    OSSL_HPKE_CTX_free(NULL);
    if (!TEST_false(OSSL_HPKE_CTX_set_seq(NULL, 1)))
        goto end;
    if (!TEST_false(OSSL_HPKE_CTX_get_seq(NULL, &lseq)))
        goto end;
    if (!TEST_false(OSSL_HPKE_CTX_set1_authpub(NULL, pub, publen)))
        goto end;
    if (!TEST_false(OSSL_HPKE_CTX_set1_authpriv(NULL, privp)))
        goto end;
    if (!TEST_false(OSSL_HPKE_CTX_set1_ikme(NULL, NULL, 0)))
        goto end;
    if (!TEST_false(OSSL_HPKE_CTX_set1_psk(NULL, NULL, NULL, 0)))
        goto end;

    /* bad suite calls */
    hpke_suite.aead_id = 0xbad;
    if (!TEST_false(OSSL_HPKE_suite_check(hpke_suite)))
        goto end;
    hpke_suite.aead_id = OSSL_HPKE_AEAD_ID_AES_GCM_128;
    if (!TEST_false(OSSL_HPKE_suite_check(bad_suite)))
        goto end;
    if (!TEST_false(OSSL_HPKE_get_recommended_ikmelen(bad_suite)))
        goto end;
    if (!TEST_false(OSSL_HPKE_get_public_encap_size(bad_suite)))
        goto end;
    if (!TEST_false(OSSL_HPKE_get_ciphertext_size(bad_suite, 0)))
        goto end;
    if (!TEST_false(OSSL_HPKE_keygen(bad_suite, pub, &publen, &privp,
                                     NULL, 0, testctx, badpropq)))
        goto end;
    if (!TEST_false(OSSL_HPKE_keygen(bad_suite, pub, &publen, &privp,
                                     NULL, 0, testctx, NULL)))
        goto end;

    /* dodgy keygen calls */
    /* no pub */
    if (!TEST_false(OSSL_HPKE_keygen(hpke_suite, NULL, &publen, &privp,
                                     NULL, 0, testctx, NULL)))
        goto end;
    /* ikmlen but NULL ikm */
    if (!TEST_false(OSSL_HPKE_keygen(hpke_suite, pub, &publen, &privp,
                                     NULL, 80, testctx, NULL)))
        goto end;
    /* zero ikmlen but ikm */
    if (!TEST_false(OSSL_HPKE_keygen(hpke_suite, pub, &publen, &privp,
                                     fake_ikm, 0, testctx, NULL)))
        goto end;
    /* GIANT ikmlen */
    if (!TEST_false(OSSL_HPKE_keygen(hpke_suite, pub, &publen, &privp,
                                     fake_ikm, -1, testctx, NULL)))
        goto end;
    /* short publen */
    publen = 10;
    if (!TEST_false(OSSL_HPKE_keygen(hpke_suite, pub, &publen, &privp,
                                     NULL, 0, testctx, NULL)))
        goto end;
    publen = sizeof(pub);

    /* encap/decap with NULLs */
    if (!TEST_false(OSSL_HPKE_encap(NULL, NULL, NULL, NULL, 0, NULL, 0)))
        goto end;
    if (!TEST_false(OSSL_HPKE_decap(NULL, NULL, 0, NULL, NULL, 0)))
        goto end;

    /*
     * run through a sender/recipient set of calls but with
     * failing calls interspersed whenever possible
     */
    /* good keygen */
    if (!TEST_true(OSSL_HPKE_keygen(hpke_suite, pub, &publen, &privp,
                                    NULL, 0, testctx, NULL)))
        goto end;

    /* a psk context with no psk => encap fail */
    if (!TEST_ptr(ctx = OSSL_HPKE_CTX_new(OSSL_HPKE_MODE_PSK, hpke_suite,
                                          OSSL_HPKE_ROLE_SENDER,
                                          testctx, NULL)))
        goto end;
    /* set bad length psk */
    if (!TEST_false(OSSL_HPKE_CTX_set1_psk(ctx, "foo",
                                           (unsigned char *)"bar", -1)))
        goto end;
    /* set bad length pskid */
    memset(giant_pskid, 'A', sizeof(giant_pskid) - 1);
    giant_pskid[sizeof(giant_pskid) - 1] = '\0';
    if (!TEST_false(OSSL_HPKE_CTX_set1_psk(ctx, giant_pskid,
                                           (unsigned char *)"bar", 3)))
        goto end;
    /* still no psk really set so encap fails */
    if (!TEST_false(OSSL_HPKE_encap(ctx, enc, &enclen, pub, publen, NULL, 0)))
        goto end;
    OSSL_HPKE_CTX_free(ctx);

    /* bad suite */
    if (!TEST_ptr_null(ctx = OSSL_HPKE_CTX_new(hpke_mode, bad_suite,
                                               OSSL_HPKE_ROLE_SENDER,
                                               testctx, NULL)))
        goto end;
    /* bad mode */
    if (!TEST_ptr_null(ctx = OSSL_HPKE_CTX_new(bad_mode, hpke_suite,
                                               OSSL_HPKE_ROLE_SENDER,
                                               testctx, NULL)))
        goto end;
    /* make good ctx */
    if (!TEST_ptr(ctx = OSSL_HPKE_CTX_new(hpke_mode, hpke_suite,
                                          OSSL_HPKE_ROLE_SENDER,
                                          testctx, NULL)))
        goto end;
    /* too long ikm */
    if (!TEST_false(OSSL_HPKE_CTX_set1_ikme(ctx, fake_ikm, -1)))
        goto end;
    /* zero length ikm */
    if (!TEST_false(OSSL_HPKE_CTX_set1_ikme(ctx, fake_ikm, 0)))
        goto end;
    /* NULL authpub */
    if (!TEST_false(OSSL_HPKE_CTX_set1_authpub(ctx, NULL, 0)))
        goto end;
    /* NULL auth priv */
    if (!TEST_false(OSSL_HPKE_CTX_set1_authpriv(ctx, NULL)))
        goto end;
    /* priv good, but mode is bad */
    if (!TEST_false(OSSL_HPKE_CTX_set1_authpriv(ctx, privp)))
        goto end;
    /* bad mode for psk */
    if (!TEST_false(OSSL_HPKE_CTX_set1_psk(ctx, "foo",
                                           (unsigned char *)"bar", 3)))
        goto end;
    /* seal before encap */
    if (!TEST_false(OSSL_HPKE_seal(ctx, cipher, &cipherlen, NULL, 0,
                                   plain, plainlen)))
        goto end;
    /* encap with dodgy public */
    if (!TEST_false(OSSL_HPKE_encap(ctx, enc, &enclen, pub, 1, NULL, 0)))
        goto end;
    /* encap with too big info */
    if (!TEST_false(OSSL_HPKE_encap(ctx, enc, &enclen, pub, 1, info, -1)))
        goto end;
    /* encap with NULL info & non-zero infolen */
    if (!TEST_false(OSSL_HPKE_encap(ctx, enc, &enclen, pub, 1, NULL, 1)))
        goto end;
    /* encap with non-NULL info & zero infolen */
    if (!TEST_false(OSSL_HPKE_encap(ctx, enc, &enclen, pub, 1, info, 0)))
        goto end;
    /* encap with too small enc */
    if (!TEST_false(OSSL_HPKE_encap(ctx, smallenc, &smallenclen, pub, 1, NULL, 0)))
        goto end;
    /* good encap */
    if (!TEST_true(OSSL_HPKE_encap(ctx, enc, &enclen, pub, publen, NULL, 0)))
        goto end;
    /* second encap fail */
    if (!TEST_false(OSSL_HPKE_encap(ctx, enc, &enclen, pub, publen, NULL, 0)))
        goto end;
    plainlen = 0;
    /* should fail for no plaintext */
    if (!TEST_false(OSSL_HPKE_seal(ctx, cipher, &cipherlen, NULL, 0,
                                   plain, plainlen)))
        goto end;
    plainlen = sizeof(plain);
    /* working seal */
    if (!TEST_true(OSSL_HPKE_seal(ctx, cipher, &cipherlen, NULL, 0,
                                  plain, plainlen)))
        goto end;

    /* receiver side */
    /* decap fail with psk mode but no psk set */
    if (!TEST_ptr(rctx = OSSL_HPKE_CTX_new(OSSL_HPKE_MODE_PSK, hpke_suite,
                                           OSSL_HPKE_ROLE_RECEIVER,
                                           testctx, NULL)))
        goto end;
    if (!TEST_false(OSSL_HPKE_decap(rctx, enc, enclen, privp, NULL, 0)))
        goto end;
    /* done with PSK mode */
    OSSL_HPKE_CTX_free(rctx);

    /* back good calls for base mode  */
    if (!TEST_ptr(rctx = OSSL_HPKE_CTX_new(hpke_mode, hpke_suite,
                                           OSSL_HPKE_ROLE_RECEIVER,
                                           testctx, NULL)))
        goto end;
    /* open before decap */
    if (!TEST_false(OSSL_HPKE_open(rctx, clear, &clearlen, NULL, 0,
                                   cipher, cipherlen)))
        goto end;
    /* decap with info too long */
    if (!TEST_false(OSSL_HPKE_decap(rctx, enc, enclen, privp, info, -1)))
        goto end;
    /* good decap */
    if (!TEST_true(OSSL_HPKE_decap(rctx, enc, enclen, privp, NULL, 0)))
        goto end;
    /* second decap fail */
    if (!TEST_false(OSSL_HPKE_decap(rctx, enc, enclen, privp, NULL, 0)))
        goto end;
    /* no space for recovered clear */
    clearlen = 0;
    if (!TEST_false(OSSL_HPKE_open(rctx, clear, &clearlen, NULL, 0,
                                   cipher, cipherlen)))
        goto end;
    clearlen = OSSL_HPKE_TSTSIZE;
    /* seq wrap around test */
    if (!TEST_true(OSSL_HPKE_CTX_set_seq(rctx, -1)))
        goto end;
    if (!TEST_false(OSSL_HPKE_open(rctx, clear, &clearlen, NULL, 0,
                                   cipher, cipherlen)))
        goto end;
    if (!TEST_true(OSSL_HPKE_CTX_set_seq(rctx, 0)))
        goto end;
    if (!TEST_true(OSSL_HPKE_open(rctx, clear, &clearlen, NULL, 0,
                                  cipher, cipherlen)))
        goto end;
    if (!TEST_mem_eq(plain, plainlen, clear, clearlen))
        goto end;
    erv = 1;
end:
    OSSL_HPKE_CTX_free(ctx);
    OSSL_HPKE_CTX_free(rctx);
    EVP_PKEY_free(privp);
    return erv;
}

#ifndef OPENSSL_NO_ECX
/* from RFC 9180 Appendix A.1.1 */
static const unsigned char ikm25519[] = {
    0x72, 0x68, 0x60, 0x0d, 0x40, 0x3f, 0xce, 0x43,
    0x15, 0x61, 0xae, 0xf5, 0x83, 0xee, 0x16, 0x13,
    0x52, 0x7c, 0xff, 0x65, 0x5c, 0x13, 0x43, 0xf2,
    0x98, 0x12, 0xe6, 0x67, 0x06, 0xdf, 0x32, 0x34
};
static const unsigned char pub25519[] = {
    0x37, 0xfd, 0xa3, 0x56, 0x7b, 0xdb, 0xd6, 0x28,
    0xe8, 0x86, 0x68, 0xc3, 0xc8, 0xd7, 0xe9, 0x7d,
    0x1d, 0x12, 0x53, 0xb6, 0xd4, 0xea, 0x6d, 0x44,
    0xc1, 0x50, 0xf7, 0x41, 0xf1, 0xbf, 0x44, 0x31
};
#endif

/* from RFC9180 Appendix A.3.1 */
static const unsigned char ikmp256[] = {
    0x42, 0x70, 0xe5, 0x4f, 0xfd, 0x08, 0xd7, 0x9d,
    0x59, 0x28, 0x02, 0x0a, 0xf4, 0x68, 0x6d, 0x8f,
    0x6b, 0x7d, 0x35, 0xdb, 0xe4, 0x70, 0x26, 0x5f,
    0x1f, 0x5a, 0xa2, 0x28, 0x16, 0xce, 0x86, 0x0e
};
static const unsigned char pubp256[] = {
    0x04, 0xa9, 0x27, 0x19, 0xc6, 0x19, 0x5d, 0x50,
    0x85, 0x10, 0x4f, 0x46, 0x9a, 0x8b, 0x98, 0x14,
    0xd5, 0x83, 0x8f, 0xf7, 0x2b, 0x60, 0x50, 0x1e,
    0x2c, 0x44, 0x66, 0xe5, 0xe6, 0x7b, 0x32, 0x5a,
    0xc9, 0x85, 0x36, 0xd7, 0xb6, 0x1a, 0x1a, 0xf4,
    0xb7, 0x8e, 0x5b, 0x7f, 0x95, 0x1c, 0x09, 0x00,
    0xbe, 0x86, 0x3c, 0x40, 0x3c, 0xe6, 0x5c, 0x9b,
    0xfc, 0xb9, 0x38, 0x26, 0x57, 0x22, 0x2d, 0x18,
    0xc4
};

/*
 * A test vector that exercises the counter iteration
 * for p256. This was contributed by Ilari L. on the
 * CFRG list, see the mail archive:
 * https://mailarchive.ietf.org/arch/msg/cfrg/4zwl_y5YN6OU9oeWZOMHNOlOa2w/
 */
static const unsigned char ikmiter[] = {
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x03, 0x01, 0x38, 0xb5, 0xec
};
static const unsigned char pubiter[] = {
    0x04, 0x7d, 0x0c, 0x87, 0xff, 0xd5, 0xd1, 0x45,
    0x54, 0xa7, 0x51, 0xdf, 0xa3, 0x99, 0x26, 0xa9,
    0xe3, 0x0e, 0x7c, 0x3c, 0x65, 0x62, 0x4f, 0x4b,
    0x5f, 0xb3, 0xad, 0x7a, 0xa4, 0xda, 0xc2, 0x4a,
    0xd8, 0xf5, 0xbe, 0xd0, 0xe8, 0x6e, 0xb8, 0x84,
    0x1c, 0xe4, 0x89, 0x2e, 0x0f, 0xc3, 0x87, 0xbb,
    0xdb, 0xfe, 0x16, 0x0d, 0x58, 0x9c, 0x89, 0x2d,
    0xd4, 0xb1, 0x46, 0x4a, 0xc3, 0x51, 0xc5, 0x6f,
    0xb6
};

/* from RFC9180 Appendix A.6.1 */
static const unsigned char ikmp521[] = {
    0x7f, 0x06, 0xab, 0x82, 0x15, 0x10, 0x5f, 0xc4,
    0x6a, 0xce, 0xeb, 0x2e, 0x3d, 0xc5, 0x02, 0x8b,
    0x44, 0x36, 0x4f, 0x96, 0x04, 0x26, 0xeb, 0x0d,
    0x8e, 0x40, 0x26, 0xc2, 0xf8, 0xb5, 0xd7, 0xe7,
    0xa9, 0x86, 0x68, 0x8f, 0x15, 0x91, 0xab, 0xf5,
    0xab, 0x75, 0x3c, 0x35, 0x7a, 0x5d, 0x6f, 0x04,
    0x40, 0x41, 0x4b, 0x4e, 0xd4, 0xed, 0xe7, 0x13,
    0x17, 0x77, 0x2a, 0xc9, 0x8d, 0x92, 0x39, 0xf7,
    0x09, 0x04
};
static const unsigned char pubp521[] = {
    0x04, 0x01, 0x38, 0xb3, 0x85, 0xca, 0x16, 0xbb,
    0x0d, 0x5f, 0xa0, 0xc0, 0x66, 0x5f, 0xbb, 0xd7,
    0xe6, 0x9e, 0x3e, 0xe2, 0x9f, 0x63, 0x99, 0x1d,
    0x3e, 0x9b, 0x5f, 0xa7, 0x40, 0xaa, 0xb8, 0x90,
    0x0a, 0xae, 0xed, 0x46, 0xed, 0x73, 0xa4, 0x90,
    0x55, 0x75, 0x84, 0x25, 0xa0, 0xce, 0x36, 0x50,
    0x7c, 0x54, 0xb2, 0x9c, 0xc5, 0xb8, 0x5a, 0x5c,
    0xee, 0x6b, 0xae, 0x0c, 0xf1, 0xc2, 0x1f, 0x27,
    0x31, 0xec, 0xe2, 0x01, 0x3d, 0xc3, 0xfb, 0x7c,
    0x8d, 0x21, 0x65, 0x4b, 0xb1, 0x61, 0xb4, 0x63,
    0x96, 0x2c, 0xa1, 0x9e, 0x8c, 0x65, 0x4f, 0xf2,
    0x4c, 0x94, 0xdd, 0x28, 0x98, 0xde, 0x12, 0x05,
    0x1f, 0x1e, 0xd0, 0x69, 0x22, 0x37, 0xfb, 0x02,
    0xb2, 0xf8, 0xd1, 0xdc, 0x1c, 0x73, 0xe9, 0xb3,
    0x66, 0xb5, 0x29, 0xeb, 0x43, 0x6e, 0x98, 0xa9,
    0x96, 0xee, 0x52, 0x2a, 0xef, 0x86, 0x3d, 0xd5,
    0x73, 0x9d, 0x2f, 0x29, 0xb0
};

static int test_hpke_random_suites(void)
{
    OSSL_HPKE_SUITE def_suite = OSSL_HPKE_SUITE_DEFAULT;
    OSSL_HPKE_SUITE suite = OSSL_HPKE_SUITE_DEFAULT;
    OSSL_HPKE_SUITE suite2 = { 0xff01, 0xff02, 0xff03 };
    unsigned char enc[200];
    size_t enclen = sizeof(enc);
    unsigned char ct[500];
    size_t ctlen = sizeof(ct);

    /* test with NULL/0 inputs */
    if (!TEST_false(OSSL_HPKE_get_grease_value(NULL, NULL,
                                               NULL, NULL, NULL, 0,
                                               testctx, NULL)))
        return 0;
    enclen = 10;
    if (!TEST_false(OSSL_HPKE_get_grease_value(&def_suite, &suite2,
                                               enc, &enclen, ct, ctlen,
                                               testctx, NULL)))
        return 0;

    enclen = sizeof(enc); /* reset, 'cause get_grease() will have set */
    /* test with a should-be-good suite */
    if (!TEST_true(OSSL_HPKE_get_grease_value(&def_suite, &suite2,
                                              enc, &enclen, ct, ctlen,
                                              testctx, NULL)))
        return 0;
    /* no suggested suite */
    enclen = sizeof(enc); /* reset, 'cause get_grease() will have set */
    if (!TEST_true(OSSL_HPKE_get_grease_value(NULL, &suite2,
                                              enc, &enclen,
                                              ct, ctlen,
                                              testctx, NULL)))
        return 0;
    /* suggested suite with P-521, just to be sure we hit long values */
    enclen = sizeof(enc); /* reset, 'cause get_grease() will have set */
    suite.kem_id = OSSL_HPKE_KEM_ID_P521;
    if (!TEST_true(OSSL_HPKE_get_grease_value(&suite, &suite2,
                                              enc, &enclen, ct, ctlen,
                                              testctx, NULL)))
        return 0;
    enclen = sizeof(enc);
    ctlen = 2; /* too-short cttext (can't fit an aead tag) */
    if (!TEST_false(OSSL_HPKE_get_grease_value(NULL, &suite2,
                                               enc, &enclen, ct, ctlen,
                                               testctx, NULL)))
        return 0;

    ctlen = sizeof(ct);
    enclen = sizeof(enc);

    suite.kem_id = OSSL_HPKE_KEM_ID_X25519; /* back to default */
    suite.aead_id = 0x1234; /* bad aead */
    if (!TEST_false(OSSL_HPKE_get_grease_value(&suite, &suite2,
                                               enc, &enclen, ct, ctlen,
                                               testctx, NULL)))
        return 0;
    enclen = sizeof(enc);
    suite.aead_id = def_suite.aead_id; /* good aead */
    suite.kdf_id = 0x3451; /* bad kdf */
    if (!TEST_false(OSSL_HPKE_get_grease_value(&suite, &suite2,
                                               enc, &enclen, ct, ctlen,
                                               testctx, NULL)))
        return 0;
    enclen = sizeof(enc);
    suite.kdf_id = def_suite.kdf_id; /* good kdf */
    suite.kem_id = 0x4517; /* bad kem */
    if (!TEST_false(OSSL_HPKE_get_grease_value(&suite, &suite2,
                                               enc, &enclen, ct, ctlen,
                                               testctx, NULL)))
        return 0;
    return 1;
}

/*
 * @brief generate a key pair from initial key material (ikm) and check public
 * @param kem_id the KEM to use (RFC9180 code point)
 * @ikm is the initial key material buffer
 * @ikmlen is the length of ikm
 * @pub is the public key buffer
 * @publen is the length of the public key
 * @return 1 for good, other otherwise
 *
 * This calls OSSL_HPKE_keygen specifying only the IKM, then
 * compares the key pair values with the already-known values
 * that were input.
 */
static int test_hpke_one_ikm_gen(uint16_t kem_id,
                                 const unsigned char *ikm, size_t ikmlen,
                                 const unsigned char *pub, size_t publen)
{
    OSSL_HPKE_SUITE hpke_suite = OSSL_HPKE_SUITE_DEFAULT;
    unsigned char lpub[OSSL_HPKE_TSTSIZE];
    size_t lpublen = OSSL_HPKE_TSTSIZE;
    EVP_PKEY *sk = NULL;

    hpke_suite.kem_id = kem_id;
    if (!TEST_true(OSSL_HPKE_keygen(hpke_suite, lpub, &lpublen, &sk,
                                    ikm, ikmlen, testctx, NULL)))
        return 0;
    if (!TEST_ptr(sk))
        return 0;
    EVP_PKEY_free(sk);
    if (!TEST_mem_eq(pub, publen, lpub, lpublen))
        return 0;
    return 1;
}

/*
 * @brief test some uses of IKM produce the expected public keys
 */
static int test_hpke_ikms(void)
{
    int res = 1;

#ifndef OPENSSL_NO_ECX
    res = test_hpke_one_ikm_gen(OSSL_HPKE_KEM_ID_X25519,
                                ikm25519, sizeof(ikm25519),
                                pub25519, sizeof(pub25519));
    if (res != 1)
        return res;
#endif

    res = test_hpke_one_ikm_gen(OSSL_HPKE_KEM_ID_P521,
                                ikmp521, sizeof(ikmp521),
                                pubp521, sizeof(pubp521));
    if (res != 1)
        return res;

    res = test_hpke_one_ikm_gen(OSSL_HPKE_KEM_ID_P256,
                                ikmp256, sizeof(ikmp256),
                                pubp256, sizeof(pubp256));
    if (res != 1)
        return res;

    res = test_hpke_one_ikm_gen(OSSL_HPKE_KEM_ID_P256,
                                ikmiter, sizeof(ikmiter),
                                pubiter, sizeof(pubiter));
    if (res != 1)
        return res;

    return res;
}

/*
 * Test that use of a compressed format auth public key works
 * We'll do a typical round-trip for auth mode but provide the
 * auth public key in compressed form. That should work.
 */
static int test_hpke_compressed(void)
{
    int erv = 0;
    EVP_PKEY *privp = NULL;
    unsigned char pub[OSSL_HPKE_TSTSIZE];
    size_t publen = sizeof(pub);
    EVP_PKEY *authpriv = NULL;
    unsigned char authpub[OSSL_HPKE_TSTSIZE];
    size_t authpublen = sizeof(authpub);
    int hpke_mode = OSSL_HPKE_MODE_AUTH;
    OSSL_HPKE_SUITE hpke_suite = OSSL_HPKE_SUITE_DEFAULT;
    OSSL_HPKE_CTX *ctx = NULL;
    OSSL_HPKE_CTX *rctx = NULL;
    unsigned char plain[] = "quick brown fox";
    size_t plainlen = sizeof(plain);
    unsigned char enc[OSSL_HPKE_TSTSIZE];
    size_t enclen = sizeof(enc);
    unsigned char cipher[OSSL_HPKE_TSTSIZE];
    size_t cipherlen = sizeof(cipher);
    unsigned char clear[OSSL_HPKE_TSTSIZE];
    size_t clearlen = sizeof(clear);

    hpke_suite.kem_id = OSSL_HPKE_KEM_ID_P256;

    /* generate auth key pair */
    if (!TEST_true(OSSL_HPKE_keygen(hpke_suite, authpub, &authpublen, &authpriv,
                                    NULL, 0, testctx, NULL)))
        goto end;
    /* now get the compressed form public key */
    if (!TEST_true(EVP_PKEY_set_utf8_string_param(authpriv,
                      OSSL_PKEY_PARAM_EC_POINT_CONVERSION_FORMAT,
                      OSSL_PKEY_EC_POINT_CONVERSION_FORMAT_COMPRESSED)))
        goto end;
    if (!TEST_true(EVP_PKEY_get_octet_string_param(authpriv,
                                                   OSSL_PKEY_PARAM_PUB_KEY,
                                                   authpub,
                                                   sizeof(authpub),
                                                   &authpublen)))
        goto end;

    /* sender side as usual */
    if (!TEST_true(OSSL_HPKE_keygen(hpke_suite, pub, &publen, &privp,
                                    NULL, 0, testctx, NULL)))
        goto end;
    if (!TEST_ptr(ctx = OSSL_HPKE_CTX_new(hpke_mode, hpke_suite,
                                          OSSL_HPKE_ROLE_SENDER,
                                          testctx, NULL)))
        goto end;
    if (!TEST_true(OSSL_HPKE_CTX_set1_authpriv(ctx, authpriv)))
        goto end;
    if (!TEST_true(OSSL_HPKE_encap(ctx, enc, &enclen, pub, publen, NULL, 0)))
        goto end;
    if (!TEST_true(OSSL_HPKE_seal(ctx, cipher, &cipherlen, NULL, 0,
                                  plain, plainlen)))
        goto end;

    /* receiver side providing compressed form of auth public */
    if (!TEST_ptr(rctx = OSSL_HPKE_CTX_new(hpke_mode, hpke_suite,
                                           OSSL_HPKE_ROLE_RECEIVER,
                                           testctx, NULL)))
        goto end;
    if (!TEST_true(OSSL_HPKE_CTX_set1_authpub(rctx, authpub, authpublen)))
        goto end;
    if (!TEST_true(OSSL_HPKE_decap(rctx, enc, enclen, privp, NULL, 0)))
        goto end;
    if (!TEST_true(OSSL_HPKE_open(rctx, clear, &clearlen, NULL, 0,
                                  cipher, cipherlen)))
        goto end;
    erv = 1;

end:
    EVP_PKEY_free(privp);
    EVP_PKEY_free(authpriv);
    OSSL_HPKE_CTX_free(ctx);
    OSSL_HPKE_CTX_free(rctx);
    return erv;
}

/*
 * Test that nonce reuse calls are prevented as we expect
 */
static int test_hpke_noncereuse(void)
{
    int erv = 0;
    EVP_PKEY *privp = NULL;
    unsigned char pub[OSSL_HPKE_TSTSIZE];
    size_t publen = sizeof(pub);
    int hpke_mode = OSSL_HPKE_MODE_BASE;
    OSSL_HPKE_SUITE hpke_suite = OSSL_HPKE_SUITE_DEFAULT;
    OSSL_HPKE_CTX *ctx = NULL;
    OSSL_HPKE_CTX *rctx = NULL;
    unsigned char plain[] = "quick brown fox";
    size_t plainlen = sizeof(plain);
    unsigned char enc[OSSL_HPKE_TSTSIZE];
    size_t enclen = sizeof(enc);
    unsigned char cipher[OSSL_HPKE_TSTSIZE];
    size_t cipherlen = sizeof(cipher);
    unsigned char clear[OSSL_HPKE_TSTSIZE];
    size_t clearlen = sizeof(clear);
    uint64_t seq = 0xbad1dea;

    /* sender side is not allowed set seq once some crypto done */
    if (!TEST_true(OSSL_HPKE_keygen(hpke_suite, pub, &publen, &privp,
                                    NULL, 0, testctx, NULL)))
        goto end;
    if (!TEST_ptr(ctx = OSSL_HPKE_CTX_new(hpke_mode, hpke_suite,
                                          OSSL_HPKE_ROLE_SENDER,
                                          testctx, NULL)))
        goto end;
    /* set seq will fail before any crypto done */
    if (!TEST_false(OSSL_HPKE_CTX_set_seq(ctx, seq)))
        goto end;
    if (!TEST_true(OSSL_HPKE_encap(ctx, enc, &enclen, pub, publen, NULL, 0)))
        goto end;
    /* set seq will also fail after some crypto done */
    if (!TEST_false(OSSL_HPKE_CTX_set_seq(ctx, seq + 1)))
        goto end;
    if (!TEST_true(OSSL_HPKE_seal(ctx, cipher, &cipherlen, NULL, 0,
                                  plain, plainlen)))
        goto end;

    /* receiver side is allowed control seq */
    if (!TEST_ptr(rctx = OSSL_HPKE_CTX_new(hpke_mode, hpke_suite,
                                           OSSL_HPKE_ROLE_RECEIVER,
                                           testctx, NULL)))
        goto end;
    /* set seq will work before any crypto done */
    if (!TEST_true(OSSL_HPKE_CTX_set_seq(rctx, seq)))
        goto end;
    if (!TEST_true(OSSL_HPKE_decap(rctx, enc, enclen, privp, NULL, 0)))
        goto end;
    /* set seq will work for receivers even after crypto done */
    if (!TEST_true(OSSL_HPKE_CTX_set_seq(rctx, seq)))
        goto end;
    /* but that value isn't good so decap will fail */
    if (!TEST_false(OSSL_HPKE_open(rctx, clear, &clearlen, NULL, 0,
                                   cipher, cipherlen)))
        goto end;
    /* reset seq to correct value and _open() should work */
    if (!TEST_true(OSSL_HPKE_CTX_set_seq(rctx, 0)))
        goto end;
    if (!TEST_true(OSSL_HPKE_open(rctx, clear, &clearlen, NULL, 0,
                                  cipher, cipherlen)))
        goto end;
    erv = 1;

end:
    EVP_PKEY_free(privp);
    OSSL_HPKE_CTX_free(ctx);
    OSSL_HPKE_CTX_free(rctx);
    return erv;
}

typedef enum OPTION_choice {
    OPT_ERR = -1,
    OPT_EOF = 0,
    OPT_VERBOSE,
    OPT_TEST_ENUM
} OPTION_CHOICE;

const OPTIONS *test_get_options(void)
{
    static const OPTIONS test_options[] = {
        OPT_TEST_OPTIONS_DEFAULT_USAGE,
        { "v", OPT_VERBOSE, '-', "Enable verbose mode" },
        { OPT_HELP_STR, 1, '-', "Run HPKE tests\n" },
        { NULL }
    };
    return test_options;
}

int setup_tests(void)
{
    OPTION_CHOICE o;

    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        case OPT_VERBOSE:
            verbose = 1; /* Print progress dots */
            break;
        case OPT_TEST_CASES:
            break;
        default:
            return 0;
        }
    }

    if (!test_get_libctx(&testctx, &nullprov, NULL, &deflprov, "default"))
        return 0;
#ifndef OPENSSL_NO_ECX
    ADD_TEST(export_only_test);
    ADD_TEST(x25519kdfsha256_hkdfsha256_aes128gcm_base_test);
    ADD_TEST(x25519kdfsha256_hkdfsha256_aes128gcm_psk_test);
#endif
    ADD_TEST(P256kdfsha256_hkdfsha256_aes128gcm_base_test);
    ADD_TEST(test_hpke_export);
    ADD_TEST(test_hpke_modes_suites);
    ADD_TEST(test_hpke_suite_strs);
    ADD_TEST(test_hpke_grease);
    ADD_TEST(test_hpke_ikms);
    ADD_TEST(test_hpke_random_suites);
    ADD_TEST(test_hpke_oddcalls);
    ADD_TEST(test_hpke_compressed);
    ADD_TEST(test_hpke_noncereuse);
    return 1;
}

void cleanup_tests(void)
{
    OSSL_PROVIDER_unload(deflprov);
    OSSL_PROVIDER_unload(nullprov);
    OSSL_LIB_CTX_free(testctx);
}
