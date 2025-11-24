/*
 * Copyright 2011-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * SRP is deprecated, so we're going to have to use some deprecated APIs in
 * order to test it.
 */
#define OPENSSL_SUPPRESS_DEPRECATED

#include <openssl/opensslconf.h>
# include "testutil.h"

#ifdef OPENSSL_NO_SRP
# include <stdio.h>
#else

# include <openssl/srp.h>
# include <openssl/rand.h>
# include <openssl/err.h>

# define RANDOM_SIZE 32         /* use 256 bits on each side */

static int run_srp(const char *username, const char *client_pass,
                   const char *server_pass)
{
    int ret = 0;
    BIGNUM *s = NULL;
    BIGNUM *v = NULL;
    BIGNUM *a = NULL;
    BIGNUM *b = NULL;
    BIGNUM *u = NULL;
    BIGNUM *x = NULL;
    BIGNUM *Apub = NULL;
    BIGNUM *Bpub = NULL;
    BIGNUM *Kclient = NULL;
    BIGNUM *Kserver = NULL;
    unsigned char rand_tmp[RANDOM_SIZE];
    /* use builtin 1024-bit params */
    const SRP_gN *GN;

    if (!TEST_ptr(GN = SRP_get_default_gN("1024")))
        return 0;

    /* Set up server's password entry */
    if (!TEST_true(SRP_create_verifier_BN(username, server_pass,
                                          &s, &v, GN->N, GN->g)))
        goto end;

    test_output_bignum("N", GN->N);
    test_output_bignum("g", GN->g);
    test_output_bignum("Salt", s);
    test_output_bignum("Verifier", v);

    /* Server random */
    RAND_bytes(rand_tmp, sizeof(rand_tmp));
    b = BN_bin2bn(rand_tmp, sizeof(rand_tmp), NULL);
    if (!TEST_BN_ne_zero(b))
        goto end;
    test_output_bignum("b", b);

    /* Server's first message */
    Bpub = SRP_Calc_B(b, GN->N, GN->g, v);
    test_output_bignum("B", Bpub);

    if (!TEST_true(SRP_Verify_B_mod_N(Bpub, GN->N)))
        goto end;

    /* Client random */
    RAND_bytes(rand_tmp, sizeof(rand_tmp));
    a = BN_bin2bn(rand_tmp, sizeof(rand_tmp), NULL);
    if (!TEST_BN_ne_zero(a))
        goto end;
    test_output_bignum("a", a);

    /* Client's response */
    Apub = SRP_Calc_A(a, GN->N, GN->g);
    test_output_bignum("A", Apub);

    if (!TEST_true(SRP_Verify_A_mod_N(Apub, GN->N)))
        goto end;

    /* Both sides calculate u */
    u = SRP_Calc_u(Apub, Bpub, GN->N);

    /* Client's key */
    x = SRP_Calc_x(s, username, client_pass);
    Kclient = SRP_Calc_client_key(GN->N, Bpub, GN->g, x, a, u);
    test_output_bignum("Client's key", Kclient);

    /* Server's key */
    Kserver = SRP_Calc_server_key(Apub, v, u, b, GN->N);
    test_output_bignum("Server's key", Kserver);

    if (!TEST_BN_eq(Kclient, Kserver))
        goto end;

    ret = 1;

end:
    BN_clear_free(Kclient);
    BN_clear_free(Kserver);
    BN_clear_free(x);
    BN_free(u);
    BN_free(Apub);
    BN_clear_free(a);
    BN_free(Bpub);
    BN_clear_free(b);
    BN_free(s);
    BN_clear_free(v);

    return ret;
}

static int check_bn(const char *name, const BIGNUM *bn, const char *hexbn)
{
    BIGNUM *tmp = NULL;
    int r;

    if (!TEST_true(BN_hex2bn(&tmp, hexbn)))
        return 0;

    if (BN_cmp(bn, tmp) != 0)
        TEST_error("unexpected %s value", name);
    r = TEST_BN_eq(bn, tmp);
    BN_free(tmp);
    return r;
}

