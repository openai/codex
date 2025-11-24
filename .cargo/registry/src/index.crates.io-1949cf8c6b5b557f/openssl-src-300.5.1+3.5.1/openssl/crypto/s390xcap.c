/*
 * Copyright 2010-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <setjmp.h>
#include <signal.h>
#include "internal/cryptlib.h"
#include "crypto/ctype.h"
#include "s390x_arch.h"

#if defined(OPENSSL_SYS_LINUX) && !defined(FIPS_MODULE)
# include <sys/types.h>
# include <sys/stat.h>
# include <fcntl.h>
# include <asm/zcrypt.h>
# include <sys/ioctl.h>
# include <unistd.h>
#endif

#if defined(__GLIBC__) && defined(__GLIBC_PREREQ)
# if __GLIBC_PREREQ(2, 16)
#  include <sys/auxv.h>
#  if defined(HWCAP_S390_STFLE) && defined(HWCAP_S390_VX)
#   define OSSL_IMPLEMENT_GETAUXVAL
#  endif
# endif
#endif

#define LEN     128
#define STR_(S) #S
#define STR(S)  STR_(S)

#define TOK_FUNC(NAME)                                                  \
    (sscanf(tok_begin,                                                  \
            " " STR(NAME) " : %" STR(LEN) "[^:] : "                     \
            "%" STR(LEN) "s %" STR(LEN) "s ",                           \
            tok[0], tok[1], tok[2]) == 2) {                             \
                                                                        \
        off = (tok[0][0] == '~') ? 1 : 0;                               \
        if (sscanf(tok[0] + off, "%llx", &cap->NAME[0]) != 1)           \
            goto ret;                                                   \
        if (off)                                                        \
            cap->NAME[0] = ~cap->NAME[0];                               \
                                                                        \
        off = (tok[1][0] == '~') ? 1 : 0;                               \
        if (sscanf(tok[1] + off, "%llx", &cap->NAME[1]) != 1)           \
            goto ret;                                                   \
        if (off)                                                        \
            cap->NAME[1] = ~cap->NAME[1];                               \
    }

#define TOK_CPU_ALIAS(NAME, STRUCT_NAME)                                \
    (sscanf(tok_begin,                                                  \
            " %" STR(LEN) "s %" STR(LEN) "s ",                          \
            tok[0], tok[1]) == 1                                        \
     && !strcmp(tok[0], #NAME)) {                                       \
            memcpy(cap, &STRUCT_NAME, sizeof(*cap));                    \
    }

#define TOK_CPU(NAME) TOK_CPU_ALIAS(NAME, NAME)

#ifndef OSSL_IMPLEMENT_GETAUXVAL
static sigjmp_buf ill_jmp;
static void ill_handler(int sig)
{
    siglongjmp(ill_jmp, sig);
}

void OPENSSL_vx_probe(void);
#endif

static const char *env;
static int parse_env(struct OPENSSL_s390xcap_st *cap, int *cex);

void OPENSSL_s390x_facilities(void);
void OPENSSL_s390x_functions(void);

struct OPENSSL_s390xcap_st OPENSSL_s390xcap_P;

#ifdef S390X_MOD_EXP
int OPENSSL_s390xcex;
int OPENSSL_s390xcex_nodev;

#if defined(__GNUC__)
__attribute__ ((visibility("hidden")))
#endif
void OPENSSL_s390x_cleanup(void);

#if defined(__GNUC__)
__attribute__ ((visibility("hidden")))
#endif
void OPENSSL_s390x_cleanup(void)
{
    if (OPENSSL_s390xcex != -1) {
        (void)close(OPENSSL_s390xcex);
        OPENSSL_s390xcex = -1;
    }
}
#endif

#if defined(__GNUC__) && defined(__linux)
__attribute__ ((visibility("hidden")))
#endif
void OPENSSL_cpuid_setup(void)
{
    struct OPENSSL_s390xcap_st cap;
    int cex = 1;

    if (OPENSSL_s390xcap_P.stfle[0])
        return;

    /* set a bit that will not be tested later */
    OPENSSL_s390xcap_P.stfle[0] |= S390X_CAPBIT(0);

