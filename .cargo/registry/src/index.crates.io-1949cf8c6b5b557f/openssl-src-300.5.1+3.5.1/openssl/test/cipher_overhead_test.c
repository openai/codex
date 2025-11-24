/*
 * Copyright 2016-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/nelem.h"
#include "testutil.h"
#include "../ssl/ssl_local.h"

static int cipher_enabled(const SSL_CIPHER *ciph)
{
    /*
     * ssl_cipher_get_overhead() actually works with AEAD ciphers even if the
     * underlying implementation is not present.
     */
    if ((ciph->algorithm_mac & SSL_AEAD) != 0)
        return 1;

    if (ciph->algorithm_enc != SSL_eNULL
            && EVP_get_cipherbynid(SSL_CIPHER_get_cipher_nid(ciph)) == NULL)
        return 0;

    if (EVP_get_digestbynid(SSL_CIPHER_get_digest_nid(ciph)) == NULL)
        return 0;

    return 1;
}

static int cipher_overhead(void)
{
    int ret = 1, i, n = ssl3_num_ciphers();
    const SSL_CIPHER *ciph;
    size_t mac, in, blk, ex;

    for (i = 0; i < n; i++) {
        ciph = ssl3_get_cipher(i);
        if (!ciph->min_dtls)
            continue;
        if (!cipher_enabled(ciph)) {
            TEST_skip("Skipping disabled cipher %s", ciph->name);
            continue;
        }
        if (!TEST_true(ssl_cipher_get_overhead(ciph, &mac, &in, &blk, &ex))) {
            TEST_info("Failed getting %s", ciph->name);
            ret = 0;
        } else {
            TEST_info("Cipher %s: %zu %zu %zu %zu",
                      ciph->name, mac, in, blk, ex);
        }
    }
    return ret;
}

int setup_tests(void)
{
    ADD_TEST(cipher_overhead);
    return 1;
}
