/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_AES_PLATFORM_H
# define OSSL_AES_PLATFORM_H
# pragma once

# include <openssl/aes.h>

# ifdef VPAES_ASM
int vpaes_set_encrypt_key(const unsigned char *userKey, int bits,
                          AES_KEY *key);
int vpaes_set_decrypt_key(const unsigned char *userKey, int bits,
                          AES_KEY *key);
void vpaes_encrypt(const unsigned char *in, unsigned char *out,
                   const AES_KEY *key);
void vpaes_decrypt(const unsigned char *in, unsigned char *out,
                   const AES_KEY *key);
void vpaes_cbc_encrypt(const unsigned char *in,
                       unsigned char *out,
                       size_t length,
                       const AES_KEY *key, unsigned char *ivec, int enc);
# endif /* VPAES_ASM */

# ifdef BSAES_ASM
void ossl_bsaes_cbc_encrypt(const unsigned char *in, unsigned char *out,
                            size_t length, const AES_KEY *key,
                            unsigned char ivec[16], int enc);
void ossl_bsaes_ctr32_encrypt_blocks(const unsigned char *in,
                                     unsigned char *out, size_t len,
                                     const AES_KEY *key,
                                     const unsigned char ivec[16]);
void ossl_bsaes_xts_encrypt(const unsigned char *inp, unsigned char *out,
                            size_t len, const AES_KEY *key1,
                            const AES_KEY *key2, const unsigned char iv[16]);
void ossl_bsaes_xts_decrypt(const unsigned char *inp, unsigned char *out,
                            size_t len, const AES_KEY *key1,
                            const AES_KEY *key2, const unsigned char iv[16]);
# endif /* BSAES_ASM */

# ifdef AES_CTR_ASM
void AES_ctr32_encrypt(const unsigned char *in, unsigned char *out,
                       size_t blocks, const AES_KEY *key,
                       const unsigned char ivec[AES_BLOCK_SIZE]);
# endif /*  AES_CTR_ASM */

# ifdef AES_XTS_ASM
void AES_xts_encrypt(const unsigned char *inp, unsigned char *out, size_t len,
                     const AES_KEY *key1, const AES_KEY *key2,
                     const unsigned char iv[16]);
void AES_xts_decrypt(const unsigned char *inp, unsigned char *out, size_t len,
                     const AES_KEY *key1, const AES_KEY *key2,
                     const unsigned char iv[16]);
# endif /* AES_XTS_ASM */

# if defined(OPENSSL_CPUID_OBJ)
#  if (defined(__powerpc__) || defined(__POWERPC__) || defined(_ARCH_PPC))
#   include "crypto/ppc_arch.h"
#   ifdef VPAES_ASM
#    define VPAES_CAPABLE (OPENSSL_ppccap_P & PPC_ALTIVEC)
#   endif
#   if !defined(OPENSSL_SYS_MACOSX)
#    define HWAES_CAPABLE  (OPENSSL_ppccap_P & PPC_CRYPTO207)
#    define HWAES_set_encrypt_key aes_p8_set_encrypt_key
#    define HWAES_set_decrypt_key aes_p8_set_decrypt_key
#    define HWAES_encrypt aes_p8_encrypt
#    define HWAES_decrypt aes_p8_decrypt
#    define HWAES_cbc_encrypt aes_p8_cbc_encrypt
#    define HWAES_ctr32_encrypt_blocks aes_p8_ctr32_encrypt_blocks
#    define HWAES_xts_encrypt aes_p8_xts_encrypt
#    define HWAES_xts_decrypt aes_p8_xts_decrypt
#   endif /* OPENSSL_SYS_MACOSX */
#   if !defined(OPENSSL_SYS_AIX) && !defined(OPENSSL_SYS_MACOSX)
#    define PPC_AES_GCM_CAPABLE (OPENSSL_ppccap_P & PPC_MADD300)
#    define AES_GCM_ENC_BYTES 128
#    define AES_GCM_DEC_BYTES 128
size_t ppc_aes_gcm_encrypt(const unsigned char *in, unsigned char *out,
                           size_t len, const void *key, unsigned char ivec[16],
                           u64 *Xi);
size_t ppc_aes_gcm_decrypt(const unsigned char *in, unsigned char *out,
                           size_t len, const void *key, unsigned char ivec[16],
                           u64 *Xi);
