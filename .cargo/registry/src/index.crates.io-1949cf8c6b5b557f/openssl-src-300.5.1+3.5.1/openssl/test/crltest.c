/*
 * Copyright 2015-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/nelem.h"
#include <string.h>
#include <openssl/bio.h>
#include <openssl/crypto.h>
#include <openssl/err.h>
#include <openssl/pem.h>
#include <openssl/x509.h>

#include "testutil.h"

#define PARAM_TIME 1474934400 /* Sep 27th, 2016 */

static const char *kCRLTestRoot[] = {
    "-----BEGIN CERTIFICATE-----\n",
    "MIIDbzCCAlegAwIBAgIJAODri7v0dDUFMA0GCSqGSIb3DQEBCwUAME4xCzAJBgNV\n",
    "BAYTAlVTMRMwEQYDVQQIDApDYWxpZm9ybmlhMRYwFAYDVQQHDA1Nb3VudGFpbiBW\n",
    "aWV3MRIwEAYDVQQKDAlCb3JpbmdTU0wwHhcNMTYwOTI2MTUwNjI2WhcNMjYwOTI0\n",
    "MTUwNjI2WjBOMQswCQYDVQQGEwJVUzETMBEGA1UECAwKQ2FsaWZvcm5pYTEWMBQG\n",
    "A1UEBwwNTW91bnRhaW4gVmlldzESMBAGA1UECgwJQm9yaW5nU1NMMIIBIjANBgkq\n",
    "hkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAo16WiLWZuaymsD8n5SKPmxV1y6jjgr3B\n",
    "S/dUBpbrzd1aeFzNlI8l2jfAnzUyp+I21RQ+nh/MhqjGElkTtK9xMn1Y+S9GMRh+\n",
    "5R/Du0iCb1tCZIPY07Tgrb0KMNWe0v2QKVVruuYSgxIWodBfxlKO64Z8AJ5IbnWp\n",
    "uRqO6rctN9qUoMlTIAB6dL4G0tDJ/PGFWOJYwOMEIX54bly2wgyYJVBKiRRt4f7n\n",
    "8H922qmvPNA9idmX9G1VAtgV6x97XXi7ULORIQvn9lVQF6nTYDBJhyuPB+mLThbL\n",
    "P2o9orxGx7aCtnnBZUIxUvHNOI0FaSaZH7Fi0xsZ/GkG2HZe7ImPJwIDAQABo1Aw\n",
    "TjAdBgNVHQ4EFgQUWPt3N5cZ/CRvubbrkqfBnAqhq94wHwYDVR0jBBgwFoAUWPt3\n",
    "N5cZ/CRvubbrkqfBnAqhq94wDAYDVR0TBAUwAwEB/zANBgkqhkiG9w0BAQsFAAOC\n",
    "AQEAORu6M0MOwXy+3VEBwNilfTxyqDfruQsc1jA4PT8Oe8zora1WxE1JB4q2FJOz\n",
    "EAuM3H/NXvEnBuN+ITvKZAJUfm4NKX97qmjMJwLKWe1gVv+VQTr63aR7mgWJReQN\n",
    "XdMztlVeZs2dppV6uEg3ia1X0G7LARxGpA9ETbMyCpb39XxlYuTClcbA5ftDN99B\n",
    "3Xg9KNdd++Ew22O3HWRDvdDpTO/JkzQfzi3sYwUtzMEonENhczJhGf7bQMmvL/w5\n",
    "24Wxj4Z7KzzWIHsNqE/RIs6RV3fcW61j/mRgW2XyoWnMVeBzvcJr9NXp4VQYmFPw\n",
    "amd8GKMZQvP0ufGnUn7D7uartA==\n",
    "-----END CERTIFICATE-----\n",
    NULL
};