/* SRP test vectors from RFC5054 */
static int run_srp_kat(void)
{
    int ret = 0;
    BIGNUM *s = NULL;
    BIGNUM *v = NULL;
    BIGNUM *a = NULL;
    BIGNUM *b = NULL;
    BIGNUM *u = NULL;
    BIGNUM *x = NULL;
    BIGNUM *Apub = NULL;
    BIGNUM *Bpub = NULL;
    BIGNUM *Kclient = NULL;
    BIGNUM *Kserver = NULL;
    /* use builtin 1024-bit params */
    const SRP_gN *GN;

    if (!TEST_ptr(GN = SRP_get_default_gN("1024")))
        goto err;
    BN_hex2bn(&s, "BEB25379D1A8581EB5A727673A2441EE");
    /* Set up server's password entry */
    if (!TEST_true(SRP_create_verifier_BN("alice", "password123", &s, &v, GN->N,
                                          GN->g)))
        goto err;

    TEST_info("checking v");
    if (!TEST_true(check_bn("v", v,
                 "7E273DE8696FFC4F4E337D05B4B375BEB0DDE1569E8FA00A9886D812"
                 "9BADA1F1822223CA1A605B530E379BA4729FDC59F105B4787E5186F5"
                 "C671085A1447B52A48CF1970B4FB6F8400BBF4CEBFBB168152E08AB5"
                 "EA53D15C1AFF87B2B9DA6E04E058AD51CC72BFC9033B564E26480D78"
                 "E955A5E29E7AB245DB2BE315E2099AFB")))
        goto err;
    TEST_note("    okay");

    /* Server random */
    BN_hex2bn(&b, "E487CB59D31AC550471E81F00F6928E01DDA08E974A004F49E61F5D1"
                  "05284D20");

    /* Server's first message */
    Bpub = SRP_Calc_B(b, GN->N, GN->g, v);
    if (!TEST_true(SRP_Verify_B_mod_N(Bpub, GN->N)))
        goto err;

    TEST_info("checking B");
    if (!TEST_true(check_bn("B", Bpub,
                  "BD0C61512C692C0CB6D041FA01BB152D4916A1E77AF46AE105393011"
                  "BAF38964DC46A0670DD125B95A981652236F99D9B681CBF87837EC99"
                  "6C6DA04453728610D0C6DDB58B318885D7D82C7F8DEB75CE7BD4FBAA"
                  "37089E6F9C6059F388838E7A00030B331EB76840910440B1B27AAEAE"
                  "EB4012B7D7665238A8E3FB004B117B58")))
        goto err;
    TEST_note("    okay");

    /* Client random */
    BN_hex2bn(&a, "60975527035CF2AD1989806F0407210BC81EDC04E2762A56AFD529DD"
                  "DA2D4393");

    /* Client's response */
    Apub = SRP_Calc_A(a, GN->N, GN->g);
    if (!TEST_true(SRP_Verify_A_mod_N(Apub, GN->N)))
        goto err;

    TEST_info("checking A");
    if (!TEST_true(check_bn("A", Apub,
                  "61D5E490F6F1B79547B0704C436F523DD0E560F0C64115BB72557EC4"
                  "4352E8903211C04692272D8B2D1A5358A2CF1B6E0BFCF99F921530EC"
                  "8E39356179EAE45E42BA92AEACED825171E1E8B9AF6D9C03E1327F44"
                  "BE087EF06530E69F66615261EEF54073CA11CF5858F0EDFDFE15EFEA"
                  "B349EF5D76988A3672FAC47B0769447B")))
        goto err;
    TEST_note("    okay");

    /* Both sides calculate u */
    u = SRP_Calc_u(Apub, Bpub, GN->N);

    if (!TEST_true(check_bn("u", u,
                    "CE38B9593487DA98554ED47D70A7AE5F462EF019")))
        goto err;

    /* Client's key */
    x = SRP_Calc_x(s, "alice", "password123");
    Kclient = SRP_Calc_client_key(GN->N, Bpub, GN->g, x, a, u);
    TEST_info("checking client's key");
    if (!TEST_true(check_bn("Client's key", Kclient,
                  "B0DC82BABCF30674AE450C0287745E7990A3381F63B387AAF271A10D"
                  "233861E359B48220F7C4693C9AE12B0A6F67809F0876E2D013800D6C"
                  "41BB59B6D5979B5C00A172B4A2A5903A0BDCAF8A709585EB2AFAFA8F"
                  "3499B200210DCC1F10EB33943CD67FC88A2F39A4BE5BEC4EC0A3212D"
                  "C346D7E474B29EDE8A469FFECA686E5A")))
        goto err;
    TEST_note("    okay");

    /* Server's key */
    Kserver = SRP_Calc_server_key(Apub, v, u, b, GN->N);
    TEST_info("checking server's key");
    if (!TEST_true(check_bn("Server's key", Kserver,
                  "B0DC82BABCF30674AE450C0287745E7990A3381F63B387AAF271A10D"
                  "233861E359B48220F7C4693C9AE12B0A6F67809F0876E2D013800D6C"
                  "41BB59B6D5979B5C00A172B4A2A5903A0BDCAF8A709585EB2AFAFA8F"
                  "3499B200210DCC1F10EB33943CD67FC88A2F39A4BE5BEC4EC0A3212D"
                  "C346D7E474B29EDE8A469FFECA686E5A")))
        goto err;
    TEST_note("    okay");

    ret = 1;

err:
    BN_clear_free(Kclient);
    BN_clear_free(Kserver);
    BN_clear_free(x);
    BN_free(u);
    BN_free(Apub);
    BN_clear_free(a);
    BN_free(Bpub);
    BN_clear_free(b);
    BN_free(s);
    BN_clear_free(v);

    return ret;
}

static int run_srp_tests(void)
{
    /* "Negative" test, expect a mismatch */
    TEST_info("run_srp: expecting a mismatch");
    if (!TEST_false(run_srp("alice", "password1", "password2")))
        return 0;

    /* "Positive" test, should pass */
    TEST_info("run_srp: expecting a match");
    if (!TEST_true(run_srp("alice", "password", "password")))
        return 0;

    return 1;
}
#endif

int setup_tests(void)
{
#ifdef OPENSSL_NO_SRP
    printf("No SRP support\n");
#else
    ADD_TEST(run_srp_tests);
    ADD_TEST(run_srp_kat);
#endif
    return 1;
}
