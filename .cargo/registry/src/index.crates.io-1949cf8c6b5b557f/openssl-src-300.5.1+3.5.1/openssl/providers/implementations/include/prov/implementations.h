/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core.h>
#include <openssl/types.h>

/* Digests */
extern const OSSL_DISPATCH ossl_sha1_functions[];
extern const OSSL_DISPATCH ossl_sha224_functions[];
extern const OSSL_DISPATCH ossl_sha256_functions[];
extern const OSSL_DISPATCH ossl_sha256_192_functions[];
extern const OSSL_DISPATCH ossl_sha384_functions[];
extern const OSSL_DISPATCH ossl_sha512_functions[];
extern const OSSL_DISPATCH ossl_sha512_224_functions[];
extern const OSSL_DISPATCH ossl_sha512_256_functions[];
extern const OSSL_DISPATCH ossl_sha3_224_functions[];
extern const OSSL_DISPATCH ossl_sha3_256_functions[];
extern const OSSL_DISPATCH ossl_sha3_384_functions[];
extern const OSSL_DISPATCH ossl_sha3_512_functions[];
extern const OSSL_DISPATCH ossl_keccak_224_functions[];
extern const OSSL_DISPATCH ossl_keccak_256_functions[];
extern const OSSL_DISPATCH ossl_keccak_384_functions[];
extern const OSSL_DISPATCH ossl_keccak_512_functions[];
extern const OSSL_DISPATCH ossl_keccak_kmac_128_functions[];
extern const OSSL_DISPATCH ossl_keccak_kmac_256_functions[];
extern const OSSL_DISPATCH ossl_shake_128_functions[];
extern const OSSL_DISPATCH ossl_shake_256_functions[];
extern const OSSL_DISPATCH ossl_blake2s256_functions[];
extern const OSSL_DISPATCH ossl_blake2b512_functions[];
extern const OSSL_DISPATCH ossl_md5_functions[];
extern const OSSL_DISPATCH ossl_md5_sha1_functions[];
extern const OSSL_DISPATCH ossl_sm3_functions[];
extern const OSSL_DISPATCH ossl_md2_functions[];
extern const OSSL_DISPATCH ossl_md4_functions[];
extern const OSSL_DISPATCH ossl_mdc2_functions[];
extern const OSSL_DISPATCH ossl_wp_functions[];
extern const OSSL_DISPATCH ossl_ripemd160_functions[];
extern const OSSL_DISPATCH ossl_nullmd_functions[];

/* Ciphers */
extern const OSSL_DISPATCH ossl_null_functions[];
extern const OSSL_DISPATCH ossl_aes256ecb_functions[];
extern const OSSL_DISPATCH ossl_aes192ecb_functions[];
extern const OSSL_DISPATCH ossl_aes128ecb_functions[];
extern const OSSL_DISPATCH ossl_aes256cbc_functions[];
extern const OSSL_DISPATCH ossl_aes192cbc_functions[];
extern const OSSL_DISPATCH ossl_aes128cbc_functions[];
extern const OSSL_DISPATCH ossl_aes256cbc_cts_functions[];
extern const OSSL_DISPATCH ossl_aes192cbc_cts_functions[];
extern const OSSL_DISPATCH ossl_aes128cbc_cts_functions[];
extern const OSSL_DISPATCH ossl_aes256ofb_functions[];
extern const OSSL_DISPATCH ossl_aes192ofb_functions[];
extern const OSSL_DISPATCH ossl_aes128ofb_functions[];
extern const OSSL_DISPATCH ossl_aes256cfb_functions[];
extern const OSSL_DISPATCH ossl_aes192cfb_functions[];
extern const OSSL_DISPATCH ossl_aes128cfb_functions[];
extern const OSSL_DISPATCH ossl_aes256cfb1_functions[];
extern const OSSL_DISPATCH ossl_aes192cfb1_functions[];
extern const OSSL_DISPATCH ossl_aes128cfb1_functions[];
extern const OSSL_DISPATCH ossl_aes256cfb8_functions[];
extern const OSSL_DISPATCH ossl_aes192cfb8_functions[];
extern const OSSL_DISPATCH ossl_aes128cfb8_functions[];
extern const OSSL_DISPATCH ossl_aes256ctr_functions[];
extern const OSSL_DISPATCH ossl_aes192ctr_functions[];
extern const OSSL_DISPATCH ossl_aes128ctr_functions[];
extern const OSSL_DISPATCH ossl_aes256xts_functions[];
extern const OSSL_DISPATCH ossl_aes128xts_functions[];
#ifndef OPENSSL_NO_OCB
extern const OSSL_DISPATCH ossl_aes256ocb_functions[];
extern const OSSL_DISPATCH ossl_aes192ocb_functions[];
extern const OSSL_DISPATCH ossl_aes128ocb_functions[];
#endif /* OPENSSL_NO_OCB */
extern const OSSL_DISPATCH ossl_aes256gcm_functions[];
extern const OSSL_DISPATCH ossl_aes192gcm_functions[];
extern const OSSL_DISPATCH ossl_aes128gcm_functions[];
extern const OSSL_DISPATCH ossl_aes256ccm_functions[];
extern const OSSL_DISPATCH ossl_aes192ccm_functions[];
extern const OSSL_DISPATCH ossl_aes128ccm_functions[];
extern const OSSL_DISPATCH ossl_aes256wrap_functions[];
extern const OSSL_DISPATCH ossl_aes192wrap_functions[];
extern const OSSL_DISPATCH ossl_aes128wrap_functions[];
extern const OSSL_DISPATCH ossl_aes256wrappad_functions[];
extern const OSSL_DISPATCH ossl_aes192wrappad_functions[];
extern const OSSL_DISPATCH ossl_aes128wrappad_functions[];
extern const OSSL_DISPATCH ossl_aes256wrapinv_functions[];
extern const OSSL_DISPATCH ossl_aes192wrapinv_functions[];
extern const OSSL_DISPATCH ossl_aes128wrapinv_functions[];
extern const OSSL_DISPATCH ossl_aes256wrappadinv_functions[];
extern const OSSL_DISPATCH ossl_aes192wrappadinv_functions[];
extern const OSSL_DISPATCH ossl_aes128wrappadinv_functions[];
extern const OSSL_DISPATCH ossl_aes256cbc_hmac_sha1_functions[];
extern const OSSL_DISPATCH ossl_aes128cbc_hmac_sha1_functions[];
extern const OSSL_DISPATCH ossl_aes256cbc_hmac_sha256_functions[];
extern const OSSL_DISPATCH ossl_aes128cbc_hmac_sha256_functions[];