#    define AES_GCM_ASM_PPC(gctx) ((gctx)->ctr==aes_p8_ctr32_encrypt_blocks && \
                                   (gctx)->gcm.funcs.ghash==gcm_ghash_p8)
void gcm_ghash_p8(u64 Xi[2],const u128 Htable[16],const u8 *inp, size_t len);
#   endif /* OPENSSL_SYS_AIX || OPENSSL_SYS_MACOSX */
#  endif /* PPC */

#  if (defined(__arm__) || defined(__arm) || defined(__aarch64__) || defined(_M_ARM64)) 
#   include "arm_arch.h"
#   if __ARM_MAX_ARCH__>=7
#    if defined(BSAES_ASM)
#     define BSAES_CAPABLE (OPENSSL_armcap_P & ARMV7_NEON)
#    endif
#    if defined(VPAES_ASM)
#     define VPAES_CAPABLE (OPENSSL_armcap_P & ARMV7_NEON)
#    endif
#    define HWAES_CAPABLE (OPENSSL_armcap_P & ARMV8_AES)
#    define HWAES_set_encrypt_key aes_v8_set_encrypt_key
#    define HWAES_set_decrypt_key aes_v8_set_decrypt_key
#    define HWAES_encrypt aes_v8_encrypt
#    define HWAES_decrypt aes_v8_decrypt
#    define HWAES_cbc_encrypt aes_v8_cbc_encrypt
#    define HWAES_ecb_encrypt aes_v8_ecb_encrypt
#    if __ARM_MAX_ARCH__>=8 && (defined(__aarch64__) || defined(_M_ARM64))
#     define ARMv8_HWAES_CAPABLE (OPENSSL_armcap_P & ARMV8_AES)
#     define HWAES_xts_encrypt aes_v8_xts_encrypt
#     define HWAES_xts_decrypt aes_v8_xts_decrypt
#    endif
#    define HWAES_ctr32_encrypt_blocks aes_v8_ctr32_encrypt_blocks
#    define HWAES_ctr32_encrypt_blocks_unroll12_eor3 aes_v8_ctr32_encrypt_blocks_unroll12_eor3
#    define AES_PMULL_CAPABLE ((OPENSSL_armcap_P & ARMV8_PMULL) && (OPENSSL_armcap_P & ARMV8_AES))
#    define AES_UNROLL12_EOR3_CAPABLE (OPENSSL_armcap_P & ARMV8_UNROLL12_EOR3)
#    define AES_GCM_ENC_BYTES 512
#    define AES_GCM_DEC_BYTES 512
#    if __ARM_MAX_ARCH__>=8 && (defined(__aarch64__) || defined(_M_ARM64))
#     define AES_gcm_encrypt armv8_aes_gcm_encrypt
#     define AES_gcm_decrypt armv8_aes_gcm_decrypt
#     define AES_GCM_ASM(gctx) (((gctx)->ctr==aes_v8_ctr32_encrypt_blocks_unroll12_eor3 || \
                                 (gctx)->ctr==aes_v8_ctr32_encrypt_blocks) && \
                                (gctx)->gcm.funcs.ghash==gcm_ghash_v8)
/* The [unroll8_eor3_]aes_gcm_(enc|dec)_(128|192|256)_kernel() functions
 * take input length in BITS and return number of BYTES processed */
size_t aes_gcm_enc_128_kernel(const uint8_t *plaintext, uint64_t plaintext_length, uint8_t *ciphertext,
                              uint64_t *Xi, unsigned char ivec[16], const void *key);
size_t aes_gcm_enc_192_kernel(const uint8_t *plaintext, uint64_t plaintext_length, uint8_t *ciphertext,
                              uint64_t *Xi, unsigned char ivec[16], const void *key);
size_t aes_gcm_enc_256_kernel(const uint8_t *plaintext, uint64_t plaintext_length, uint8_t *ciphertext,
                              uint64_t *Xi, unsigned char ivec[16], const void *key);
size_t aes_gcm_dec_128_kernel(const uint8_t *ciphertext, uint64_t plaintext_length, uint8_t *plaintext,
                              uint64_t *Xi, unsigned char ivec[16], const void *key);
size_t aes_gcm_dec_192_kernel(const uint8_t *ciphertext, uint64_t plaintext_length, uint8_t *plaintext,
                              uint64_t *Xi, unsigned char ivec[16], const void *key);