static const char *kCRLTestLeaf[] = {
    "-----BEGIN CERTIFICATE-----\n",
    "MIIDkDCCAnigAwIBAgICEAAwDQYJKoZIhvcNAQELBQAwTjELMAkGA1UEBhMCVVMx\n",
    "EzARBgNVBAgMCkNhbGlmb3JuaWExFjAUBgNVBAcMDU1vdW50YWluIFZpZXcxEjAQ\n",
    "BgNVBAoMCUJvcmluZ1NTTDAeFw0xNjA5MjYxNTA4MzFaFw0xNzA5MjYxNTA4MzFa\n",
    "MEsxCzAJBgNVBAYTAlVTMRMwEQYDVQQIDApDYWxpZm9ybmlhMRIwEAYDVQQKDAlC\n",
    "b3JpbmdTU0wxEzARBgNVBAMMCmJvcmluZy5zc2wwggEiMA0GCSqGSIb3DQEBAQUA\n",
    "A4IBDwAwggEKAoIBAQDc5v1S1M0W+QWM+raWfO0LH8uvqEwuJQgODqMaGnSlWUx9\n",
    "8iQcnWfjyPja3lWg9K62hSOFDuSyEkysKHDxijz5R93CfLcfnVXjWQDJe7EJTTDP\n",
    "ozEvxN6RjAeYv7CF000euYr3QT5iyBjg76+bon1p0jHZBJeNPP1KqGYgyxp+hzpx\n",
    "e0gZmTlGAXd8JQK4v8kpdYwD6PPifFL/jpmQpqOtQmH/6zcLjY4ojmqpEdBqIKIX\n",
    "+saA29hMq0+NK3K+wgg31RU+cVWxu3tLOIiesETkeDgArjWRS1Vkzbi4v9SJxtNu\n",
    "OZuAxWiynRJw3JwH/OFHYZIvQqz68ZBoj96cepjPAgMBAAGjezB5MAkGA1UdEwQC\n",
    "MAAwLAYJYIZIAYb4QgENBB8WHU9wZW5TU0wgR2VuZXJhdGVkIENlcnRpZmljYXRl\n",
    "MB0GA1UdDgQWBBTGn0OVVh/aoYt0bvEKG+PIERqnDzAfBgNVHSMEGDAWgBRY+3c3\n",
    "lxn8JG+5tuuSp8GcCqGr3jANBgkqhkiG9w0BAQsFAAOCAQEAd2nM8gCQN2Dc8QJw\n",
    "XSZXyuI3DBGGCHcay/3iXu0JvTC3EiQo8J6Djv7WLI0N5KH8mkm40u89fJAB2lLZ\n",
    "ShuHVtcC182bOKnePgwp9CNwQ21p0rDEu/P3X46ZvFgdxx82E9xLa0tBB8PiPDWh\n",
    "lV16jbaKTgX5AZqjnsyjR5o9/mbZVupZJXx5Syq+XA8qiJfstSYJs4KyKK9UOjql\n",
    "ICkJVKpi2ahDBqX4MOH4SLfzVk8pqSpviS6yaA1RXqjpkxiN45WWaXDldVHMSkhC\n",
    "5CNXsXi4b1nAntu89crwSLA3rEwzCWeYj+BX7e1T9rr3oJdwOU/2KQtW1js1yQUG\n",
    "tjJMFw==\n",
    "-----END CERTIFICATE-----\n",
    NULL
};