#ifndef OPENSSL_NO_ARIA
extern const OSSL_DISPATCH ossl_aria256gcm_functions[];
extern const OSSL_DISPATCH ossl_aria192gcm_functions[];
extern const OSSL_DISPATCH ossl_aria128gcm_functions[];
extern const OSSL_DISPATCH ossl_aria256ccm_functions[];
extern const OSSL_DISPATCH ossl_aria192ccm_functions[];
extern const OSSL_DISPATCH ossl_aria128ccm_functions[];
extern const OSSL_DISPATCH ossl_aria256ecb_functions[];
extern const OSSL_DISPATCH ossl_aria192ecb_functions[];
extern const OSSL_DISPATCH ossl_aria128ecb_functions[];
extern const OSSL_DISPATCH ossl_aria256cbc_functions[];
extern const OSSL_DISPATCH ossl_aria192cbc_functions[];
extern const OSSL_DISPATCH ossl_aria128cbc_functions[];
extern const OSSL_DISPATCH ossl_aria256ofb_functions[];
extern const OSSL_DISPATCH ossl_aria192ofb_functions[];
extern const OSSL_DISPATCH ossl_aria128ofb_functions[];
extern const OSSL_DISPATCH ossl_aria256cfb_functions[];
extern const OSSL_DISPATCH ossl_aria192cfb_functions[];
extern const OSSL_DISPATCH ossl_aria128cfb_functions[];
extern const OSSL_DISPATCH ossl_aria256cfb1_functions[];
extern const OSSL_DISPATCH ossl_aria192cfb1_functions[];
extern const OSSL_DISPATCH ossl_aria128cfb1_functions[];
extern const OSSL_DISPATCH ossl_aria256cfb8_functions[];
extern const OSSL_DISPATCH ossl_aria192cfb8_functions[];
extern const OSSL_DISPATCH ossl_aria128cfb8_functions[];
extern const OSSL_DISPATCH ossl_aria256ctr_functions[];
extern const OSSL_DISPATCH ossl_aria192ctr_functions[];
extern const OSSL_DISPATCH ossl_aria128ctr_functions[];
#endif /* OPENSSL_NO_ARIA */
#ifndef OPENSSL_NO_CAMELLIA
extern const OSSL_DISPATCH ossl_camellia256ecb_functions[];
extern const OSSL_DISPATCH ossl_camellia192ecb_functions[];
extern const OSSL_DISPATCH ossl_camellia128ecb_functions[];
extern const OSSL_DISPATCH ossl_camellia256cbc_functions[];
extern const OSSL_DISPATCH ossl_camellia192cbc_functions[];
extern const OSSL_DISPATCH ossl_camellia128cbc_functions[];
extern const OSSL_DISPATCH ossl_camellia256cbc_cts_functions[];
extern const OSSL_DISPATCH ossl_camellia192cbc_cts_functions[];
extern const OSSL_DISPATCH ossl_camellia128cbc_cts_functions[];
extern const OSSL_DISPATCH ossl_camellia256ofb_functions[];
extern const OSSL_DISPATCH ossl_camellia192ofb_functions[];
extern const OSSL_DISPATCH ossl_camellia128ofb_functions[];
extern const OSSL_DISPATCH ossl_camellia256cfb_functions[];
extern const OSSL_DISPATCH ossl_camellia192cfb_functions[];
extern const OSSL_DISPATCH ossl_camellia128cfb_functions[];
extern const OSSL_DISPATCH ossl_camellia256cfb1_functions[];
extern const OSSL_DISPATCH ossl_camellia192cfb1_functions[];
extern const OSSL_DISPATCH ossl_camellia128cfb1_functions[];
extern const OSSL_DISPATCH ossl_camellia256cfb8_functions[];
extern const OSSL_DISPATCH ossl_camellia192cfb8_functions[];
extern const OSSL_DISPATCH ossl_camellia128cfb8_functions[];
extern const OSSL_DISPATCH ossl_camellia256ctr_functions[];
extern const OSSL_DISPATCH ossl_camellia192ctr_functions[];
extern const OSSL_DISPATCH ossl_camellia128ctr_functions[];
#endif /* OPENSSL_NO_CAMELLIA */
#ifndef OPENSSL_NO_BF
extern const OSSL_DISPATCH ossl_blowfish128ecb_functions[];
extern const OSSL_DISPATCH ossl_blowfish128cbc_functions[];
extern const OSSL_DISPATCH ossl_blowfish128ofb64_functions[];
extern const OSSL_DISPATCH ossl_blowfish128cfb64_functions[];
#endif /* OPENSSL_NO_BF */
#ifndef OPENSSL_NO_IDEA
extern const OSSL_DISPATCH ossl_idea128ecb_functions[];
extern const OSSL_DISPATCH ossl_idea128cbc_functions[];
extern const OSSL_DISPATCH ossl_idea128ofb64_functions[];
extern const OSSL_DISPATCH ossl_idea128cfb64_functions[];
#endif /* OPENSSL_NO_IDEA */
#ifndef OPENSSL_NO_CAST
extern const OSSL_DISPATCH ossl_cast5128ecb_functions[];
extern const OSSL_DISPATCH ossl_cast5128cbc_functions[];
extern const OSSL_DISPATCH ossl_cast5128ofb64_functions[];
extern const OSSL_DISPATCH ossl_cast5128cfb64_functions[];
#endif /* OPENSSL_NO_CAST */
#ifndef OPENSSL_NO_SEED
extern const OSSL_DISPATCH ossl_seed128ecb_functions[];
extern const OSSL_DISPATCH ossl_seed128cbc_functions[];
extern const OSSL_DISPATCH ossl_seed128ofb128_functions[];
extern const OSSL_DISPATCH ossl_seed128cfb128_functions[];
#endif /* OPENSSL_NO_SEED */
#ifndef OPENSSL_NO_SM4
extern const OSSL_DISPATCH ossl_sm4128gcm_functions[];
extern const OSSL_DISPATCH ossl_sm4128ccm_functions[];
extern const OSSL_DISPATCH ossl_sm4128ecb_functions[];
extern const OSSL_DISPATCH ossl_sm4128cbc_functions[];
extern const OSSL_DISPATCH ossl_sm4128ctr_functions[];
extern const OSSL_DISPATCH ossl_sm4128ofb128_functions[];
extern const OSSL_DISPATCH ossl_sm4128cfb128_functions[];
extern const OSSL_DISPATCH ossl_sm4128xts_functions[];
#endif /* OPENSSL_NO_SM4 */
#ifndef OPENSSL_NO_RC5
extern const OSSL_DISPATCH ossl_rc5128ecb_functions[];
extern const OSSL_DISPATCH ossl_rc5128cbc_functions[];
extern const OSSL_DISPATCH ossl_rc5128ofb64_functions[];
extern const OSSL_DISPATCH ossl_rc5128cfb64_functions[];
#endif /* OPENSSL_NO_RC5 */
#ifndef OPENSSL_NO_RC2
extern const OSSL_DISPATCH ossl_rc2128ecb_functions[];
extern const OSSL_DISPATCH ossl_rc2128cbc_functions[];
extern const OSSL_DISPATCH ossl_rc240cbc_functions[];
extern const OSSL_DISPATCH ossl_rc264cbc_functions[];
extern const OSSL_DISPATCH ossl_rc2128cfb128_functions[];
extern const OSSL_DISPATCH ossl_rc2128ofb128_functions[];
#endif /* OPENSSL_NO_RC2 */
#ifndef OPENSSL_NO_DES
extern const OSSL_DISPATCH ossl_tdes_ede3_ecb_functions[];
extern const OSSL_DISPATCH ossl_tdes_ede3_cbc_functions[];
# ifndef FIPS_MODULE
extern const OSSL_DISPATCH ossl_tdes_ede3_ofb_functions[];
extern const OSSL_DISPATCH ossl_tdes_ede3_cfb_functions[];
extern const OSSL_DISPATCH ossl_tdes_ede3_cfb8_functions[];
extern const OSSL_DISPATCH ossl_tdes_ede3_cfb1_functions[];