size_t aes_gcm_dec_256_kernel(const uint8_t *ciphertext, uint64_t plaintext_length, uint8_t *plaintext,
                              uint64_t *Xi, unsigned char ivec[16], const void *key);
size_t unroll8_eor3_aes_gcm_enc_128_kernel(const uint8_t *plaintext, uint64_t plaintext_length, uint8_t *ciphertext,
                              uint64_t *Xi, unsigned char ivec[16], const void *key);
size_t unroll8_eor3_aes_gcm_enc_192_kernel(const uint8_t *plaintext, uint64_t plaintext_length, uint8_t *ciphertext,
                              uint64_t *Xi, unsigned char ivec[16], const void *key);
size_t unroll8_eor3_aes_gcm_enc_256_kernel(const uint8_t *plaintext, uint64_t plaintext_length, uint8_t *ciphertext,
                              uint64_t *Xi, unsigned char ivec[16], const void *key);
size_t unroll8_eor3_aes_gcm_dec_128_kernel(const uint8_t *ciphertext, uint64_t plaintext_length, uint8_t *plaintext,
                              uint64_t *Xi, unsigned char ivec[16], const void *key);
size_t unroll8_eor3_aes_gcm_dec_192_kernel(const uint8_t *ciphertext, uint64_t plaintext_length, uint8_t *plaintext,
                              uint64_t *Xi, unsigned char ivec[16], const void *key);
size_t unroll8_eor3_aes_gcm_dec_256_kernel(const uint8_t *ciphertext, uint64_t plaintext_length, uint8_t *plaintext,
                              uint64_t *Xi, unsigned char ivec[16], const void *key);
size_t armv8_aes_gcm_encrypt(const unsigned char *in, unsigned char *out, size_t len, const void *key,
                             unsigned char ivec[16], u64 *Xi);
size_t armv8_aes_gcm_decrypt(const unsigned char *in, unsigned char *out, size_t len, const void *key,
                             unsigned char ivec[16], u64 *Xi);
void gcm_ghash_v8(u64 Xi[2],const u128 Htable[16],const u8 *inp, size_t len);
#    endif
#   endif
#  endif
# endif /* OPENSSL_CPUID_OBJ */

# if     defined(AES_ASM) &&     ( \
         defined(__x86_64)       || defined(__x86_64__)  || \
         defined(_M_AMD64)       || defined(_M_X64)      )
#  define AES_CBC_HMAC_SHA_CAPABLE 1
#  define AESNI_CBC_HMAC_SHA_CAPABLE (OPENSSL_ia32cap_P[1]&(1<<(57-32)))
# endif

# if defined(__loongarch__) || defined(__loongarch64)
#  include "loongarch_arch.h"
#  if defined(VPAES_ASM)
#   define VPAES_CAPABLE  (OPENSSL_loongarch_hwcap_P & LOONGARCH_HWCAP_LSX)
#  endif
# endif

# if     defined(AES_ASM) && !defined(I386_ONLY) &&      (  \
         ((defined(__i386)       || defined(__i386__)    || \
           defined(_M_IX86)) && defined(OPENSSL_IA32_SSE2))|| \
         defined(__x86_64)       || defined(__x86_64__)  || \
         defined(_M_AMD64)       || defined(_M_X64)      )

/* AES-NI section */

#  define AESNI_CAPABLE   (OPENSSL_ia32cap_P[1]&(1<<(57-32)))
#  ifdef VPAES_ASM
#   define VPAES_CAPABLE   (OPENSSL_ia32cap_P[1]&(1<<(41-32)))
#  endif
#  ifdef BSAES_ASM
#   define BSAES_CAPABLE   (OPENSSL_ia32cap_P[1]&(1<<(41-32)))
#  endif

#  define AES_GCM_ENC_BYTES 32
#  define AES_GCM_DEC_BYTES 16

int aesni_set_encrypt_key(const unsigned char *userKey, int bits,
                          AES_KEY *key);
int aesni_set_decrypt_key(const unsigned char *userKey, int bits,
                          AES_KEY *key);

void aesni_encrypt(const unsigned char *in, unsigned char *out,
                   const AES_KEY *key);
void aesni_decrypt(const unsigned char *in, unsigned char *out,
                   const AES_KEY *key);

void aesni_ecb_encrypt(const unsigned char *in,
                       unsigned char *out,
                       size_t length, const AES_KEY *key, int enc);
