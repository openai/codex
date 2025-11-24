/*
 * Copyright 2020-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/deprecated.h"

#include <openssl/rsa.h>
#include <openssl/dsa.h>
#include <openssl/dh.h>
#include <openssl/ec.h>
#include <openssl/err.h>
#include <openssl/proverr.h>
#include <openssl/core_names.h>
#include <openssl/obj_mac.h>
#include "prov/securitycheck.h"

int ossl_fips_config_securitycheck_enabled(OSSL_LIB_CTX *libctx)
{
#if !defined(OPENSSL_NO_FIPS_SECURITYCHECKS)
    return ossl_fips_config_security_checks(libctx);
#else
    return 0;
#endif /* OPENSSL_NO_FIPS_SECURITYCHECKS */
}

int ossl_digest_rsa_sign_get_md_nid(const EVP_MD *md)
{
    return ossl_digest_get_approved_nid(md);
}

int ossl_fips_ind_rsa_key_check(OSSL_FIPS_IND *ind, int id,
                                OSSL_LIB_CTX *libctx,
                                const RSA *rsa, const char *desc, int protect)
{
    int key_approved = ossl_rsa_check_key_size(rsa, protect);

    if (!key_approved) {
        if (!ossl_FIPS_IND_on_unapproved(ind, id, libctx, desc, "Key size",
                                         ossl_fips_config_securitycheck_enabled)) {
                ERR_raise_data(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH,
                               "operation: %s", desc);
            return 0;
        }
    }
    return 1;
}

# ifndef OPENSSL_NO_EC
int ossl_fips_ind_ec_key_check(OSSL_FIPS_IND *ind, int id,
                               OSSL_LIB_CTX *libctx,
                               const EC_GROUP *group, const char *desc,
                               int protect)
{
    int curve_allowed, strength_allowed;

    if (group == NULL)
        return 0;

    curve_allowed = ossl_ec_check_curve_allowed(group);
    strength_allowed = ossl_ec_check_security_strength(group, protect);

    if (!strength_allowed || !curve_allowed) {
        if (!ossl_FIPS_IND_on_unapproved(ind, id, libctx, desc, "EC Key",
                                         ossl_fips_config_securitycheck_enabled)) {
            if (!curve_allowed)
                ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_CURVE);
            if (!strength_allowed)
                ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
            return 0;
        }
    }
    return 1;
}
#endif

int ossl_fips_ind_digest_exch_check(OSSL_FIPS_IND *ind, int id,
                                    OSSL_LIB_CTX *libctx,
                                    const EVP_MD *md, const char *desc)
{
    int nid = ossl_digest_get_approved_nid(md);
    int approved = (nid != NID_undef && nid != NID_sha1);

    if (!approved) {
        if (!ossl_FIPS_IND_on_unapproved(ind, id, libctx, desc, "Digest",
                                         ossl_fips_config_securitycheck_enabled)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_DIGEST);
            return 0;
        }
    }
    return 1;
}

int ossl_fips_ind_digest_sign_check(OSSL_FIPS_IND *ind, int id,
                                    OSSL_LIB_CTX *libctx,
                                    int nid, int sha1_allowed,
                                    const char *desc,
                                    OSSL_FIPS_IND_CHECK_CB *config_check_f)
{
    int approved;

    if (nid == NID_undef)
        approved = 0;
    else
        approved = sha1_allowed || nid != NID_sha1;

    if (!approved) {
        if (!ossl_FIPS_IND_on_unapproved(ind, id, libctx, desc, "Digest SHA1",
                                         config_check_f)) {
            ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_DIGEST);
            return 0;
        }
    }
    return 1;
}
