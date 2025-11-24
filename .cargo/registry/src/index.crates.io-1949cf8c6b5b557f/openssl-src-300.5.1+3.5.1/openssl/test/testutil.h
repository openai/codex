/*
 * Copyright 2014-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_TESTUTIL_H
# define OSSL_TESTUTIL_H

# include <stdarg.h>
# include "internal/common.h" /* for HAS_PREFIX */

# include <openssl/provider.h>
# include <openssl/err.h>
# include <openssl/e_os2.h>
# include <openssl/bn.h>
# include <openssl/x509.h>
# include "opt.h"

/*-
 * Simple unit tests should implement setup_tests().
 * This function should return zero if the registration process fails.
 * To register tests, call ADD_TEST or ADD_ALL_TESTS:
 *
 * int setup_tests(void)
 * {
 *     ADD_TEST(test_foo);
 *     ADD_ALL_TESTS(test_bar, num_test_bar);
 *     return 1;
 * }
 *
 * Tests that require clean up after execution should implement:
 *
 * void cleanup_tests(void);
 *
 * The cleanup_tests function will be called even if setup_tests()
 * returns failure.
 *
 * In some cases, early initialization before the framework is set up
 * may be needed.  In such a case, this should be implemented:
 *
 * int global_init(void);
 *
 * This function should return zero if there is an unrecoverable error and
 * non-zero if the initialization was successful.
 */

/* Adds a simple test case. */
# define ADD_TEST(test_function) add_test(#test_function, test_function)

/*
 * Simple parameterized tests. Calls test_function(idx) for each 0 <= idx < num.
 */