void aesni_cbc_encrypt(const unsigned char *in,
                       unsigned char *out,
                       size_t length,
                       const AES_KEY *key, unsigned char *ivec, int enc);
#  ifndef OPENSSL_NO_OCB
void aesni_ocb_encrypt(const unsigned char *in, unsigned char *out,
                       size_t blocks, const void *key,
                       size_t start_block_num,
                       unsigned char offset_i[16],
                       const unsigned char L_[][16],
                       unsigned char checksum[16]);
void aesni_ocb_decrypt(const unsigned char *in, unsigned char *out,
                       size_t blocks, const void *key,
                       size_t start_block_num,
                       unsigned char offset_i[16],
                       const unsigned char L_[][16],
                       unsigned char checksum[16]);
#  endif /* OPENSSL_NO_OCB */

void aesni_ctr32_encrypt_blocks(const unsigned char *in,
                                unsigned char *out,
                                size_t blocks,
                                const void *key, const unsigned char *ivec);

void aesni_xts_encrypt(const unsigned char *in,
                       unsigned char *out,
                       size_t length,
                       const AES_KEY *key1, const AES_KEY *key2,
                       const unsigned char iv[16]);

void aesni_xts_decrypt(const unsigned char *in,
                       unsigned char *out,
                       size_t length,
                       const AES_KEY *key1, const AES_KEY *key2,
                       const unsigned char iv[16]);

int aesni_xts_avx512_eligible(void);

void aesni_xts_128_encrypt_avx512(const unsigned char *inp, unsigned char *out,
                                  size_t len, const AES_KEY *key1,
                                  const AES_KEY *key2,
                                  const unsigned char iv[16]);
void aesni_xts_128_decrypt_avx512(const unsigned char *inp, unsigned char *out,
                                  size_t len, const AES_KEY *key1,
                                  const AES_KEY *key2,
                                  const unsigned char iv[16]);

void aesni_xts_256_encrypt_avx512(const unsigned char *inp, unsigned char *out,
                                  size_t len, const AES_KEY *key1,
                                  const AES_KEY *key2,
                                  const unsigned char iv[16]);
void aesni_xts_256_decrypt_avx512(const unsigned char *inp, unsigned char *out,
                                  size_t len, const AES_KEY *key1,
                                  const AES_KEY *key2,
                                  const unsigned char iv[16]);

void aesni_ccm64_encrypt_blocks(const unsigned char *in,
                                unsigned char *out,
                                size_t blocks,
                                const void *key,
                                const unsigned char ivec[16],
                                unsigned char cmac[16]);

void aesni_ccm64_decrypt_blocks(const unsigned char *in,
                                unsigned char *out,
                                size_t blocks,
                                const void *key,
                                const unsigned char ivec[16],
                                unsigned char cmac[16]);

#  if defined(__x86_64) || defined(__x86_64__) || defined(_M_AMD64) || defined(_M_X64)
size_t aesni_gcm_encrypt(const unsigned char *in, unsigned char *out, size_t len,
                         const void *key, unsigned char ivec[16], u64 *Xi);
size_t aesni_gcm_decrypt(const unsigned char *in, unsigned char *out, size_t len,
                         const void *key, unsigned char ivec[16], u64 *Xi);
void gcm_ghash_avx(u64 Xi[2], const u128 Htable[16], const u8 *in, size_t len);

#   define AES_gcm_encrypt aesni_gcm_encrypt
#   define AES_gcm_decrypt aesni_gcm_decrypt
#   define AES_GCM_ASM(ctx)    (ctx->ctr == aesni_ctr32_encrypt_blocks && \
                                ctx->gcm.funcs.ghash == gcm_ghash_avx)
#  endif


# elif defined(AES_ASM) && (defined(__sparc) || defined(__sparc__))

/* Fujitsu SPARC64 X support */
#  include "crypto/sparc_arch.h"

#  define SPARC_AES_CAPABLE       (OPENSSL_sparcv9cap_P[1] & CFR_AES)
#  define HWAES_CAPABLE           (OPENSSL_sparcv9cap_P[0] & SPARCV9_FJAESX)
#  define HWAES_set_encrypt_key aes_fx_set_encrypt_key
#  define HWAES_set_decrypt_key aes_fx_set_decrypt_key
#  define HWAES_encrypt aes_fx_encrypt
#  define HWAES_decrypt aes_fx_decrypt
#  define HWAES_cbc_encrypt aes_fx_cbc_encrypt
#  define HWAES_ctr32_encrypt_blocks aes_fx_ctr32_encrypt_blocks

