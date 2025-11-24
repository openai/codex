/*
 * Copyright 2011-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CRYPTO_ARM_ARCH_H
# define OSSL_CRYPTO_ARM_ARCH_H

# if !defined(__ARM_ARCH__)
#  if defined(__CC_ARM)
#   define __ARM_ARCH__ __TARGET_ARCH_ARM
#   if defined(__BIG_ENDIAN)
#    define __ARMEB__
#   else
#    define __ARMEL__
#   endif
#  elif defined(__GNUC__)
#   if   defined(__aarch64__)
#    define __ARM_ARCH__ 8
  /*
   * Why doesn't gcc define __ARM_ARCH__? Instead it defines
   * bunch of below macros. See all_architectures[] table in
   * gcc/config/arm/arm.c. On a side note it defines
   * __ARMEL__/__ARMEB__ for little-/big-endian.
   */
#   elif defined(__ARM_ARCH)
#    define __ARM_ARCH__ __ARM_ARCH
#   elif defined(__ARM_ARCH_8A__)
#    define __ARM_ARCH__ 8
#   elif defined(__ARM_ARCH_7__) || defined(__ARM_ARCH_7A__)     || \
        defined(__ARM_ARCH_7R__)|| defined(__ARM_ARCH_7M__)     || \
        defined(__ARM_ARCH_7EM__)
#    define __ARM_ARCH__ 7
#   elif defined(__ARM_ARCH_6__) || defined(__ARM_ARCH_6J__)     || \
        defined(__ARM_ARCH_6K__)|| defined(__ARM_ARCH_6M__)     || \
        defined(__ARM_ARCH_6Z__)|| defined(__ARM_ARCH_6ZK__)    || \
        defined(__ARM_ARCH_6T2__)
#    define __ARM_ARCH__ 6
#   elif defined(__ARM_ARCH_5__) || defined(__ARM_ARCH_5T__)     || \
        defined(__ARM_ARCH_5E__)|| defined(__ARM_ARCH_5TE__)    || \
        defined(__ARM_ARCH_5TEJ__)
#    define __ARM_ARCH__ 5
#   elif defined(__ARM_ARCH_4__) || defined(__ARM_ARCH_4T__)
#    define __ARM_ARCH__ 4
#   else
#    error "unsupported ARM architecture"
#   endif
#  elif defined(__ARM_ARCH)
#   define __ARM_ARCH__ __ARM_ARCH
#  endif
# endif

# if !defined(__ARM_MAX_ARCH__)
#  define __ARM_MAX_ARCH__ __ARM_ARCH__
# endif

# if __ARM_MAX_ARCH__<__ARM_ARCH__
#  error "__ARM_MAX_ARCH__ can't be less than __ARM_ARCH__"
# elif __ARM_MAX_ARCH__!=__ARM_ARCH__
#  if __ARM_ARCH__<7 && __ARM_MAX_ARCH__>=7 && defined(__ARMEB__)
#   error "can't build universal big-endian binary"
#  endif
# endif

# ifndef __ASSEMBLER__
extern unsigned int OPENSSL_armcap_P;
extern unsigned int OPENSSL_arm_midr;
extern unsigned int OPENSSL_armv8_rsa_neonized;
# endif

# define ARMV7_NEON      (1<<0)
# define ARMV7_TICK      (1<<1)
# define ARMV8_AES       (1<<2)
# define ARMV8_SHA1      (1<<3)
# define ARMV8_SHA256    (1<<4)
# define ARMV8_PMULL     (1<<5)
# define ARMV8_SHA512    (1<<6)
# define ARMV8_CPUID     (1<<7)
# define ARMV8_RNG       (1<<8)
# define ARMV8_SM3       (1<<9)
# define ARMV8_SM4       (1<<10)
# define ARMV8_SHA3      (1<<11)
# define ARMV8_UNROLL8_EOR3      (1<<12)
# define ARMV8_SVE       (1<<13)
# define ARMV8_SVE2      (1<<14)
# define ARMV8_HAVE_SHA3_AND_WORTH_USING     (1<<15)
# define ARMV8_UNROLL12_EOR3     (1<<16)

/*
 * MIDR_EL1 system register
 *
 * 63___ _ ___32_31___ _ ___24_23_____20_19_____16_15__ _ __4_3_______0
 * |            |             |         |         |          |        |
 * |RES0        | Implementer | Variant | Arch    | PartNum  |Revision|
 * |____ _ _____|_____ _ _____|_________|_______ _|____ _ ___|________|
 *
 */

# define ARM_CPU_IMP_ARM           0x41
# define HISI_CPU_IMP              0x48
# define ARM_CPU_IMP_APPLE         0x61
# define ARM_CPU_IMP_MICROSOFT     0x6D
# define ARM_CPU_IMP_AMPERE        0xC0

# define ARM_CPU_PART_CORTEX_A72   0xD08
# define ARM_CPU_PART_N1           0xD0C
# define ARM_CPU_PART_V1           0xD40
# define ARM_CPU_PART_N2           0xD49
# define HISI_CPU_PART_KP920       0xD01
# define ARM_CPU_PART_V2           0xD4F