extern const OSSL_DISPATCH ossl_tdes_ede2_ecb_functions[];
extern const OSSL_DISPATCH ossl_tdes_ede2_cbc_functions[];
extern const OSSL_DISPATCH ossl_tdes_ede2_ofb_functions[];
extern const OSSL_DISPATCH ossl_tdes_ede2_cfb_functions[];

extern const OSSL_DISPATCH ossl_tdes_desx_cbc_functions[];
extern const OSSL_DISPATCH ossl_tdes_wrap_cbc_functions[];

extern const OSSL_DISPATCH ossl_des_ecb_functions[];
extern const OSSL_DISPATCH ossl_des_cbc_functions[];
extern const OSSL_DISPATCH ossl_des_ofb64_functions[];
extern const OSSL_DISPATCH ossl_des_cfb64_functions[];
extern const OSSL_DISPATCH ossl_des_cfb1_functions[];
extern const OSSL_DISPATCH ossl_des_cfb8_functions[];
# endif /* FIPS_MODULE */
#endif /* OPENSSL_NO_DES */

#ifndef OPENSSL_NO_RC4
extern const OSSL_DISPATCH ossl_rc440_functions[];
extern const OSSL_DISPATCH ossl_rc4128_functions[];
# ifndef OPENSSL_NO_MD5
extern const OSSL_DISPATCH ossl_rc4_hmac_ossl_md5_functions[];
# endif /* OPENSSL_NO_MD5 */
#endif /* OPENSSL_NO_RC4 */
#ifndef OPENSSL_NO_CHACHA
extern const OSSL_DISPATCH ossl_chacha20_functions[];
# ifndef OPENSSL_NO_POLY1305
extern const OSSL_DISPATCH ossl_chacha20_ossl_poly1305_functions[];
# endif /* OPENSSL_NO_POLY1305 */
#endif /* OPENSSL_NO_CHACHA */


