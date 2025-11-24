/*
 * Copyright 2011-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <openssl/crypto.h>
#ifdef __APPLE__
#include <sys/sysctl.h>
#else
#include <setjmp.h>
#include <signal.h>
#endif
#include "internal/cryptlib.h"
#ifdef _WIN32
#include <windows.h>
#else
#include <unistd.h>
#endif
#include "arm_arch.h"

unsigned int OPENSSL_armcap_P = 0;
unsigned int OPENSSL_arm_midr = 0;
unsigned int OPENSSL_armv8_rsa_neonized = 0;

#ifdef _WIN32
void OPENSSL_cpuid_setup(void)
{
    OPENSSL_armcap_P |= ARMV7_NEON;
    OPENSSL_armv8_rsa_neonized = 1;
    if (IsProcessorFeaturePresent(PF_ARM_V8_CRYPTO_INSTRUCTIONS_AVAILABLE)) {
        /* These are all covered by one call in Windows */
        OPENSSL_armcap_P |= ARMV8_AES;
        OPENSSL_armcap_P |= ARMV8_PMULL;
        OPENSSL_armcap_P |= ARMV8_SHA1;
        OPENSSL_armcap_P |= ARMV8_SHA256;
    }
}

uint32_t OPENSSL_rdtsc(void)
{
    return 0;
}
#elif __ARM_MAX_ARCH__ < 7
void OPENSSL_cpuid_setup(void)
{
}

uint32_t OPENSSL_rdtsc(void)
{
    return 0;
}
#else /* !_WIN32 && __ARM_MAX_ARCH__ >= 7 */

 /* 3 ways of handling things here: __APPLE__,  getauxval() or SIGILL detect */

 /* First determine if getauxval() is available (OSSL_IMPLEMENT_GETAUXVAL) */

# if defined(__GNUC__) && __GNUC__>=2
void OPENSSL_cpuid_setup(void) __attribute__ ((constructor));
# endif

# if defined(__GLIBC__) && defined(__GLIBC_PREREQ)
#  if __GLIBC_PREREQ(2, 16)
#   include <sys/auxv.h>
#   define OSSL_IMPLEMENT_GETAUXVAL
#  endif
# elif defined(__ANDROID_API__)
/* see https://developer.android.google.cn/ndk/guides/cpu-features */
#  if __ANDROID_API__ >= 18
#   include <sys/auxv.h>
#   define OSSL_IMPLEMENT_GETAUXVAL
#  endif
# endif
# if defined(__FreeBSD__) || defined(__OpenBSD__)
#  include <sys/param.h>
#  if (defined(__FreeBSD__) && __FreeBSD_version >= 1200000) || \
    (defined(__OpenBSD__) && OpenBSD >= 202409)
#   include <sys/auxv.h>
#   define OSSL_IMPLEMENT_GETAUXVAL

static unsigned long getauxval(unsigned long key)
{
  unsigned long val = 0ul;

  if (elf_aux_info((int)key, &val, sizeof(val)) != 0)
    return 0ul;

  return val;
}
#  endif
# endif

/*
 * Android: according to https://developer.android.com/ndk/guides/cpu-features,
 * getauxval is supported starting with API level 18
 */
# if defined(__ANDROID__) && defined(__ANDROID_API__) && __ANDROID_API__ >= 18
#  include <sys/auxv.h>
#  define OSSL_IMPLEMENT_GETAUXVAL
# endif

/*
 * ARM puts the feature bits for Crypto Extensions in AT_HWCAP2, whereas
 * AArch64 used AT_HWCAP.
 */
# ifndef AT_HWCAP
#  define AT_HWCAP               16
# endif
# ifndef AT_HWCAP2
#  define AT_HWCAP2              26
# endif
# if defined(__arm__) || defined (__arm)
#  define OSSL_HWCAP                  AT_HWCAP
#  define OSSL_HWCAP_NEON             (1 << 12)

#  define OSSL_HWCAP_CE               AT_HWCAP2
#  define OSSL_HWCAP_CE_AES           (1 << 0)
#  define OSSL_HWCAP_CE_PMULL         (1 << 1)
#  define OSSL_HWCAP_CE_SHA1          (1 << 2)
#  define OSSL_HWCAP_CE_SHA256        (1 << 3)
# elif defined(__aarch64__)
#  define OSSL_HWCAP                  AT_HWCAP
#  define OSSL_HWCAP_NEON             (1 << 1)