# define APPLE_CPU_PART_M1_ICESTORM         0x022
# define APPLE_CPU_PART_M1_FIRESTORM        0x023
# define APPLE_CPU_PART_M1_ICESTORM_PRO     0x024
# define APPLE_CPU_PART_M1_FIRESTORM_PRO    0x025
# define APPLE_CPU_PART_M1_ICESTORM_MAX     0x028
# define APPLE_CPU_PART_M1_FIRESTORM_MAX    0x029
# define APPLE_CPU_PART_M2_BLIZZARD         0x032
# define APPLE_CPU_PART_M2_AVALANCHE        0x033
# define APPLE_CPU_PART_M2_BLIZZARD_PRO     0x034
# define APPLE_CPU_PART_M2_AVALANCHE_PRO    0x035
# define APPLE_CPU_PART_M2_BLIZZARD_MAX     0x038
# define APPLE_CPU_PART_M2_AVALANCHE_MAX    0x039

# define MICROSOFT_CPU_PART_COBALT_100      0xD49

# define MIDR_PARTNUM_SHIFT       4
# define MIDR_PARTNUM_MASK        (0xfffU << MIDR_PARTNUM_SHIFT)
# define MIDR_PARTNUM(midr)       \
           (((midr) & MIDR_PARTNUM_MASK) >> MIDR_PARTNUM_SHIFT)

# define MIDR_IMPLEMENTER_SHIFT   24
# define MIDR_IMPLEMENTER_MASK    (0xffU << MIDR_IMPLEMENTER_SHIFT)
# define MIDR_IMPLEMENTER(midr)   \
           (((midr) & MIDR_IMPLEMENTER_MASK) >> MIDR_IMPLEMENTER_SHIFT)

# define MIDR_ARCHITECTURE_SHIFT  16
# define MIDR_ARCHITECTURE_MASK   (0xfU << MIDR_ARCHITECTURE_SHIFT)
# define MIDR_ARCHITECTURE(midr)  \
           (((midr) & MIDR_ARCHITECTURE_MASK) >> MIDR_ARCHITECTURE_SHIFT)

# define MIDR_CPU_MODEL_MASK \
           (MIDR_IMPLEMENTER_MASK | \
            MIDR_PARTNUM_MASK     | \
            MIDR_ARCHITECTURE_MASK)

# define MIDR_CPU_MODEL(imp, partnum) \
           (((imp)     << MIDR_IMPLEMENTER_SHIFT)  | \
            (0xfU      << MIDR_ARCHITECTURE_SHIFT) | \
            ((partnum) << MIDR_PARTNUM_SHIFT))

# define MIDR_IS_CPU_MODEL(midr, imp, partnum) \
           (((midr) & MIDR_CPU_MODEL_MASK) == MIDR_CPU_MODEL(imp, partnum))

#if defined(__ASSEMBLER__)

   /*
    * Support macros for
    *   - Armv8.3-A Pointer Authentication and
    *   - Armv8.5-A Branch Target Identification
    * features which require emitting a .note.gnu.property section with the
    * appropriate architecture-dependent feature bits set.
    * Read more: "ELF for the Arm® 64-bit Architecture"
    */

#  if defined(__ARM_FEATURE_BTI_DEFAULT) && __ARM_FEATURE_BTI_DEFAULT == 1
#   define GNU_PROPERTY_AARCH64_BTI (1 << 0)   /* Has Branch Target Identification */
#   define AARCH64_VALID_CALL_TARGET hint #34  /* BTI 'c' */
#  else
#   define GNU_PROPERTY_AARCH64_BTI 0  /* No Branch Target Identification */
#   define AARCH64_VALID_CALL_TARGET
#  endif

#  if defined(__ARM_FEATURE_PAC_DEFAULT) && \
       (__ARM_FEATURE_PAC_DEFAULT & 1) == 1  /* Signed with A-key */
#   define GNU_PROPERTY_AARCH64_POINTER_AUTH \
     (1 << 1)                                       /* Has Pointer Authentication */
#   define AARCH64_SIGN_LINK_REGISTER hint #25      /* PACIASP */
#   define AARCH64_VALIDATE_LINK_REGISTER hint #29  /* AUTIASP */
#  elif defined(__ARM_FEATURE_PAC_DEFAULT) && \
       (__ARM_FEATURE_PAC_DEFAULT & 2) == 2  /* Signed with B-key */
#   define GNU_PROPERTY_AARCH64_POINTER_AUTH \
     (1 << 1)                                       /* Has Pointer Authentication */
#   define AARCH64_SIGN_LINK_REGISTER hint #27      /* PACIBSP */
#   define AARCH64_VALIDATE_LINK_REGISTER hint #31  /* AUTIBSP */
#  else
#   define GNU_PROPERTY_AARCH64_POINTER_AUTH 0  /* No Pointer Authentication */
#   if GNU_PROPERTY_AARCH64_BTI != 0
#    define AARCH64_SIGN_LINK_REGISTER AARCH64_VALID_CALL_TARGET
#   else
#    define AARCH64_SIGN_LINK_REGISTER
#   endif
#   define AARCH64_VALIDATE_LINK_REGISTER
#  endif

#  if GNU_PROPERTY_AARCH64_POINTER_AUTH != 0 || GNU_PROPERTY_AARCH64_BTI != 0
    .pushsection .note.gnu.property, "a";
    .balign 8;
    .long 4;
    .long 0x10;
    .long 0x5;
    .asciz "GNU";
    .long 0xc0000000; /* GNU_PROPERTY_AARCH64_FEATURE_1_AND */
    .long 4;
    .long (GNU_PROPERTY_AARCH64_POINTER_AUTH | GNU_PROPERTY_AARCH64_BTI);
    .long 0;
    .popsection;
#  endif

# endif  /* defined __ASSEMBLER__ */

# define IS_CPU_SUPPORT_UNROLL8_EOR3() \
           (OPENSSL_armcap_P & ARMV8_UNROLL8_EOR3)

#endif