#ifndef OPENSSL_NO_SIV
extern const OSSL_DISPATCH ossl_aes128siv_functions[];
extern const OSSL_DISPATCH ossl_aes192siv_functions[];
extern const OSSL_DISPATCH ossl_aes256siv_functions[];
extern const OSSL_DISPATCH ossl_aes128gcm_siv_functions[];
extern const OSSL_DISPATCH ossl_aes192gcm_siv_functions[];
extern const OSSL_DISPATCH ossl_aes256gcm_siv_functions[];
#endif /* OPENSSL_NO_SIV */

/* MACs */
extern const OSSL_DISPATCH ossl_blake2bmac_functions[];
extern const OSSL_DISPATCH ossl_blake2smac_functions[];
extern const OSSL_DISPATCH ossl_cmac_functions[];
extern const OSSL_DISPATCH ossl_gmac_functions[];
extern const OSSL_DISPATCH ossl_hmac_functions[];
#ifdef FIPS_MODULE
extern const OSSL_DISPATCH ossl_hmac_internal_functions[];
extern const OSSL_DISPATCH ossl_kmac128_internal_functions[];
extern const OSSL_DISPATCH ossl_kmac256_internal_functions[];
#endif
extern const OSSL_DISPATCH ossl_kmac128_functions[];
extern const OSSL_DISPATCH ossl_kmac256_functions[];
extern const OSSL_DISPATCH ossl_siphash_functions[];
extern const OSSL_DISPATCH ossl_poly1305_functions[];

/* KDFs / PRFs */
extern const OSSL_DISPATCH ossl_kdf_pbkdf1_functions[];
extern const OSSL_DISPATCH ossl_kdf_pbkdf2_functions[];
extern const OSSL_DISPATCH ossl_kdf_pvk_functions[];
extern const OSSL_DISPATCH ossl_kdf_pkcs12_functions[];
#ifndef OPENSSL_NO_SCRYPT
extern const OSSL_DISPATCH ossl_kdf_scrypt_functions[];
#endif
extern const OSSL_DISPATCH ossl_kdf_tls1_prf_functions[];
extern const OSSL_DISPATCH ossl_kdf_hkdf_functions[];
extern const OSSL_DISPATCH ossl_kdf_tls1_3_kdf_functions[];
extern const OSSL_DISPATCH ossl_kdf_sshkdf_functions[];
extern const OSSL_DISPATCH ossl_kdf_sskdf_functions[];
extern const OSSL_DISPATCH ossl_kdf_x963_kdf_functions[];
extern const OSSL_DISPATCH ossl_kdf_kbkdf_functions[];
extern const OSSL_DISPATCH ossl_kdf_x942_kdf_functions[];
extern const OSSL_DISPATCH ossl_kdf_krb5kdf_functions[];
extern const OSSL_DISPATCH ossl_kdf_hmac_drbg_functions[];
#ifndef OPENSSL_NO_ARGON2
extern const OSSL_DISPATCH ossl_kdf_argon2i_functions[];
extern const OSSL_DISPATCH ossl_kdf_argon2d_functions[];
extern const OSSL_DISPATCH ossl_kdf_argon2id_functions[];
#endif

/* RNGs */
extern const OSSL_DISPATCH ossl_test_rng_functions[];
extern const OSSL_DISPATCH ossl_seed_src_functions[];
extern const OSSL_DISPATCH ossl_jitter_functions[];
extern const OSSL_DISPATCH ossl_crng_test_functions[];
extern const OSSL_DISPATCH ossl_drbg_hash_functions[];
extern const OSSL_DISPATCH ossl_drbg_ossl_hmac_functions[];
extern const OSSL_DISPATCH ossl_drbg_ctr_functions[];
extern const OSSL_DISPATCH crngt_functions[];