#  define OSSL_HWCAP_CE               AT_HWCAP
#  define OSSL_HWCAP_CE_AES           (1 << 3)
#  define OSSL_HWCAP_CE_PMULL         (1 << 4)
#  define OSSL_HWCAP_CE_SHA1          (1 << 5)
#  define OSSL_HWCAP_CE_SHA256        (1 << 6)
#  define OSSL_HWCAP_CPUID            (1 << 11)
#  define OSSL_HWCAP_SHA3             (1 << 17)
#  define OSSL_HWCAP_CE_SM3           (1 << 18)
#  define OSSL_HWCAP_CE_SM4           (1 << 19)
#  define OSSL_HWCAP_CE_SHA512        (1 << 21)
#  define OSSL_HWCAP_SVE              (1 << 22)
                                      /* AT_HWCAP2 */
#  define OSSL_HWCAP2                 26
#  define OSSL_HWCAP2_SVE2            (1 << 1)
#  define OSSL_HWCAP2_RNG             (1 << 16)
# endif

uint32_t _armv7_tick(void);

uint32_t OPENSSL_rdtsc(void)
{
    if (OPENSSL_armcap_P & ARMV7_TICK)
        return _armv7_tick();
    else
        return 0;
}

# ifdef __aarch64__
size_t OPENSSL_rndr_asm(unsigned char *buf, size_t len);
size_t OPENSSL_rndrrs_asm(unsigned char *buf, size_t len);

size_t OPENSSL_rndr_bytes(unsigned char *buf, size_t len);
size_t OPENSSL_rndrrs_bytes(unsigned char *buf, size_t len);

static size_t OPENSSL_rndr_wrapper(size_t (*func)(unsigned char *, size_t), unsigned char *buf, size_t len)
{
    size_t buffer_size = 0;
    int i;

    for (i = 0; i < 8; i++) {
        buffer_size = func(buf, len);
        if (buffer_size == len)
            break;
        usleep(5000);  /* 5000 microseconds (5 milliseconds) */
    }
    return buffer_size;
}

size_t OPENSSL_rndr_bytes(unsigned char *buf, size_t len)
{
    return OPENSSL_rndr_wrapper(OPENSSL_rndr_asm, buf, len);
}

size_t OPENSSL_rndrrs_bytes(unsigned char *buf, size_t len)
{
    return OPENSSL_rndr_wrapper(OPENSSL_rndrrs_asm, buf, len);
}
# endif

# if !defined(__APPLE__) && !defined(OSSL_IMPLEMENT_GETAUXVAL)
static sigset_t all_masked;

static sigjmp_buf ill_jmp;
static void ill_handler(int sig)
{
    siglongjmp(ill_jmp, sig);
}

/*
 * Following subroutines could have been inlined, but not all
 * ARM compilers support inline assembler, and we'd then have to
 * worry about the compiler optimising out the detection code...
 */
void _armv7_neon_probe(void);
void _armv8_aes_probe(void);
void _armv8_sha1_probe(void);
void _armv8_sha256_probe(void);
void _armv8_pmull_probe(void);
#  ifdef __aarch64__
void _armv8_sm3_probe(void);
void _armv8_sm4_probe(void);
void _armv8_sha512_probe(void);
void _armv8_eor3_probe(void);
void _armv8_sve_probe(void);
void _armv8_sve2_probe(void);
void _armv8_rng_probe(void);
#  endif
# endif /* !__APPLE__ && !OSSL_IMPLEMENT_GETAUXVAL */

/* We only call _armv8_cpuid_probe() if (OPENSSL_armcap_P & ARMV8_CPUID) != 0 */
unsigned int _armv8_cpuid_probe(void);

# if defined(__APPLE__)
/*
 * Checks the specified integer sysctl, returning `value` if it's 1, otherwise returning 0.
 */
static unsigned int sysctl_query(const char *name, unsigned int value)
{
    unsigned int sys_value = 0;
    size_t len = sizeof(sys_value);

    return (sysctlbyname(name, &sys_value, &len, NULL, 0) == 0 && sys_value == 1) ? value : 0;
}
# elif !defined(OSSL_IMPLEMENT_GETAUXVAL)
/*
 * Calls a provided probe function, which may SIGILL. If it doesn't, return `value`, otherwise return 0.
 */
