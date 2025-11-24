/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <stdint.h>
#include <openssl/crypto.h>
#include "internal/cryptlib.h"

#define OPENSSL_RISCVCAP_IMPL
#include "crypto/riscv_arch.h"

#ifdef OSSL_RISCV_HWPROBE
# include <unistd.h>
# include <sys/syscall.h>
# include <asm/hwprobe.h>
# include <sys/auxv.h>
#endif

extern size_t riscv_vlen_asm(void);

static void parse_env(const char *envstr);
static void strtoupper(char *str);

static size_t vlen = 0;

#ifdef OSSL_RISCV_HWPROBE
unsigned int OPENSSL_riscv_hwcap_P = 0;
#endif

uint32_t OPENSSL_rdtsc(void)
{
    return 0;
}

size_t OPENSSL_instrument_bus(unsigned int *out, size_t cnt)
{
    return 0;
}

size_t OPENSSL_instrument_bus2(unsigned int *out, size_t cnt, size_t max)
{
    return 0;
}

static void strtoupper(char *str)
{
    for (char *x = str; *x; ++x)
        *x = toupper((unsigned char)*x);
}

/* parse_env() parses a RISC-V architecture string. An example of such a string
 * is "rv64gc_zba_zbb_zbc_zbs". Currently, the rv64gc part is ignored
 * and we simply search for "_[extension]" in the arch string to see if we
 * should enable a given extension.
 */
#define BUFLEN 256
static void parse_env(const char *envstr)
{
    char envstrupper[BUFLEN];
    char buf[BUFLEN];

    /* Convert env str to all uppercase */
    OPENSSL_strlcpy(envstrupper, envstr, sizeof(envstrupper));
    strtoupper(envstrupper);

    for (size_t i = 0; i < kRISCVNumCaps; ++i) {
        /* Prefix capability with underscore in preparation for search */
        BIO_snprintf(buf, BUFLEN, "_%s", RISCV_capabilities[i].name);
        if (strstr(envstrupper, buf) != NULL) {
            /* Match, set relevant bit in OPENSSL_riscvcap_P[] */
            OPENSSL_riscvcap_P[RISCV_capabilities[i].index] |=
                (1 << RISCV_capabilities[i].bit_offset);
        }
    }
}

#ifdef OSSL_RISCV_HWPROBE
static long riscv_hwprobe(struct riscv_hwprobe *pairs, size_t pair_count,
                          size_t cpu_count, unsigned long *cpus,
                          unsigned int flags)
{
    return syscall(__NR_riscv_hwprobe, pairs, pair_count, cpu_count, cpus, flags);
}

static void hwprobe_to_cap(void)
{
    long ret;
    struct riscv_hwprobe pairs[OSSL_RISCV_HWPROBE_PAIR_COUNT] = {
        OSSL_RISCV_HWPROBE_PAIR_CONTENT
    };

    ret = riscv_hwprobe(pairs, OSSL_RISCV_HWPROBE_PAIR_COUNT, 0, NULL, 0);
    /* if hwprobe syscall does not exist, ret would be -ENOSYS */
    if (ret == 0) {
        for (size_t i = 0; i < kRISCVNumCaps; ++i) {
            for (size_t j = 0; j != OSSL_RISCV_HWPROBE_PAIR_COUNT; ++j) {
                if (pairs[j].key == RISCV_capabilities[i].hwprobe_key
                        && (pairs[j].value & RISCV_capabilities[i].hwprobe_value)
                           != 0)
                    if (!IS_IN_DEPEND_VECTOR(RISCV_capabilities[i].bit_offset) || VECTOR_CAPABLE)
                        /* Match, set relevant bit in OPENSSL_riscvcap_P[] */
                        OPENSSL_riscvcap_P[RISCV_capabilities[i].index] |=
                            (1 << RISCV_capabilities[i].bit_offset);
            }
        }
    }
}
#endif /* OSSL_RISCV_HWPROBE */

size_t riscv_vlen(void)
{
    return vlen;
}

# if defined(__GNUC__) && __GNUC__>=2
__attribute__ ((constructor))
# endif
void OPENSSL_cpuid_setup(void)
{
    char *e;
    static int trigger = 0;

    if (trigger != 0)
        return;
    trigger = 1;

    if ((e = getenv("OPENSSL_riscvcap"))) {
        parse_env(e);
    }
#ifdef OSSL_RISCV_HWPROBE
    else {
        OPENSSL_riscv_hwcap_P = getauxval(AT_HWCAP);
        hwprobe_to_cap();
    }
#endif

    if (RISCV_HAS_V()) {
        vlen = riscv_vlen_asm();
    }
}
