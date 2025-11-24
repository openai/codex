/*
 * Copyright 2020-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/ec.h>
#include "crypto/ec.h"
#include "internal/nelem.h"

typedef struct ec_name2nid_st {
    const char *name;
    int nid;
} EC_NAME2NID;

static const EC_NAME2NID curve_list[] = {
    /* prime field curves */
    /* secg curves */
    {"secp112r1", NID_secp112r1 },
    {"secp112r2", NID_secp112r2 },
    {"secp128r1", NID_secp128r1 },
    {"secp128r2", NID_secp128r2 },
    {"secp160k1", NID_secp160k1 },
    {"secp160r1", NID_secp160r1 },
    {"secp160r2", NID_secp160r2 },
    {"secp192k1", NID_secp192k1 },
    {"secp224k1", NID_secp224k1 },
    {"secp224r1", NID_secp224r1 },
    {"secp256k1", NID_secp256k1 },
    {"secp384r1", NID_secp384r1 },
    {"secp521r1", NID_secp521r1 },
    /* X9.62 curves */
    {"prime192v1", NID_X9_62_prime192v1 },
    {"prime192v2", NID_X9_62_prime192v2 },
    {"prime192v3", NID_X9_62_prime192v3 },
    {"prime239v1", NID_X9_62_prime239v1 },
    {"prime239v2", NID_X9_62_prime239v2 },
    {"prime239v3", NID_X9_62_prime239v3 },
    {"prime256v1", NID_X9_62_prime256v1 },
    /* characteristic two field curves */
    /* NIST/SECG curves */
    {"sect113r1", NID_sect113r1 },
    {"sect113r2", NID_sect113r2 },
    {"sect131r1", NID_sect131r1 },
    {"sect131r2", NID_sect131r2 },
    {"sect163k1", NID_sect163k1 },
    {"sect163r1", NID_sect163r1 },
    {"sect163r2", NID_sect163r2 },
    {"sect193r1", NID_sect193r1 },
    {"sect193r2", NID_sect193r2 },
    {"sect233k1", NID_sect233k1 },
    {"sect233r1", NID_sect233r1 },
    {"sect239k1", NID_sect239k1 },
    {"sect283k1", NID_sect283k1 },
    {"sect283r1", NID_sect283r1 },
    {"sect409k1", NID_sect409k1 },
    {"sect409r1", NID_sect409r1 },
    {"sect571k1", NID_sect571k1 },
    {"sect571r1", NID_sect571r1 },
    /* X9.62 curves */
    {"c2pnb163v1", NID_X9_62_c2pnb163v1 },
    {"c2pnb163v2", NID_X9_62_c2pnb163v2 },
    {"c2pnb163v3", NID_X9_62_c2pnb163v3 },
    {"c2pnb176v1", NID_X9_62_c2pnb176v1 },
    {"c2tnb191v1", NID_X9_62_c2tnb191v1 },
    {"c2tnb191v2", NID_X9_62_c2tnb191v2 },
    {"c2tnb191v3", NID_X9_62_c2tnb191v3 },
    {"c2pnb208w1", NID_X9_62_c2pnb208w1 },
    {"c2tnb239v1", NID_X9_62_c2tnb239v1 },
    {"c2tnb239v2", NID_X9_62_c2tnb239v2 },
    {"c2tnb239v3", NID_X9_62_c2tnb239v3 },
    {"c2pnb272w1", NID_X9_62_c2pnb272w1 },
    {"c2pnb304w1", NID_X9_62_c2pnb304w1 },
    {"c2tnb359v1", NID_X9_62_c2tnb359v1 },
    {"c2pnb368w1", NID_X9_62_c2pnb368w1 },
    {"c2tnb431r1", NID_X9_62_c2tnb431r1 },
    /*
     * the WAP/WTLS curves [unlike SECG, spec has its own OIDs for curves
     * from X9.62]
     */
    {"wap-wsg-idm-ecid-wtls1", NID_wap_wsg_idm_ecid_wtls1 },
    {"wap-wsg-idm-ecid-wtls3", NID_wap_wsg_idm_ecid_wtls3 },
    {"wap-wsg-idm-ecid-wtls4", NID_wap_wsg_idm_ecid_wtls4 },
    {"wap-wsg-idm-ecid-wtls5", NID_wap_wsg_idm_ecid_wtls5 },
    {"wap-wsg-idm-ecid-wtls6", NID_wap_wsg_idm_ecid_wtls6 },
    {"wap-wsg-idm-ecid-wtls7", NID_wap_wsg_idm_ecid_wtls7 },
    {"wap-wsg-idm-ecid-wtls8", NID_wap_wsg_idm_ecid_wtls8 },
    {"wap-wsg-idm-ecid-wtls9", NID_wap_wsg_idm_ecid_wtls9 },
    {"wap-wsg-idm-ecid-wtls10", NID_wap_wsg_idm_ecid_wtls10 },
    {"wap-wsg-idm-ecid-wtls11", NID_wap_wsg_idm_ecid_wtls11 },
    {"wap-wsg-idm-ecid-wtls12", NID_wap_wsg_idm_ecid_wtls12 },
    /* IPSec curves */
    {"Oakley-EC2N-3", NID_ipsec3 },
    {"Oakley-EC2N-4", NID_ipsec4 },
    /* brainpool curves */
    {"brainpoolP160r1", NID_brainpoolP160r1 },
    {"brainpoolP160t1", NID_brainpoolP160t1 },
    {"brainpoolP192r1", NID_brainpoolP192r1 },
    {"brainpoolP192t1", NID_brainpoolP192t1 },
    {"brainpoolP224r1", NID_brainpoolP224r1 },
    {"brainpoolP224t1", NID_brainpoolP224t1 },
    {"brainpoolP256r1", NID_brainpoolP256r1 },
    {"brainpoolP256t1", NID_brainpoolP256t1 },
    {"brainpoolP320r1", NID_brainpoolP320r1 },
    {"brainpoolP320t1", NID_brainpoolP320t1 },
    {"brainpoolP384r1", NID_brainpoolP384r1 },
    {"brainpoolP384t1", NID_brainpoolP384t1 },
    {"brainpoolP512r1", NID_brainpoolP512r1 },
    {"brainpoolP512t1", NID_brainpoolP512t1 },
    /* SM2 curve */
    {"SM2", NID_sm2 },
};

