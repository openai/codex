/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#include <stdio.h>
#include <string.h>
#include <openssl/bio.h>
#include <openssl/evp.h>
#include <openssl/rand.h>

#include "testutil.h"

/* 2047 bytes of "#ooooooooo..." + NUL terminator */
static char gunk[2048];

typedef struct {
    char *prefix;
    char *encoded;
    unsigned bytes;
    int trunc;
    char *suffix;
    int retry;
    int no_nl;
} test_case;

#define BUFMAX 0xa0000          /* Encode at most 640kB. */
#define sEOF "-EOF"             /* '-' as in PEM and MIME boundaries */
#define junk "#foo"             /* Skipped initial content */

#define EOF_RETURN (-1729)      /* Distinct from -1, etc., internal results */
#define NLEN 6
#define NVAR 5
/*
 * Junk suffixed variants don't make sense with padding or truncated groups
 * because we will typically stop with an error before seeing the suffix, but
 * with retriable BIOs may never look at the suffix after detecting padding.
 */
#define NPAD 6
#define NVARPAD (NVAR * NPAD - NPAD + 1)

static char *prefixes[NVAR] = { "", junk, gunk, "", "" };
static char *suffixes[NVAR] = { "", "", "", sEOF, junk };
static unsigned lengths[6] = { 0, 3, 48, 192, 768, 1536 };
static unsigned linelengths[] = {
    4, 8, 16, 28, 40, 64, 80, 128, 256, 512, 1023, 0
};
static unsigned wscnts[] = { 0, 1, 2, 4, 8, 16, 0xFFFF };

/* Generate `len` random octets */
static unsigned char *genbytes(unsigned len)
{
    unsigned char *buf = NULL;

    if (len > 0 && len <= BUFMAX && (buf = OPENSSL_malloc(len)) != NULL)
        RAND_bytes(buf, len);

    return buf;
}

/* Append one base64 codepoint, adding newlines after every `llen` bytes */
static int memout(BIO *mem, char c, int llen, int *pos)
{
    if (BIO_write(mem, &c, 1) != 1)
        return 0;
    if (++*pos == llen) {
        *pos = 0;
        c = '\n';
        if (BIO_write(mem, &c, 1) != 1)
            return 0;
    }
    return 1;
}

/* Encode and append one 6-bit slice, randomly prepending some whitespace */
static int memoutws(BIO *mem, char c, unsigned wscnt, unsigned llen, int *pos)
{
    if (wscnt > 0
        && (test_random() % llen) < wscnt
        && memout(mem, ' ', llen, pos) == 0)
        return 0;
    return memout(mem, c, llen, pos);
}

/*
 * Encode an octet string in base64, approximately `llen` bytes per line,
 * with up to roughly `wscnt` additional space characters inserted at random
 * before some of the base64 code points.
 */