void aes_t4_set_encrypt_key(const unsigned char *key, int bits, AES_KEY *ks);
void aes_t4_set_decrypt_key(const unsigned char *key, int bits, AES_KEY *ks);
void aes_t4_encrypt(const unsigned char *in, unsigned char *out,
                    const AES_KEY *key);
void aes_t4_decrypt(const unsigned char *in, unsigned char *out,
                    const AES_KEY *key);
/*
 * Key-length specific subroutines were chosen for following reason.
 * Each SPARC T4 core can execute up to 8 threads which share core's
 * resources. Loading as much key material to registers allows to
 * minimize references to shared memory interface, as well as amount
 * of instructions in inner loops [much needed on T4]. But then having
 * non-key-length specific routines would require conditional branches
 * either in inner loops or on subroutines' entries. Former is hardly
 * acceptable, while latter means code size increase to size occupied
 * by multiple key-length specific subroutines, so why fight?
 */
void aes128_t4_cbc_encrypt(const unsigned char *in, unsigned char *out,
                           size_t len, const AES_KEY *key,
                           unsigned char *ivec, int /*unused*/);
void aes128_t4_cbc_decrypt(const unsigned char *in, unsigned char *out,
                           size_t len, const AES_KEY *key,
                           unsigned char *ivec, int /*unused*/);
void aes192_t4_cbc_encrypt(const unsigned char *in, unsigned char *out,
                           size_t len, const AES_KEY *key,
                           unsigned char *ivec, int /*unused*/);
void aes192_t4_cbc_decrypt(const unsigned char *in, unsigned char *out,
                           size_t len, const AES_KEY *key,
                           unsigned char *ivec, int /*unused*/);
void aes256_t4_cbc_encrypt(const unsigned char *in, unsigned char *out,
                           size_t len, const AES_KEY *key,
                           unsigned char *ivec, int /*unused*/);
void aes256_t4_cbc_decrypt(const unsigned char *in, unsigned char *out,
                           size_t len, const AES_KEY *key,
                           unsigned char *ivec, int /*unused*/);
void aes128_t4_ctr32_encrypt(const unsigned char *in, unsigned char *out,
                             size_t blocks, const AES_KEY *key,
                             unsigned char *ivec);
void aes192_t4_ctr32_encrypt(const unsigned char *in, unsigned char *out,
                             size_t blocks, const AES_KEY *key,
                             unsigned char *ivec);
void aes256_t4_ctr32_encrypt(const unsigned char *in, unsigned char *out,
                             size_t blocks, const AES_KEY *key,
                             unsigned char *ivec);
void aes128_t4_xts_encrypt(const unsigned char *in, unsigned char *out,
                           size_t blocks, const AES_KEY *key1,
                           const AES_KEY *key2, const unsigned char *ivec);
void aes128_t4_xts_decrypt(const unsigned char *in, unsigned char *out,
                           size_t blocks, const AES_KEY *key1,
                           const AES_KEY *key2, const unsigned char *ivec);
void aes256_t4_xts_encrypt(const unsigned char *in, unsigned char *out,
                           size_t blocks, const AES_KEY *key1,
                           const AES_KEY *key2, const unsigned char *ivec);
void aes256_t4_xts_decrypt(const unsigned char *in, unsigned char *out,
                           size_t blocks, const AES_KEY *key1,
                           const AES_KEY *key2, const unsigned char *ivec);

# elif defined(OPENSSL_CPUID_OBJ) && defined(__s390__)
/* IBM S390X support */
#  include "s390x_arch.h"


/* Convert key size to function code: [16,24,32] -> [18,19,20]. */
#  define S390X_AES_FC(keylen)  (S390X_AES_128 + ((((keylen) << 3) - 128) >> 6))

/* Most modes of operation need km for partial block processing. */
#  define S390X_aes_128_CAPABLE (OPENSSL_s390xcap_P.km[0] &  \
                                S390X_CAPBIT(S390X_AES_128))
#  define S390X_aes_192_CAPABLE (OPENSSL_s390xcap_P.km[0] &  \
                                S390X_CAPBIT(S390X_AES_192))
