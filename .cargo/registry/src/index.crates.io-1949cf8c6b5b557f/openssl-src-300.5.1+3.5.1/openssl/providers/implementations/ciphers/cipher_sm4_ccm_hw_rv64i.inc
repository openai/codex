/*
 * Copyright 2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*-
 * RV64I ZVKSED support for SM4 CCM.
 * This file is included by cipher_sm4_ccm_hw.c
 */

static int rv64i_zvksed_sm4_ccm_initkey(PROV_CCM_CTX *ctx,
                                        const unsigned char *key,
                                        size_t keylen)
{
    PROV_SM4_CCM_CTX *actx = (PROV_SM4_CCM_CTX *)ctx;

    SM4_HW_CCM_SET_KEY_FN(rv64i_zvksed_sm4_set_encrypt_key,
                          rv64i_zvksed_sm4_encrypt, NULL, NULL);
    return 1;
}

static const PROV_CCM_HW rv64i_zvksed_sm4_ccm = {
    rv64i_zvksed_sm4_ccm_initkey,
    ossl_ccm_generic_setiv,
    ossl_ccm_generic_setaad,
    ossl_ccm_generic_auth_encrypt,
    ossl_ccm_generic_auth_decrypt,
    ossl_ccm_generic_gettag
};

const PROV_CCM_HW *ossl_prov_sm4_hw_ccm(size_t keybits)
{
    if (RISCV_HAS_ZVKB_AND_ZVKSED() && riscv_vlen() >= 128)
        return &rv64i_zvksed_sm4_ccm;
    else
        return &ccm_sm4;
}
