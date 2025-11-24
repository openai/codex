/*
 * Copyright 2005-2021 The OpenSSL Project Authors. All Rights Reserved.
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
#include <sys/time.h>
#include <unistd.h>
#include <openssl/bn.h>
#include "internal/cryptlib.h"
#include "crypto/sparc_arch.h"

#if defined(__GNUC__) && defined(__linux)
__attribute__ ((visibility("hidden")))
#endif
unsigned int OPENSSL_sparcv9cap_P[2] = { SPARCV9_TICK_PRIVILEGED, 0 };

unsigned long _sparcv9_rdtick(void);
void _sparcv9_vis1_probe(void);
unsigned long _sparcv9_vis1_instrument(void);
void _sparcv9_vis2_probe(void);
void _sparcv9_fmadd_probe(void);
unsigned long _sparcv9_rdcfr(void);
void _sparcv9_vis3_probe(void);
void _sparcv9_fjaesx_probe(void);
unsigned long _sparcv9_random(void);
size_t _sparcv9_vis1_instrument_bus(unsigned int *, size_t);
size_t _sparcv9_vis1_instrument_bus2(unsigned int *, size_t, size_t);

uint32_t OPENSSL_rdtsc(void)
{
    if (OPENSSL_sparcv9cap_P[0] & SPARCV9_TICK_PRIVILEGED)
#if defined(__sun) && defined(__SVR4)
        return gethrtime();
#else
        return 0;
#endif
    else
        return _sparcv9_rdtick();
}

size_t OPENSSL_instrument_bus(unsigned int *out, size_t cnt)
{
    if ((OPENSSL_sparcv9cap_P[0] & (SPARCV9_TICK_PRIVILEGED | SPARCV9_BLK)) ==
        SPARCV9_BLK)
        return _sparcv9_vis1_instrument_bus(out, cnt);
    else
        return 0;
}

size_t OPENSSL_instrument_bus2(unsigned int *out, size_t cnt, size_t max)
{
    if ((OPENSSL_sparcv9cap_P[0] & (SPARCV9_TICK_PRIVILEGED | SPARCV9_BLK)) ==
        SPARCV9_BLK)
        return _sparcv9_vis1_instrument_bus2(out, cnt, max);
    else
        return 0;
}

static sigjmp_buf common_jmp;
static void common_handler(int sig)
{
    siglongjmp(common_jmp, sig);
}

#if defined(__sun) && defined(__SVR4)
# if defined(__GNUC__) && __GNUC__>=2
extern unsigned int getisax(unsigned int vec[], unsigned int sz) __attribute__ ((weak));
# elif defined(__SUNPRO_C)
#pragma weak getisax
extern unsigned int getisax(unsigned int vec[], unsigned int sz);
# else
static unsigned int (*getisax) (unsigned int vec[], unsigned int sz) = NULL;
# endif
#endif

void OPENSSL_cpuid_setup(void)
{
    char *e;
    struct sigaction common_act, ill_oact, bus_oact;
    sigset_t all_masked, oset;
    static int trigger = 0;

    if (trigger)
        return;
    trigger = 1;

    if ((e = getenv("OPENSSL_sparcv9cap"))) {
        OPENSSL_sparcv9cap_P[0] = strtoul(e, NULL, 0);
        if ((e = strchr(e, ':')))
            OPENSSL_sparcv9cap_P[1] = strtoul(e + 1, NULL, 0);
        return;
    }

#if defined(__sun) && defined(__SVR4)
    if (getisax != NULL) {
        unsigned int vec[2] = { 0, 0 };

        if (getisax (vec,2)) {
            if (vec[0]&0x00020) OPENSSL_sparcv9cap_P[0] |= SPARCV9_VIS1;
            if (vec[0]&0x00040) OPENSSL_sparcv9cap_P[0] |= SPARCV9_VIS2;
            if (vec[0]&0x00080) OPENSSL_sparcv9cap_P[0] |= SPARCV9_BLK;
            if (vec[0]&0x00100) OPENSSL_sparcv9cap_P[0] |= SPARCV9_FMADD;
            if (vec[0]&0x00400) OPENSSL_sparcv9cap_P[0] |= SPARCV9_VIS3;
            if (vec[0]&0x01000) OPENSSL_sparcv9cap_P[0] |= SPARCV9_FJHPCACE;
            if (vec[0]&0x02000) OPENSSL_sparcv9cap_P[0] |= SPARCV9_FJDESX;
            if (vec[0]&0x08000) OPENSSL_sparcv9cap_P[0] |= SPARCV9_IMA;
            if (vec[0]&0x10000) OPENSSL_sparcv9cap_P[0] |= SPARCV9_FJAESX;
            if (vec[1]&0x00008) OPENSSL_sparcv9cap_P[0] |= SPARCV9_VIS4;

            /* reconstruct %cfr copy */
            OPENSSL_sparcv9cap_P[1] = (vec[0]>>17)&0x3ff;
            OPENSSL_sparcv9cap_P[1] |= (OPENSSL_sparcv9cap_P[1]&CFR_MONTMUL)<<1;
            if (vec[0]&0x20000000) OPENSSL_sparcv9cap_P[1] |= CFR_CRC32C;
            if (vec[1]&0x00000020) OPENSSL_sparcv9cap_P[1] |= CFR_XMPMUL;
            if (vec[1]&0x00000040)
                OPENSSL_sparcv9cap_P[1] |= CFR_XMONTMUL|CFR_XMONTSQR;

            /* Some heuristics */
            /* all known VIS2-capable CPUs have unprivileged tick counter */
            if (OPENSSL_sparcv9cap_P[0]&SPARCV9_VIS2)
                OPENSSL_sparcv9cap_P[0] &= ~SPARCV9_TICK_PRIVILEGED;

            OPENSSL_sparcv9cap_P[0] |= SPARCV9_PREFER_FPU;

            /* detect UltraSPARC-Tx, see sparccpud.S for details... */
            if ((OPENSSL_sparcv9cap_P[0]&SPARCV9_VIS1) &&
                _sparcv9_vis1_instrument() >= 12)
                OPENSSL_sparcv9cap_P[0] &= ~(SPARCV9_VIS1 | SPARCV9_PREFER_FPU);
        }

        if (sizeof(size_t) == 8)
            OPENSSL_sparcv9cap_P[0] |= SPARCV9_64BIT_STACK;

        return;
    }
