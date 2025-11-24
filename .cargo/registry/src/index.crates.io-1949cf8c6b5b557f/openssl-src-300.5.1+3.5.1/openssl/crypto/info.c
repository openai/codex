/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/crypto.h>
#include "crypto/rand.h"
#include "crypto/dso_conf.h"
#include "internal/thread_once.h"
#include "internal/cryptlib.h"
#include "internal/e_os.h"
#include "buildinf.h"

#ifndef OPENSSL_NO_JITTER
# include <stdio.h>
# include <jitterentropy.h>
#endif

#if defined(__arm__) || defined(__arm) || defined(__aarch64__)
# include "arm_arch.h"
# define CPU_INFO_STR_LEN 128
#elif defined(__s390__) || defined(__s390x__)
# include "s390x_arch.h"
# define CPU_INFO_STR_LEN 2048
#elif defined(__riscv)
# include "crypto/riscv_arch.h"
# define CPU_INFO_STR_LEN 2048
#else
# define CPU_INFO_STR_LEN 256
#endif

/* extern declaration to avoid warning */
extern char ossl_cpu_info_str[];

static char *seed_sources = NULL;

char ossl_cpu_info_str[CPU_INFO_STR_LEN] = "";
#define CPUINFO_PREFIX "CPUINFO: "

static CRYPTO_ONCE init_info = CRYPTO_ONCE_STATIC_INIT;

