/*
 * Copyright 2011-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* We need to use some engine deprecated APIs */
#define OPENSSL_SUPPRESS_DEPRECATED

#include <openssl/opensslconf.h>

#include <stdio.h>
#include <string.h>
#include "crypto/engine.h"
#include "internal/cryptlib.h"
#include <openssl/rand.h>
#include <openssl/err.h>
#include <openssl/crypto.h>

#if defined(__has_feature)
# if __has_feature(memory_sanitizer)
#  include <sanitizer/msan_interface.h>
# endif
#endif

#if (defined(__i386)   || defined(__i386__)   || defined(_M_IX86) || \
     defined(__x86_64) || defined(__x86_64__) || \
     defined(_M_AMD64) || defined (_M_X64)) && defined(OPENSSL_CPUID_OBJ)

size_t OPENSSL_ia32_rdrand_bytes(unsigned char *buf, size_t len);

static int get_random_bytes(unsigned char *buf, int num)
{
    if (num < 0) {
        return 0;
    }

# if defined(__has_feature)
#  if __has_feature(memory_sanitizer)
    /*
     * MemorySanitizer fails to understand asm and produces false positive
     * use-of-uninitialized-value warnings.
     */
    __msan_unpoison(buf, num);
#  endif
# endif

    return (size_t)num == OPENSSL_ia32_rdrand_bytes(buf, (size_t)num);
}

static int random_status(void)
{
    return 1;
}

static RAND_METHOD rdrand_meth = {
    NULL,                       /* seed */
    get_random_bytes,
    NULL,                       /* cleanup */
    NULL,                       /* add */
    get_random_bytes,
    random_status,
};

static int rdrand_init(ENGINE *e)
{
    return 1;
}

static const char *engine_e_rdrand_id = "rdrand";
static const char *engine_e_rdrand_name = "Intel RDRAND engine";

static int bind_helper(ENGINE *e)
{
    if (!ENGINE_set_id(e, engine_e_rdrand_id) ||
        !ENGINE_set_name(e, engine_e_rdrand_name) ||
        !ENGINE_set_flags(e, ENGINE_FLAGS_NO_REGISTER_ALL) ||
        !ENGINE_set_init_function(e, rdrand_init) ||
        !ENGINE_set_RAND(e, &rdrand_meth))
        return 0;

    return 1;
}

static ENGINE *ENGINE_rdrand(void)
{
    ENGINE *ret = ENGINE_new();
    if (ret == NULL)
        return NULL;
    if (!bind_helper(ret)) {
        ENGINE_free(ret);
        return NULL;
    }
    return ret;
}

void engine_load_rdrand_int(void)
{
    if (OPENSSL_ia32cap_P[1] & (1 << (62 - 32))) {
        ENGINE *toadd = ENGINE_rdrand();
        if (!toadd)
            return;
        ERR_set_mark();
        ENGINE_add(toadd);
        /*
        * If the "add" worked, it gets a structural reference. So either way, we
        * release our just-created reference.
        */
        ENGINE_free(toadd);
        /*
        * If the "add" didn't work, it was probably a conflict because it was
        * already added (eg. someone calling ENGINE_load_blah then calling
        * ENGINE_load_builtin_engines() perhaps).
        */
        ERR_pop_to_mark();
    }
}
#else
void engine_load_rdrand_int(void)
{
}
#endif
