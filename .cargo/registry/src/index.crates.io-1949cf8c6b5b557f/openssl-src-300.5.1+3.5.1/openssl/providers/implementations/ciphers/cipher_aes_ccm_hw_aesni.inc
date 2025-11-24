/*
 * Copyright 2001-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*-
 * AES-NI support for AES CCM.
 * This file is included by cipher_aes_ccm_hw.c
 */

static int ccm_aesni_initkey(PROV_CCM_CTX *ctx, const unsigned char *key,
                             size_t keylen)
{
    PROV_AES_CCM_CTX *actx = (PROV_AES_CCM_CTX *)ctx;

    AES_HW_CCM_SET_KEY_FN(aesni_set_encrypt_key, aesni_encrypt,
                          aesni_ccm64_encrypt_blocks,
                          aesni_ccm64_decrypt_blocks);
    return 1;
}

static const PROV_CCM_HW aesni_ccm = {
    ccm_aesni_initkey,
    ossl_ccm_generic_setiv,
    ossl_ccm_generic_setaad,
    ossl_ccm_generic_auth_encrypt,
    ossl_ccm_generic_auth_decrypt,
    ossl_ccm_generic_gettag
};

const PROV_CCM_HW *ossl_prov_aes_hw_ccm(size_t keybits)
{
    return AESNI_CAPABLE ? &aesni_ccm : &aes_ccm;
}