DEFINE_RUN_ONCE_STATIC(init_info_strings)
{
#if defined(OPENSSL_CPUID_OBJ)
# if defined(__i386)   || defined(__i386__)   || defined(_M_IX86) || \
     defined(__x86_64) || defined(__x86_64__) || \
     defined(_M_AMD64) || defined(_M_X64)
    const char *env;

    BIO_snprintf(ossl_cpu_info_str, sizeof(ossl_cpu_info_str),
                 CPUINFO_PREFIX "OPENSSL_ia32cap=0x%.16llx:0x%.16llx:0x%.16llx:0x%.16llx:0x%.16llx",
                 (unsigned long long)OPENSSL_ia32cap_P[0] |
                 (unsigned long long)OPENSSL_ia32cap_P[1] << 32,
                 (unsigned long long)OPENSSL_ia32cap_P[2] |
                 (unsigned long long)OPENSSL_ia32cap_P[3] << 32,
                 (unsigned long long)OPENSSL_ia32cap_P[4] |
                 (unsigned long long)OPENSSL_ia32cap_P[5] << 32,
                 (unsigned long long)OPENSSL_ia32cap_P[6] |
                 (unsigned long long)OPENSSL_ia32cap_P[7] << 32,
                 (unsigned long long)OPENSSL_ia32cap_P[8] |
                 (unsigned long long)OPENSSL_ia32cap_P[9] << 32);

    if ((env = getenv("OPENSSL_ia32cap")) != NULL)
        BIO_snprintf(ossl_cpu_info_str + strlen(ossl_cpu_info_str),
                     sizeof(ossl_cpu_info_str) - strlen(ossl_cpu_info_str),
                     " env:%s", env);
# elif defined(__arm__) || defined(__arm) || defined(__aarch64__)
    const char *env;

    BIO_snprintf(ossl_cpu_info_str, sizeof(ossl_cpu_info_str),
                 CPUINFO_PREFIX "OPENSSL_armcap=0x%x", OPENSSL_armcap_P);
    if ((env = getenv("OPENSSL_armcap")) != NULL)
        BIO_snprintf(ossl_cpu_info_str + strlen(ossl_cpu_info_str),
                     sizeof(ossl_cpu_info_str) - strlen(ossl_cpu_info_str),
                     " env:%s", env);
# elif defined(__s390__) || defined(__s390x__)
    const char *env;

    BIO_snprintf(ossl_cpu_info_str, sizeof(ossl_cpu_info_str),
                 CPUINFO_PREFIX "OPENSSL_s390xcap="
                 "stfle:0x%llx:0x%llx:0x%llx:0x%llx:"
                 "kimd:0x%llx:0x%llx:"
                 "klmd:0x%llx:0x%llx:"
                 "km:0x%llx:0x%llx:"
                 "kmc:0x%llx:0x%llx:"
                 "kmac:0x%llx:0x%llx:"
                 "kmctr:0x%llx:0x%llx:"
                 "kmo:0x%llx:0x%llx:"
                 "kmf:0x%llx:0x%llx:"
                 "prno:0x%llx:0x%llx:"
                 "kma:0x%llx:0x%llx:"
                 "pcc:0x%llx:0x%llx:"
                 "kdsa:0x%llx:0x%llx",
                 OPENSSL_s390xcap_P.stfle[0], OPENSSL_s390xcap_P.stfle[1],
                 OPENSSL_s390xcap_P.stfle[2], OPENSSL_s390xcap_P.stfle[3],
                 OPENSSL_s390xcap_P.kimd[0], OPENSSL_s390xcap_P.kimd[1],
                 OPENSSL_s390xcap_P.klmd[0], OPENSSL_s390xcap_P.klmd[1],
                 OPENSSL_s390xcap_P.km[0], OPENSSL_s390xcap_P.km[1],
                 OPENSSL_s390xcap_P.kmc[0], OPENSSL_s390xcap_P.kmc[1],
                 OPENSSL_s390xcap_P.kmac[0], OPENSSL_s390xcap_P.kmac[1],
                 OPENSSL_s390xcap_P.kmctr[0], OPENSSL_s390xcap_P.kmctr[1],
                 OPENSSL_s390xcap_P.kmo[0], OPENSSL_s390xcap_P.kmo[1],
                 OPENSSL_s390xcap_P.kmf[0], OPENSSL_s390xcap_P.kmf[1],
                 OPENSSL_s390xcap_P.prno[0], OPENSSL_s390xcap_P.prno[1],
                 OPENSSL_s390xcap_P.kma[0], OPENSSL_s390xcap_P.kma[1],
                 OPENSSL_s390xcap_P.pcc[0], OPENSSL_s390xcap_P.pcc[1],
                 OPENSSL_s390xcap_P.kdsa[0], OPENSSL_s390xcap_P.kdsa[1]);
    if ((env = getenv("OPENSSL_s390xcap")) != NULL)
        BIO_snprintf(ossl_cpu_info_str + strlen(ossl_cpu_info_str),
                     sizeof(ossl_cpu_info_str) - strlen(ossl_cpu_info_str),
                     " env:%s", env);
# elif defined(__riscv)
    const char *env;
    char sep = '=';

    BIO_snprintf(ossl_cpu_info_str, sizeof(ossl_cpu_info_str),
                 CPUINFO_PREFIX "OPENSSL_riscvcap");
    for (size_t i = 0; i < kRISCVNumCaps; ++i) {
        if (OPENSSL_riscvcap_P[RISCV_capabilities[i].index]
                & (1 << RISCV_capabilities[i].bit_offset)) {
            /* Match, display the name */
            BIO_snprintf(ossl_cpu_info_str + strlen(ossl_cpu_info_str),
                         sizeof(ossl_cpu_info_str) - strlen(ossl_cpu_info_str),
                         "%c%s", sep, RISCV_capabilities[i].name);
            /* Only the first sep is '=' */
            sep = '_';
        }
    }
    /* If no capability is found, add back the = */
    if (sep == '=') {
        BIO_snprintf(ossl_cpu_info_str + strlen(ossl_cpu_info_str),
                     sizeof(ossl_cpu_info_str) - strlen(ossl_cpu_info_str),
                     "%c", sep);
    }
    if ((env = getenv("OPENSSL_riscvcap")) != NULL)
        BIO_snprintf(ossl_cpu_info_str + strlen(ossl_cpu_info_str),
                     sizeof(ossl_cpu_info_str) - strlen(ossl_cpu_info_str),
                     " env:%s", env);
# endif
#endif

    {
        static char seeds[512] = "";

#define add_seeds_string(str)                                           \
        do {                                                            \
            if (seeds[0] != '\0')                                       \
                OPENSSL_strlcat(seeds, " ", sizeof(seeds));             \
            OPENSSL_strlcat(seeds, str, sizeof(seeds));                 \
        } while (0)
#define add_seeds_stringlist(label, strlist)                            \
        do {                                                            \
            add_seeds_string(label "(");                                \
            {                                                           \
                const char *dev[] =  { strlist, NULL };                 \
                const char **p;                                         \
                int first = 1;                                          \
                                                                        \
                for (p = dev; *p != NULL; p++) {                        \
                    if (!first)                                         \
                        OPENSSL_strlcat(seeds, " ", sizeof(seeds));     \
                    first = 0;                                          \
                    OPENSSL_strlcat(seeds, *p, sizeof(seeds));          \
                }                                                       \
            }                                                           \
            OPENSSL_strlcat(seeds, ")", sizeof(seeds));                 \
        } while (0)

#ifdef OPENSSL_RAND_SEED_NONE
        add_seeds_string("none");
#endif
#ifdef OPENSSL_RAND_SEED_RDTSC
        add_seeds_string("rdtsc");
#endif
#ifdef OPENSSL_RAND_SEED_RDCPU
# ifdef __aarch64__
        add_seeds_string("rndr ( rndrrs rndr )");
# else
        add_seeds_string("rdrand ( rdseed rdrand )");
# endif
#endif
#ifdef OPENSSL_RAND_SEED_GETRANDOM
        add_seeds_string("getrandom-syscall");
#endif
#ifdef OPENSSL_RAND_SEED_DEVRANDOM
        add_seeds_stringlist("random-device", DEVRANDOM);
#endif
#ifdef OPENSSL_RAND_SEED_EGD
        add_seeds_stringlist("EGD", DEVRANDOM_EGD);
#endif
#ifdef OPENSSL_RAND_SEED_OS
        add_seeds_string("os-specific");
#endif
#ifndef OPENSSL_NO_JITTER
        {
            char buf[32];

            BIO_snprintf(buf, sizeof(buf), "JITTER (%d)", jent_version());
            add_seeds_string(buf);
        }
#endif
        seed_sources = seeds;
    }
    return 1;
}

