/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/e_os.h"
#include "internal/cryptlib.h"
#include "crypto/cryptlib.h"
#include <stdio.h>
#include <stdlib.h>
#include <limits.h>
#include <openssl/crypto.h>

/*
 * the following pointers may be changed as long as 'allow_customize' is set
 */
static int allow_customize = 1;
static CRYPTO_malloc_fn malloc_impl = CRYPTO_malloc;
static CRYPTO_realloc_fn realloc_impl = CRYPTO_realloc;
static CRYPTO_free_fn free_impl = CRYPTO_free;

#if !defined(OPENSSL_NO_CRYPTO_MDEBUG) && !defined(FIPS_MODULE)
# include "internal/tsan_assist.h"

# ifdef TSAN_REQUIRES_LOCKING
#  define INCREMENT(x) /* empty */
#  define LOAD(x) 0
# else  /* TSAN_REQUIRES_LOCKING */
static TSAN_QUALIFIER int malloc_count;
static TSAN_QUALIFIER int realloc_count;
static TSAN_QUALIFIER int free_count;

#  define INCREMENT(x) tsan_counter(&(x))
#  define LOAD(x)      tsan_load(&x)
# endif /* TSAN_REQUIRES_LOCKING */

static char md_failbuf[CRYPTO_MEM_CHECK_MAX_FS + 1];
static char *md_failstring = NULL;
static long md_count;
static int md_fail_percent = 0;
static int md_tracefd = -1;

static void parseit(void);
static int shouldfail(void);

# define FAILTEST() if (shouldfail()) return NULL

#else

# define INCREMENT(x) /* empty */
# define FAILTEST() /* empty */
#endif

int CRYPTO_set_mem_functions(CRYPTO_malloc_fn malloc_fn,
                             CRYPTO_realloc_fn realloc_fn,
                             CRYPTO_free_fn free_fn)
{
    if (!allow_customize)
        return 0;
    if (malloc_fn != NULL)
        malloc_impl = malloc_fn;
    if (realloc_fn != NULL)
        realloc_impl = realloc_fn;
    if (free_fn != NULL)
        free_impl = free_fn;
    return 1;
}

void CRYPTO_get_mem_functions(CRYPTO_malloc_fn *malloc_fn,
                              CRYPTO_realloc_fn *realloc_fn,
                              CRYPTO_free_fn *free_fn)
{
    if (malloc_fn != NULL)
        *malloc_fn = malloc_impl;
    if (realloc_fn != NULL)
        *realloc_fn = realloc_impl;
    if (free_fn != NULL)
        *free_fn = free_impl;
}

#if !defined(OPENSSL_NO_CRYPTO_MDEBUG) && !defined(FIPS_MODULE)
void CRYPTO_get_alloc_counts(int *mcount, int *rcount, int *fcount)
{
    if (mcount != NULL)
        *mcount = LOAD(malloc_count);
    if (rcount != NULL)
        *rcount = LOAD(realloc_count);
    if (fcount != NULL)
        *fcount = LOAD(free_count);
}

/*
 * Parse a "malloc failure spec" string.  This likes like a set of fields
 * separated by semicolons.  Each field has a count and an optional failure
 * percentage.  For example:
 *          100@0;100@25;0@0
 *    or    100;100@25;0
 * This means 100 mallocs succeed, then next 100 fail 25% of the time, and
 * all remaining (count is zero) succeed.
 * The failure percentge can have 2 digits after the comma.  For example:
 *          0@0.01
 * This means 0.01% of all allocations will fail.
 */
static void parseit(void)
{
    char *semi = strchr(md_failstring, ';');
    char *atsign;

    if (semi != NULL)
        *semi++ = '\0';

    /* Get the count (atol will stop at the @ if there), and percentage */
    md_count = atol(md_failstring);
    atsign = strchr(md_failstring, '@');
    md_fail_percent = atsign == NULL ? 0 : (int)(atof(atsign + 1) * 100 + 0.5);

    if (semi != NULL)
        md_failstring = semi;
}

/*
 * Windows doesn't have random() and srandom(), but it has rand() and srand().
 * Some rand() implementations aren't good, but we're not
 * dealing with secure randomness here.
 */