static int encode(unsigned const char *buf, unsigned buflen, char *encoded,
                  int trunc, unsigned llen, unsigned wscnt, BIO *mem)
{
    static const unsigned char b64[65] =
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    int pos = 0;
    char nl = '\n';

    /* Use a verbatim encoding when provided */
    if (encoded != NULL) {
        int elen = strlen(encoded);

        return BIO_write(mem, encoded, elen) == elen;
    }

    /* Encode full 3-octet groups */
    while (buflen > 2) {
        unsigned long v = buf[0] << 16 | buf[1] << 8 | buf[2];

        if (memoutws(mem, b64[v >> 18], wscnt, llen, &pos) == 0
            || memoutws(mem, b64[(v >> 12) & 0x3f], wscnt, llen, &pos) == 0
            || memoutws(mem, b64[(v >> 6) & 0x3f], wscnt, llen, &pos) == 0
            || memoutws(mem, b64[v & 0x3f], wscnt, llen, &pos) == 0)
            return 0;
        buf += 3;
        buflen -= 3;
    }

    /* Encode and pad final 1 or 2 octet group */
    if (buflen == 2) {
        unsigned long v = buf[0] << 8 | buf[1];

        if (memoutws(mem, b64[(v >> 10) & 0x3f], wscnt, llen, &pos) == 0
            || memoutws(mem, b64[(v >> 4) & 0x3f], wscnt, llen, &pos) == 0
            || memoutws(mem, b64[(v & 0xf) << 2], wscnt, llen, &pos) == 0
            || memoutws(mem, '=', wscnt, llen, &pos) == 0)
            return 0;
    } else if (buflen == 1) {
        unsigned long v = buf[0];

        if (memoutws(mem, b64[v >> 2], wscnt, llen, &pos) == 0
            || memoutws(mem, b64[(v & 0x3) << 4], wscnt, llen, &pos) == 0
            || memoutws(mem, '=', wscnt, llen, &pos) == 0
            || memoutws(mem, '=', wscnt, llen, &pos) == 0)
            return 0;
    }

    while (trunc-- > 0)
        if (memoutws(mem, 'A', wscnt, llen, &pos) == 0)
            return 0;

    /* Terminate last line */
    if (pos > 0 && BIO_write(mem, &nl, 1) != 1)
        return 0;

    return 1;
}

static int genb64(char *prefix, char *suffix, unsigned const char *buf,
                  unsigned buflen, int trunc, char *encoded, unsigned llen,
                  unsigned wscnt, char **out)
{
    int preflen = strlen(prefix);
    int sufflen = strlen(suffix);
    int outlen;
    char newline = '\n';
    BUF_MEM *bptr;
    BIO *mem = BIO_new(BIO_s_mem());

    if (mem == NULL)
        return -1;

    if ((*prefix && (BIO_write(mem, prefix, preflen) != preflen
                     || BIO_write(mem, &newline, 1) != 1))
        || encode(buf, buflen, encoded, trunc, llen, wscnt, mem) <= 0
        || (*suffix && (BIO_write(mem, suffix, sufflen) != sufflen
                        || BIO_write(mem, &newline, 1) != 1))) {
        BIO_free(mem);
        return -1;
    }

    /* Orphan the memory BIO's data buffer */
    BIO_get_mem_ptr(mem, &bptr);
    *out = bptr->data;
    outlen = bptr->length;
    bptr->data = NULL;
    (void) BIO_set_close(mem, BIO_NOCLOSE);
    BIO_free(mem);
    BUF_MEM_free(bptr);

    return outlen;
}

