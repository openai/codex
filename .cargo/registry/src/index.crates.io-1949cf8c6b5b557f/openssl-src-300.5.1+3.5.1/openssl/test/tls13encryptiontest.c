/*
 * Copyright 2016-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/ssl.h>
#include <openssl/evp.h>
#include "../ssl/ssl_local.h"
#include "../ssl/record/record_local.h"
#include "internal/recordmethod.h"
#include "../ssl/record/methods/recmethod_local.h"
#include "internal/nelem.h"
#include "testutil.h"

/*
 * Based on the test vectors provided in:
 * https://tools.ietf.org/html/draft-ietf-tls-tls13-vectors-06
 */

typedef struct {
    /*
     * We split these into 3 chunks in order to work around the 509 character
     * limit that the standard specifies for string literals
     */
    const char *plaintext[3];
    const char *ciphertext[3];
    const char *key;
    const char *iv;
    const char *seq;
} RECORD_DATA;

/*
 * Note 1: The plaintext values given here have an additional "16" or "17" byte
 * added to the end when compared to the official vectors. The official vectors
 * do not include the inner content type, but we require it.
 *
 * Note 2: These are the vectors for the "Simple 1-RTT Handshake"
 */
static RECORD_DATA refdata[] = {
    {
        /*
         * Server: EncryptedExtensions, Certificate, CertificateVerify and
         *         Finished
         */
        {
            "080000240022000a00140012001d00170018001901000101010201030104001c"
            "00024001000000000b0001b9000001b50001b0308201ac30820115a003020102"
            "020102300d06092a864886f70d01010b0500300e310c300a0603550403130372"
            "7361301e170d3136303733303031323335395a170d3236303733303031323335"
            "395a300e310c300a0603550403130372736130819f300d06092a864886f70d01"
            "0101050003818d0030818902818100b4bb498f8279303d980836399b36c6988c"
            "0c68de55e1bdb826d3901a2461eafd2de49a91d015abbc9a95137ace6c1af19e",
            "aa6af98c7ced43120998e187a80ee0ccb0524b1b018c3e0b63264d449a6d38e2"
            "2a5fda430846748030530ef0461c8ca9d9efbfae8ea6d1d03e2bd193eff0ab9a"
            "8002c47428a6d35a8d88d79f7f1e3f0203010001a31a301830090603551d1304"
            "023000300b0603551d0f0404030205a0300d06092a864886f70d01010b050003"
            "81810085aad2a0e5b9276b908c65f73a7267170618a54c5f8a7b337d2df7a594"
            "365417f2eae8f8a58c8f8172f9319cf36b7fd6c55b80f21a03015156726096fd"
            "335e5e67f2dbf102702e608ccae6bec1fc63a42a99be5c3eb7107c3c54e9b9eb",
            "2bd5203b1c3b84e0a8b2f759409ba3eac9d91d402dcc0cc8f8961229ac9187b4"
            "2b4de100000f00008408040080754040d0ddab8cf0e2da2bc4995b868ad745c8"
            "e1564e33cde17880a42392cc624aeef6b67bb3f0ae71d9d54a2309731d87dc59"
            "f642d733be2eb27484ad8a8c8eb3516a7ac57f2625e2b5c0888a8541f4e734f7"
            "3d054761df1dd02f0e3e9a33cfa10b6e3eb4ebf7ac053b01fdabbddfc54133bc"
            "d24c8bbdceb223b2aa03452a2914000020ac86acbc9cd25a45b57ad5b64db15d"
            "4405cf8c80e314583ebf3283ef9a99310c16"
        },
        {
            "f10b26d8fcaf67b5b828f712122216a1cd14187465b77637cbcd78539128bb93"
            "246dcca1af56f1eaa271666077455bc54965d85f05f9bd36d6996171eb536aff"
            "613eeddc42bad5a2d2227c4606f1215f980e7afaf56bd3b85a51be130003101a"
            "758d077b1c891d8e7a22947e5a229851fd42a9dd422608f868272abf92b3d43f"
            "b46ac420259346067f66322fd708885680f4b4433c29116f2dfa529e09bba53c"
            "7cd920121724809eaddcc84307ef46fc51a0b33d99d39db337fcd761ce0f2b02"
            "dc73dedb6fddb77c4f8099bde93d5bee08bcf2131f29a2a37ff07949e8f8bcdd",
            "3e8310b8bf8b3444c85aaf0d2aeb2d4f36fd14d5cb51fcebff418b3827136ab9"
            "529e9a3d3f35e4c0ae749ea2dbc94982a1281d3e6daab719aa4460889321a008"
            "bf10fa06ac0c61cc122cc90d5e22c0030c986ae84a33a0c47df174bcfbd50bf7"
            "8ffdf24051ab423db63d5815db2f830040f30521131c98c66f16c362addce2fb"
            "a0602cf0a7dddf22e8def7516cdfee95b4056cc9ad38c95352335421b5b1ffba"
            "df75e5212fdad7a75f52a2801486a1eec3539580bee0e4b337cda6085ac9eccd"
            "1a0f1a46cebfbb5cdfa3251ac28c3bc826148c6d8c1eb6a06f77f6ff632c6a83",
            "e283e8f9df7c6dbabf1c6ea40629a85b43ab0c73d34f9d5072832a104eda3f75"
            "f5d83da6e14822a18e14099d749eafd823ca2ac7542086501eca206ce7887920"
            "008573757ce2f230a890782b99cc682377beee812756d04f9025135fb599d746"
            "fefe7316c922ac265ca0d29021375adb63c1509c3e242dfb92b8dee891f7368c"
            "4058399b8db9075f2dcc8216194e503b6652d87d2cb41f99adfdcc5be5ec7e1e"
            "6326ac22d70bd3ba652827532d669aff005173597f8039c3ea4922d3ec757670"
            "222f6ac29b93e90d7ad3f6dd96328e429cfcfd5cca22707fe2d86ad1dcb0be75"
            "6e8e"
        },
        "c66cb1aec519df44c91e10995511ac8b",
        "f7f6884c4981716c2d0d29a4",
        "0000000000000000"
    },
    {
        /* Client: Finished */
        {
            "14000020b9027a0204b972b52cdefa58950fa1580d68c9cb124dbe691a7178f2"
            "5c554b2316", "", ""
        },
        {
            "9539b4ae2f87fd8e616b295628ea953d9e3858db274970d19813ec136cae7d96"
            "e0417775fcabd3d8858fdc60240912d218f5afb21c", "", ""
        },
        "2679a43e1d76784034ea1797d5ad2649",
        "5482405290dd0d2f81c0d942",
        "0000000000000000"
    },
    {
        /* Server: NewSessionTicket */
        {
            "040000c90000001e2fd3992f02000000b2ff099f9676cdff8b0bf8825d000000"
            "007905a9d28efeef4a47c6f9b06a0cecdb0070d920b898997c75b79636943ed4"
            "2046a96142bd084a04acfa0c490f452d756dea02c0f927259f1f3231ac0d541a"
            "769129b740ce38090842b828c27fd729f59737ba98aa7b42e043c5da28f8dca8"
            "590b2df410d5134fd6c4cacad8b30370602afa35d265bf4d127976bb36dbda6a"
            "626f0270e20eebc73d6fcae2b1a0da122ee9042f76be56ebf41aa469c3d2c9da"
            "9197d80008002a00040000040016", "", ""
        },
        {
            "3680c2b2109d25caa26c3b06eea9fdc5cb31613ba702176596da2e886bf6af93"
            "507bd68161ad9cb4780653842e1041ecbf0088a65ac4ef438419dd1d95ddd9bd"
            "2ad4484e7e167d0e6c008448ae58a0418713b6fc6c51e4bb23a537fb75a74f73"
            "de31fe6aa0bc522515f8b25f8955428b5de5ac06762cec22b0aa78c94385ef8e"
            "70fa24945b7c1f268510871689bbbbfaf2e7f4a19277024f95f1143ab12a31ec"
            "63adb128cb390711fd6d06a498df3e98615d8eb102e23353b480efcca5e8e026"
            "7a6d0fe2441f14c8c9664aefb2cfff6ae9e0442728b6a0940c1e824fda06",
            "", ""

        },
        "a688ebb5ac826d6f42d45c0cc44b9b7d",
        "c1cad4425a438b5de714830a",
        "0000000000000000"
    },
    {
        /* Client: Application Data */
        {
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
            "202122232425262728292a2b2c2d2e2f303117", "", ""
        },
        {
            "8c3497da00ae023e53c01b4324b665404c1b49e78fe2bf4d17f6348ae8340551"
            "e363a0cd05f2179c4fef5ad689b5cae0bae94adc63632e571fb79aa91544c639"
            "4d28a1", "", ""

        },
        "88b96ad686c84be55ace18a59cce5c87",
        "b99dc58cd5ff5ab082fdad19",
        "0000000000000000"
    },


    {
        /* Server: Application Data */
        {
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
            "202122232425262728292a2b2c2d2e2f303117", "", ""
        },
        {
            "f65f49fd2df6cd2347c3d30166e3cfddb6308a5906c076112c6a37ff1dbd406b"
            "5813c0abd734883017a6b2833186b13c14da5d75f33d8760789994e27d82043a"
            "b88d65", "", ""
        },
        "a688ebb5ac826d6f42d45c0cc44b9b7d",
        "c1cad4425a438b5de714830a",
        "0000000000000001"
    },
    {
        /* Client: CloseNotify */
        {
            "010015", "", ""
        },
        {
            "2c2148163d7938a35f6acf2a6606f8cbd1d9f2", "", ""
        },
        "88b96ad686c84be55ace18a59cce5c87",
        "b99dc58cd5ff5ab082fdad19",
        "0000000000000001"
    },
    {
        /* Server: CloseNotify */
        {
            "010015", "", ""
        },
        {
            "f8141ebdb5eda511e0bce639a56ff9ea825a21", "", ""

        },
        "a688ebb5ac826d6f42d45c0cc44b9b7d",
        "c1cad4425a438b5de714830a",
        "0000000000000002"
    }
};