static const char *kBasicCRL[] = {
    "-----BEGIN X509 CRL-----\n",
    "MIIBpzCBkAIBATANBgkqhkiG9w0BAQsFADBOMQswCQYDVQQGEwJVUzETMBEGA1UE\n",
    "CAwKQ2FsaWZvcm5pYTEWMBQGA1UEBwwNTW91bnRhaW4gVmlldzESMBAGA1UECgwJ\n",
    "Qm9yaW5nU1NMFw0xNjA5MjYxNTEwNTVaFw0xNjEwMjYxNTEwNTVaoA4wDDAKBgNV\n",
    "HRQEAwIBATANBgkqhkiG9w0BAQsFAAOCAQEAnrBKKgvd9x9zwK9rtUvVeFeJ7+LN\n",
    "ZEAc+a5oxpPNEsJx6hXoApYEbzXMxuWBQoCs5iEBycSGudct21L+MVf27M38KrWo\n",
    "eOkq0a2siqViQZO2Fb/SUFR0k9zb8xl86Zf65lgPplALun0bV/HT7MJcl04Tc4os\n",
    "dsAReBs5nqTGNEd5AlC1iKHvQZkM//MD51DspKnDpsDiUVi54h9C1SpfZmX8H2Vv\n",
    "diyu0fZ/bPAM3VAGawatf/SyWfBMyKpoPXEG39oAzmjjOj8en82psn7m474IGaho\n",
    "/vBbhl1ms5qQiLYPjm4YELtnXQoFyC72tBjbdFd/ZE9k4CNKDbxFUXFbkw==\n",
    "-----END X509 CRL-----\n",
    NULL
};

static const char *kRevokedCRL[] = {
    "-----BEGIN X509 CRL-----\n",
    "MIIBvjCBpwIBATANBgkqhkiG9w0BAQsFADBOMQswCQYDVQQGEwJVUzETMBEGA1UE\n",
    "CAwKQ2FsaWZvcm5pYTEWMBQGA1UEBwwNTW91bnRhaW4gVmlldzESMBAGA1UECgwJ\n",
    "Qm9yaW5nU1NMFw0xNjA5MjYxNTEyNDRaFw0xNjEwMjYxNTEyNDRaMBUwEwICEAAX\n",
    "DTE2MDkyNjE1MTIyNlqgDjAMMAoGA1UdFAQDAgECMA0GCSqGSIb3DQEBCwUAA4IB\n",
    "AQCUGaM4DcWzlQKrcZvI8TMeR8BpsvQeo5BoI/XZu2a8h//PyRyMwYeaOM+3zl0d\n",
    "sjgCT8b3C1FPgT+P2Lkowv7rJ+FHJRNQkogr+RuqCSPTq65ha4WKlRGWkMFybzVH\n",
    "NloxC+aU3lgp/NlX9yUtfqYmJek1CDrOOGPrAEAwj1l/BUeYKNGqfBWYJQtPJu+5\n",
    "OaSvIYGpETCZJscUWODmLEb/O3DM438vLvxonwGqXqS0KX37+CHpUlyhnSovxXxp\n",
    "Pz4aF+L7OtczxL0GYtD2fR9B7TDMqsNmHXgQrixvvOY7MUdLGbd4RfJL3yA53hyO\n",
    "xzfKY2TzxLiOmctG0hXFkH5J\n",
    "-----END X509 CRL-----\n",
    NULL
};

static const char *kInvalidCRL[] = {
    "-----BEGIN X509 CRL-----\n",
    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n",
    "-----END X509 CRL-----\n",
    NULL
};

static const char *kBadIssuerCRL[] = {
    "-----BEGIN X509 CRL-----\n",
    "MIIBwjCBqwIBATANBgkqhkiG9w0BAQsFADBSMQswCQYDVQQGEwJVUzETMBEGA1UE\n",
    "CAwKQ2FsaWZvcm5pYTEWMBQGA1UEBwwNTW91bnRhaW4gVmlldzEWMBQGA1UECgwN\n",
    "Tm90IEJvcmluZ1NTTBcNMTYwOTI2MTUxMjQ0WhcNMTYxMDI2MTUxMjQ0WjAVMBMC\n",
    "AhAAFw0xNjA5MjYxNTEyMjZaoA4wDDAKBgNVHRQEAwIBAjANBgkqhkiG9w0BAQsF\n",
    "AAOCAQEAlBmjOA3Fs5UCq3GbyPEzHkfAabL0HqOQaCP12btmvIf/z8kcjMGHmjjP\n",
    "t85dHbI4Ak/G9wtRT4E/j9i5KML+6yfhRyUTUJKIK/kbqgkj06uuYWuFipURlpDB\n",
    "cm81RzZaMQvmlN5YKfzZV/clLX6mJiXpNQg6zjhj6wBAMI9ZfwVHmCjRqnwVmCUL\n",
    "TybvuTmkryGBqREwmSbHFFjg5ixG/ztwzON/Ly78aJ8Bql6ktCl9+/gh6VJcoZ0q\n",
    "L8V8aT8+Ghfi+zrXM8S9BmLQ9n0fQe0wzKrDZh14EK4sb7zmOzFHSxm3eEXyS98g\n",
    "Od4cjsc3ymNk88S4jpnLRtIVxZB+SQ==\n",
    "-----END X509 CRL-----\n",
    NULL
};