static unsigned int arm_probe_for(void (*probe)(void), volatile unsigned int value)
{
    if (sigsetjmp(ill_jmp, 1) == 0) {
        probe();
        return value;
    } else {
        /* The probe function gave us SIGILL */
        return 0;
    }
}
# endif

void OPENSSL_cpuid_setup(void)
{
    const char *e;
# if !defined(__APPLE__) && !defined(OSSL_IMPLEMENT_GETAUXVAL)
    struct sigaction ill_oact, ill_act;
    sigset_t oset;
# endif
    static int trigger = 0;

    if (trigger)
        return;
    trigger = 1;

    OPENSSL_armcap_P = 0;

    if ((e = getenv("OPENSSL_armcap"))) {
        OPENSSL_armcap_P = (unsigned int)strtoul(e, NULL, 0);
        return;
    }

# if defined(__APPLE__)
#  if !defined(__aarch64__)
    /*
     * Capability probing by catching SIGILL appears to be problematic
     * on iOS. But since Apple universe is "monocultural", it's actually
     * possible to simply set pre-defined processor capability mask.
     */
    if (1) {
        OPENSSL_armcap_P = ARMV7_NEON;
        return;
    }
#  else
    {
        /*
         * From
         * https://github.com/llvm/llvm-project/blob/412237dcd07e5a2afbb1767858262a5f037149a3/llvm/lib/Target/AArch64/AArch64.td#L719
         * all of these have been available on 64-bit Apple Silicon from the
         * beginning (the A7).
         */
        OPENSSL_armcap_P |= ARMV7_NEON | ARMV8_PMULL | ARMV8_AES | ARMV8_SHA1 | ARMV8_SHA256;

        /* More recent extensions are indicated by sysctls */
        OPENSSL_armcap_P |= sysctl_query("hw.optional.armv8_2_sha512", ARMV8_SHA512);
        OPENSSL_armcap_P |= sysctl_query("hw.optional.armv8_2_sha3", ARMV8_SHA3);

        if (OPENSSL_armcap_P & ARMV8_SHA3) {
            char uarch[64];

            size_t len = sizeof(uarch);
            if ((sysctlbyname("machdep.cpu.brand_string", uarch, &len, NULL, 0) == 0) &&
               ((strncmp(uarch, "Apple M1", 8) == 0) ||
                (strncmp(uarch, "Apple M2", 8) == 0) ||
                (strncmp(uarch, "Apple M3", 8) == 0) ||
                (strncmp(uarch, "Apple M4", 8) == 0))) {
                OPENSSL_armcap_P |= ARMV8_UNROLL8_EOR3;
                OPENSSL_armcap_P |= ARMV8_HAVE_SHA3_AND_WORTH_USING;
            }
        }
    }
#  endif       /* __aarch64__ */

# elif defined(OSSL_IMPLEMENT_GETAUXVAL)

    if (getauxval(OSSL_HWCAP) & OSSL_HWCAP_NEON) {
        unsigned long hwcap = getauxval(OSSL_HWCAP_CE);

        OPENSSL_armcap_P |= ARMV7_NEON;

        if (hwcap & OSSL_HWCAP_CE_AES)
            OPENSSL_armcap_P |= ARMV8_AES;

        if (hwcap & OSSL_HWCAP_CE_PMULL)
            OPENSSL_armcap_P |= ARMV8_PMULL;

        if (hwcap & OSSL_HWCAP_CE_SHA1)
            OPENSSL_armcap_P |= ARMV8_SHA1;

        if (hwcap & OSSL_HWCAP_CE_SHA256)
            OPENSSL_armcap_P |= ARMV8_SHA256;

#  ifdef __aarch64__
        if (hwcap & OSSL_HWCAP_CE_SM4)
            OPENSSL_armcap_P |= ARMV8_SM4;

        if (hwcap & OSSL_HWCAP_CE_SHA512)
            OPENSSL_armcap_P |= ARMV8_SHA512;

        if (hwcap & OSSL_HWCAP_CPUID)
            OPENSSL_armcap_P |= ARMV8_CPUID;

        if (hwcap & OSSL_HWCAP_CE_SM3)
            OPENSSL_armcap_P |= ARMV8_SM3;
        if (hwcap & OSSL_HWCAP_SHA3)
            OPENSSL_armcap_P |= ARMV8_SHA3;
#  endif
    }
#  ifdef __aarch64__
        if (getauxval(OSSL_HWCAP) & OSSL_HWCAP_SVE)
            OPENSSL_armcap_P |= ARMV8_SVE;

        if (getauxval(OSSL_HWCAP2) & OSSL_HWCAP2_SVE2)
            OPENSSL_armcap_P |= ARMV8_SVE2;

        if (getauxval(OSSL_HWCAP2) & OSSL_HWCAP2_RNG)
            OPENSSL_armcap_P |= ARMV8_RNG;
#  endif

# else /* !__APPLE__ && !OSSL_IMPLEMENT_GETAUXVAL */

    /* If all else fails, do brute force SIGILL-based feature detection */

    sigfillset(&all_masked);
    sigdelset(&all_masked, SIGILL);
    sigdelset(&all_masked, SIGTRAP);
    sigdelset(&all_masked, SIGFPE);
    sigdelset(&all_masked, SIGBUS);
    sigdelset(&all_masked, SIGSEGV);

    memset(&ill_act, 0, sizeof(ill_act));
    ill_act.sa_handler = ill_handler;
    ill_act.sa_mask = all_masked;

    sigprocmask(SIG_SETMASK, &ill_act.sa_mask, &oset);
    sigaction(SIGILL, &ill_act, &ill_oact);

    OPENSSL_armcap_P |= arm_probe_for(_armv7_neon_probe, ARMV7_NEON);

    if (OPENSSL_armcap_P & ARMV7_NEON) {

        OPENSSL_armcap_P |= arm_probe_for(_armv8_pmull_probe, ARMV8_PMULL | ARMV8_AES);
        if (!(OPENSSL_armcap_P & ARMV8_AES)) {
            OPENSSL_armcap_P |= arm_probe_for(_armv8_aes_probe, ARMV8_AES);
        }

        OPENSSL_armcap_P |= arm_probe_for(_armv8_sha1_probe, ARMV8_SHA1);
        OPENSSL_armcap_P |= arm_probe_for(_armv8_sha256_probe, ARMV8_SHA256);

#  if defined(__aarch64__)
        OPENSSL_armcap_P |= arm_probe_for(_armv8_sm3_probe, ARMV8_SM3);
        OPENSSL_armcap_P |= arm_probe_for(_armv8_sm4_probe, ARMV8_SM4);
        OPENSSL_armcap_P |= arm_probe_for(_armv8_sha512_probe, ARMV8_SHA512);
        OPENSSL_armcap_P |= arm_probe_for(_armv8_eor3_probe, ARMV8_SHA3);
#  endif
    }
#  ifdef __aarch64__
    OPENSSL_armcap_P |= arm_probe_for(_armv8_sve_probe, ARMV8_SVE);
    OPENSSL_armcap_P |= arm_probe_for(_armv8_sve2_probe, ARMV8_SVE2);
    OPENSSL_armcap_P |= arm_probe_for(_armv8_rng_probe, ARMV8_RNG);
#  endif

    /*
     * Probing for ARMV7_TICK is known to produce unreliable results,
     * so we only use the feature when the user explicitly enables it
     * with OPENSSL_armcap.
     */

    sigaction(SIGILL, &ill_oact, NULL);
    sigprocmask(SIG_SETMASK, &oset, NULL);

# endif /* __APPLE__, OSSL_IMPLEMENT_GETAUXVAL */

# ifdef __aarch64__
    if (OPENSSL_armcap_P & ARMV8_CPUID)
        OPENSSL_arm_midr = _armv8_cpuid_probe();

    if ((MIDR_IS_CPU_MODEL(OPENSSL_arm_midr, ARM_CPU_IMP_ARM, ARM_CPU_PART_CORTEX_A72) ||
         MIDR_IS_CPU_MODEL(OPENSSL_arm_midr, ARM_CPU_IMP_ARM, ARM_CPU_PART_N1)) &&
        (OPENSSL_armcap_P & ARMV7_NEON)) {
            OPENSSL_armv8_rsa_neonized = 1;
    }
    if ((MIDR_IS_CPU_MODEL(OPENSSL_arm_midr, ARM_CPU_IMP_ARM, ARM_CPU_PART_V1) ||
         MIDR_IS_CPU_MODEL(OPENSSL_arm_midr, ARM_CPU_IMP_ARM, ARM_CPU_PART_N2) ||
         MIDR_IS_CPU_MODEL(OPENSSL_arm_midr, ARM_CPU_IMP_MICROSOFT, MICROSOFT_CPU_PART_COBALT_100) ||
         MIDR_IS_CPU_MODEL(OPENSSL_arm_midr, ARM_CPU_IMP_ARM, ARM_CPU_PART_V2) ||
         MIDR_IMPLEMENTER(OPENSSL_arm_midr) == ARM_CPU_IMP_AMPERE) &&
        (OPENSSL_armcap_P & ARMV8_SHA3))
        OPENSSL_armcap_P |= ARMV8_UNROLL8_EOR3;
    if ((MIDR_IS_CPU_MODEL(OPENSSL_arm_midr, ARM_CPU_IMP_ARM, ARM_CPU_PART_V1) ||
         MIDR_IS_CPU_MODEL(OPENSSL_arm_midr, ARM_CPU_IMP_ARM, ARM_CPU_PART_V2) ||
         MIDR_IMPLEMENTER(OPENSSL_arm_midr) == ARM_CPU_IMP_AMPERE) &&
        (OPENSSL_armcap_P & ARMV8_SHA3))
        OPENSSL_armcap_P |= ARMV8_UNROLL12_EOR3;
    if ((MIDR_IS_CPU_MODEL(OPENSSL_arm_midr, ARM_CPU_IMP_APPLE, APPLE_CPU_PART_M1_FIRESTORM)     ||
         MIDR_IS_CPU_MODEL(OPENSSL_arm_midr, ARM_CPU_IMP_APPLE, APPLE_CPU_PART_M1_ICESTORM)      ||
         MIDR_IS_CPU_MODEL(OPENSSL_arm_midr, ARM_CPU_IMP_APPLE, APPLE_CPU_PART_M1_FIRESTORM_PRO) ||
         MIDR_IS_CPU_MODEL(OPENSSL_arm_midr, ARM_CPU_IMP_APPLE, APPLE_CPU_PART_M1_ICESTORM_PRO)  ||
         MIDR_IS_CPU_MODEL(OPENSSL_arm_midr, ARM_CPU_IMP_APPLE, APPLE_CPU_PART_M1_FIRESTORM_MAX) ||
         MIDR_IS_CPU_MODEL(OPENSSL_arm_midr, ARM_CPU_IMP_APPLE, APPLE_CPU_PART_M1_ICESTORM_MAX)  ||
         MIDR_IS_CPU_MODEL(OPENSSL_arm_midr, ARM_CPU_IMP_APPLE, APPLE_CPU_PART_M2_AVALANCHE)     ||
         MIDR_IS_CPU_MODEL(OPENSSL_arm_midr, ARM_CPU_IMP_APPLE, APPLE_CPU_PART_M2_BLIZZARD)      ||
         MIDR_IS_CPU_MODEL(OPENSSL_arm_midr, ARM_CPU_IMP_APPLE, APPLE_CPU_PART_M2_AVALANCHE_PRO) ||
         MIDR_IS_CPU_MODEL(OPENSSL_arm_midr, ARM_CPU_IMP_APPLE, APPLE_CPU_PART_M2_BLIZZARD_PRO)  ||
         MIDR_IS_CPU_MODEL(OPENSSL_arm_midr, ARM_CPU_IMP_APPLE, APPLE_CPU_PART_M2_AVALANCHE_MAX) ||
         MIDR_IS_CPU_MODEL(OPENSSL_arm_midr, ARM_CPU_IMP_APPLE, APPLE_CPU_PART_M2_BLIZZARD_MAX)) &&
        (OPENSSL_armcap_P & ARMV8_SHA3))
        OPENSSL_armcap_P |= ARMV8_HAVE_SHA3_AND_WORTH_USING;
# endif
}
#endif /* _WIN32, __ARM_MAX_ARCH__ >= 7 */