# ifdef _WIN32
#  define random() rand()
#  define srandom(seed) srand(seed)
# endif
/*
 * See if the current malloc should fail.
 */
static int shouldfail(void)
{
    int roll = (int)(random() % 10000);
    int shoulditfail = roll < md_fail_percent;
# ifndef _WIN32
/* suppressed on Windows as POSIX-like file descriptors are non-inheritable */
    int len;
    char buff[80];

    if (md_tracefd > 0) {
        BIO_snprintf(buff, sizeof(buff),
                     "%c C%ld %%%d R%d\n",
                     shoulditfail ? '-' : '+', md_count, md_fail_percent, roll);
        len = strlen(buff);
        if (write(md_tracefd, buff, len) != len)
            perror("shouldfail write failed");
    }
# endif

    if (md_count) {
        /* If we used up this one, go to the next. */
        if (--md_count == 0)
            parseit();
    }

    return shoulditfail;
}

void ossl_malloc_setup_failures(void)
{
    const char *cp = getenv("OPENSSL_MALLOC_FAILURES");
    size_t cplen = 0;

    if (cp != NULL) {
        /* if the value is too long we'll just ignore it */
        cplen = strlen(cp);
        if (cplen <= CRYPTO_MEM_CHECK_MAX_FS) {
            strncpy(md_failbuf, cp, CRYPTO_MEM_CHECK_MAX_FS);
            md_failstring = md_failbuf;
            parseit();
        }
    }
    if ((cp = getenv("OPENSSL_MALLOC_FD")) != NULL)
        md_tracefd = atoi(cp);
    if ((cp = getenv("OPENSSL_MALLOC_SEED")) != NULL)
        srandom(atoi(cp));
}
#endif

void *CRYPTO_malloc(size_t num, const char *file, int line)
{
    void *ptr;

    INCREMENT(malloc_count);
    if (malloc_impl != CRYPTO_malloc) {
        ptr = malloc_impl(num, file, line);
        if (ptr != NULL || num == 0)
            return ptr;
        goto err;
    }

    if (num == 0)
        return NULL;

    FAILTEST();
    if (allow_customize) {
        /*
         * Disallow customization after the first allocation. We only set this
         * if necessary to avoid a store to the same cache line on every
         * allocation.
         */
        allow_customize = 0;
    }

    ptr = malloc(num);
    if (ptr != NULL)
        return ptr;
 err:
    /*
     * ossl_err_get_state_int() in err.c uses CRYPTO_zalloc(num, NULL, 0) for
     * ERR_STATE allocation. Prevent mem alloc error loop while reporting error.
     */
    if (file != NULL || line != 0) {
        ERR_new();
        ERR_set_debug(file, line, NULL);
        ERR_set_error(ERR_LIB_CRYPTO, ERR_R_MALLOC_FAILURE, NULL);
    }
    return NULL;
}

void *CRYPTO_zalloc(size_t num, const char *file, int line)
{
    void *ret;

    ret = CRYPTO_malloc(num, file, line);
    if (ret != NULL)
        memset(ret, 0, num);

    return ret;
}

void *CRYPTO_aligned_alloc(size_t num, size_t alignment, void **freeptr,
                           const char *file, int line)
{
    void *ret;

    *freeptr = NULL;

#if defined(OPENSSL_SMALL_FOOTPRINT)
    ret = freeptr = NULL;
    return ret;
#endif

    /* Allow non-malloc() allocations as long as no malloc_impl is provided. */
    if (malloc_impl == CRYPTO_malloc) {
#if defined(_BSD_SOURCE) || (defined(_POSIX_C_SOURCE) && _POSIX_C_SOURCE >= 200112L)
        if (posix_memalign(&ret, alignment, num))
            return NULL;
        *freeptr = ret;
        return ret;
#elif defined(_ISOC11_SOURCE)
        ret = *freeptr = aligned_alloc(alignment, num);
        return ret;
#endif
    }

    /* we have to do this the hard way */

    /*
     * Note: Windows supports an _aligned_malloc call, but we choose
     * not to use it here, because allocations from that function
     * require that they be freed via _aligned_free.  Given that
     * we can't differentiate plain malloc blocks from blocks obtained
     * via _aligned_malloc, just avoid its use entirely
     */

    /*
     * Step 1: Allocate an amount of memory that is <alignment>
     * bytes bigger than requested
     */
    *freeptr = CRYPTO_malloc(num + alignment, file, line);
    if (*freeptr == NULL)
        return NULL;

    /*
     * Step 2: Add <alignment - 1> bytes to the pointer
     * This will cross the alignment boundary that is
     * requested
     */
    ret = (void *)((char *)*freeptr + (alignment - 1));

    /*
     * Step 3: Use the alignment as a mask to translate the
     * least significant bits of the allocation at the alignment
     * boundary to 0.  ret now holds a pointer to the memory
     * buffer at the requested alignment
     * NOTE: It is a documented requirement that alignment be a
     * power of 2, which is what allows this to work
     */
    ret = (void *)((uintptr_t)ret & (uintptr_t)(~(alignment - 1)));
    return ret;
}