/*
 * This is kBasicCRL but with a critical issuing distribution point
 * extension.
 */
static const char *kKnownCriticalCRL[] = {
    "-----BEGIN X509 CRL-----\n",
    "MIIBujCBowIBATANBgkqhkiG9w0BAQsFADBOMQswCQYDVQQGEwJVUzETMBEGA1UE\n",
    "CAwKQ2FsaWZvcm5pYTEWMBQGA1UEBwwNTW91bnRhaW4gVmlldzESMBAGA1UECgwJ\n",
    "Qm9yaW5nU1NMFw0xNjA5MjYxNTEwNTVaFw0xNjEwMjYxNTEwNTVaoCEwHzAKBgNV\n",
    "HRQEAwIBATARBgNVHRwBAf8EBzAFoQMBAf8wDQYJKoZIhvcNAQELBQADggEBAA+3\n",
    "i+5e5Ub8sccfgOBs6WVJFI9c8gvJjrJ8/dYfFIAuCyeocs7DFXn1n13CRZ+URR/Q\n",
    "mVWgU28+xeusuSPYFpd9cyYTcVyNUGNTI3lwgcE/yVjPaOmzSZKdPakApRxtpKKQ\n",
    "NN/56aQz3bnT/ZSHQNciRB8U6jiD9V30t0w+FDTpGaG+7bzzUH3UVF9xf9Ctp60A\n",
    "3mfLe0scas7owSt4AEFuj2SPvcE7yvdOXbu+IEv21cEJUVExJAbhvIweHXh6yRW+\n",
    "7VVeiNzdIjkZjyTmAzoXGha4+wbxXyBRbfH+XWcO/H+8nwyG8Gktdu2QB9S9nnIp\n",
    "o/1TpfOMSGhMyMoyPrk=\n",
    "-----END X509 CRL-----\n",
    NULL
};

/*
 * kUnknownCriticalCRL is kBasicCRL but with an unknown critical extension.
 */
static const char *kUnknownCriticalCRL[] = {
    "-----BEGIN X509 CRL-----\n",
    "MIIBvDCBpQIBATANBgkqhkiG9w0BAQsFADBOMQswCQYDVQQGEwJVUzETMBEGA1UE\n",
    "CAwKQ2FsaWZvcm5pYTEWMBQGA1UEBwwNTW91bnRhaW4gVmlldzESMBAGA1UECgwJ\n",
    "Qm9yaW5nU1NMFw0xNjA5MjYxNTEwNTVaFw0xNjEwMjYxNTEwNTVaoCMwITAKBgNV\n",
    "HRQEAwIBATATBgwqhkiG9xIEAYS3CQABAf8EADANBgkqhkiG9w0BAQsFAAOCAQEA\n",
    "GvBP0xqL509InMj/3493YVRV+ldTpBv5uTD6jewzf5XdaxEQ/VjTNe5zKnxbpAib\n",
    "Kf7cwX0PMSkZjx7k7kKdDlEucwVvDoqC+O9aJcqVmM6GDyNb9xENxd0XCXja6MZC\n",
    "yVgP4AwLauB2vSiEprYJyI1APph3iAEeDm60lTXX/wBM/tupQDDujKh2GPyvBRfJ\n",
    "+wEDwGg3ICwvu4gO4zeC5qnFR+bpL9t5tOMAQnVZ0NWv+k7mkd2LbHdD44dxrfXC\n",
    "nhtfERx99SDmC/jtUAJrGhtCO8acr7exCeYcduN7KKCm91OeCJKK6OzWst0Og1DB\n",
    "kwzzU2rL3G65CrZ7H0SZsQ==\n",
    "-----END X509 CRL-----\n",
    NULL
};