static int test_bio_base64_run(test_case *t, int llen, int wscnt)
{
    unsigned char *raw;
    unsigned char *out;
    unsigned out_len;
    char *encoded = NULL;
    int elen;
    BIO *bio, *b64;
    int n, n1, n2;
    int ret;

    /*
     * Pre-encoded data always encodes NUL octets.  If all we care about is the
     * length, and not the payload, use random bytes.
     */
    if (t->encoded != NULL)
        raw = OPENSSL_zalloc(t->bytes);
    else
        raw = genbytes(t->bytes);

    if (raw == NULL && t->bytes > 0) {
        TEST_error("out of memory");
        return -1;
    }

    out_len = t->bytes + 1024;
    out = OPENSSL_malloc(out_len);
    if (out == NULL) {
        OPENSSL_free(raw);
        TEST_error("out of memory");
        return -1;
    }

    elen = genb64(t->prefix, t->suffix, raw, t->bytes, t->trunc, t->encoded,
                  llen, wscnt, &encoded);
    if (elen < 0 || (bio = BIO_new(BIO_s_mem())) == NULL) {
        OPENSSL_free(raw);
        OPENSSL_free(out);
        OPENSSL_free(encoded);
        TEST_error("out of memory");
        return -1;
    }
    if (t->retry)
        BIO_set_mem_eof_return(bio, EOF_RETURN);
    else
        BIO_set_mem_eof_return(bio, 0);

    /*
     * When the input is long enough, and the source bio is retriable, exercise
     * retries by writting the input to the underlying BIO in two steps (1024
     * bytes, then the rest) and trying to decode some data after each write.
     */
    n1 = elen;
    if (t->retry)
        n1 = elen / 2;
    if (n1 > 0)
        BIO_write(bio, encoded, n1);

    b64 = BIO_new(BIO_f_base64());
    if (t->no_nl)
        BIO_set_flags(b64, BIO_FLAGS_BASE64_NO_NL);
    BIO_push(b64, bio);

    n = BIO_read(b64, out, out_len);

    if (n1 < elen) {
        /* Append the rest of the input, and read again */
        BIO_write(bio, encoded + n1, elen - n1);
        if (n > 0) {
            n2 = BIO_read(b64, out + n, out_len - n);
            if (n2 > 0)
                n += n2;
        } else if (n == EOF_RETURN) {
            n = BIO_read(b64, out, out_len);
        }
    }

    /* Turn retry-related negative results to normal (0) EOF */
    if (n < 0 && n == EOF_RETURN)
        n = 0;

    /* Turn off retries */
    if (t->retry)
        BIO_set_mem_eof_return(bio, 0);

    if (n < (int) out_len)
        /* Perform the last read, checking its result */
        ret = BIO_read(b64, out + n, out_len - n);
    else {
        /* Should not happen, given extra space in out_len */
        TEST_error("Unexpectedly long decode output");
        ret = -1;
    }

    /*
     * Expect an error to be detected with:
     *
     * - truncated groups,
     * - non-base64 suffixes (other than soft EOF) for non-empty or oneline
     *   input
     * - non-base64 prefixes in NO_NL mode
     *
     * Otherwise, check the decoded content
     */
    if (t->trunc > 0
        || ((t->bytes > 0 || t->no_nl) && *t->suffix && *t->suffix != '-')
        || (t->no_nl && *t->prefix)) {
        if ((ret = ret < 0 ? 0 : -1) != 0)
            TEST_error("Final read result was non-negative");
    } else if (ret != 0
             || n != (int) t->bytes
             || (n > 0 && memcmp(raw, out, n) != 0)) {
        TEST_error("Failed to decode expected data");
        ret = -1;
    }

    BIO_free_all(b64);
    OPENSSL_free(out);
    OPENSSL_free(raw);
    OPENSSL_free(encoded);

    return ret;
}

static int generic_case(test_case *t, int verbose)
{
    unsigned *llen;
    unsigned *wscnt;
    int ok = 1;

    for (llen = linelengths; *llen > 0; ++llen) {
        for (wscnt = wscnts; *wscnt * 2 < *llen; ++wscnt) {
            int extra = t->no_nl ? 64 : 0;

            /*
             * Use a longer line for NO_NL tests, in particular, eventually
             * exceeding 1k bytes.
             */
            if (test_bio_base64_run(t, *llen + extra, *wscnt) != 0)
                ok = 0;

            if (verbose) {
                fprintf(stderr, "bio_base64_test: ok=%d", ok);
                if (*t->prefix)
                    fprintf(stderr, ", prefix='%s'", t->prefix);
                if (t->encoded)
                    fprintf(stderr, ", data='%s'", t->encoded);
                else
                    fprintf(stderr, ", datalen=%u", t->bytes);
                if (t->trunc)
                    fprintf(stderr, ", trunc=%d", t->trunc);
                if (*t->suffix)
                    fprintf(stderr, ", suffix='%s'", t->suffix);
                fprintf(stderr, ", linelen=%u", *llen);
                fprintf(stderr, ", wscount=%u", *wscnt);
                if (t->retry)
                    fprintf(stderr, ", retriable");
                if (t->no_nl)
                    fprintf(stderr, ", oneline");
                fputc('\n', stderr);
            }

            /* For verbatim input no effect from varying llen or wscnt */
            if (t->encoded)
                return ok;
        }
        /*
         * Longer 'llen' has no effect once we're sure to not have multiple
         * lines of data
         */
        if (*llen > t->bytes + (t->bytes >> 1))
            break;
    }
    return ok;
}