/* Key management */
extern const OSSL_DISPATCH ossl_dh_keymgmt_functions[];
extern const OSSL_DISPATCH ossl_dhx_keymgmt_functions[];
extern const OSSL_DISPATCH ossl_dsa_keymgmt_functions[];
extern const OSSL_DISPATCH ossl_rsa_keymgmt_functions[];
extern const OSSL_DISPATCH ossl_rsapss_keymgmt_functions[];
extern const OSSL_DISPATCH ossl_kdf_keymgmt_functions[];
extern const OSSL_DISPATCH ossl_mac_legacy_keymgmt_functions[];
extern const OSSL_DISPATCH ossl_cmac_legacy_keymgmt_functions[];
#ifndef OPENSSL_NO_EC
extern const OSSL_DISPATCH ossl_ec_keymgmt_functions[];
# ifndef OPENSSL_NO_ECX
extern const OSSL_DISPATCH ossl_x25519_keymgmt_functions[];
extern const OSSL_DISPATCH ossl_x448_keymgmt_functions[];
extern const OSSL_DISPATCH ossl_ed25519_keymgmt_functions[];
extern const OSSL_DISPATCH ossl_ed448_keymgmt_functions[];
# endif
# ifndef OPENSSL_NO_SM2
extern const OSSL_DISPATCH ossl_sm2_keymgmt_functions[];
# endif
#endif
extern const OSSL_DISPATCH ossl_ml_dsa_44_keymgmt_functions[];
extern const OSSL_DISPATCH ossl_ml_dsa_65_keymgmt_functions[];
extern const OSSL_DISPATCH ossl_ml_dsa_87_keymgmt_functions[];
#ifndef OPENSSL_NO_ML_KEM
extern const OSSL_DISPATCH ossl_ml_kem_512_keymgmt_functions[];
extern const OSSL_DISPATCH ossl_ml_kem_768_keymgmt_functions[];
extern const OSSL_DISPATCH ossl_ml_kem_1024_keymgmt_functions[];
# ifndef OPENSSL_NO_EC
#  ifndef OPENSSL_NO_ECX
extern const OSSL_DISPATCH ossl_mlx_x25519_kem_kmgmt_functions[];
extern const OSSL_DISPATCH ossl_mlx_x448_kem_kmgmt_functions[];
#  endif
extern const OSSL_DISPATCH ossl_mlx_p256_kem_kmgmt_functions[];
extern const OSSL_DISPATCH ossl_mlx_p384_kem_kmgmt_functions[];
# endif
#endif
#ifndef OPENSSL_NO_SLH_DSA
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_128s_keymgmt_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_128f_keymgmt_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_192s_keymgmt_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_192f_keymgmt_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_256s_keymgmt_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_256f_keymgmt_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_128s_keymgmt_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_128f_keymgmt_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_192s_keymgmt_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_192f_keymgmt_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_256s_keymgmt_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_256f_keymgmt_functions[];
#endif /* OPENSSL_NO_SLH_DSA */

/* Key Exchange */
extern const OSSL_DISPATCH ossl_dh_keyexch_functions[];
#ifndef OPENSSL_NO_EC
extern const OSSL_DISPATCH ossl_ecdh_keyexch_functions[];
# ifndef OPENSSL_NO_ECX
extern const OSSL_DISPATCH ossl_x25519_keyexch_functions[];
extern const OSSL_DISPATCH ossl_x448_keyexch_functions[];
# endif
#endif
extern const OSSL_DISPATCH ossl_kdf_tls1_prf_keyexch_functions[];
extern const OSSL_DISPATCH ossl_kdf_hkdf_keyexch_functions[];
extern const OSSL_DISPATCH ossl_kdf_scrypt_keyexch_functions[];