/*
 * kUnknownCriticalCRL2 is kBasicCRL but with a critical issuing distribution
 * point extension followed by an unknown critical extension
 */
static const char *kUnknownCriticalCRL2[] = {
    "-----BEGIN X509 CRL-----\n",
    "MIIBzzCBuAIBATANBgkqhkiG9w0BAQsFADBOMQswCQYDVQQGEwJVUzETMBEGA1UE\n",
    "CAwKQ2FsaWZvcm5pYTEWMBQGA1UEBwwNTW91bnRhaW4gVmlldzESMBAGA1UECgwJ\n",
    "Qm9yaW5nU1NMFw0xNjA5MjYxNTEwNTVaFw0xNjEwMjYxNTEwNTVaoDYwNDAKBgNV\n",
    "HRQEAwIBATARBgNVHRwBAf8EBzAFoQMBAf8wEwYMKoZIhvcSBAGEtwkAAQH/BAAw\n",
    "DQYJKoZIhvcNAQELBQADggEBACTcpQC8jXL12JN5YzOcQ64ubQIe0XxRAd30p7qB\n",
    "BTXGpgqBjrjxRfLms7EBYodEXB2oXMsDq3km0vT1MfYdsDD05S+SQ9CDsq/pUfaC\n",
    "E2WNI5p8WircRnroYvbN2vkjlRbMd1+yNITohXYXCJwjEOAWOx3XIM10bwPYBv4R\n",
    "rDobuLHoMgL3yHgMHmAkP7YpkBucNqeBV8cCdeAZLuhXFWi6yfr3r/X18yWbC/r2\n",
    "2xXdkrSqXLFo7ToyP8YKTgiXpya4x6m53biEYwa2ULlas0igL6DK7wjYZX95Uy7H\n",
    "GKljn9weIYiMPV/BzGymwfv2EW0preLwtyJNJPaxbdin6Jc=\n",
    "-----END X509 CRL-----\n",
    NULL
};

static const char **unknown_critical_crls[] = {
    kUnknownCriticalCRL, kUnknownCriticalCRL2
};

static X509 *test_root = NULL;
static X509 *test_leaf = NULL;

/*
 * Glue an array of strings together.  Return a BIO and put the string
 * into |*out| so we can free it.
 */
static BIO *glue2bio(const char **pem, char **out)
{
    size_t s = 0;

    *out = glue_strings(pem, &s);
    return BIO_new_mem_buf(*out, s);
}

/*
 * Create a CRL from an array of strings.
 */
static X509_CRL *CRL_from_strings(const char **pem)
{
    X509_CRL *crl;
    char *p;
    BIO *b = glue2bio(pem, &p);

    if (b == NULL) {
        OPENSSL_free(p);
        return NULL;
    }

    crl = PEM_read_bio_X509_CRL(b, NULL, NULL, NULL);

    OPENSSL_free(p);
    BIO_free(b);
    return crl;
}

/*
 * Create an X509 from an array of strings.
 */
static X509 *X509_from_strings(const char **pem)
{
    X509 *x;
    char *p;
    BIO *b = glue2bio(pem, &p);

    if (b == NULL) {
        OPENSSL_free(p);
        return NULL;
    }

    x = PEM_read_bio_X509(b, NULL, NULL, NULL);

    OPENSSL_free(p);
    BIO_free(b);
    return x;
}

/*
 * Verify |leaf| certificate (chained up to |root|).  |crls| if
 * not NULL, is a list of CRLs to include in the verification. It is
 * also free'd before returning, which is kinda yucky but convenient.
 * Returns a value from X509_V_ERR_xxx or X509_V_OK.
 */
