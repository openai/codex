/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*-
 * RISC-V 32 ZKND ZKNE support for AES CCM.
 * This file is included by cipher_aes_ccm_hw.c
 */

static int ccm_rv32i_zknd_zkne_initkey(PROV_CCM_CTX *ctx, const unsigned char *key,
                                       size_t keylen)
{
    PROV_AES_CCM_CTX *actx = (PROV_AES_CCM_CTX *)ctx;

    AES_HW_CCM_SET_KEY_FN(rv32i_zkne_set_encrypt_key, rv32i_zkne_encrypt,
                          NULL, NULL);
    return 1;
}

static int ccm_rv32i_zbkb_zknd_zkne_initkey(PROV_CCM_CTX *ctx, const unsigned char *key,
                                            size_t keylen)
{
    PROV_AES_CCM_CTX *actx = (PROV_AES_CCM_CTX *)ctx;

    AES_HW_CCM_SET_KEY_FN(rv32i_zbkb_zkne_set_encrypt_key, rv32i_zkne_encrypt,
                          NULL, NULL);
    return 1;
}

static const PROV_CCM_HW rv32i_zknd_zkne_ccm = {
    ccm_rv32i_zknd_zkne_initkey,
    ossl_ccm_generic_setiv,
    ossl_ccm_generic_setaad,
    ossl_ccm_generic_auth_encrypt,
    ossl_ccm_generic_auth_decrypt,
    ossl_ccm_generic_gettag
};

static const PROV_CCM_HW rv32i_zbkb_zknd_zkne_ccm = {
    ccm_rv32i_zbkb_zknd_zkne_initkey,
    ossl_ccm_generic_setiv,
    ossl_ccm_generic_setaad,
    ossl_ccm_generic_auth_encrypt,
    ossl_ccm_generic_auth_decrypt,
    ossl_ccm_generic_gettag
};

const PROV_CCM_HW *ossl_prov_aes_hw_ccm(size_t keybits)
{
    if (RISCV_HAS_ZBKB_AND_ZKND_AND_ZKNE())
        return &rv32i_zbkb_zknd_zkne_ccm;
    if (RISCV_HAS_ZKND_AND_ZKNE())
        return &rv32i_zknd_zkne_ccm;
    return &aes_ccm;
}