void *CRYPTO_realloc(void *str, size_t num, const char *file, int line)
{
    INCREMENT(realloc_count);
    if (realloc_impl != CRYPTO_realloc)
        return realloc_impl(str, num, file, line);

    if (str == NULL)
        return CRYPTO_malloc(num, file, line);

    if (num == 0) {
        CRYPTO_free(str, file, line);
        return NULL;
    }

    FAILTEST();
    return realloc(str, num);
}

void *CRYPTO_clear_realloc(void *str, size_t old_len, size_t num,
                           const char *file, int line)
{
    void *ret = NULL;

    if (str == NULL)
        return CRYPTO_malloc(num, file, line);

    if (num == 0) {
        CRYPTO_clear_free(str, old_len, file, line);
        return NULL;
    }

    /* Can't shrink the buffer since memcpy below copies |old_len| bytes. */
    if (num < old_len) {
        OPENSSL_cleanse((char*)str + num, old_len - num);
        return str;
    }

    ret = CRYPTO_malloc(num, file, line);
    if (ret != NULL) {
        memcpy(ret, str, old_len);
        CRYPTO_clear_free(str, old_len, file, line);
    }
    return ret;
}

void CRYPTO_free(void *str, const char *file, int line)
{
    INCREMENT(free_count);
    if (free_impl != CRYPTO_free) {
        free_impl(str, file, line);
        return;
    }

    free(str);
}

void CRYPTO_clear_free(void *str, size_t num, const char *file, int line)
{
    if (str == NULL)
        return;
    if (num)
        OPENSSL_cleanse(str, num);
    CRYPTO_free(str, file, line);
}

#if !defined(OPENSSL_NO_CRYPTO_MDEBUG)

# ifndef OPENSSL_NO_DEPRECATED_3_0
int CRYPTO_mem_ctrl(int mode)
{
    (void)mode;
    return -1;
}

int CRYPTO_set_mem_debug(int flag)
{
    (void)flag;
    return -1;
}

int CRYPTO_mem_debug_push(const char *info, const char *file, int line)
{
    (void)info; (void)file; (void)line;
    return 0;
}

int CRYPTO_mem_debug_pop(void)
{
    return 0;
}

void CRYPTO_mem_debug_malloc(void *addr, size_t num, int flag,
                             const char *file, int line)
{
    (void)addr; (void)num; (void)flag; (void)file; (void)line;
}

void CRYPTO_mem_debug_realloc(void *addr1, void *addr2, size_t num, int flag,
                              const char *file, int line)
{
    (void)addr1; (void)addr2; (void)num; (void)flag; (void)file; (void)line;
}

void CRYPTO_mem_debug_free(void *addr, int flag,
                           const char *file, int line)
{
    (void)addr; (void)flag; (void)file; (void)line;
}

int CRYPTO_mem_leaks(BIO *b)
{
    (void)b;
    return -1;
}

#  ifndef OPENSSL_NO_STDIO
int CRYPTO_mem_leaks_fp(FILE *fp)
{
    (void)fp;
    return -1;
}
#  endif

int CRYPTO_mem_leaks_cb(int (*cb)(const char *str, size_t len, void *u),
                        void *u)
{
    (void)cb; (void)u;
    return -1;
}

# endif

#endif
