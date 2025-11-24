/*
 * Copyright 2017-2024 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright 2017 Ribose Inc. All Rights Reserved.
 * Ported from Ribose contributions from Botan.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include "internal/sm3.h"

#define DATA_ORDER_IS_BIG_ENDIAN

#define HASH_LONG               SM3_WORD
#define HASH_CTX                SM3_CTX
#define HASH_CBLOCK             SM3_CBLOCK
#define HASH_UPDATE             ossl_sm3_update
#define HASH_TRANSFORM          ossl_sm3_transform
#define HASH_FINAL              ossl_sm3_final
#define HASH_MAKE_STRING(c, s)              \
      do {                                  \
        unsigned long ll;                   \
        ll=(c)->A; (void)HOST_l2c(ll, (s)); \
        ll=(c)->B; (void)HOST_l2c(ll, (s)); \
        ll=(c)->C; (void)HOST_l2c(ll, (s)); \
        ll=(c)->D; (void)HOST_l2c(ll, (s)); \
        ll=(c)->E; (void)HOST_l2c(ll, (s)); \
        ll=(c)->F; (void)HOST_l2c(ll, (s)); \
        ll=(c)->G; (void)HOST_l2c(ll, (s)); \
        ll=(c)->H; (void)HOST_l2c(ll, (s)); \
      } while (0)

#if defined(OPENSSL_SM3_ASM)
# if defined(__aarch64__) || defined(_M_ARM64)
#  include "crypto/arm_arch.h"
#  define HWSM3_CAPABLE (OPENSSL_armcap_P & ARMV8_SM3)
void ossl_hwsm3_block_data_order(SM3_CTX *c, const void *p, size_t num);
# endif
# if defined(OPENSSL_CPUID_OBJ) && defined(__riscv) && __riscv_xlen == 64
#  include "crypto/riscv_arch.h"
#  define HWSM3_CAPABLE 1
void ossl_hwsm3_block_data_order(SM3_CTX *c, const void *p, size_t num);
# endif
#endif

#if defined(HWSM3_CAPABLE)
# define HASH_BLOCK_DATA_ORDER (HWSM3_CAPABLE ? ossl_hwsm3_block_data_order \
                                              : ossl_sm3_block_data_order)
#else
# define HASH_BLOCK_DATA_ORDER   ossl_sm3_block_data_order
#endif

void ossl_sm3_block_data_order(SM3_CTX *c, const void *p, size_t num);
void ossl_sm3_transform(SM3_CTX *c, const unsigned char *data);

#include "crypto/md32_common.h"

#ifndef PEDANTIC
# if defined(__GNUC__) && __GNUC__>=2 && \
     !defined(OPENSSL_NO_ASM) && !defined(OPENSSL_NO_INLINE_ASM)
#  if defined(__riscv_zksh)
#   define P0(x) ({ MD32_REG_T ret;        \
                       asm ("sm3p0 %0, %1" \
                       : "=r"(ret)         \
                       : "r"(x)); ret;     })
#   define P1(x) ({ MD32_REG_T ret;        \
                       asm ("sm3p1 %0, %1" \
                       : "=r"(ret)         \
                       : "r"(x)); ret;     })
#  endif
# endif
#endif

#ifndef P0
# define P0(X) (X ^ ROTATE(X, 9) ^ ROTATE(X, 17))
#endif
#ifndef P1
# define P1(X) (X ^ ROTATE(X, 15) ^ ROTATE(X, 23))
#endif

#define FF0(X,Y,Z) (X ^ Y ^ Z)
#define GG0(X,Y,Z) (X ^ Y ^ Z)

#define FF1(X,Y,Z) ((X & Y) | ((X | Y) & Z))
#define GG1(X,Y,Z) ((Z ^ (X & (Y ^ Z))))

#define EXPAND(W0,W7,W13,W3,W10) \
   (P1(W0 ^ W7 ^ ROTATE(W13, 15)) ^ ROTATE(W3, 7) ^ W10)

#define RND(A, B, C, D, E, F, G, H, TJ, Wi, Wj, FF, GG)           \
     do {                                                         \
       const SM3_WORD A12 = ROTATE(A, 12);                        \
       const SM3_WORD A12_SM = A12 + E + TJ;                      \
       const SM3_WORD SS1 = ROTATE(A12_SM, 7);                    \
       const SM3_WORD TT1 = FF(A, B, C) + D + (SS1 ^ A12) + (Wj); \
       const SM3_WORD TT2 = GG(E, F, G) + H + SS1 + Wi;           \
       B = ROTATE(B, 9);                                          \
       D = TT1;                                                   \
       F = ROTATE(F, 19);                                         \
       H = P0(TT2);                                               \
     } while(0)

#define R1(A,B,C,D,E,F,G,H,TJ,Wi,Wj) \
   RND(A,B,C,D,E,F,G,H,TJ,Wi,Wj,FF0,GG0)

#define R2(A,B,C,D,E,F,G,H,TJ,Wi,Wj) \
   RND(A,B,C,D,E,F,G,H,TJ,Wi,Wj,FF1,GG1)

#define SM3_A 0x7380166fUL
#define SM3_B 0x4914b2b9UL
#define SM3_C 0x172442d7UL
#define SM3_D 0xda8a0600UL
#define SM3_E 0xa96f30bcUL
#define SM3_F 0x163138aaUL
#define SM3_G 0xe38dee4dUL
#define SM3_H 0xb0fb0e4eUL