static int quotrem(int i, unsigned int m, int *q)
{
    *q = i / m;
    return i - *q * m;
}

static int test_bio_base64_generated(int idx)
{
    test_case t;
    int variant;
    int lencase;
    int padcase;
    int q = idx;

    lencase = quotrem(q, NLEN, &q);
    variant = quotrem(q, NVARPAD, &q);
    padcase = quotrem(variant, NPAD, &variant);
    t.retry = quotrem(q, 2, &q);
    t.no_nl = quotrem(q, 2, &q);

    if (q != 0) {
        fprintf(stderr, "Test index out of range: %d", idx);
        return 0;
    }

    t.prefix = prefixes[variant];
    t.encoded = NULL;
    t.bytes  = lengths[lencase];
    t.trunc = 0;
    if (padcase && padcase < 3)
        t.bytes  += padcase;
    else if (padcase >= 3)
        t.trunc = padcase - 2;
    t.suffix = suffixes[variant];

    if (padcase != 0 && (*t.suffix && *t.suffix != '-')) {
        TEST_error("Unexpected suffix test after padding");
        return 0;
    }

    return generic_case(&t, 0);
}

static int test_bio_base64_corner_case_bug(int idx)
{
    test_case t;
    int q = idx;

    t.retry = quotrem(q, 2, &q);
    t.no_nl = quotrem(q, 2, &q);

    if (q != 0) {
        fprintf(stderr, "Test index out of range: %d", idx);
        return 0;
    }

    /* 9 bytes of skipped non-base64 input + newline */
    t.prefix = "#foo\n#bar";

    /* 9 bytes on 2nd and subsequent lines */
    t.encoded = "A\nAAA\nAAAA\n";
    t.suffix = "";

    /* Expected decode length */
    t.bytes = 6;
    t.trunc = 0;    /* ignored */

    return generic_case(&t, 0);
}

int setup_tests(void)
{
    int numidx;

    memset(gunk, 'o', sizeof(gunk));
    gunk[0] = '#';
    gunk[sizeof(gunk) - 1] = '\0';

    /*
     * Test 5 variants of prefix or suffix
     *
     *  - both empty
     *  - short junk prefix
     *  - long gunk prefix (> internal BIO 1k buffer size),
     *  - soft EOF suffix
     *  - junk suffix (expect to detect an error)
     *
     * For 6 input lengths of randomly generated raw input:
     *
     *  0, 3, 48, 192, 768 and 1536
     *
     * corresponding to encoded lengths (plus linebreaks and ignored
     * whitespace) of:
     *
     *  0, 4, 64, 256, 1024 and 2048
     *
     * Followed by zero, one or two additional bytes that may involve padding,
     * or else (truncation) 1, 2 or 3 bytes with missing padding.
     * Only the first four variants make sense with padding or truncated
     * groups.
     *
     * With two types of underlying BIO
     *
     *  - Non-retriable underlying BIO
     *  - Retriable underlying BIO
     *
     * And with/without the BIO_FLAGS_BASE64_NO_NL flag, where now an error is
     * expected with the junk and gunk prefixes, however, but the "soft EOF"
     * suffix is still accepted.
     *
     * Internally, each test may loop over a range of encoded line lengths and
     * whitespace average "densities".
     */
    numidx = NLEN * (NVAR * NPAD - NPAD + 1) * 2 * 2;
    ADD_ALL_TESTS(test_bio_base64_generated, numidx);

    /*
     * Corner case in original code that skips ignored input, when the ignored
     * length is one byte longer than the total of the second and later lines
     * of valid input in the first 1k bytes of input.  No content variants,
     * just BIO retry status and oneline flags vary.
     */
    numidx = 2 * 2;
    ADD_ALL_TESTS(test_bio_base64_corner_case_bug, numidx);

    return 1;
}