#  define S390X_aes_256_CAPABLE (OPENSSL_s390xcap_P.km[0] &  \
                                S390X_CAPBIT(S390X_AES_256))

#  define S390X_aes_128_cbc_CAPABLE     1       /* checked by callee */
#  define S390X_aes_192_cbc_CAPABLE     1
#  define S390X_aes_256_cbc_CAPABLE     1

#  define S390X_aes_128_ecb_CAPABLE     S390X_aes_128_CAPABLE
#  define S390X_aes_192_ecb_CAPABLE     S390X_aes_192_CAPABLE
#  define S390X_aes_256_ecb_CAPABLE     S390X_aes_256_CAPABLE

#  define S390X_aes_128_ofb_CAPABLE (S390X_aes_128_CAPABLE &&           \
                                    (OPENSSL_s390xcap_P.kmo[0] &        \
                                     S390X_CAPBIT(S390X_AES_128)))
#  define S390X_aes_192_ofb_CAPABLE (S390X_aes_192_CAPABLE &&           \
                                    (OPENSSL_s390xcap_P.kmo[0] &        \
                                     S390X_CAPBIT(S390X_AES_192)))
#  define S390X_aes_256_ofb_CAPABLE (S390X_aes_256_CAPABLE &&           \
                                    (OPENSSL_s390xcap_P.kmo[0] &        \
                                     S390X_CAPBIT(S390X_AES_256)))

#  define S390X_aes_128_cfb_CAPABLE (S390X_aes_128_CAPABLE &&           \
                                    (OPENSSL_s390xcap_P.kmf[0] &        \
                                     S390X_CAPBIT(S390X_AES_128)))
#  define S390X_aes_192_cfb_CAPABLE (S390X_aes_192_CAPABLE &&           \
                                    (OPENSSL_s390xcap_P.kmf[0] &        \
                                     S390X_CAPBIT(S390X_AES_192)))
#  define S390X_aes_256_cfb_CAPABLE (S390X_aes_256_CAPABLE &&           \
                                    (OPENSSL_s390xcap_P.kmf[0] &        \
                                     S390X_CAPBIT(S390X_AES_256)))
#  define S390X_aes_128_cfb8_CAPABLE (OPENSSL_s390xcap_P.kmf[0] &       \
                                     S390X_CAPBIT(S390X_AES_128))
#  define S390X_aes_192_cfb8_CAPABLE (OPENSSL_s390xcap_P.kmf[0] &       \
                                     S390X_CAPBIT(S390X_AES_192))
#  define S390X_aes_256_cfb8_CAPABLE (OPENSSL_s390xcap_P.kmf[0] &       \
                                     S390X_CAPBIT(S390X_AES_256))
#  define S390X_aes_128_cfb1_CAPABLE    0
#  define S390X_aes_192_cfb1_CAPABLE    0
#  define S390X_aes_256_cfb1_CAPABLE    0

#  define S390X_aes_128_ctr_CAPABLE     1       /* checked by callee */
#  define S390X_aes_192_ctr_CAPABLE     1
#  define S390X_aes_256_ctr_CAPABLE     1

#  define S390X_aes_128_xts_CAPABLE     1       /* checked by callee */
#  define S390X_aes_256_xts_CAPABLE     1

# define S390X_aes_128_gcm_CAPABLE (S390X_aes_128_CAPABLE &&        \
                                    (OPENSSL_s390xcap_P.kma[0] &    \
                                     S390X_CAPBIT(S390X_AES_128)))
# define S390X_aes_192_gcm_CAPABLE (S390X_aes_192_CAPABLE &&        \
                                    (OPENSSL_s390xcap_P.kma[0] &    \
                                     S390X_CAPBIT(S390X_AES_192)))
# define S390X_aes_256_gcm_CAPABLE (S390X_aes_256_CAPABLE &&        \
                                    (OPENSSL_s390xcap_P.kma[0] &    \
                                     S390X_CAPBIT(S390X_AES_256)))

#  define S390X_aes_128_ccm_CAPABLE (S390X_aes_128_CAPABLE &&       \
                                    (OPENSSL_s390xcap_P.kmac[0] &   \
                                     S390X_CAPBIT(S390X_AES_128)))
#  define S390X_aes_192_ccm_CAPABLE (S390X_aes_192_CAPABLE &&       \
                                    (OPENSSL_s390xcap_P.kmac[0] &   \
                                     S390X_CAPBIT(S390X_AES_192)))