/* Signature */
extern const OSSL_DISPATCH ossl_dsa_signature_functions[];
extern const OSSL_DISPATCH ossl_dsa_sha1_signature_functions[];
extern const OSSL_DISPATCH ossl_dsa_sha224_signature_functions[];
extern const OSSL_DISPATCH ossl_dsa_sha256_signature_functions[];
extern const OSSL_DISPATCH ossl_dsa_sha384_signature_functions[];
extern const OSSL_DISPATCH ossl_dsa_sha512_signature_functions[];
extern const OSSL_DISPATCH ossl_dsa_sha3_224_signature_functions[];
extern const OSSL_DISPATCH ossl_dsa_sha3_256_signature_functions[];
extern const OSSL_DISPATCH ossl_dsa_sha3_384_signature_functions[];
extern const OSSL_DISPATCH ossl_dsa_sha3_512_signature_functions[];
extern const OSSL_DISPATCH ossl_rsa_signature_functions[];
#ifndef OPENSSL_NO_MD5
extern const OSSL_DISPATCH ossl_rsa_md5_signature_functions[];
#endif
#ifndef OPENSSL_NO_RMD160
extern const OSSL_DISPATCH ossl_rsa_ripemd160_signature_functions[];
#endif
extern const OSSL_DISPATCH ossl_rsa_sha1_signature_functions[];
extern const OSSL_DISPATCH ossl_rsa_sha224_signature_functions[];
extern const OSSL_DISPATCH ossl_rsa_sha256_signature_functions[];
extern const OSSL_DISPATCH ossl_rsa_sha384_signature_functions[];
extern const OSSL_DISPATCH ossl_rsa_sha512_signature_functions[];
extern const OSSL_DISPATCH ossl_rsa_sha512_224_signature_functions[];
extern const OSSL_DISPATCH ossl_rsa_sha512_256_signature_functions[];
extern const OSSL_DISPATCH ossl_rsa_sha3_224_signature_functions[];
extern const OSSL_DISPATCH ossl_rsa_sha3_256_signature_functions[];
extern const OSSL_DISPATCH ossl_rsa_sha3_384_signature_functions[];
extern const OSSL_DISPATCH ossl_rsa_sha3_512_signature_functions[];
#ifndef OPENSSL_NO_SM3
extern const OSSL_DISPATCH ossl_rsa_sm3_signature_functions[];
#endif
extern const OSSL_DISPATCH ossl_ed25519_signature_functions[];
extern const OSSL_DISPATCH ossl_ed25519ph_signature_functions[];
extern const OSSL_DISPATCH ossl_ed25519ctx_signature_functions[];
extern const OSSL_DISPATCH ossl_ed448_signature_functions[];
extern const OSSL_DISPATCH ossl_ed448ph_signature_functions[];
extern const OSSL_DISPATCH ossl_ecdsa_signature_functions[];
extern const OSSL_DISPATCH ossl_ecdsa_sha1_signature_functions[];
extern const OSSL_DISPATCH ossl_ecdsa_sha224_signature_functions[];
extern const OSSL_DISPATCH ossl_ecdsa_sha256_signature_functions[];
extern const OSSL_DISPATCH ossl_ecdsa_sha384_signature_functions[];
extern const OSSL_DISPATCH ossl_ecdsa_sha512_signature_functions[];
extern const OSSL_DISPATCH ossl_ecdsa_sha3_224_signature_functions[];
extern const OSSL_DISPATCH ossl_ecdsa_sha3_256_signature_functions[];
extern const OSSL_DISPATCH ossl_ecdsa_sha3_384_signature_functions[];
extern const OSSL_DISPATCH ossl_ecdsa_sha3_512_signature_functions[];
extern const OSSL_DISPATCH ossl_mac_legacy_hmac_signature_functions[];
extern const OSSL_DISPATCH ossl_mac_legacy_siphash_signature_functions[];
extern const OSSL_DISPATCH ossl_mac_legacy_poly1305_signature_functions[];
extern const OSSL_DISPATCH ossl_mac_legacy_cmac_signature_functions[];
extern const OSSL_DISPATCH ossl_sm2_signature_functions[];
extern const OSSL_DISPATCH ossl_ml_dsa_44_signature_functions[];
extern const OSSL_DISPATCH ossl_ml_dsa_65_signature_functions[];
extern const OSSL_DISPATCH ossl_ml_dsa_87_signature_functions[];
#ifndef OPENSSL_NO_SLH_DSA
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_128s_signature_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_128f_signature_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_192s_signature_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_192f_signature_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_256s_signature_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_256f_signature_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_128s_signature_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_128f_signature_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_192s_signature_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_192f_signature_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_256s_signature_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_256f_signature_functions[];
#endif /* OPENSSL_NO_SLH_DSA */

/* Asym Cipher */
extern const OSSL_DISPATCH ossl_rsa_asym_cipher_functions[];
#ifndef OPENSSL_NO_SM2
extern const OSSL_DISPATCH ossl_sm2_asym_cipher_functions[];
#endif

/* Asym Key encapsulation  */
extern const OSSL_DISPATCH ossl_rsa_asym_kem_functions[];
#ifndef OPENSSL_NO_EC
extern const OSSL_DISPATCH ossl_ec_asym_kem_functions[];
# ifndef OPENSSL_NO_ECX
extern const OSSL_DISPATCH ossl_ecx_asym_kem_functions[];
# endif
#endif
#ifndef OPENSSL_NO_ML_KEM
extern const OSSL_DISPATCH ossl_ml_kem_asym_kem_functions[];
# ifndef OPENSSL_NO_EC
extern const OSSL_DISPATCH ossl_mlx_kem_asym_kem_functions[];
# endif
#endif

/* Encoders */
extern const OSSL_DISPATCH ossl_rsa_to_PKCS1_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_rsa_to_PKCS1_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_rsa_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_rsa_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_rsa_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_rsa_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_rsa_to_RSA_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_rsa_to_RSA_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_rsa_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_rsa_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_rsa_to_msblob_encoder_functions[];
extern const OSSL_DISPATCH ossl_rsa_to_pvk_encoder_functions[];
extern const OSSL_DISPATCH ossl_rsa_to_text_encoder_functions[];
extern const OSSL_DISPATCH ossl_rsa_to_type_specific_keypair_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_rsa_to_type_specific_keypair_pem_encoder_functions[];

extern const OSSL_DISPATCH ossl_rsapss_to_PKCS1_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_rsapss_to_PKCS1_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_rsapss_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_rsapss_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_rsapss_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_rsapss_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_rsapss_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_rsapss_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_rsapss_to_text_encoder_functions[];

extern const OSSL_DISPATCH ossl_dh_to_DH_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_dh_to_DH_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_dh_to_PKCS3_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_dh_to_PKCS3_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_dh_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_dh_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_dh_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_dh_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_dh_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_dh_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_dh_to_type_specific_params_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_dh_to_type_specific_params_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_dh_to_text_encoder_functions[];

extern const OSSL_DISPATCH ossl_dhx_to_DHX_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_dhx_to_DHX_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_dhx_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_dhx_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_dhx_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_dhx_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_dhx_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_dhx_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_dhx_to_X9_42_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_dhx_to_X9_42_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_dhx_to_type_specific_params_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_dhx_to_type_specific_params_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_dhx_to_text_encoder_functions[];

