/*
 * Copyright 2017-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CRYPTO_S390X_ARCH_H
# define OSSL_CRYPTO_S390X_ARCH_H

# ifndef __ASSEMBLER__

#include "crypto/bn.h"

void s390x_kimd(const unsigned char *in, size_t len, unsigned int fc,
                void *param);
void s390x_klmd(const unsigned char *in, size_t inlen, unsigned char *out,
                size_t outlen, unsigned int fc, void *param);
void s390x_km(const unsigned char *in, size_t len, unsigned char *out,
              unsigned int fc, void *param);
void s390x_kmac(const unsigned char *in, size_t len, unsigned int fc,
                void *param);
void s390x_kmo(const unsigned char *in, size_t len, unsigned char *out,
               unsigned int fc, void *param);
void s390x_kmf(const unsigned char *in, size_t len, unsigned char *out,
               unsigned int fc, void *param);
void s390x_kma(const unsigned char *aad, size_t alen, const unsigned char *in,
               size_t len, unsigned char *out, unsigned int fc, void *param);
int s390x_pcc(unsigned int fc, void *param);
int s390x_kdsa(unsigned int fc, void *param, const unsigned char *in,
               size_t len);

void s390x_flip_endian32(unsigned char dst[32], const unsigned char src[32]);
void s390x_flip_endian64(unsigned char dst[64], const unsigned char src[64]);

int s390x_x25519_mul(unsigned char u_dst[32],
                     const unsigned char u_src[32],
                     const unsigned char d_src[32]);
int s390x_x448_mul(unsigned char u_dst[56],
                   const unsigned char u_src[56],
                   const unsigned char d_src[56]);
int s390x_ed25519_mul(unsigned char x_dst[32],
                      unsigned char y_dst[32],
                      const unsigned char x_src[32],
                      const unsigned char y_src[32],
                      const unsigned char d_src[32]);
int s390x_ed448_mul(unsigned char x_dst[57],
                    unsigned char y_dst[57],
                    const unsigned char x_src[57],
                    const unsigned char y_src[57],
                    const unsigned char d_src[57]);

/*
 * The field elements of OPENSSL_s390xcap_P are the 64-bit words returned by
 * the STFLE instruction followed by the 64-bit word pairs returned by
 * instructions' QUERY functions. If STFLE returns fewer data or an instruction
 * is not supported, the corresponding field elements are zero.
 */
struct OPENSSL_s390xcap_st {
    unsigned long long stfle[4];
    unsigned long long kimd[2];
    unsigned long long klmd[2];
    unsigned long long km[2];
    unsigned long long kmc[2];
    unsigned long long kmac[2];
    unsigned long long kmctr[2];
    unsigned long long kmo[2];
    unsigned long long kmf[2];
    unsigned long long prno[2];
    unsigned long long kma[2];
    unsigned long long pcc[2];
    unsigned long long kdsa[2];
};

#  if defined(__GNUC__) && defined(__linux)
__attribute__((visibility("hidden")))
#  endif
extern struct OPENSSL_s390xcap_st OPENSSL_s390xcap_P;

#  ifdef S390X_MOD_EXP
#   if defined(__GNUC__) && defined(__linux)
__attribute__((visibility("hidden")))
#   endif
extern int OPENSSL_s390xcex;
#   if defined(__GNUC__) && defined(__linux)
__attribute__((visibility("hidden")))
#   endif
extern int OPENSSL_s390xcex_nodev;
#  endif

/* Max number of 64-bit words currently returned by STFLE */
#  define S390X_STFLE_MAX       3

/* convert facility bit number or function code to bit mask */
#  define S390X_CAPBIT(i)       (1ULL << (63 - (i) % 64))

# endif

/* OPENSSL_s390xcap_P offsets [bytes] */
# define S390X_STFLE            0x00
# define S390X_KIMD             0x20
# define S390X_KLMD             0x30
# define S390X_KM               0x40
# define S390X_KMC              0x50
# define S390X_KMAC             0x60
# define S390X_KMCTR            0x70
# define S390X_KMO              0x80
# define S390X_KMF              0x90
# define S390X_PRNO             0xa0
# define S390X_KMA              0xb0
# define S390X_PCC              0xc0
# define S390X_KDSA             0xd0