#  define S390X_aes_256_ccm_CAPABLE (S390X_aes_256_CAPABLE &&       \
                                    (OPENSSL_s390xcap_P.kmac[0] &   \
                                     S390X_CAPBIT(S390X_AES_256)))
#  define S390X_CCM_AAD_FLAG    0x40

#  ifndef OPENSSL_NO_OCB
#   define S390X_aes_128_ocb_CAPABLE    0
#   define S390X_aes_192_ocb_CAPABLE    0
#   define S390X_aes_256_ocb_CAPABLE    0
#  endif /* OPENSSL_NO_OCB */

#  ifndef OPENSSL_NO_SIV
#   define S390X_aes_128_siv_CAPABLE    0
#   define S390X_aes_192_siv_CAPABLE    0
#   define S390X_aes_256_siv_CAPABLE    0
#  endif /* OPENSSL_NO_SIV */

/* Convert key size to function code: [16,24,32] -> [18,19,20]. */
#  define S390X_AES_FC(keylen)  (S390X_AES_128 + ((((keylen) << 3) - 128) >> 6))
# elif defined(OPENSSL_CPUID_OBJ) && defined(__riscv) && __riscv_xlen == 64
/* RISC-V 64 support */
#  include "riscv_arch.h"

/* Zkne and Zknd extensions (scalar crypto AES). */
int rv64i_zkne_set_encrypt_key(const unsigned char *userKey, const int bits,
                               AES_KEY *key);
int rv64i_zknd_set_decrypt_key(const unsigned char *userKey, const int bits,
                               AES_KEY *key);
void rv64i_zkne_encrypt(const unsigned char *in, unsigned char *out,
                        const AES_KEY *key);
void rv64i_zknd_decrypt(const unsigned char *in, unsigned char *out,
                        const AES_KEY *key);
/* Zvkned extension (vector crypto AES). */
int rv64i_zvkned_set_encrypt_key(const unsigned char *userKey, const int bits,
                                 AES_KEY *key);
int rv64i_zvkned_set_decrypt_key(const unsigned char *userKey, const int bits,
                                 AES_KEY *key);
void rv64i_zvkned_encrypt(const unsigned char *in, unsigned char *out,
                          const AES_KEY *key);
void rv64i_zvkned_decrypt(const unsigned char *in, unsigned char *out,
                          const AES_KEY *key);

void rv64i_zvkned_cbc_encrypt(const unsigned char *in, unsigned char *out,
                              size_t length, const AES_KEY *key,
                              unsigned char *ivec, const int enc);

void rv64i_zvkned_cbc_decrypt(const unsigned char *in, unsigned char *out,
                              size_t length, const AES_KEY *key,
                              unsigned char *ivec, const int enc);

void rv64i_zvkned_ecb_encrypt(const unsigned char *in, unsigned char *out,
                              size_t length, const AES_KEY *key,
                              const int enc);

void rv64i_zvkned_ecb_decrypt(const unsigned char *in, unsigned char *out,
                              size_t length, const AES_KEY *key,
                              const int enc);

void rv64i_zvkb_zvkned_ctr32_encrypt_blocks(const unsigned char *in,
                                            unsigned char *out, size_t blocks,
                                            const void *key,
                                            const unsigned char ivec[16]);

size_t rv64i_zvkb_zvkg_zvkned_aes_gcm_encrypt(const unsigned char *in,
                                              unsigned char *out, size_t len,
                                              const void *key,
                                              unsigned char ivec[16], u64 *Xi);

size_t rv64i_zvkb_zvkg_zvkned_aes_gcm_decrypt(const unsigned char *in,
                                              unsigned char *out, size_t len,
                                              const void *key,
                                              unsigned char ivec[16], u64 *Xi);

void rv64i_zvbb_zvkg_zvkned_aes_xts_encrypt(const unsigned char *in,
                                            unsigned char *out, size_t length,
                                            const AES_KEY *key1,
                                            const AES_KEY *key2,
                                            const unsigned char iv[16]);

void rv64i_zvbb_zvkg_zvkned_aes_xts_decrypt(const unsigned char *in,
                                            unsigned char *out, size_t length,
                                            const AES_KEY *key1,
                                            const AES_KEY *key2,
                                            const unsigned char iv[16]);

