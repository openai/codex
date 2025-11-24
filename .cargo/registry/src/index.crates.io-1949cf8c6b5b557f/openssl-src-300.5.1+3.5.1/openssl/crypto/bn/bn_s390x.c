/*
 * Copyright 2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "crypto/bn.h"
#include "crypto/s390x_arch.h"

#ifdef S390X_MOD_EXP

# include <sys/types.h>
# include <sys/stat.h>
# include <fcntl.h>
# include <asm/zcrypt.h>
# include <sys/ioctl.h>
# include <unistd.h>
# include <errno.h>

static int s390x_mod_exp_hw(BIGNUM *r, const BIGNUM *a, const BIGNUM *p,
                            const BIGNUM *m)
{
    struct ica_rsa_modexpo me;
    unsigned char *buffer;
    size_t size;
    int res = 0;

    if (OPENSSL_s390xcex == -1 || OPENSSL_s390xcex_nodev)
        return 0;
    size = BN_num_bytes(m);
    buffer = OPENSSL_zalloc(4 * size);
    if (buffer == NULL)
        return 0;
    me.inputdata = buffer;
    me.inputdatalength = size;
    me.outputdata = buffer + size;
    me.outputdatalength = size;
    me.b_key = buffer + 2 * size;
    me.n_modulus = buffer + 3 * size;
    if (BN_bn2binpad(a, me.inputdata, size) == -1
        || BN_bn2binpad(p, me.b_key, size) == -1
        || BN_bn2binpad(m, me.n_modulus, size) == -1)
        goto dealloc;
    if (ioctl(OPENSSL_s390xcex, ICARSAMODEXPO, &me) != -1) {
        if (BN_bin2bn(me.outputdata, size, r) != NULL)
            res = 1;
    } else if (errno == EBADF || errno == ENOTTY) {
        /*
         * In this cases, someone (e.g. a sandbox) closed the fd.
         * Make sure to not further use this hardware acceleration.
         * In case of ENOTTY the file descriptor was already reused for another
         * file. Do not attempt to use or close that file descriptor anymore.
         */
        OPENSSL_s390xcex = -1;
    } else if (errno == ENODEV) {
        /*
         * No crypto card(s) available to handle RSA requests.
         * Make sure to not further use this hardware acceleration,
         * but do not close the file descriptor.
         */
        OPENSSL_s390xcex_nodev = 1;
    }
 dealloc:
    OPENSSL_clear_free(buffer, 4 * size);
    return res;
}

int s390x_mod_exp(BIGNUM *r, const BIGNUM *a, const BIGNUM *p,
                  const BIGNUM *m, BN_CTX *ctx, BN_MONT_CTX *m_ctx)
{
    if (s390x_mod_exp_hw(r, a, p, m) == 1)
        return 1;
    return BN_mod_exp_mont(r, a, p, m, ctx, m_ctx);
}

int s390x_crt(BIGNUM *r, const BIGNUM *i, const BIGNUM *p, const BIGNUM *q,
              const BIGNUM *dmp, const BIGNUM *dmq, const BIGNUM *iqmp)
{
    struct ica_rsa_modexpo_crt crt;
    unsigned char *buffer, *part;
    size_t size, plen, qlen;
    int res = 0;

    if (OPENSSL_s390xcex == -1 || OPENSSL_s390xcex_nodev)
        return 0;
    /*-
     * Hardware-accelerated CRT can only deal with p>q.  Fall back to
     * software in the (hopefully rare) other cases.
     */
    if (BN_ucmp(p, q) != 1)
        return 0;
    plen = BN_num_bytes(p);
    qlen = BN_num_bytes(q);
    size = (plen > qlen ? plen : qlen);
    buffer = OPENSSL_zalloc(9 * size + 24);
    if (buffer == NULL)
        return 0;
    part = buffer;
    crt.inputdata = part;
    crt.inputdatalength = 2 * size;
    part += 2 * size;
    crt.outputdata = part;
    crt.outputdatalength = 2 * size;
    part += 2 * size;
    crt.bp_key = part;
    part += size + 8;
    crt.bq_key = part;
    part += size;
    crt.np_prime = part;
    part += size + 8;
    crt.nq_prime = part;
    part += size;
    crt.u_mult_inv = part;
    if (BN_bn2binpad(i, crt.inputdata, crt.inputdatalength) == -1
        || BN_bn2binpad(p, crt.np_prime, size + 8) == -1
        || BN_bn2binpad(q, crt.nq_prime, size) == -1
        || BN_bn2binpad(dmp, crt.bp_key, size + 8) == -1
        || BN_bn2binpad(dmq, crt.bq_key, size) == -1
        || BN_bn2binpad(iqmp, crt.u_mult_inv, size + 8) == -1)
        goto dealloc;
    if (ioctl(OPENSSL_s390xcex, ICARSACRT, &crt) != -1) {
        if (BN_bin2bn(crt.outputdata, crt.outputdatalength, r) != NULL)
            res = 1;
    } else if (errno == EBADF || errno == ENOTTY) {
        /*
         * In this cases, someone (e.g. a sandbox) closed the fd.
         * Make sure to not further use this hardware acceleration.
         * In case of ENOTTY the file descriptor was already reused for another
         * file. Do not attempt to use or close that file descriptor anymore.
         */
        OPENSSL_s390xcex = -1;
    } else if (errno == ENODEV) {
        /*
         * No crypto card(s) available to handle RSA requests.
         * Make sure to not further use this hardware acceleration,
         * but do not close the file descriptor.
         */
        OPENSSL_s390xcex_nodev = 1;
    }
 dealloc:
    OPENSSL_clear_free(buffer, 9 * size + 24);
    return res;
}

#else
int s390x_mod_exp(BIGNUM *r, const BIGNUM *a, const BIGNUM *p,
                  const BIGNUM *m, BN_CTX *ctx, BN_MONT_CTX *m_ctx)
{
    return BN_mod_exp_mont(r, a, p, m, ctx, m_ctx);
}

int s390x_crt(BIGNUM *r, const BIGNUM *i, const BIGNUM *p, const BIGNUM *q,
              const BIGNUM *dmp, const BIGNUM *dmq, const BIGNUM *iqmp)
{
    return 0;
}

#endif