#if defined(OSSL_IMPLEMENT_GETAUXVAL)
    {
        const unsigned long hwcap = getauxval(AT_HWCAP);

        /* protection against missing store-facility-list-extended */
        if (hwcap & HWCAP_S390_STFLE)
            OPENSSL_s390x_facilities();

        /* protection against disabled vector facility */
        if (!(hwcap & HWCAP_S390_VX)) {
            OPENSSL_s390xcap_P.stfle[2] &= ~(S390X_CAPBIT(S390X_VX)
                                             | S390X_CAPBIT(S390X_VXD)
                                             | S390X_CAPBIT(S390X_VXE));
        }
    }
#else
    {
        sigset_t oset;
        struct sigaction ill_act, oact_ill, oact_fpe;

        memset(&ill_act, 0, sizeof(ill_act));
        ill_act.sa_handler = ill_handler;
        sigfillset(&ill_act.sa_mask);
        sigdelset(&ill_act.sa_mask, SIGILL);
        sigdelset(&ill_act.sa_mask, SIGFPE);
        sigdelset(&ill_act.sa_mask, SIGTRAP);

        sigprocmask(SIG_SETMASK, &ill_act.sa_mask, &oset);
        sigaction(SIGILL, &ill_act, &oact_ill);
        sigaction(SIGFPE, &ill_act, &oact_fpe);

        /* protection against missing store-facility-list-extended */
        if (sigsetjmp(ill_jmp, 1) == 0)
            OPENSSL_s390x_facilities();

        /* protection against disabled vector facility */
        if ((OPENSSL_s390xcap_P.stfle[2] & S390X_CAPBIT(S390X_VX))
            && (sigsetjmp(ill_jmp, 1) == 0)) {
            OPENSSL_vx_probe();
        } else {
            OPENSSL_s390xcap_P.stfle[2] &= ~(S390X_CAPBIT(S390X_VX)
                                             | S390X_CAPBIT(S390X_VXD)
                                             | S390X_CAPBIT(S390X_VXE));
        }

        sigaction(SIGFPE, &oact_fpe, NULL);
        sigaction(SIGILL, &oact_ill, NULL);
        sigprocmask(SIG_SETMASK, &oset, NULL);
    }
#endif

    env = getenv("OPENSSL_s390xcap");
    if (env != NULL) {
        if (!parse_env(&cap, &cex))
            env = NULL;
    }

    if (env != NULL) {
        OPENSSL_s390xcap_P.stfle[0] &= cap.stfle[0];
        OPENSSL_s390xcap_P.stfle[1] &= cap.stfle[1];
        OPENSSL_s390xcap_P.stfle[2] &= cap.stfle[2];
    }

    OPENSSL_s390x_functions(); /* check OPENSSL_s390xcap_P.stfle */

    if (env != NULL) {
        OPENSSL_s390xcap_P.kimd[0] &= cap.kimd[0];
        OPENSSL_s390xcap_P.kimd[1] &= cap.kimd[1];
        OPENSSL_s390xcap_P.klmd[0] &= cap.klmd[0];
        OPENSSL_s390xcap_P.klmd[1] &= cap.klmd[1];
        OPENSSL_s390xcap_P.km[0] &= cap.km[0];
        OPENSSL_s390xcap_P.km[1] &= cap.km[1];
        OPENSSL_s390xcap_P.kmc[0] &= cap.kmc[0];
        OPENSSL_s390xcap_P.kmc[1] &= cap.kmc[1];
        OPENSSL_s390xcap_P.kmac[0] &= cap.kmac[0];
        OPENSSL_s390xcap_P.kmac[1] &= cap.kmac[1];
        OPENSSL_s390xcap_P.kmctr[0] &= cap.kmctr[0];
        OPENSSL_s390xcap_P.kmctr[1] &= cap.kmctr[1];
        OPENSSL_s390xcap_P.kmo[0] &= cap.kmo[0];
        OPENSSL_s390xcap_P.kmo[1] &= cap.kmo[1];
        OPENSSL_s390xcap_P.kmf[0] &= cap.kmf[0];
        OPENSSL_s390xcap_P.kmf[1] &= cap.kmf[1];
        OPENSSL_s390xcap_P.prno[0] &= cap.prno[0];
        OPENSSL_s390xcap_P.prno[1] &= cap.prno[1];
        OPENSSL_s390xcap_P.kma[0] &= cap.kma[0];
        OPENSSL_s390xcap_P.kma[1] &= cap.kma[1];
        OPENSSL_s390xcap_P.pcc[0] &= cap.pcc[0];
        OPENSSL_s390xcap_P.pcc[1] &= cap.pcc[1];
        OPENSSL_s390xcap_P.kdsa[0] &= cap.kdsa[0];
        OPENSSL_s390xcap_P.kdsa[1] &= cap.kdsa[1];
    }