#endif

    /* Initial value, fits UltraSPARC-I&II... */
    OPENSSL_sparcv9cap_P[0] = SPARCV9_PREFER_FPU | SPARCV9_TICK_PRIVILEGED;

    sigfillset(&all_masked);
    sigdelset(&all_masked, SIGILL);
    sigdelset(&all_masked, SIGTRAP);
# ifdef SIGEMT
    sigdelset(&all_masked, SIGEMT);
# endif
    sigdelset(&all_masked, SIGFPE);
    sigdelset(&all_masked, SIGBUS);
    sigdelset(&all_masked, SIGSEGV);
    sigprocmask(SIG_SETMASK, &all_masked, &oset);

    memset(&common_act, 0, sizeof(common_act));
    common_act.sa_handler = common_handler;
    common_act.sa_mask = all_masked;

    sigaction(SIGILL, &common_act, &ill_oact);
    sigaction(SIGBUS, &common_act, &bus_oact); /* T1 fails 16-bit ldda [on
                                                * Linux] */

    if (sigsetjmp(common_jmp, 1) == 0) {
        _sparcv9_rdtick();
        OPENSSL_sparcv9cap_P[0] &= ~SPARCV9_TICK_PRIVILEGED;
    }

    if (sigsetjmp(common_jmp, 1) == 0) {
        _sparcv9_vis1_probe();
        OPENSSL_sparcv9cap_P[0] |= SPARCV9_VIS1 | SPARCV9_BLK;
        /* detect UltraSPARC-Tx, see sparccpud.S for details... */
        if (_sparcv9_vis1_instrument() >= 12)
            OPENSSL_sparcv9cap_P[0] &= ~(SPARCV9_VIS1 | SPARCV9_PREFER_FPU);
        else {
            _sparcv9_vis2_probe();
            OPENSSL_sparcv9cap_P[0] |= SPARCV9_VIS2;
        }
    }

    if (sigsetjmp(common_jmp, 1) == 0) {
        _sparcv9_fmadd_probe();
        OPENSSL_sparcv9cap_P[0] |= SPARCV9_FMADD;
    }

    /*
     * VIS3 flag is tested independently from VIS1, unlike VIS2 that is,
     * because VIS3 defines even integer instructions.
     */
    if (sigsetjmp(common_jmp, 1) == 0) {
        _sparcv9_vis3_probe();
        OPENSSL_sparcv9cap_P[0] |= SPARCV9_VIS3;
    }

    if (sigsetjmp(common_jmp, 1) == 0) {
        _sparcv9_fjaesx_probe();
        OPENSSL_sparcv9cap_P[0] |= SPARCV9_FJAESX;
    }

    /*
     * In wait for better solution _sparcv9_rdcfr is masked by
     * VIS3 flag, because it goes to uninterruptible endless
     * loop on UltraSPARC II running Solaris. Things might be
     * different on Linux...
     */
    if ((OPENSSL_sparcv9cap_P[0] & SPARCV9_VIS3) &&
        sigsetjmp(common_jmp, 1) == 0) {
        OPENSSL_sparcv9cap_P[1] = (unsigned int)_sparcv9_rdcfr();
    }

    sigaction(SIGBUS, &bus_oact, NULL);
    sigaction(SIGILL, &ill_oact, NULL);

    sigprocmask(SIG_SETMASK, &oset, NULL);

    if (sizeof(size_t) == 8)
        OPENSSL_sparcv9cap_P[0] |= SPARCV9_64BIT_STACK;
# ifdef __linux
    else {
        int ret = syscall(340);

        if (ret >= 0 && ret & 1)
            OPENSSL_sparcv9cap_P[0] |= SPARCV9_64BIT_STACK;
    }
# endif
}