/*
 * Same thing as OPENSSL_hexstr2buf() but enables us to pass the string in
 * 3 chunks
 */
static unsigned char *multihexstr2buf(const char *str[3], size_t *len)
{
    size_t outer, inner, curr = 0;
    unsigned char *outbuf;
    size_t totlen = 0;

    /* Check lengths of all input strings are even */
    for (outer = 0; outer < 3; outer++) {
        totlen += strlen(str[outer]);
        if ((totlen & 1) != 0)
            return NULL;
    }

    totlen /= 2;
    outbuf = OPENSSL_malloc(totlen);
    if (outbuf == NULL)
        return NULL;

    for (outer = 0; outer < 3; outer++) {
        for (inner = 0; str[outer][inner] != 0; inner += 2) {
            int hi, lo;

            hi = OPENSSL_hexchar2int(str[outer][inner]);
            lo = OPENSSL_hexchar2int(str[outer][inner + 1]);

            if (hi < 0 || lo < 0) {
                OPENSSL_free(outbuf);
                return NULL;
            }
            outbuf[curr++] = (hi << 4) | lo;
        }
    }

    *len = totlen;
    return outbuf;
}

static int load_record(TLS_RL_RECORD *rec, RECORD_DATA *recd,
                       unsigned char **key, unsigned char *iv, size_t ivlen,
                       unsigned char *seq)
{
    unsigned char *pt = NULL, *sq = NULL, *ivtmp = NULL;
    size_t ptlen;

    *key = OPENSSL_hexstr2buf(recd->key, NULL);
    ivtmp = OPENSSL_hexstr2buf(recd->iv, NULL);
    sq = OPENSSL_hexstr2buf(recd->seq, NULL);
    pt = multihexstr2buf(recd->plaintext, &ptlen);

    if (*key == NULL || ivtmp == NULL || sq == NULL || pt == NULL)
        goto err;

    rec->data = rec->input = OPENSSL_malloc(ptlen + EVP_GCM_TLS_TAG_LEN);

    if (rec->data == NULL)
        goto err;

    rec->length = ptlen;
    memcpy(rec->data, pt, ptlen);
    OPENSSL_free(pt);
    memcpy(seq, sq, SEQ_NUM_SIZE);
    OPENSSL_free(sq);
    memcpy(iv, ivtmp, ivlen);
    OPENSSL_free(ivtmp);

    return 1;
 err:
    OPENSSL_free(*key);
    *key = NULL;
    OPENSSL_free(ivtmp);
    OPENSSL_free(sq);
    OPENSSL_free(pt);
    return 0;
}