const char *OSSL_EC_curve_nid2name(int nid)
{
    size_t i;

    if (nid <= 0)
        return NULL;

    for (i = 0; i < OSSL_NELEM(curve_list); i++) {
        if (curve_list[i].nid == nid)
            return curve_list[i].name;
    }
    return NULL;
}

int ossl_ec_curve_name2nid(const char *name)
{
    size_t i;
    int nid;

    if (name != NULL) {
        if ((nid = ossl_ec_curve_nist2nid_int(name)) != NID_undef)
            return nid;

        for (i = 0; i < OSSL_NELEM(curve_list); i++) {
            if (OPENSSL_strcasecmp(curve_list[i].name, name) == 0)
                return curve_list[i].nid;
        }
    }

    return NID_undef;
}

/* Functions to translate between common NIST curve names and NIDs */

static const EC_NAME2NID nist_curves[] = {
    {"B-163", NID_sect163r2},
    {"B-233", NID_sect233r1},
    {"B-283", NID_sect283r1},
    {"B-409", NID_sect409r1},
    {"B-571", NID_sect571r1},
    {"K-163", NID_sect163k1},
    {"K-233", NID_sect233k1},
    {"K-283", NID_sect283k1},
    {"K-409", NID_sect409k1},
    {"K-571", NID_sect571k1},
    {"P-192", NID_X9_62_prime192v1},
    {"P-224", NID_secp224r1},
    {"P-256", NID_X9_62_prime256v1},
    {"P-384", NID_secp384r1},
    {"P-521", NID_secp521r1}
};

const char *ossl_ec_curve_nid2nist_int(int nid)
{
    size_t i;
    for (i = 0; i < OSSL_NELEM(nist_curves); i++) {
        if (nist_curves[i].nid == nid)
            return nist_curves[i].name;
    }
    return NULL;
}

int ossl_ec_curve_nist2nid_int(const char *name)
{
    size_t i;
    for (i = 0; i < OSSL_NELEM(nist_curves); i++) {
        if (strcmp(nist_curves[i].name, name) == 0)
            return nist_curves[i].nid;
    }
    return NID_undef;
}