static int verify(X509 *leaf, X509 *root, STACK_OF(X509_CRL) *crls,
                  unsigned long flags)
{
    X509_STORE_CTX *ctx = X509_STORE_CTX_new();
    X509_STORE *store = X509_STORE_new();
    X509_VERIFY_PARAM *param = X509_VERIFY_PARAM_new();
    STACK_OF(X509) *roots = sk_X509_new_null();
    int status = X509_V_ERR_UNSPECIFIED;

    if (!TEST_ptr(ctx)
        || !TEST_ptr(store)
        || !TEST_ptr(param)
        || !TEST_ptr(roots))
        goto err;

    /* Create a stack; upref the cert because we free it below. */
    if (!TEST_true(X509_up_ref(root)))
        goto err;
    if (!TEST_true(sk_X509_push(roots, root))) {
        X509_free(root);
        goto err;
    }
    if (!TEST_true(X509_STORE_CTX_init(ctx, store, leaf, NULL)))
        goto err;
    X509_STORE_CTX_set0_trusted_stack(ctx, roots);
    X509_STORE_CTX_set0_crls(ctx, crls);
    X509_VERIFY_PARAM_set_time(param, PARAM_TIME);
    if (!TEST_long_eq((long)X509_VERIFY_PARAM_get_time(param), PARAM_TIME))
        goto err;
    X509_VERIFY_PARAM_set_depth(param, 16);
    if (flags)
        X509_VERIFY_PARAM_set_flags(param, flags);
    X509_STORE_CTX_set0_param(ctx, param);
    param = NULL;

    ERR_clear_error();
    status = X509_verify_cert(ctx) == 1 ? X509_V_OK
                                        : X509_STORE_CTX_get_error(ctx);
err:
    OSSL_STACK_OF_X509_free(roots);
    sk_X509_CRL_pop_free(crls, X509_CRL_free);
    X509_VERIFY_PARAM_free(param);
    X509_STORE_CTX_free(ctx);
    X509_STORE_free(store);
    return status;
}

/*
 * Create a stack of CRL's.  Upref each one because we call pop_free on
 * the stack and need to keep the CRL's around until the test exits.
 * Yes this crashes on malloc failure; it forces us to debug.
 */
static STACK_OF(X509_CRL) *make_CRL_stack(X509_CRL *x1, X509_CRL *x2)
{
    STACK_OF(X509_CRL) *sk = sk_X509_CRL_new_null();

    if (x1 != NULL) {
        if (!X509_CRL_up_ref(x1))
            goto err;
        if (!sk_X509_CRL_push(sk, x1)) {
            X509_CRL_free(x1);
            goto err;
        }
    }

    if (x2 != NULL) {
        if (!X509_CRL_up_ref(x2))
            goto err;
        if (!sk_X509_CRL_push(sk, x2)) {
            X509_CRL_free(x2);
            goto err;
        }
    }

    return sk;

err:
    sk_X509_CRL_pop_free(sk, X509_CRL_free);
    return NULL;
}

static int test_basic_crl(void)
{
    X509_CRL *basic_crl = CRL_from_strings(kBasicCRL);
    X509_CRL *revoked_crl = CRL_from_strings(kRevokedCRL);
    int r;

    r = TEST_ptr(basic_crl)
        && TEST_ptr(revoked_crl)
        && TEST_int_eq(verify(test_leaf, test_root,
                              make_CRL_stack(basic_crl, NULL),
                              X509_V_FLAG_CRL_CHECK), X509_V_OK)
        && TEST_int_eq(verify(test_leaf, test_root,
                              make_CRL_stack(basic_crl, revoked_crl),
                              X509_V_FLAG_CRL_CHECK), X509_V_ERR_CERT_REVOKED);
    X509_CRL_free(basic_crl);
    X509_CRL_free(revoked_crl);
    return r;
}

