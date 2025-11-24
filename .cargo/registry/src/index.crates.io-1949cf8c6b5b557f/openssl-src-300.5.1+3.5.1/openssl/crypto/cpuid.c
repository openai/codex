/*
 * Copyright 1998-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/e_os.h"
#include "crypto/cryptlib.h"

#if     defined(__i386)   || defined(__i386__)   || defined(_M_IX86) || \
        defined(__x86_64) || defined(__x86_64__) || \
        defined(_M_AMD64) || defined(_M_X64)

extern unsigned int OPENSSL_ia32cap_P[OPENSSL_IA32CAP_P_MAX_INDEXES];

# if defined(OPENSSL_CPUID_OBJ)

/*
 * Purpose of these minimalistic and character-type-agnostic subroutines
 * is to break dependency on MSVCRT (on Windows) and locale. This makes
 * OPENSSL_cpuid_setup safe to use as "constructor". "Character-type-
 * agnostic" means that they work with either wide or 8-bit characters,
 * exploiting the fact that first 127 characters can be simply casted
 * between the sets, while the rest would be simply rejected by ossl_is*
 * subroutines.
 */
#  ifdef _WIN32
typedef WCHAR variant_char;
#   define OPENSSL_IA32CAP_P_MAX_CHAR_SIZE 256
static variant_char *ossl_getenv(const char *name)
{
    /*
     * Since we pull only one environment variable, it's simpler to
     * just ignore |name| and use equivalent wide-char L-literal.
     * As well as to ignore excessively long values...
     */
    static WCHAR value[OPENSSL_IA32CAP_P_MAX_CHAR_SIZE];
    DWORD len = GetEnvironmentVariableW(L"OPENSSL_ia32cap", value, OPENSSL_IA32CAP_P_MAX_CHAR_SIZE);

    return (len > 0 && len < OPENSSL_IA32CAP_P_MAX_CHAR_SIZE) ? value : NULL;
}
#  else
typedef char variant_char;
#   define ossl_getenv getenv
#  endif

#  include "crypto/ctype.h"

static int todigit(variant_char c)
{
    if (ossl_isdigit(c))
        return c - '0';
    else if (ossl_isxdigit(c))
        return ossl_tolower(c) - 'a' + 10;

    /* return largest base value to make caller terminate the loop */
    return 16;
}

static uint64_t ossl_strtouint64(const variant_char *str)
{
    uint64_t ret = 0;
    unsigned int digit, base = 10;

    if (*str == '0') {
        base = 8, str++;
        if (ossl_tolower(*str) == 'x')
            base = 16, str++;
    }

    while ((digit = todigit(*str++)) < base)
        ret = ret * base + digit;

    return ret;
}

static variant_char *ossl_strchr(const variant_char *str, char srch)
{   variant_char c;

    while ((c = *str)) {
        if (c == srch)
            return (variant_char *)str;
        str++;
    }

    return NULL;
}

#  define OPENSSL_CPUID_SETUP
typedef uint64_t IA32CAP;

void OPENSSL_cpuid_setup(void)
{
    static int trigger = 0;
    IA32CAP OPENSSL_ia32_cpuid(unsigned int *);
    IA32CAP vec;
    const variant_char *env;
    int index = 2;

    if (trigger)
        return;

    trigger = 1;
    if ((env = ossl_getenv("OPENSSL_ia32cap")) != NULL) {
        int off = (env[0] == '~') ? 1 : 0;

        vec = ossl_strtouint64(env + off);

        if (off) {
            IA32CAP mask = vec;
            vec = OPENSSL_ia32_cpuid(OPENSSL_ia32cap_P) & ~mask;
            if (mask & (1<<24)) {
                /*
                 * User disables FXSR bit, mask even other capabilities
                 * that operate exclusively on XMM, so we don't have to
                 * double-check all the time. We mask PCLMULQDQ, AMD XOP,
                 * AES-NI and AVX. Formally speaking we don't have to
                 * do it in x86_64 case, but we can safely assume that
                 * x86_64 users won't actually flip this flag.
                 */
                vec &= ~((IA32CAP)(1<<1|1<<11|1<<25|1<<28) << 32);
            }
        } else if (env[0] == ':') {
            vec = OPENSSL_ia32_cpuid(OPENSSL_ia32cap_P);
        }

        /* Processed indexes 0, 1 */
        if ((env = ossl_strchr(env, ':')) != NULL)
            env++;
        for (; index < OPENSSL_IA32CAP_P_MAX_INDEXES; index += 2) {
            if ((env != NULL) && (env[0] != '\0')) {
                /* if env[0] == ':' current index is skipped */
                if (env[0] != ':') {
                    IA32CAP vecx;

                    off = (env[0] == '~') ? 1 : 0;
                    vecx = ossl_strtouint64(env + off);
                    if (off) {
                        OPENSSL_ia32cap_P[index] &= ~(unsigned int)vecx;
                        OPENSSL_ia32cap_P[index + 1] &= ~(unsigned int)(vecx >> 32);
                    } else {
                        OPENSSL_ia32cap_P[index] = (unsigned int)vecx;
                        OPENSSL_ia32cap_P[index + 1] = (unsigned int)(vecx >> 32);
                    }
                }
                /* skip delimeter */
                if ((env = ossl_strchr(env, ':')) != NULL)
                    env++;
            } else { /* zeroize the next two indexes */
                OPENSSL_ia32cap_P[index] = 0;
                OPENSSL_ia32cap_P[index + 1] = 0;
            }
        }

        /* If AVX10 is disabled, zero out its detailed cap bits */
        if (!(OPENSSL_ia32cap_P[6] & (1 << 19)))
            OPENSSL_ia32cap_P[9] = 0;
    } else {
        vec = OPENSSL_ia32_cpuid(OPENSSL_ia32cap_P);
    }

    /*
     * |(1<<10) sets a reserved bit to signal that variable
     * was initialized already... This is to avoid interference
     * with cpuid snippets in ELF .init segment.
     */
    OPENSSL_ia32cap_P[0] = (unsigned int)vec | (1 << 10);
    OPENSSL_ia32cap_P[1] = (unsigned int)(vec >> 32);
}
# else
unsigned int OPENSSL_ia32cap_P[OPENSSL_IA32CAP_P_MAX_INDEXES];
# endif
#endif

#ifndef OPENSSL_CPUID_OBJ
# ifndef OPENSSL_CPUID_SETUP
void OPENSSL_cpuid_setup(void)
{
}
# endif

/*
 * The rest are functions that are defined in the same assembler files as
 * the CPUID functionality.
 */

/*
 * The volatile is used to ensure that the compiler generates code that reads
 * all values from the array and doesn't try to optimize this away. The standard
 * doesn't actually require this behavior if the original data pointed to is
 * not volatile, but compilers do this in practice anyway.
 *
 * There are also assembler versions of this function.
 */
# undef CRYPTO_memcmp
int CRYPTO_memcmp(const void *in_a, const void *in_b, size_t len)
{
    size_t i;
    const volatile unsigned char *a = in_a;
    const volatile unsigned char *b = in_b;
    unsigned char x = 0;

    for (i = 0; i < len; i++)
        x |= a[i] ^ b[i];

    return x;
}

/*
 * For systems that don't provide an instruction counter register or equivalent.
 */
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
#endif
