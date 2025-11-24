/*
 * Copyright 2022-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_SM4_PLATFORM_H
# define OSSL_SM4_PLATFORM_H
# pragma once

# if defined(OPENSSL_CPUID_OBJ)
#  if defined(__aarch64__) ||  defined (_M_ARM64)
#   include "arm_arch.h"
extern unsigned int OPENSSL_arm_midr;
static inline int vpsm4_capable(void)
{
    return (OPENSSL_armcap_P & ARMV8_CPUID) &&
            (MIDR_IS_CPU_MODEL(OPENSSL_arm_midr, ARM_CPU_IMP_ARM, ARM_CPU_PART_V1) ||
             MIDR_IS_CPU_MODEL(OPENSSL_arm_midr, ARM_CPU_IMP_ARM, ARM_CPU_PART_N1));
}
static inline int vpsm4_ex_capable(void)
{
    return (OPENSSL_armcap_P & ARMV8_CPUID) &&
            (MIDR_IS_CPU_MODEL(OPENSSL_arm_midr, HISI_CPU_IMP, HISI_CPU_PART_KP920));
}
#   if defined(VPSM4_ASM)
#    define VPSM4_CAPABLE vpsm4_capable()
#    define VPSM4_EX_CAPABLE vpsm4_ex_capable()
#   endif
#   define HWSM4_CAPABLE (OPENSSL_armcap_P & ARMV8_SM4)
#   define HWSM4_set_encrypt_key sm4_v8_set_encrypt_key
#   define HWSM4_set_decrypt_key sm4_v8_set_decrypt_key
#   define HWSM4_encrypt sm4_v8_encrypt
#   define HWSM4_decrypt sm4_v8_decrypt
#   define HWSM4_cbc_encrypt sm4_v8_cbc_encrypt
#   define HWSM4_ecb_encrypt sm4_v8_ecb_encrypt
#   define HWSM4_ctr32_encrypt_blocks sm4_v8_ctr32_encrypt_blocks
#  elif defined(OPENSSL_CPUID_OBJ) && defined(__riscv) && __riscv_xlen == 64
/* RV64 support */
#   include "riscv_arch.h"
/* Zvksed extension (vector crypto SM4). */
int rv64i_zvksed_sm4_set_encrypt_key(const unsigned char *userKey,
                                     SM4_KEY *key);
int rv64i_zvksed_sm4_set_decrypt_key(const unsigned char *userKey,
                                     SM4_KEY *key);
void rv64i_zvksed_sm4_encrypt(const unsigned char *in, unsigned char *out,
                              const SM4_KEY *key);
void rv64i_zvksed_sm4_decrypt(const unsigned char *in, unsigned char *out,
                              const SM4_KEY *key);
#  endif /* RV64 */
# endif /* OPENSSL_CPUID_OBJ */

# if defined(HWSM4_CAPABLE)
int HWSM4_set_encrypt_key(const unsigned char *userKey, SM4_KEY *key);
int HWSM4_set_decrypt_key(const unsigned char *userKey, SM4_KEY *key);
void HWSM4_encrypt(const unsigned char *in, unsigned char *out,
                   const SM4_KEY *key);
void HWSM4_decrypt(const unsigned char *in, unsigned char *out,
                   const SM4_KEY *key);
void HWSM4_cbc_encrypt(const unsigned char *in, unsigned char *out,
                       size_t length, const SM4_KEY *key,
                       unsigned char *ivec, const int enc);
void HWSM4_ecb_encrypt(const unsigned char *in, unsigned char *out,
                       size_t length, const SM4_KEY *key,
                       const int enc);
void HWSM4_ctr32_encrypt_blocks(const unsigned char *in, unsigned char *out,
                                size_t len, const void *key,
                                const unsigned char ivec[16]);
# endif /* HWSM4_CAPABLE */

# ifdef VPSM4_CAPABLE
int vpsm4_set_encrypt_key(const unsigned char *userKey, SM4_KEY *key);
int vpsm4_set_decrypt_key(const unsigned char *userKey, SM4_KEY *key);
void vpsm4_encrypt(const unsigned char *in, unsigned char *out,
                   const SM4_KEY *key);
void vpsm4_decrypt(const unsigned char *in, unsigned char *out,
                   const SM4_KEY *key);
void vpsm4_cbc_encrypt(const unsigned char *in, unsigned char *out,
                       size_t length, const SM4_KEY *key,
                       unsigned char *ivec, const int enc);
void vpsm4_ecb_encrypt(const unsigned char *in, unsigned char *out,
                       size_t length, const SM4_KEY *key,
                       const int enc);
void vpsm4_ctr32_encrypt_blocks(const unsigned char *in, unsigned char *out,
                                size_t len, const void *key,
                                const unsigned char ivec[16]);
void vpsm4_xts_encrypt(const unsigned char *in, unsigned char *out,
                       size_t len, const SM4_KEY *key1, const SM4_KEY *key2,
                       const unsigned char ivec[16], const int enc);
void vpsm4_xts_encrypt_gb(const unsigned char *in, unsigned char *out,
                          size_t len, const SM4_KEY *key1, const SM4_KEY *key2,
                          const unsigned char ivec[16], const int enc);
# endif /* VPSM4_CAPABLE */

# ifdef VPSM4_EX_CAPABLE
int vpsm4_ex_set_encrypt_key(const unsigned char *userKey, SM4_KEY *key);
int vpsm4_ex_set_decrypt_key(const unsigned char *userKey, SM4_KEY *key);
void vpsm4_ex_encrypt(const unsigned char *in, unsigned char *out,
                      const SM4_KEY *key);
void vpsm4_ex_decrypt(const unsigned char *in, unsigned char *out,
                      const SM4_KEY *key);
void vpsm4_ex_cbc_encrypt(const unsigned char *in, unsigned char *out,
                          size_t length, const SM4_KEY *key,
                          unsigned char *ivec, const int enc);
void vpsm4_ex_ecb_encrypt(const unsigned char *in, unsigned char *out,
                          size_t length, const SM4_KEY *key,
                          const int enc);
void vpsm4_ex_ctr32_encrypt_blocks(const unsigned char *in, unsigned char *out,
                                   size_t len, const void *key,
                                   const unsigned char ivec[16]);
void vpsm4_ex_xts_encrypt(const unsigned char *in, unsigned char *out,
                          size_t len, const SM4_KEY *key1, const SM4_KEY *key2,
                          const unsigned char ivec[16], const int enc);
void vpsm4_ex_xts_encrypt_gb(const unsigned char *in, unsigned char *out,
                             size_t len, const SM4_KEY *key1,
                             const SM4_KEY *key2, const unsigned char ivec[16],
                             const int enc);
# endif /* VPSM4_EX_CAPABLE */

#endif /* OSSL_SM4_PLATFORM_H */