static int test_no_crl(void)
{
    return TEST_int_eq(verify(test_leaf, test_root, NULL,
                              X509_V_FLAG_CRL_CHECK),
                       X509_V_ERR_UNABLE_TO_GET_CRL);
}

static int test_bad_issuer_crl(void)
{
    X509_CRL *bad_issuer_crl = CRL_from_strings(kBadIssuerCRL);
    int r;

    r = TEST_ptr(bad_issuer_crl)
        && TEST_int_eq(verify(test_leaf, test_root,
                              make_CRL_stack(bad_issuer_crl, NULL),
                              X509_V_FLAG_CRL_CHECK),
                       X509_V_ERR_UNABLE_TO_GET_CRL);
    X509_CRL_free(bad_issuer_crl);
    return r;
}

static int test_known_critical_crl(void)
{
    X509_CRL *known_critical_crl = CRL_from_strings(kKnownCriticalCRL);
    int r;

    r = TEST_ptr(known_critical_crl)
        && TEST_int_eq(verify(test_leaf, test_root,
                              make_CRL_stack(known_critical_crl, NULL),
                              X509_V_FLAG_CRL_CHECK), X509_V_OK);
    X509_CRL_free(known_critical_crl);
    return r;
}

static int test_unknown_critical_crl(int n)
{
    X509_CRL *unknown_critical_crl = CRL_from_strings(unknown_critical_crls[n]);
    int r;

    r = TEST_ptr(unknown_critical_crl)
        && TEST_int_eq(verify(test_leaf, test_root,
                              make_CRL_stack(unknown_critical_crl, NULL),
                              X509_V_FLAG_CRL_CHECK),
                       X509_V_ERR_UNHANDLED_CRITICAL_CRL_EXTENSION);
    X509_CRL_free(unknown_critical_crl);
    return r;
}

static int test_reuse_crl(int idx)
{
    X509_CRL *result, *reused_crl = CRL_from_strings(kBasicCRL);
    X509_CRL *addref_crl = NULL;
    char *p = NULL;
    BIO *b = NULL;
    int r = 0;

    if (!TEST_ptr(reused_crl))
        goto err;

    if (idx & 1) {
        if (!TEST_true(X509_CRL_up_ref(reused_crl)))
            goto err;
	addref_crl = reused_crl;
    }

    idx >>= 1;
    b = glue2bio(idx == 2 ? kRevokedCRL : kInvalidCRL + idx, &p);

    if (!TEST_ptr(b))
        goto err;

    result = PEM_read_bio_X509_CRL(b, &reused_crl, NULL, NULL);

    switch (idx) {
    case 0: /* valid PEM + invalid DER */
        if (!TEST_ptr_null(result)
                || !TEST_ptr_null(reused_crl))
            goto err;
        break;
    case 1: /* invalid PEM */
        if (!TEST_ptr_null(result)
                || !TEST_ptr(reused_crl))
            goto err;
        break;
    case 2:
        if (!TEST_ptr(result)
                || !TEST_ptr(reused_crl)
                || !TEST_ptr_eq(result, reused_crl))
            goto err;
        break;
    }

    r = 1;

 err:
    OPENSSL_free(p);
    BIO_free(b);
    X509_CRL_free(reused_crl);
    X509_CRL_free(addref_crl);
    return r;
}

int setup_tests(void)
{
    if (!TEST_ptr(test_root = X509_from_strings(kCRLTestRoot))
        || !TEST_ptr(test_leaf = X509_from_strings(kCRLTestLeaf)))
        return 0;

    ADD_TEST(test_no_crl);
    ADD_TEST(test_basic_crl);
    ADD_TEST(test_bad_issuer_crl);
    ADD_TEST(test_known_critical_crl);
    ADD_ALL_TESTS(test_unknown_critical_crl, OSSL_NELEM(unknown_critical_crls));
    ADD_ALL_TESTS(test_reuse_crl, 6);
    return 1;
}

void cleanup_tests(void)
{
    X509_free(test_root);
    X509_free(test_leaf);
}