# define ADD_ALL_TESTS(test_function, num) \
    add_all_tests(#test_function, test_function, num, 1)
/*
 * A variant of the same without TAP output.
 */
# define ADD_ALL_TESTS_NOSUBTEST(test_function, num) \
    add_all_tests(#test_function, test_function, num, 0)

/*-
 * Test cases that share common setup should use the helper
 * SETUP_TEST_FIXTURE and EXECUTE_TEST macros for test case functions.
 *
 * SETUP_TEST_FIXTURE will call set_up() to create a new TEST_FIXTURE_TYPE
 * object called "fixture". It will also allocate the "result" variable used
 * by EXECUTE_TEST. set_up() should take a const char* specifying the test
 * case name and return a TEST_FIXTURE_TYPE by reference.
 * If case set_up() fails then 0 is returned.
 *
 * EXECUTE_TEST will pass fixture to execute_func() by reference, call
 * tear_down(), and return the result of execute_func(). execute_func() should
 * take a TEST_FIXTURE_TYPE by reference and return 1 on success and 0 on
 * failure.  The tear_down function is responsible for deallocation of the
 * result variable, if required.
 *
 * Unit tests can define their own SETUP_TEST_FIXTURE and EXECUTE_TEST
 * variations like so:
 *
 * #define SETUP_FOOBAR_TEST_FIXTURE()\
 *   SETUP_TEST_FIXTURE(FOOBAR_TEST_FIXTURE, set_up_foobar)
 *
 * #define EXECUTE_FOOBAR_TEST()\
 *   EXECUTE_TEST(execute_foobar, tear_down_foobar)
 *
 * Then test case functions can take the form:
 *
 * static int test_foobar_feature()
 *      {
 *      SETUP_FOOBAR_TEST_FIXTURE();
 *      [...set individual members of fixture...]
 *      EXECUTE_FOOBAR_TEST();
 *      }
 */
# define SETUP_TEST_FIXTURE(TEST_FIXTURE_TYPE, set_up)\
    TEST_FIXTURE_TYPE *fixture = set_up(TEST_CASE_NAME); \
    int result = 0; \
\
    if (fixture == NULL) \
        return 0


# define EXECUTE_TEST(execute_func, tear_down)\
    if (fixture != NULL) {\
        result = execute_func(fixture);\
        tear_down(fixture);\
    }

/*
 * TEST_CASE_NAME is defined as the name of the test case function where
 * possible; otherwise we get by with the file name and line number.
 */
# if !defined(__STDC_VERSION__) || __STDC_VERSION__ < 199901L
#  if defined(_MSC_VER)
#   define TEST_CASE_NAME __FUNCTION__
#  else
#   define testutil_stringify_helper(s) #s
#   define testutil_stringify(s) testutil_stringify_helper(s)
#   define TEST_CASE_NAME __FILE__ ":" testutil_stringify(__LINE__)
#  endif                        /* _MSC_VER */
# else
#  define TEST_CASE_NAME __func__
# endif                         /* __STDC_VERSION__ */


/* The default test enum which should be common to all tests */
# define OPT_TEST_ENUM \
    OPT_TEST_HELP = 500, \
    OPT_TEST_LIST, \
    OPT_TEST_SINGLE, \
    OPT_TEST_ITERATION, \
    OPT_TEST_INDENT, \
    OPT_TEST_SEED

/* The Default test OPTIONS common to all tests (without a usage string) */
# define OPT_TEST_OPTIONS \
    { OPT_HELP_STR, 1,  '-', "Valid options are:\n" }, \
    { "help", OPT_TEST_HELP, '-', "Display this summary" }, \
    { "list", OPT_TEST_LIST, '-', "Display the list of tests available" }, \
    { "test", OPT_TEST_SINGLE, 's', "Run a single test by id or name" }, \
    { "iter", OPT_TEST_ITERATION, 'n', "Run a single iteration of a test" }, \
    { "indent", OPT_TEST_INDENT,'p', "Number of tabs added to output" }, \
    { "seed", OPT_TEST_SEED, 'n', "Seed value to randomize tests with" }

/* The Default test OPTIONS common to all tests starting with an additional usage string */
# define OPT_TEST_OPTIONS_WITH_EXTRA_USAGE(usage) \
    { OPT_HELP_STR, 1, '-', "Usage: %s [options] " usage }, \
    OPT_TEST_OPTIONS

/* The Default test OPTIONS common to all tests with an default usage string */
# define OPT_TEST_OPTIONS_DEFAULT_USAGE \
    { OPT_HELP_STR, 1, '-', "Usage: %s [options]\n" }, \
    OPT_TEST_OPTIONS

/*
 * Optional Cases that need to be ignored by the test app when using opt_next(),
 * (that are handled internally).
 */
# define OPT_TEST_CASES \
         OPT_TEST_HELP: \
    case OPT_TEST_LIST: \
    case OPT_TEST_SINGLE: \
    case OPT_TEST_ITERATION: \
    case OPT_TEST_INDENT: \
    case OPT_TEST_SEED

/*
 * Tests that use test_get_argument() that dont have any additional options
 * (i.e- dont use opt_next()) can use this to set the usage string.
 * It embeds test_get_options() which gives default command line options for
 * the test system.
 *
 * Tests that need to use opt_next() need to specify
 *  (1) test_get_options() containing an options[] which should include either
 *    OPT_TEST_OPTIONS_DEFAULT_USAGE or
 *    OPT_TEST_OPTIONS_WITH_EXTRA_USAGE(...).
 *  (2) An enum outside the test_get_options() which contains OPT_TEST_ENUM, as
 *      well as the additional options that need to be handled.
 *  (3) case OPT_TEST_CASES: break; inside the opt_next() handling code.
 */
# define OPT_TEST_DECLARE_USAGE(usage_str) \
const OPTIONS *test_get_options(void) \
{ \
    enum { OPT_TEST_ENUM }; \
    static const OPTIONS options[] = { \
        OPT_TEST_OPTIONS_WITH_EXTRA_USAGE(usage_str), \
        { NULL } \
    }; \
    return options; \
}

/*
 * Used to read non optional command line values that follow after the options.
 * Returns NULL if there is no argument.
 */
char *test_get_argument(size_t n);
/* Return the number of additional non optional command line arguments */
size_t test_get_argument_count(void);

/*
 * Skip over common test options. Should be called before calling
 * test_get_argument()
 */
int test_skip_common_options(void);

/*
 * Get a library context for the tests, populated with the specified provider
 * and configuration. If default_null_prov is not NULL, a "null" provider is
 * loaded into the default library context to prevent it being used.
 * If libctx is NULL, the specified provider is loaded into the default library
 * context.
 */
int test_get_libctx(OSSL_LIB_CTX **libctx, OSSL_PROVIDER **default_null_prov,
                    const char *config_file,
                    OSSL_PROVIDER **provider, const char *module_name);
int test_arg_libctx(OSSL_LIB_CTX **libctx, OSSL_PROVIDER **default_null_prov,
                    OSSL_PROVIDER **provider, int argn, const char *usage);

/*
 * Internal helpers. Test programs shouldn't use these directly, but should
 * rather link to one of the helper main() methods.
 */

void add_test(const char *test_case_name, int (*test_fn) (void));
void add_all_tests(const char *test_case_name, int (*test_fn)(int idx), int num,
                   int subtest);

/*
 * Declarations for user defined functions.
 * The first two return a boolean indicating that the test should not proceed.
 */
int global_init(void);
int setup_tests(void);
void cleanup_tests(void);

/*
 * Helper functions to detect specific versions of the FIPS provider being in use.
 * Because of FIPS rules, code changes after a module has been validated are
 * difficult and because we provide a hard guarantee of ABI and behavioural
 * stability going forwards, it is a requirement to have tests be conditional
 * on specific FIPS provider versions.  Without this, bug fixes cannot be tested
 * in later releases.
 *
 * The reason for not including e.g. a less than test is to help avoid any
 * temptation to use FIPS provider version numbers that don't exist.  Until the
 * `new' provider is validated, its version isn't set in stone.  Thus a change
 * in test behaviour must depend on already validated module versions only.
 *
 * In all cases, the function returns true if:
 *      1. the FIPS provider version matches the criteria specified or
 *      2. the FIPS provider isn't being used.
 */
int fips_provider_version_eq(OSSL_LIB_CTX *libctx, int major, int minor, int patch);
int fips_provider_version_ne(OSSL_LIB_CTX *libctx, int major, int minor, int patch);
int fips_provider_version_le(OSSL_LIB_CTX *libctx, int major, int minor, int patch);
int fips_provider_version_lt(OSSL_LIB_CTX *libctx, int major, int minor, int patch);
int fips_provider_version_gt(OSSL_LIB_CTX *libctx, int major, int minor, int patch);
int fips_provider_version_ge(OSSL_LIB_CTX *libctx, int major, int minor, int patch);

/*
 * This function matches fips provider version with (potentially multiple)
 * <operator>maj.min.patch version strings in versions.
 * The operator can be one of = ! <= or > comparison symbols.
 * If the fips provider matches all the version comparisons (or if there is no
 * fips provider available) the function returns 1.
 * If the fips provider does not match the version comparisons, it returns 0.
 * On error the function returns -1.
 */
int fips_provider_version_match(OSSL_LIB_CTX *libctx, const char *versions);

/*
 * Used to supply test specific command line options,
 * If non optional parameters are used, then the first entry in the OPTIONS[]
 * should contain:
 * { OPT_HELP_STR, 1, '-', "<list of non-optional commandline params>\n"},
 * The last entry should always be { NULL }.
 *
 * Run the test locally using './test/test_name -help' to check the usage.
 */
const OPTIONS *test_get_options(void);

/*
 *  Test assumption verification helpers.
 */

# define PRINTF_FORMAT(a, b)
# if defined(__GNUC__) && defined(__STDC_VERSION__) \
    && !defined(__MINGW32__) && !defined(__MINGW64__) \
    && !defined(__APPLE__)
  /*
   * Because we support the 'z' modifier, which made its appearance in C99,
   * we can't use __attribute__ with pre C99 dialects.
   */
#  if __STDC_VERSION__ >= 199901L
#   undef PRINTF_FORMAT
#   define PRINTF_FORMAT(a, b)   __attribute__ ((format(printf, a, b)))
#  endif
# endif

# define DECLARE_COMPARISON(type, name, opname)                         \
    int test_ ## name ## _ ## opname(const char *, int,                 \
                                     const char *, const char *,        \
                                     const type, const type);

# define DECLARE_COMPARISONS(type, name)                                \
    DECLARE_COMPARISON(type, name, eq)                                  \
    DECLARE_COMPARISON(type, name, ne)                                  \
    DECLARE_COMPARISON(type, name, lt)                                  \
    DECLARE_COMPARISON(type, name, le)                                  \
    DECLARE_COMPARISON(type, name, gt)                                  \
    DECLARE_COMPARISON(type, name, ge)

DECLARE_COMPARISONS(int, int)
DECLARE_COMPARISONS(unsigned int, uint)
DECLARE_COMPARISONS(char, char)
DECLARE_COMPARISONS(unsigned char, uchar)
DECLARE_COMPARISONS(long, long)
DECLARE_COMPARISONS(unsigned long, ulong)
DECLARE_COMPARISONS(int64_t, int64_t)
DECLARE_COMPARISONS(uint64_t, uint64_t)
DECLARE_COMPARISONS(double, double)
DECLARE_COMPARISONS(time_t, time_t)

/*
 * Because this comparison uses a printf format specifier that's not
 * universally known (yet), we provide an option to not have it declared.
 */
# ifndef TESTUTIL_NO_size_t_COMPARISON
DECLARE_COMPARISONS(size_t, size_t)
# endif

/*
 * Pointer comparisons against other pointers and null.
 * These functions return 1 if the test is true.
 * Otherwise, they return 0 and pretty-print diagnostics.
 * These should not be called directly, use the TEST_xxx macros below instead.
 */
DECLARE_COMPARISON(void *, ptr, eq)
DECLARE_COMPARISON(void *, ptr, ne)
int test_ptr(const char *file, int line, const char *s, const void *p);
int test_ptr_null(const char *file, int line, const char *s, const void *p);

/*
 * Equality tests for strings where NULL is a legitimate value.
 * These calls return 1 if the two passed strings compare true.
 * Otherwise, they return 0 and pretty-print diagnostics.
 * These should not be called directly, use the TEST_xxx macros below instead.
 */
DECLARE_COMPARISON(char *, str, eq)
DECLARE_COMPARISON(char *, str, ne)

/*
 * Same as above, but for strncmp.
 */
int test_strn_eq(const char *file, int line, const char *, const char *,
                 const char *a, size_t an, const char *b, size_t bn);
int test_strn_ne(const char *file, int line, const char *, const char *,
                 const char *a, size_t an, const char *b, size_t bn);

/*
 * Equality test for memory blocks where NULL is a legitimate value.
 * These calls return 1 if the two memory blocks compare true.
 * Otherwise, they return 0 and pretty-print diagnostics.
 * These should not be called directly, use the TEST_xxx macros below instead.
 */
int test_mem_eq(const char *, int, const char *, const char *,
                const void *, size_t, const void *, size_t);
int test_mem_ne(const char *, int, const char *, const char *,
                const void *, size_t, const void *, size_t);

/*
 * Check a boolean result for being true or false.
 * They return 1 if the condition is true (i.e. the value is non-zero).
 * Otherwise, they return 0 and pretty-prints diagnostics using |s|.
 * These should not be called directly, use the TEST_xxx macros below instead.
 */
int test_true(const char *file, int line, const char *s, int b);
int test_false(const char *file, int line, const char *s, int b);

/*
 * Comparisons between BIGNUMs.
 * BIGNUMS can be compared against other BIGNUMs or zero.
 * Some additional equality tests against 1 & specific values are provided.
 * Tests for parity are included as well.
 */
DECLARE_COMPARISONS(BIGNUM *, BN)
int test_BN_eq_zero(const char *file, int line, const char *s, const BIGNUM *a);
int test_BN_ne_zero(const char *file, int line, const char *s, const BIGNUM *a);
int test_BN_lt_zero(const char *file, int line, const char *s, const BIGNUM *a);
int test_BN_le_zero(const char *file, int line, const char *s, const BIGNUM *a);
int test_BN_gt_zero(const char *file, int line, const char *s, const BIGNUM *a);
int test_BN_ge_zero(const char *file, int line, const char *s, const BIGNUM *a);
int test_BN_eq_one(const char *file, int line, const char *s, const BIGNUM *a);
int test_BN_odd(const char *file, int line, const char *s, const BIGNUM *a);
int test_BN_even(const char *file, int line, const char *s, const BIGNUM *a);
int test_BN_eq_word(const char *file, int line, const char *bns, const char *ws,
                    const BIGNUM *a, BN_ULONG w);
int test_BN_abs_eq_word(const char *file, int line, const char *bns,
                        const char *ws, const BIGNUM *a, BN_ULONG w);

/*
 * Pretty print a failure message.
 * These should not be called directly, use the TEST_xxx macros below instead.
 */
void test_error(const char *file, int line, const char *desc, ...)
    PRINTF_FORMAT(3, 4);
void test_error_c90(const char *desc, ...) PRINTF_FORMAT(1, 2);
void test_info(const char *file, int line, const char *desc, ...)
    PRINTF_FORMAT(3, 4);
void test_info_c90(const char *desc, ...) PRINTF_FORMAT(1, 2);
void test_note(const char *desc, ...) PRINTF_FORMAT(1, 2);
int test_skip(const char *file, int line, const char *desc, ...)
    PRINTF_FORMAT(3, 4);
int test_skip_c90(const char *desc, ...) PRINTF_FORMAT(1, 2);
void test_openssl_errors(void);
void test_perror(const char *s);

/*
 * The following macros provide wrapper calls to the test functions with
 * a default description that indicates the file and line number of the error.
 *
 * The following macros guarantee to evaluate each argument exactly once.
 * This allows constructs such as: if (!TEST_ptr(ptr = OPENSSL_malloc(..)))
 * to produce better contextual output than:
 *      ptr = OPENSSL_malloc(..);
 *      if (!TEST_ptr(ptr))
 */
# define TEST_int_eq(a, b)    test_int_eq(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_int_ne(a, b)    test_int_ne(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_int_lt(a, b)    test_int_lt(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_int_le(a, b)    test_int_le(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_int_gt(a, b)    test_int_gt(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_int_ge(a, b)    test_int_ge(__FILE__, __LINE__, #a, #b, a, b)

# define TEST_uint_eq(a, b)   test_uint_eq(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_uint_ne(a, b)   test_uint_ne(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_uint_lt(a, b)   test_uint_lt(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_uint_le(a, b)   test_uint_le(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_uint_gt(a, b)   test_uint_gt(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_uint_ge(a, b)   test_uint_ge(__FILE__, __LINE__, #a, #b, a, b)

# define TEST_char_eq(a, b)   test_char_eq(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_char_ne(a, b)   test_char_ne(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_char_lt(a, b)   test_char_lt(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_char_le(a, b)   test_char_le(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_char_gt(a, b)   test_char_gt(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_char_ge(a, b)   test_char_ge(__FILE__, __LINE__, #a, #b, a, b)

# define TEST_uchar_eq(a, b)  test_uchar_eq(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_uchar_ne(a, b)  test_uchar_ne(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_uchar_lt(a, b)  test_uchar_lt(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_uchar_le(a, b)  test_uchar_le(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_uchar_gt(a, b)  test_uchar_gt(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_uchar_ge(a, b)  test_uchar_ge(__FILE__, __LINE__, #a, #b, a, b)

# define TEST_long_eq(a, b)   test_long_eq(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_long_ne(a, b)   test_long_ne(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_long_lt(a, b)   test_long_lt(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_long_le(a, b)   test_long_le(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_long_gt(a, b)   test_long_gt(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_long_ge(a, b)   test_long_ge(__FILE__, __LINE__, #a, #b, a, b)

# define TEST_ulong_eq(a, b)  test_ulong_eq(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_ulong_ne(a, b)  test_ulong_ne(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_ulong_lt(a, b)  test_ulong_lt(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_ulong_le(a, b)  test_ulong_le(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_ulong_gt(a, b)  test_ulong_gt(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_ulong_ge(a, b)  test_ulong_ge(__FILE__, __LINE__, #a, #b, a, b)

# define TEST_int64_t_eq(a, b)  test_int64_t_eq(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_int64_t_ne(a, b)  test_int64_t_ne(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_int64_t_lt(a, b)  test_int64_t_lt(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_int64_t_le(a, b)  test_int64_t_le(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_int64_t_gt(a, b)  test_int64_t_gt(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_int64_t_ge(a, b)  test_int64_t_ge(__FILE__, __LINE__, #a, #b, a, b)

# define TEST_uint64_t_eq(a, b)  test_uint64_t_eq(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_uint64_t_ne(a, b)  test_uint64_t_ne(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_uint64_t_lt(a, b)  test_uint64_t_lt(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_uint64_t_le(a, b)  test_uint64_t_le(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_uint64_t_gt(a, b)  test_uint64_t_gt(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_uint64_t_ge(a, b)  test_uint64_t_ge(__FILE__, __LINE__, #a, #b, a, b)

# define TEST_size_t_eq(a, b) test_size_t_eq(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_size_t_ne(a, b) test_size_t_ne(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_size_t_lt(a, b) test_size_t_lt(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_size_t_le(a, b) test_size_t_le(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_size_t_gt(a, b) test_size_t_gt(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_size_t_ge(a, b) test_size_t_ge(__FILE__, __LINE__, #a, #b, a, b)

# define TEST_double_eq(a, b) test_double_eq(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_double_ne(a, b) test_double_ne(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_double_lt(a, b) test_double_lt(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_double_le(a, b) test_double_le(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_double_gt(a, b) test_double_gt(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_double_ge(a, b) test_double_ge(__FILE__, __LINE__, #a, #b, a, b)

# define TEST_time_t_eq(a, b) test_time_t_eq(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_time_t_ne(a, b) test_time_t_ne(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_time_t_lt(a, b) test_time_t_lt(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_time_t_le(a, b) test_time_t_le(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_time_t_gt(a, b) test_time_t_gt(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_time_t_ge(a, b) test_time_t_ge(__FILE__, __LINE__, #a, #b, a, b)

# define TEST_ptr_eq(a, b)    test_ptr_eq(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_ptr_ne(a, b)    test_ptr_ne(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_ptr(a)          test_ptr(__FILE__, __LINE__, #a, a)
# define TEST_ptr_null(a)     test_ptr_null(__FILE__, __LINE__, #a, a)

# define TEST_str_eq(a, b)    test_str_eq(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_str_ne(a, b)    test_str_ne(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_strn_eq(a, b, n) test_strn_eq(__FILE__, __LINE__, #a, #b, a, n, b, n)
# define TEST_strn_ne(a, b, n) test_strn_ne(__FILE__, __LINE__, #a, #b, a, n, b, n)
# define TEST_strn2_eq(a, m, b, n) test_strn_eq(__FILE__, __LINE__, #a, #b, a, m, b, n)
# define TEST_strn2_ne(a, m, b, n) test_strn_ne(__FILE__, __LINE__, #a, #b, a, m, b, n)

# define TEST_mem_eq(a, m, b, n) test_mem_eq(__FILE__, __LINE__, #a, #b, a, m, b, n)
# define TEST_mem_ne(a, m, b, n) test_mem_ne(__FILE__, __LINE__, #a, #b, a, m, b, n)

# define TEST_true(a)         test_true(__FILE__, __LINE__, #a, (a) != 0)
# define TEST_false(a)        test_false(__FILE__, __LINE__, #a, (a) != 0)

# define TEST_BN_eq(a, b)     test_BN_eq(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_BN_ne(a, b)     test_BN_ne(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_BN_lt(a, b)     test_BN_lt(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_BN_gt(a, b)     test_BN_gt(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_BN_le(a, b)     test_BN_le(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_BN_ge(a, b)     test_BN_ge(__FILE__, __LINE__, #a, #b, a, b)
# define TEST_BN_eq_zero(a)   test_BN_eq_zero(__FILE__, __LINE__, #a, a)
# define TEST_BN_ne_zero(a)   test_BN_ne_zero(__FILE__, __LINE__, #a, a)
# define TEST_BN_lt_zero(a)   test_BN_lt_zero(__FILE__, __LINE__, #a, a)
# define TEST_BN_gt_zero(a)   test_BN_gt_zero(__FILE__, __LINE__, #a, a)
# define TEST_BN_le_zero(a)   test_BN_le_zero(__FILE__, __LINE__, #a, a)
# define TEST_BN_ge_zero(a)   test_BN_ge_zero(__FILE__, __LINE__, #a, a)
# define TEST_BN_eq_one(a)    test_BN_eq_one(__FILE__, __LINE__, #a, a)
# define TEST_BN_eq_word(a, w) test_BN_eq_word(__FILE__, __LINE__, #a, #w, a, w)
# define TEST_BN_abs_eq_word(a, w) test_BN_abs_eq_word(__FILE__, __LINE__, #a, #w, a, w)
# define TEST_BN_odd(a)       test_BN_odd(__FILE__, __LINE__, #a, a)
# define TEST_BN_even(a)      test_BN_even(__FILE__, __LINE__, #a, a)

/*
 * TEST_error(desc, ...) prints an informative error message in the standard
 * format.  |desc| is a printf format string.
 */
# if !defined(__STDC_VERSION__) || __STDC_VERSION__ < 199901L
#  define TEST_error         test_error_c90
#  define TEST_info          test_info_c90
#  define TEST_skip          test_skip_c90
# else
#  define TEST_error(...)    test_error(__FILE__, __LINE__, __VA_ARGS__)
#  define TEST_info(...)     test_info(__FILE__, __LINE__, __VA_ARGS__)
#  define TEST_skip(...)     test_skip(__FILE__, __LINE__, __VA_ARGS__)
# endif
# define TEST_note           test_note
# define TEST_openssl_errors test_openssl_errors
# define TEST_perror         test_perror

extern BIO *bio_out;
extern BIO *bio_err;

/* Thread local BIO overrides. */
int set_override_bio_out(BIO *bio);
int set_override_bio_err(BIO *bio);

/*
 * Formatted output for strings, memory and bignums.
 */
void test_output_string(const char *name, const char *m, size_t l);
void test_output_bignum(const char *name, const BIGNUM *bn);
void test_output_memory(const char *name, const unsigned char *m, size_t l);


/*
 * Utilities to parse a test file.
 */
# define TESTMAXPAIRS        150

typedef struct pair_st {
    char *key;
    char *value;
} PAIR;

typedef struct stanza_st {
    const char *test_file;      /* Input file name */
    BIO *fp;                    /* Input file */
    int curr;                   /* Current line in file */
    int start;                  /* Line where test starts */
    int errors;                 /* Error count */
    int numtests;               /* Number of tests */
    int numskip;                /* Number of skipped tests */
    int numpairs;
    PAIR pairs[TESTMAXPAIRS];
    BIO *key;                   /* temp memory BIO for reading in keys */
} STANZA;

/*
 * Prepare to start reading the file |testfile| as input.
 */
int test_start_file(STANZA *s, const char *testfile);
int test_end_file(STANZA *s);

/*
 * Read a stanza from the test file.  A stanza consists of a block
 * of lines of the form
 *      key = value
 * The block is terminated by EOF or a blank line.
 * Return 1 if found, 0 on EOF or error.
 */
int test_readstanza(STANZA *s);

/*
 * Clear a stanza, release all allocated memory.
 */
void test_clearstanza(STANZA *s);

/*
 * Glue an array of strings together and return it as an allocated string.
 * Optionally return the whole length of this string in |out_len|
 */
char *glue_strings(const char *list[], size_t *out_len);

/*
 * Pseudo random number generator of low quality but having repeatability
 * across platforms.  The two calls are replacements for random(3) and
 * srandom(3).
 */
uint32_t test_random(void);
void test_random_seed(uint32_t sd);

/* Fake non-secure random number generator */
typedef int fake_random_generate_cb(unsigned char *out, size_t outlen,
                                    const char *name, EVP_RAND_CTX *ctx);

OSSL_PROVIDER *fake_rand_start(OSSL_LIB_CTX *libctx);
void fake_rand_finish(OSSL_PROVIDER *p);
void fake_rand_set_callback(EVP_RAND_CTX *ctx,
                            int (*cb)(unsigned char *out, size_t outlen,
                                      const char *name, EVP_RAND_CTX *ctx));
void fake_rand_set_public_private_callbacks(OSSL_LIB_CTX *libctx,
                                            fake_random_generate_cb *cb);

/* Create a file path from a directory and a filename */
char *test_mk_file_path(const char *dir, const char *file);

EVP_PKEY *load_pkey_pem(const char *file, OSSL_LIB_CTX *libctx);
X509 *load_cert_pem(const char *file, OSSL_LIB_CTX *libctx);
X509 *load_cert_der(const unsigned char *bytes, int len);
STACK_OF(X509) *load_certs_pem(const char *file);
X509_REQ *load_csr_der(const char *file, OSSL_LIB_CTX *libctx);
time_t test_asn1_string_to_time_t(const char *asn1_string);
#endif                          /* OSSL_TESTUTIL_H */