void gcm_ghash_rv64i_zvkg(u64 Xi[2], const u128 Htable[16], const u8 *inp,
                          size_t len);

#define AES_GCM_ENC_BYTES 64
#define AES_GCM_DEC_BYTES 64
#define AES_gcm_encrypt rv64i_zvkb_zvkg_zvkned_aes_gcm_encrypt
#define AES_gcm_decrypt rv64i_zvkb_zvkg_zvkned_aes_gcm_decrypt
#define AES_GCM_ASM(ctx)                                                       \
    (ctx->ctr == rv64i_zvkb_zvkned_ctr32_encrypt_blocks &&                     \
     ctx->gcm.funcs.ghash == gcm_ghash_rv64i_zvkg)

# elif defined(OPENSSL_CPUID_OBJ) && defined(__riscv) && __riscv_xlen == 32
/* RISC-V 32 support */
#  include "riscv_arch.h"

int rv32i_zkne_set_encrypt_key(const unsigned char *userKey, const int bits,
                               AES_KEY *key);
/* set_decrypt_key needs both zknd and zkne */
int rv32i_zknd_zkne_set_decrypt_key(const unsigned char *userKey, const int bits,
                                    AES_KEY *key);
int rv32i_zbkb_zkne_set_encrypt_key(const unsigned char *userKey, const int bits,
                                    AES_KEY *key);
int rv32i_zbkb_zknd_zkne_set_decrypt_key(const unsigned char *userKey, const int bits,
                                         AES_KEY *key);
void rv32i_zkne_encrypt(const unsigned char *in, unsigned char *out,
                        const AES_KEY *key);
void rv32i_zknd_decrypt(const unsigned char *in, unsigned char *out,
                        const AES_KEY *key);
# endif

# if defined(HWAES_CAPABLE)
int HWAES_set_encrypt_key(const unsigned char *userKey, const int bits,
                          AES_KEY *key);
int HWAES_set_decrypt_key(const unsigned char *userKey, const int bits,
                          AES_KEY *key);
void HWAES_encrypt(const unsigned char *in, unsigned char *out,
                   const AES_KEY *key);
void HWAES_decrypt(const unsigned char *in, unsigned char *out,
                   const AES_KEY *key);
void HWAES_cbc_encrypt(const unsigned char *in, unsigned char *out,
                       size_t length, const AES_KEY *key,
                       unsigned char *ivec, const int enc);
void HWAES_ecb_encrypt(const unsigned char *in, unsigned char *out,
                       size_t length, const AES_KEY *key,
                       const int enc);
void HWAES_ctr32_encrypt_blocks(const unsigned char *in, unsigned char *out,
                                size_t len, const void *key,
                                const unsigned char ivec[16]);
#  if defined(AES_UNROLL12_EOR3_CAPABLE)
void HWAES_ctr32_encrypt_blocks_unroll12_eor3(const unsigned char *in, unsigned char *out,
                                              size_t len, const void *key,
                                              const unsigned char ivec[16]);
#  endif
void HWAES_xts_encrypt(const unsigned char *inp, unsigned char *out,
                       size_t len, const AES_KEY *key1,
                       const AES_KEY *key2, const unsigned char iv[16]);
void HWAES_xts_decrypt(const unsigned char *inp, unsigned char *out,
                       size_t len, const AES_KEY *key1,
                       const AES_KEY *key2, const unsigned char iv[16]);
#  ifndef OPENSSL_NO_OCB
#   ifdef HWAES_ocb_encrypt
void HWAES_ocb_encrypt(const unsigned char *in, unsigned char *out,
                       size_t blocks, const void *key,
                       size_t start_block_num,
                       unsigned char offset_i[16],
                       const unsigned char L_[][16],
                       unsigned char checksum[16]);
#   else
#     define HWAES_ocb_encrypt ((ocb128_f)NULL)
#   endif
#   ifdef HWAES_ocb_decrypt
void HWAES_ocb_decrypt(const unsigned char *in, unsigned char *out,
                       size_t blocks, const void *key,
                       size_t start_block_num,
                       unsigned char offset_i[16],
                       const unsigned char L_[][16],
                       unsigned char checksum[16]);
#   else
#     define HWAES_ocb_decrypt ((ocb128_f)NULL)
#   endif
#  endif /* OPENSSL_NO_OCB */

# endif /* HWAES_CAPABLE */

#endif /* OSSL_AES_PLATFORM_H */