static int test_record(TLS_RL_RECORD *rec, RECORD_DATA *recd, int enc)
{
    int ret = 0;
    unsigned char *refd;
    size_t refdatalen = 0;

    if (enc)
        refd = multihexstr2buf(recd->ciphertext, &refdatalen);
    else
        refd = multihexstr2buf(recd->plaintext, &refdatalen);

    if (!TEST_ptr(refd)) {
        TEST_info("Failed to get reference data");
        goto err;
    }

    if (!TEST_mem_eq(rec->data, rec->length, refd, refdatalen))
        goto err;

    ret = 1;

 err:
    OPENSSL_free(refd);
    return ret;
}

#define TLS13_AES_128_GCM_SHA256_BYTES  ((const unsigned char *)"\x13\x01")

static int test_tls13_encryption(void)
{
    TLS_RL_RECORD rec;
    unsigned char *key = NULL;
    const EVP_CIPHER *ciph = EVP_aes_128_gcm();
    int ret = 0;
    size_t ivlen, ctr;
    unsigned char seqbuf[SEQ_NUM_SIZE];
    unsigned char iv[EVP_MAX_IV_LENGTH];
    OSSL_RECORD_LAYER *rrl = NULL, *wrl = NULL;

    /*
     * Encrypted TLSv1.3 records always have an outer content type of
     * application data, and a record version of TLSv1.2.
     */
    rec.data = NULL;
    rec.type = SSL3_RT_APPLICATION_DATA;
    rec.rec_version = TLS1_2_VERSION;

    for (ctr = 0; ctr < OSSL_NELEM(refdata); ctr++) {
        /* Load the record */
        ivlen = EVP_CIPHER_get_iv_length(ciph);
        if (!load_record(&rec, &refdata[ctr], &key, iv, ivlen, seqbuf)) {
            TEST_error("Failed loading key into EVP_CIPHER_CTX");
            goto err;
        }

        /* Set up the write record layer */
        if (!TEST_true(ossl_tls_record_method.new_record_layer(
                          NULL, NULL, TLS1_3_VERSION, OSSL_RECORD_ROLE_SERVER,
                          OSSL_RECORD_DIRECTION_WRITE,
                          OSSL_RECORD_PROTECTION_LEVEL_APPLICATION, 0, NULL, 0,
                          key, 16, iv, ivlen, NULL, 0, EVP_aes_128_gcm(),
                          EVP_GCM_TLS_TAG_LEN, 0, NULL, NULL, NULL, NULL, NULL,
                          NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
                          &wrl)))
            goto err;
        memcpy(wrl->sequence, seqbuf, sizeof(seqbuf));

        /* Encrypt it */
        if (!TEST_size_t_eq(wrl->funcs->cipher(wrl, &rec, 1, 1, NULL, 0), 1)) {
            TEST_info("Failed to encrypt record %zu", ctr);
            goto err;
        }

        if (!TEST_true(test_record(&rec, &refdata[ctr], 1))) {
            TEST_info("Record %zu encryption test failed", ctr);
            goto err;
        }

        /* Set up the read record layer */
        if (!TEST_true(ossl_tls_record_method.new_record_layer(
                          NULL, NULL, TLS1_3_VERSION, OSSL_RECORD_ROLE_SERVER,
                          OSSL_RECORD_DIRECTION_READ,
                          OSSL_RECORD_PROTECTION_LEVEL_APPLICATION, 0, NULL, 0,
                          key, 16, iv, ivlen, NULL, 0, EVP_aes_128_gcm(),
                          EVP_GCM_TLS_TAG_LEN, 0, NULL, NULL, NULL, NULL, NULL,
                          NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
                          &rrl)))
            goto err;
        memcpy(rrl->sequence, seqbuf, sizeof(seqbuf));

        /* Decrypt it */
        if (!TEST_int_eq(rrl->funcs->cipher(rrl, &rec, 1, 0, NULL, 0), 1)) {
            TEST_info("Failed to decrypt record %zu", ctr);
            goto err;
        }

        if (!TEST_true(test_record(&rec, &refdata[ctr], 0))) {
            TEST_info("Record %zu decryption test failed", ctr);
            goto err;
        }

        ossl_tls_record_method.free(rrl);
        ossl_tls_record_method.free(wrl);
        rrl = wrl = NULL;
        OPENSSL_free(rec.data);
        OPENSSL_free(key);
        rec.data = NULL;
        key = NULL;
    }

    TEST_note("PASS: %zu records tested", ctr);
    ret = 1;

 err:
    ossl_tls_record_method.free(rrl);
    ossl_tls_record_method.free(wrl);
    OPENSSL_free(rec.data);
    OPENSSL_free(key);
    return ret;
}

int setup_tests(void)
{
    ADD_TEST(test_tls13_encryption);
    return 1;
}
