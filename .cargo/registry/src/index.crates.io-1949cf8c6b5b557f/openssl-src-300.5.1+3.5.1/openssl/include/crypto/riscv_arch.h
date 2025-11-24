/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CRYPTO_RISCV_ARCH_H
# define OSSL_CRYPTO_RISCV_ARCH_H

# include <ctype.h>
# include <stdint.h>

# if defined(OPENSSL_SYS_LINUX) && !defined(FIPS_MODULE)
#  if __has_include(<asm/hwprobe.h>)
#   include <sys/syscall.h>
#   /*
     * Some environments using musl are reported to have the hwprobe.h include
     * file but not have the __NR_riscv_hwprobe define.
     */
#   ifdef __NR_riscv_hwprobe
#    define OSSL_RISCV_HWPROBE
#    include <asm/hwcap.h>
extern unsigned int OPENSSL_riscv_hwcap_P;
#    define VECTOR_CAPABLE (OPENSSL_riscv_hwcap_P & COMPAT_HWCAP_ISA_V)
#    define ZVX_MIN 15
#    define ZVX_MAX 23
#    define IS_IN_DEPEND_VECTOR(offset) ((ZVX_MIN >= offset) && (offset <= ZVX_MAX))
#   endif
#  endif
# endif

# define RISCV_DEFINE_CAP(NAME, INDEX, BIT_INDEX, \
                          HWPROBE_KEY, HWPROBE_VALUE) +1
extern uint32_t OPENSSL_riscvcap_P[ ((
# include "riscv_arch.def"
) + sizeof(uint32_t) - 1) / sizeof(uint32_t) ];

# ifdef OPENSSL_RISCVCAP_IMPL
#  define RISCV_DEFINE_CAP(NAME, INDEX, BIT_INDEX, \
                           HWPROBE_KEY, HWPROBE_VALUE) +1
uint32_t OPENSSL_riscvcap_P[ ((
#  include "riscv_arch.def"
) + sizeof(uint32_t) - 1) / sizeof(uint32_t) ];
# endif

# define RISCV_DEFINE_CAP(NAME, INDEX, BIT_INDEX,                   \
                          HWPROBE_KEY, HWPROBE_VALUE)               \
    static inline int RISCV_HAS_##NAME(void)                        \
    {                                                               \
        return (OPENSSL_riscvcap_P[INDEX] & (1 << BIT_INDEX)) != 0; \
    }
# include "riscv_arch.def"

struct RISCV_capability_s {
    const char *name;
    size_t index;
    size_t bit_offset;
# ifdef OSSL_RISCV_HWPROBE
    int32_t hwprobe_key;
    uint64_t hwprobe_value;
# endif
};

# define RISCV_DEFINE_CAP(NAME, INDEX, BIT_INDEX, \
                          OSSL_RISCV_HWPROBE_KEY, OSSL_RISCV_HWPROBE_VALUE) +1
extern const struct RISCV_capability_s RISCV_capabilities[
# include "riscv_arch.def"
];

# ifdef OPENSSL_RISCVCAP_IMPL
#  ifdef OSSL_RISCV_HWPROBE
#  define RISCV_DEFINE_CAP(NAME, INDEX, BIT_INDEX,     \
                           HWPROBE_KEY, HWPROBE_VALUE) \
    { #NAME, INDEX, BIT_INDEX, HWPROBE_KEY, HWPROBE_VALUE },
#  else
#  define RISCV_DEFINE_CAP(NAME, INDEX, BIT_INDEX,     \
                           HWPROBE_KEY, HWPROBE_VALUE) \
    { #NAME, INDEX, BIT_INDEX },
#  endif
const struct RISCV_capability_s RISCV_capabilities[] = {
#  include "riscv_arch.def"
};
# endif

# define RISCV_DEFINE_CAP(NAME, INDEX, BIT_INDEX, \
                          HWPROBE_KEY, HWPROBE_VALUE) +1
static const size_t kRISCVNumCaps =
# include "riscv_arch.def"
;

# ifdef OSSL_RISCV_HWPROBE
/*
 * Content is an array of { hwprobe_key, 0 } where
 * hwprobe_key is copied from asm/hwprobe.h.
 * It should be updated along with riscv_arch.def.
 */
#  define OSSL_RISCV_HWPROBE_PAIR_COUNT 1
#  define OSSL_RISCV_HWPROBE_PAIR_CONTENT \
   { 4, 0 },
# endif

/* Extension combination tests. */
#define RISCV_HAS_ZBB_AND_ZBC() (RISCV_HAS_ZBB() && RISCV_HAS_ZBC())
#define RISCV_HAS_ZBKB_AND_ZKND_AND_ZKNE() (RISCV_HAS_ZBKB() && RISCV_HAS_ZKND() && RISCV_HAS_ZKNE())
#define RISCV_HAS_ZKND_AND_ZKNE() (RISCV_HAS_ZKND() && RISCV_HAS_ZKNE())
/*
 * The ZVBB is the superset of ZVKB extension. We use macro here to replace the
 * `RISCV_HAS_ZVKB()` with `RISCV_HAS_ZVBB() || RISCV_HAS_ZVKB()`.
 */
#define RISCV_HAS_ZVKB() (RISCV_HAS_ZVBB() || RISCV_HAS_ZVKB())
#define RISCV_HAS_ZVKB_AND_ZVKNHA() (RISCV_HAS_ZVKB() && RISCV_HAS_ZVKNHA())
#define RISCV_HAS_ZVKB_AND_ZVKNHB() (RISCV_HAS_ZVKB() && RISCV_HAS_ZVKNHB())
#define RISCV_HAS_ZVKB_AND_ZVKSED() (RISCV_HAS_ZVKB() && RISCV_HAS_ZVKSED())
#define RISCV_HAS_ZVKB_AND_ZVKSH() (RISCV_HAS_ZVKB() && RISCV_HAS_ZVKSH())

/*
 * Get the size of a vector register in bits (VLEN).
 * If RISCV_HAS_V() is false, then this returns 0.
 */
size_t riscv_vlen(void);

#endif