/* Facility Bit Numbers */
# define S390X_MSA              17      /* message-security-assist */
# define S390X_STCKF            25      /* store-clock-fast */
# define S390X_MSA5             57      /* message-security-assist-ext. 5 */
# define S390X_MSA3             76      /* message-security-assist-ext. 3 */
# define S390X_MSA4             77      /* message-security-assist-ext. 4 */
# define S390X_MSA12            86      /* message-security-assist-ext. 12 */
# define S390X_VX               129     /* vector */
# define S390X_VXD              134     /* vector packed decimal */
# define S390X_VXE              135     /* vector enhancements 1 */
# define S390X_MSA8             146     /* message-security-assist-ext. 8 */
# define S390X_MSA9             155     /* message-security-assist-ext. 9 */

/* Function Codes */

/* all instructions */
# define S390X_QUERY            0

/* kimd/klmd */
# define S390X_SHA_1            1
# define S390X_SHA_256          2
# define S390X_SHA_512          3
# define S390X_SHA3_224         32
# define S390X_SHA3_256         33
# define S390X_SHA3_384         34
# define S390X_SHA3_512         35
# define S390X_KECCAK_224       32
# define S390X_KECCAK_256       33
# define S390X_KECCAK_384       34
# define S390X_KECCAK_512       35
# define S390X_SHAKE_128        36
# define S390X_SHAKE_256        37
# define S390X_GHASH            65

/* km/kmc/kmac/kmctr/kmo/kmf/kma */
# define S390X_AES_128          18
# define S390X_AES_192          19
# define S390X_AES_256          20

/* km */
# define S390X_XTS_AES_128      50
# define S390X_XTS_AES_256      52
# define S390X_XTS_AES_128_MSA10 82
# define S390X_XTS_AES_256_MSA10 84

/* kmac */
# define S390X_HMAC_SHA_224     112
# define S390X_HMAC_SHA_256     113
# define S390X_HMAC_SHA_384     114
# define S390X_HMAC_SHA_512     115

/* prno */
# define S390X_SHA_512_DRNG     3
# define S390X_TRNG             114

/* pcc */
# define S390X_SCALAR_MULTIPLY_P256     64
# define S390X_SCALAR_MULTIPLY_P384     65
# define S390X_SCALAR_MULTIPLY_P521     66
# define S390X_SCALAR_MULTIPLY_ED25519  72
# define S390X_SCALAR_MULTIPLY_ED448    73
# define S390X_SCALAR_MULTIPLY_X25519   80
# define S390X_SCALAR_MULTIPLY_X448     81

/* kdsa */
# define S390X_ECDSA_VERIFY_P256        1
# define S390X_ECDSA_VERIFY_P384        2
# define S390X_ECDSA_VERIFY_P521        3
# define S390X_ECDSA_SIGN_P256          9
# define S390X_ECDSA_SIGN_P384          10
# define S390X_ECDSA_SIGN_P521          11
# define S390X_EDDSA_VERIFY_ED25519     32
# define S390X_EDDSA_VERIFY_ED448       36
# define S390X_EDDSA_SIGN_ED25519       40
# define S390X_EDDSA_SIGN_ED448         44

/* Register 0 Flags */
# define S390X_DECRYPT          0x80
# define S390X_KMA_LPC          0x100
# define S390X_KMA_LAAD         0x200
# define S390X_KMA_HS           0x400
# define S390X_KDSA_D           0x80
# define S390X_KIMD_NIP         0x8000
# define S390X_KLMD_DUFOP       0x4000
# define S390X_KLMD_NIP         0x8000
# define S390X_KLMD_PS          0x100
# define S390X_KMAC_IKP         0x8000
# define S390X_KMAC_IIMP        0x4000
# define S390X_KMAC_CCUP        0x2000

#endif