const char *OPENSSL_info(int t)
{
    /*
     * We don't care about the result.  Worst case scenario, the strings
     * won't be initialised, i.e. remain NULL, which means that the info
     * isn't available anyway...
     */
    (void)RUN_ONCE(&init_info, init_info_strings);

    switch (t) {
    case OPENSSL_INFO_CONFIG_DIR:
        return ossl_get_openssldir();
    case OPENSSL_INFO_ENGINES_DIR:
        return ossl_get_enginesdir();
    case OPENSSL_INFO_MODULES_DIR:
        return ossl_get_modulesdir();
    case OPENSSL_INFO_DSO_EXTENSION:
        return DSO_EXTENSION;
    case OPENSSL_INFO_DIR_FILENAME_SEPARATOR:
#if defined(_WIN32)
        return "\\";
#elif defined(__VMS)
        return "";
#else  /* Assume POSIX */
        return "/";
#endif
    case OPENSSL_INFO_LIST_SEPARATOR:
        {
            static const char list_sep[] = { LIST_SEPARATOR_CHAR, '\0' };
            return list_sep;
        }
    case OPENSSL_INFO_SEED_SOURCE:
        return seed_sources;
    case OPENSSL_INFO_CPU_SETTINGS:
        /*
         * If successfully initialized, ossl_cpu_info_str will start
         * with CPUINFO_PREFIX, if failed it will be an empty string.
         * Strip away the CPUINFO_PREFIX which we don't need here.
         */
        if (ossl_cpu_info_str[0] != '\0')
            return ossl_cpu_info_str + strlen(CPUINFO_PREFIX);
        break;
    case OPENSSL_INFO_WINDOWS_CONTEXT:
        return ossl_get_wininstallcontext();
    default:
        break;
    }
    /* Not an error */
    return NULL;
}