extern const OSSL_DISPATCH ossl_dsa_to_DSA_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_dsa_to_DSA_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_dsa_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_dsa_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_dsa_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_dsa_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_dsa_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_dsa_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_dsa_to_type_specific_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_dsa_to_type_specific_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_dsa_to_msblob_encoder_functions[];
extern const OSSL_DISPATCH ossl_dsa_to_pvk_encoder_functions[];
extern const OSSL_DISPATCH ossl_dsa_to_text_encoder_functions[];

extern const OSSL_DISPATCH ossl_ec_to_EC_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ec_to_EC_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ec_to_blob_encoder_functions[];
extern const OSSL_DISPATCH ossl_ec_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ec_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ec_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ec_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ec_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ec_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ec_to_X9_62_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ec_to_X9_62_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ec_to_type_specific_no_pub_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ec_to_type_specific_no_pub_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ec_to_text_encoder_functions[];

#ifndef OPENSSL_NO_SM2
extern const OSSL_DISPATCH ossl_sm2_to_SM2_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_sm2_to_SM2_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_sm2_to_blob_encoder_functions[];
extern const OSSL_DISPATCH ossl_sm2_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_sm2_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_sm2_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_sm2_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_sm2_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_sm2_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_sm2_to_type_specific_no_pub_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_sm2_to_type_specific_no_pub_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_sm2_to_text_encoder_functions[];
#endif

extern const OSSL_DISPATCH ossl_ed25519_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ed25519_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ed25519_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ed25519_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ed25519_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ed25519_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ed25519_to_OSSL_current_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ed25519_to_text_encoder_functions[];

extern const OSSL_DISPATCH ossl_ed448_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ed448_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ed448_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ed448_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ed448_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ed448_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ed448_to_OSSL_current_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ed448_to_text_encoder_functions[];

extern const OSSL_DISPATCH ossl_x25519_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_x25519_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_x25519_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_x25519_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_x25519_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_x25519_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_x25519_to_OSSL_current_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_x25519_to_text_encoder_functions[];

extern const OSSL_DISPATCH ossl_x448_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_x448_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_x448_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_x448_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_x448_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_x448_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_x448_to_OSSL_current_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_x448_to_text_encoder_functions[];

extern const OSSL_DISPATCH ossl_ml_dsa_44_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_dsa_44_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_dsa_44_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_dsa_44_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_dsa_44_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_dsa_44_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_dsa_44_to_OSSL_current_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_dsa_44_to_text_encoder_functions[];

extern const OSSL_DISPATCH ossl_ml_dsa_65_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_dsa_65_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_dsa_65_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_dsa_65_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_dsa_65_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_dsa_65_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_dsa_65_to_OSSL_current_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_dsa_65_to_text_encoder_functions[];

extern const OSSL_DISPATCH ossl_ml_dsa_87_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_dsa_87_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_dsa_87_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_dsa_87_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_dsa_87_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_dsa_87_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_dsa_87_to_OSSL_current_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_dsa_87_to_text_encoder_functions[];

extern const OSSL_DISPATCH ossl_slh_dsa_sha2_128s_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_128f_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_192s_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_192f_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_256s_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_256f_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_128s_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_128f_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_192s_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_192f_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_256s_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_256f_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_128s_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_128f_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_192s_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_192f_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_256s_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_256f_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_128s_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_128f_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_192s_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_192f_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_256s_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_256f_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];

extern const OSSL_DISPATCH ossl_slh_dsa_sha2_128s_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_128f_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_192s_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_192f_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_256s_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_256f_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_128s_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_128f_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_192s_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_192f_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_256s_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_256f_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_128s_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_128f_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_192s_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_192f_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_256s_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_256f_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_128s_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_128f_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_192s_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_192f_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_256s_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_256f_to_PrivateKeyInfo_pem_encoder_functions[];

extern const OSSL_DISPATCH ossl_slh_dsa_sha2_128s_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_128f_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_192s_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_192f_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_256s_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_256f_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_128s_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_128f_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_192s_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_192f_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_256s_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_256f_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_128s_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_128f_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_192s_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_192f_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_256s_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_256f_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_128s_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_128f_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_192s_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_192f_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_256s_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_256f_to_SubjectPublicKeyInfo_pem_encoder_functions[];

extern const OSSL_DISPATCH ossl_slh_dsa_sha2_128s_to_OSSL_current_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_128f_to_OSSL_current_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_192s_to_OSSL_current_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_192f_to_OSSL_current_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_256s_to_OSSL_current_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_256f_to_OSSL_current_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_128s_to_OSSL_current_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_128f_to_OSSL_current_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_192s_to_OSSL_current_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_192f_to_OSSL_current_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_256s_to_OSSL_current_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_256f_to_OSSL_current_der_encoder_functions[];

extern const OSSL_DISPATCH ossl_slh_dsa_sha2_128s_to_text_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_128f_to_text_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_192s_to_text_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_192f_to_text_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_256s_to_text_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_sha2_256f_to_text_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_128s_to_text_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_128f_to_text_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_192s_to_text_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_192f_to_text_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_256s_to_text_encoder_functions[];
extern const OSSL_DISPATCH ossl_slh_dsa_shake_256f_to_text_encoder_functions[];

/* Decoders */
extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_dh_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_dh_decoder_functions[];
extern const OSSL_DISPATCH ossl_type_specific_params_der_to_dh_decoder_functions[];
extern const OSSL_DISPATCH ossl_DH_der_to_dh_decoder_functions[];

extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_dhx_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_dhx_decoder_functions[];
extern const OSSL_DISPATCH ossl_type_specific_params_der_to_dhx_decoder_functions[];
extern const OSSL_DISPATCH ossl_DHX_der_to_dhx_decoder_functions[];

extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_dsa_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_dsa_decoder_functions[];
extern const OSSL_DISPATCH ossl_type_specific_der_to_dsa_decoder_functions[];
extern const OSSL_DISPATCH ossl_DSA_der_to_dsa_decoder_functions[];
extern const OSSL_DISPATCH ossl_msblob_to_dsa_decoder_functions[];
extern const OSSL_DISPATCH ossl_pvk_to_dsa_decoder_functions[];

extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_ec_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_ec_decoder_functions[];
extern const OSSL_DISPATCH ossl_type_specific_no_pub_der_to_ec_decoder_functions[];
extern const OSSL_DISPATCH ossl_EC_der_to_ec_decoder_functions[];

extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_x25519_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_x25519_decoder_functions[];

extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_x448_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_x448_decoder_functions[];

extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_ed25519_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_ed25519_decoder_functions[];

extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_ed448_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_ed448_decoder_functions[];

#ifndef OPENSSL_NO_ML_KEM
extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_ml_kem_512_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_ml_kem_512_decoder_functions[];
extern const OSSL_DISPATCH ossl_ml_kem_512_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_kem_512_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_kem_512_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_kem_512_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_kem_512_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_kem_512_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_kem_512_to_text_encoder_functions[];

extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_ml_kem_768_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_ml_kem_768_decoder_functions[];
extern const OSSL_DISPATCH ossl_ml_kem_768_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_kem_768_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_kem_768_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_kem_768_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_kem_768_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_kem_768_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_kem_768_to_text_encoder_functions[];

extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_ml_kem_1024_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_ml_kem_1024_decoder_functions[];
extern const OSSL_DISPATCH ossl_ml_kem_1024_to_EncryptedPrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_kem_1024_to_EncryptedPrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_kem_1024_to_PrivateKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_kem_1024_to_PrivateKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_kem_1024_to_SubjectPublicKeyInfo_der_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_kem_1024_to_SubjectPublicKeyInfo_pem_encoder_functions[];
extern const OSSL_DISPATCH ossl_ml_kem_1024_to_text_encoder_functions[];
#endif

#ifndef OPENSSL_NO_SM2
extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_sm2_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_sm2_decoder_functions[];
extern const OSSL_DISPATCH ossl_type_specific_no_pub_der_to_sm2_decoder_functions[];
#endif

extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_slh_dsa_sha2_128s_decoder_functions[];
extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_slh_dsa_sha2_128f_decoder_functions[];
extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_slh_dsa_sha2_192s_decoder_functions[];
extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_slh_dsa_sha2_192f_decoder_functions[];
extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_slh_dsa_sha2_256s_decoder_functions[];
extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_slh_dsa_sha2_256f_decoder_functions[];
extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_slh_dsa_shake_128s_decoder_functions[];
extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_slh_dsa_shake_128f_decoder_functions[];
extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_slh_dsa_shake_192s_decoder_functions[];
extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_slh_dsa_shake_192f_decoder_functions[];
extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_slh_dsa_shake_256s_decoder_functions[];
extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_slh_dsa_shake_256f_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_slh_dsa_sha2_128s_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_slh_dsa_sha2_128f_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_slh_dsa_sha2_192s_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_slh_dsa_sha2_192f_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_slh_dsa_sha2_256s_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_slh_dsa_sha2_256f_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_slh_dsa_shake_128s_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_slh_dsa_shake_128f_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_slh_dsa_shake_192s_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_slh_dsa_shake_192f_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_slh_dsa_shake_256s_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_slh_dsa_shake_256f_decoder_functions[];

extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_rsa_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_rsa_decoder_functions[];
extern const OSSL_DISPATCH ossl_type_specific_keypair_der_to_rsa_decoder_functions[];
extern const OSSL_DISPATCH ossl_RSA_der_to_rsa_decoder_functions[];
extern const OSSL_DISPATCH ossl_msblob_to_rsa_decoder_functions[];
extern const OSSL_DISPATCH ossl_pvk_to_rsa_decoder_functions[];

extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_rsapss_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_rsapss_decoder_functions[];

extern const OSSL_DISPATCH ossl_EncryptedPrivateKeyInfo_der_to_der_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_der_decoder_functions[];
extern const OSSL_DISPATCH ossl_pem_to_der_decoder_functions[];

extern const OSSL_DISPATCH ossl_file_store_functions[];
extern const OSSL_DISPATCH ossl_winstore_store_functions[];

extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_ml_dsa_44_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_ml_dsa_44_decoder_functions[];

extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_ml_dsa_65_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_ml_dsa_65_decoder_functions[];

extern const OSSL_DISPATCH ossl_PrivateKeyInfo_der_to_ml_dsa_87_decoder_functions[];
extern const OSSL_DISPATCH ossl_SubjectPublicKeyInfo_der_to_ml_dsa_87_decoder_functions[];

extern const OSSL_DISPATCH ossl_generic_skeymgmt_functions[];
extern const OSSL_DISPATCH ossl_aes_skeymgmt_functions[];