#ifdef S390X_MOD_EXP
    if (cex == 0) {
        OPENSSL_s390xcex = -1;
    } else {
        OPENSSL_s390xcex = open("/dev/z90crypt", O_RDWR | O_CLOEXEC);
        OPENSSL_atexit(OPENSSL_s390x_cleanup);
    }
    OPENSSL_s390xcex_nodev = 0;
#endif
}

static int parse_env(struct OPENSSL_s390xcap_st *cap, int *cex)
{
    /*-
     * CPU model data
     * (only the STFLE- and QUERY-bits relevant to libcrypto are set)
     */

    /*-
     * z900 (2000) - z/Architecture POP SA22-7832-00
     * Facility detection would fail on real hw (no STFLE).
     */
    static const struct OPENSSL_s390xcap_st z900 = {
        /*.stfle  = */{0ULL, 0ULL, 0ULL, 0ULL},
        /*.kimd   = */{0ULL, 0ULL},
        /*.klmd   = */{0ULL, 0ULL},
        /*.km     = */{0ULL, 0ULL},
        /*.kmc    = */{0ULL, 0ULL},
        /*.kmac   = */{0ULL, 0ULL},
        /*.kmctr  = */{0ULL, 0ULL},
        /*.kmo    = */{0ULL, 0ULL},
        /*.kmf    = */{0ULL, 0ULL},
        /*.prno   = */{0ULL, 0ULL},
        /*.kma    = */{0ULL, 0ULL},
        /*.pcc    = */{0ULL, 0ULL},
        /*.kdsa   = */{0ULL, 0ULL},
    };

    /*-
     * z990 (2003) - z/Architecture POP SA22-7832-02
     * Implements MSA. Facility detection would fail on real hw (no STFLE).
     */
    static const struct OPENSSL_s390xcap_st z990 = {
        /*.stfle  = */{S390X_CAPBIT(S390X_MSA),
                       0ULL, 0ULL, 0ULL},
        /*.kimd   = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_SHA_1),
                       0ULL},
        /*.klmd   = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_SHA_1),
                       0ULL},
        /*.km     = */{S390X_CAPBIT(S390X_QUERY),
                       0ULL},
        /*.kmc    = */{S390X_CAPBIT(S390X_QUERY),
                       0ULL},
        /*.kmac   = */{S390X_CAPBIT(S390X_QUERY),
                       0ULL},
        /*.kmctr  = */{0ULL, 0ULL},
        /*.kmo    = */{0ULL, 0ULL},
        /*.kmf    = */{0ULL, 0ULL},
        /*.prno   = */{0ULL, 0ULL},
        /*.kma    = */{0ULL, 0ULL},
        /*.pcc    = */{0ULL, 0ULL},
        /*.kdsa   = */{0ULL, 0ULL},
    };

    /*-
     * z9 (2005) - z/Architecture POP SA22-7832-04
     * Implements MSA and MSA1.
     */
    static const struct OPENSSL_s390xcap_st z9 = {
        /*.stfle  = */{S390X_CAPBIT(S390X_MSA)
                       | S390X_CAPBIT(S390X_STCKF),
                       0ULL, 0ULL, 0ULL},
        /*.kimd   = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_SHA_1)
                       | S390X_CAPBIT(S390X_SHA_256),
                       0ULL},
        /*.klmd   = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_SHA_1)
                       | S390X_CAPBIT(S390X_SHA_256),
                       0ULL},
        /*.km     = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128),
                       0ULL},
        /*.kmc    = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128),
                       0ULL},
        /*.kmac   = */{S390X_CAPBIT(S390X_QUERY),
                       0ULL},
        /*.kmctr  = */{0ULL, 0ULL},
        /*.kmo    = */{0ULL, 0ULL},
        /*.kmf    = */{0ULL, 0ULL},
        /*.prno   = */{0ULL, 0ULL},
        /*.kma    = */{0ULL, 0ULL},
        /*.pcc    = */{0ULL, 0ULL},
        /*.kdsa   = */{0ULL, 0ULL},
    };

    /*-
     * z10 (2008) - z/Architecture POP SA22-7832-06
     * Implements MSA and MSA1-2.
     */
    static const struct OPENSSL_s390xcap_st z10 = {
        /*.stfle  = */{S390X_CAPBIT(S390X_MSA)
                       | S390X_CAPBIT(S390X_STCKF),
                       0ULL, 0ULL, 0ULL},
        /*.kimd   = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_SHA_1)
                       | S390X_CAPBIT(S390X_SHA_256)
                       | S390X_CAPBIT(S390X_SHA_512),
                       0ULL},
        /*.klmd   = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_SHA_1)
                       | S390X_CAPBIT(S390X_SHA_256)
                       | S390X_CAPBIT(S390X_SHA_512),
                       0ULL},
        /*.km     = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.kmc    = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.kmac   = */{S390X_CAPBIT(S390X_QUERY),
                       0ULL},
        /*.kmctr  = */{0ULL, 0ULL},
        /*.kmo    = */{0ULL, 0ULL},
        /*.kmf    = */{0ULL, 0ULL},
        /*.prno   = */{0ULL, 0ULL},
        /*.kma    = */{0ULL, 0ULL},
        /*.pcc    = */{0ULL, 0ULL},
        /*.kdsa   = */{0ULL, 0ULL},
    };

    /*-
     * z196 (2010) - z/Architecture POP SA22-7832-08
     * Implements MSA and MSA1-4.
     */
    static const struct OPENSSL_s390xcap_st z196 = {
        /*.stfle  = */{S390X_CAPBIT(S390X_MSA)
                       | S390X_CAPBIT(S390X_STCKF),
                       S390X_CAPBIT(S390X_MSA3)
                       | S390X_CAPBIT(S390X_MSA4),
                       0ULL, 0ULL},
        /*.kimd   = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_SHA_1)
                       | S390X_CAPBIT(S390X_SHA_256)
                       | S390X_CAPBIT(S390X_SHA_512),
                       S390X_CAPBIT(S390X_GHASH)},
        /*.klmd   = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_SHA_1)
                       | S390X_CAPBIT(S390X_SHA_256)
                       | S390X_CAPBIT(S390X_SHA_512),
                       0ULL},
        /*.km     = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256)
                       | S390X_CAPBIT(S390X_XTS_AES_128)
                       | S390X_CAPBIT(S390X_XTS_AES_256),
                       0ULL},
        /*.kmc    = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.kmac   = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.kmctr  = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.kmo    = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.kmf    = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.prno   = */{0ULL, 0ULL},
        /*.kma    = */{0ULL, 0ULL},
        /*.pcc    = */{S390X_CAPBIT(S390X_QUERY),
                       0ULL},
        /*.kdsa   = */{0ULL, 0ULL},
    };

    /*-
     * zEC12 (2012) - z/Architecture POP SA22-7832-09
     * Implements MSA and MSA1-4.
     */
    static const struct OPENSSL_s390xcap_st zEC12 = {
        /*.stfle  = */{S390X_CAPBIT(S390X_MSA)
                       | S390X_CAPBIT(S390X_STCKF),
                       S390X_CAPBIT(S390X_MSA3)
                       | S390X_CAPBIT(S390X_MSA4),
                       0ULL, 0ULL},
        /*.kimd   = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_SHA_1)
                       | S390X_CAPBIT(S390X_SHA_256)
                       | S390X_CAPBIT(S390X_SHA_512),
                   S390X_CAPBIT(S390X_GHASH)},
        /*.klmd   = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_SHA_1)
                       | S390X_CAPBIT(S390X_SHA_256)
                       | S390X_CAPBIT(S390X_SHA_512),
                       0ULL},
        /*.km     = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256)
                       | S390X_CAPBIT(S390X_XTS_AES_128)
                       | S390X_CAPBIT(S390X_XTS_AES_256),
                       0ULL},
        /*.kmc    = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.kmac   = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.kmctr  = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.kmo    = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.kmf    = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.prno   = */{0ULL, 0ULL},
        /*.kma    = */{0ULL, 0ULL},
        /*.pcc    = */{S390X_CAPBIT(S390X_QUERY),
                       0ULL},
        /*.kdsa   = */{0ULL, 0ULL},
    };

    /*-
     * z13 (2015) - z/Architecture POP SA22-7832-10
     * Implements MSA and MSA1-5.
     */
    static const struct OPENSSL_s390xcap_st z13 = {
        /*.stfle  = */{S390X_CAPBIT(S390X_MSA)
                       | S390X_CAPBIT(S390X_STCKF)
                       | S390X_CAPBIT(S390X_MSA5),
                       S390X_CAPBIT(S390X_MSA3)
                       | S390X_CAPBIT(S390X_MSA4),
                       S390X_CAPBIT(S390X_VX),
                       0ULL},
        /*.kimd   = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_SHA_1)
                       | S390X_CAPBIT(S390X_SHA_256)
                       | S390X_CAPBIT(S390X_SHA_512),
                       S390X_CAPBIT(S390X_GHASH)},
        /*.klmd   = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_SHA_1)
                       | S390X_CAPBIT(S390X_SHA_256)
                       | S390X_CAPBIT(S390X_SHA_512),
                       0ULL},
        /*.km     = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256)
                       | S390X_CAPBIT(S390X_XTS_AES_128)
                       | S390X_CAPBIT(S390X_XTS_AES_256),
                       0ULL},
        /*.kmc    = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.kmac   = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.kmctr  = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.kmo    = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.kmf    = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.prno   = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_SHA_512_DRNG),
                       0ULL},
        /*.kma    = */{0ULL, 0ULL},
        /*.pcc    = */{S390X_CAPBIT(S390X_QUERY),
                       0ULL},
        /*.kdsa   = */{0ULL, 0ULL},
    };

    /*-
     * z14 (2017) - z/Architecture POP SA22-7832-11
     * Implements MSA and MSA1-8.
     */
    static const struct OPENSSL_s390xcap_st z14 = {
        /*.stfle  = */{S390X_CAPBIT(S390X_MSA)
                       | S390X_CAPBIT(S390X_STCKF)
                       | S390X_CAPBIT(S390X_MSA5),
                       S390X_CAPBIT(S390X_MSA3)
                       | S390X_CAPBIT(S390X_MSA4),
                       S390X_CAPBIT(S390X_VX)
                       | S390X_CAPBIT(S390X_VXD)
                       | S390X_CAPBIT(S390X_VXE)
                       | S390X_CAPBIT(S390X_MSA8),
                       0ULL},
        /*.kimd   = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_SHA_1)
                       | S390X_CAPBIT(S390X_SHA_256)
                       | S390X_CAPBIT(S390X_SHA_512)
                       | S390X_CAPBIT(S390X_SHA3_224)
                       | S390X_CAPBIT(S390X_SHA3_256)
                       | S390X_CAPBIT(S390X_SHA3_384)
                       | S390X_CAPBIT(S390X_SHA3_512)
                       | S390X_CAPBIT(S390X_SHAKE_128)
                       | S390X_CAPBIT(S390X_SHAKE_256),
                       S390X_CAPBIT(S390X_GHASH)},
        /*.klmd   = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_SHA_1)
                       | S390X_CAPBIT(S390X_SHA_256)
                       | S390X_CAPBIT(S390X_SHA_512)
                       | S390X_CAPBIT(S390X_SHA3_224)
                       | S390X_CAPBIT(S390X_SHA3_256)
                       | S390X_CAPBIT(S390X_SHA3_384)
                       | S390X_CAPBIT(S390X_SHA3_512)
                       | S390X_CAPBIT(S390X_SHAKE_128)
                       | S390X_CAPBIT(S390X_SHAKE_256),
                       0ULL},
        /*.km     = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256)
                       | S390X_CAPBIT(S390X_XTS_AES_128)
                       | S390X_CAPBIT(S390X_XTS_AES_256),
                       0ULL},
        /*.kmc    = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.kmac   = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.kmctr  = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.kmo    = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.kmf    = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.prno   = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_SHA_512_DRNG),
                       S390X_CAPBIT(S390X_TRNG)},
        /*.kma    = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.pcc    = */{S390X_CAPBIT(S390X_QUERY),
                       0ULL},
        /*.kdsa   = */{0ULL, 0ULL},
    };

    /*-
     * z15 (2019) - z/Architecture POP SA22-7832-12
     * Implements MSA and MSA1-9.
     */
    static const struct OPENSSL_s390xcap_st z15 = {
        /*.stfle  = */{S390X_CAPBIT(S390X_MSA)
                       | S390X_CAPBIT(S390X_STCKF)
                       | S390X_CAPBIT(S390X_MSA5),
                       S390X_CAPBIT(S390X_MSA3)
                       | S390X_CAPBIT(S390X_MSA4),
                       S390X_CAPBIT(S390X_VX)
                       | S390X_CAPBIT(S390X_VXD)
                       | S390X_CAPBIT(S390X_VXE)
                       | S390X_CAPBIT(S390X_MSA8)
                       | S390X_CAPBIT(S390X_MSA9),
                       0ULL},
        /*.kimd   = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_SHA_1)
                       | S390X_CAPBIT(S390X_SHA_256)
                       | S390X_CAPBIT(S390X_SHA_512)
                       | S390X_CAPBIT(S390X_SHA3_224)
                       | S390X_CAPBIT(S390X_SHA3_256)
                       | S390X_CAPBIT(S390X_SHA3_384)
                       | S390X_CAPBIT(S390X_SHA3_512)
                       | S390X_CAPBIT(S390X_SHAKE_128)
                       | S390X_CAPBIT(S390X_SHAKE_256),
                       S390X_CAPBIT(S390X_GHASH)},
        /*.klmd   = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_SHA_1)
                       | S390X_CAPBIT(S390X_SHA_256)
                       | S390X_CAPBIT(S390X_SHA_512)
                       | S390X_CAPBIT(S390X_SHA3_224)
                       | S390X_CAPBIT(S390X_SHA3_256)
                       | S390X_CAPBIT(S390X_SHA3_384)
                       | S390X_CAPBIT(S390X_SHA3_512)
                       | S390X_CAPBIT(S390X_SHAKE_128)
                       | S390X_CAPBIT(S390X_SHAKE_256),
                       0ULL},
        /*.km     = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256)
                       | S390X_CAPBIT(S390X_XTS_AES_128)
                       | S390X_CAPBIT(S390X_XTS_AES_256),
                       0ULL},
        /*.kmc    = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.kmac   = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.kmctr  = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.kmo    = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.kmf    = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.prno   = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_SHA_512_DRNG),
                       S390X_CAPBIT(S390X_TRNG)},
        /*.kma    = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_AES_128)
                       | S390X_CAPBIT(S390X_AES_192)
                       | S390X_CAPBIT(S390X_AES_256),
                       0ULL},
        /*.pcc    = */{S390X_CAPBIT(S390X_QUERY),
                       S390X_CAPBIT(S390X_SCALAR_MULTIPLY_P256)
                       | S390X_CAPBIT(S390X_SCALAR_MULTIPLY_P384)
                       | S390X_CAPBIT(S390X_SCALAR_MULTIPLY_P521)
                       | S390X_CAPBIT(S390X_SCALAR_MULTIPLY_ED25519)
                       | S390X_CAPBIT(S390X_SCALAR_MULTIPLY_ED448)
                       | S390X_CAPBIT(S390X_SCALAR_MULTIPLY_X25519)
                       | S390X_CAPBIT(S390X_SCALAR_MULTIPLY_X448)},
        /*.kdsa   = */{S390X_CAPBIT(S390X_QUERY)
                       | S390X_CAPBIT(S390X_ECDSA_VERIFY_P256)
                       | S390X_CAPBIT(S390X_ECDSA_VERIFY_P384)
                       | S390X_CAPBIT(S390X_ECDSA_VERIFY_P521)
                       | S390X_CAPBIT(S390X_ECDSA_SIGN_P256)
                       | S390X_CAPBIT(S390X_ECDSA_SIGN_P384)
                       | S390X_CAPBIT(S390X_ECDSA_SIGN_P521)
                       | S390X_CAPBIT(S390X_EDDSA_VERIFY_ED25519)
                       | S390X_CAPBIT(S390X_EDDSA_VERIFY_ED448)
                       | S390X_CAPBIT(S390X_EDDSA_SIGN_ED25519)
                       | S390X_CAPBIT(S390X_EDDSA_SIGN_ED448),
                       0ULL},
    };

    /*-
     * z16 (2022) - z/Architecture POP
     * Implements MSA and MSA1-9 (same as z15, no need to repeat).
     */

    char *tok_begin, *tok_end, *buff, tok[S390X_STFLE_MAX][LEN + 1];
    int rc, off, i, n;

    buff = malloc(strlen(env) + 1);
    if (buff == NULL)
        return 0;

    rc = 0;
    memset(cap, ~0, sizeof(*cap));
    strcpy(buff, env);

    tok_begin = buff + strspn(buff, ";");
    strtok(tok_begin, ";");
    tok_end = strtok(NULL, ";");

    while (tok_begin != NULL) {
        /* stfle token */
        if ((n = sscanf(tok_begin,
                        " stfle : %" STR(LEN) "[^:] : "
                        "%" STR(LEN) "[^:] : %" STR(LEN) "s ",
                        tok[0], tok[1], tok[2]))) {
            for (i = 0; i < n; i++) {
                off = (tok[i][0] == '~') ? 1 : 0;
                if (sscanf(tok[i] + off, "%llx", &cap->stfle[i]) != 1)
                    goto ret;
                if (off)
                    cap->stfle[i] = ~cap->stfle[i];
            }
        }

        /* query function tokens */
        else if TOK_FUNC(kimd)
        else if TOK_FUNC(klmd)
        else if TOK_FUNC(km)
        else if TOK_FUNC(kmc)
        else if TOK_FUNC(kmac)
        else if TOK_FUNC(kmctr)
        else if TOK_FUNC(kmo)
        else if TOK_FUNC(kmf)
        else if TOK_FUNC(prno)
        else if TOK_FUNC(kma)
        else if TOK_FUNC(pcc)
        else if TOK_FUNC(kdsa)

        /* CPU model tokens */
        else if TOK_CPU(z900)
        else if TOK_CPU(z990)
        else if TOK_CPU(z9)
        else if TOK_CPU(z10)
        else if TOK_CPU(z196)
        else if TOK_CPU(zEC12)
        else if TOK_CPU(z13)
        else if TOK_CPU(z14)
        else if TOK_CPU(z15)
        else if TOK_CPU_ALIAS(z16, z15)

        /* nocex to deactivate cex support */
        else if (sscanf(tok_begin, " %" STR(LEN) "s %" STR(LEN) "s ",
                        tok[0], tok[1]) == 1
                && !strcmp(tok[0], "nocex")) {
            *cex = 0;
        }

        /* whitespace(ignored) or invalid tokens */
        else {
            while (*tok_begin != '\0') {
                if (!ossl_isspace(*tok_begin))
                    goto ret;
                tok_begin++;
            }
        }

        tok_begin = tok_end;
        tok_end = strtok(NULL, ";");
    }

    rc = 1;
ret:
    free(buff);
    return rc;
}
