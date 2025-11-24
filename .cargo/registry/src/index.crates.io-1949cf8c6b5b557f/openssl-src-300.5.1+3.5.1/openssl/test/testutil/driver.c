/*
 * Copyright 2016-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "../testutil.h"
#include "output.h"
#include "tu_local.h"

#include <string.h>
#include <assert.h>

#include "internal/nelem.h"
#include <openssl/bio.h>

#include "platform.h"            /* From libapps */

#if defined(_WIN32) && !defined(__BORLANDC__)
# define strdup _strdup
#endif


/*
 * Declares the structures needed to register each test case function.
 */
typedef struct test_info {
    const char *test_case_name;
    int (*test_fn) (void);
    int (*param_test_fn)(int idx);
    int num;

    /* flags */
    int subtest:1;
} TEST_INFO;

static TEST_INFO all_tests[1024];
static int num_tests = 0;
static int show_list = 0;
static int single_test = -1;
static int single_iter = -1;
static int level = 0;
static int seed = 0;
static int rand_order = 0;

/*
 * A parameterised test runs a loop of test cases.
 * |num_test_cases| counts the total number of non-subtest test cases
 * across all tests.
 */
static int num_test_cases = 0;

static int process_shared_options(void);


void add_test(const char *test_case_name, int (*test_fn) (void))
{
    assert(num_tests != OSSL_NELEM(all_tests));
    all_tests[num_tests].test_case_name = test_case_name;
    all_tests[num_tests].test_fn = test_fn;
    all_tests[num_tests].num = -1;
    ++num_tests;
    ++num_test_cases;
}

void add_all_tests(const char *test_case_name, int(*test_fn)(int idx),
                   int num, int subtest)
{
    assert(num_tests != OSSL_NELEM(all_tests));
    all_tests[num_tests].test_case_name = test_case_name;
    all_tests[num_tests].param_test_fn = test_fn;
    all_tests[num_tests].num = num;
    all_tests[num_tests].subtest = subtest;
    ++num_tests;
    if (subtest)
        ++num_test_cases;
    else
        num_test_cases += num;
}

static int gcd(int a, int b)
{
    while (b != 0) {
        int t = b;
        b = a % b;
        a = t;
    }
    return a;
}

static void set_seed(int s)
{
    seed = s;
    if (seed <= 0)
        seed = (int)time(NULL);
    test_random_seed(seed);
}


int setup_test_framework(int argc, char *argv[])
{
    char *test_rand_order = getenv("OPENSSL_TEST_RAND_ORDER");
    char *test_rand_seed = getenv("OPENSSL_TEST_RAND_SEED");
    char *TAP_levels = getenv("HARNESS_OSSL_LEVEL");

    if (TAP_levels != NULL)
        level = 4 * atoi(TAP_levels);
    test_adjust_streams_tap_level(level);
    if (test_rand_order != NULL) {
        rand_order = 1;
        set_seed(atoi(test_rand_order));
    } else if (test_rand_seed != NULL) {
        set_seed(atoi(test_rand_seed));
    } else {
        set_seed(0);
    }

#if defined(OPENSSL_SYS_VMS) && defined(__DECC)
    argv = copy_argv(&argc, argv);
#elif defined(_WIN32)
    /*
     * Replace argv[] with UTF-8 encoded strings.
     */
    win32_utf8argv(&argc, &argv);
#endif

    if (!opt_init(argc, argv, test_get_options()))
        return 0;
    return 1;
}


/*
 * This can only be called after setup() has run, since num_tests and
 * all_tests[] are setup at this point
 */
static int check_single_test_params(char *name, char *testname, char *itname)
{
    if (name != NULL) {
        int i;
        for (i = 0; i < num_tests; ++i) {
            if (strcmp(name, all_tests[i].test_case_name) == 0) {
                single_test = 1 + i;
                break;
            }
        }
        if (i >= num_tests)
            single_test = atoi(name);
    }


    /* if only iteration is specified, assume we want the first test */
    if (single_test == -1 && single_iter != -1)
        single_test = 1;

    if (single_test != -1) {
        if (single_test < 1 || single_test > num_tests) {
            test_printf_stderr("Invalid -%s value "
                               "(Value must be a valid test name OR a value between %d..%d)\n",
                               testname, 1, num_tests);
            return 0;
        }
    }
    if (single_iter != -1) {
        if (all_tests[single_test - 1].num == -1) {
            test_printf_stderr("-%s option is not valid for test %d:%s\n",
                               itname,
                               single_test,
                               all_tests[single_test - 1].test_case_name);
            return 0;
        } else if (single_iter < 1
                   || single_iter > all_tests[single_test - 1].num) {
            test_printf_stderr("Invalid -%s value for test %d:%s\t"
                               "(Value must be in the range %d..%d)\n",
                               itname, single_test,
                               all_tests[single_test - 1].test_case_name,
                               1, all_tests[single_test - 1].num);
            return 0;
        }
    }
    return 1;
}

static int process_shared_options(void)
{
    OPTION_CHOICE_DEFAULT o;
    int value;
    int ret = -1;
    char *flag_test = "";
    char *flag_iter = "";
    char *testname = NULL;

    opt_begin();
    while ((o = opt_next()) != OPT_EOF) {
        switch (o) {
        /* Ignore any test options at this level */
        default:
            break;
        case OPT_ERR:
            return ret;
        case OPT_TEST_HELP:
            opt_help(test_get_options());
            return 0;
        case OPT_TEST_LIST:
            show_list = 1;
            break;
        case OPT_TEST_SINGLE:
            flag_test = opt_flag();
            testname = opt_arg();
            break;
        case OPT_TEST_ITERATION:
            flag_iter = opt_flag();
            if (!opt_int(opt_arg(), &single_iter))
                goto end;
            break;
        case OPT_TEST_INDENT:
            if (!opt_int(opt_arg(), &value))
                goto end;
            level = 4 * value;
            test_adjust_streams_tap_level(level);
            break;
        case OPT_TEST_SEED:
            if (!opt_int(opt_arg(), &value))
                goto end;
            set_seed(value);
            break;
        }
    }
    if (!check_single_test_params(testname, flag_test, flag_iter))
        goto end;
    ret = 1;
end:
    return ret;
}


int pulldown_test_framework(int ret)
{
    set_test_title(NULL);
    return ret;
}

static void finalize(int success)
{
    if (success)
        ERR_clear_error();
    else
        ERR_print_errors_cb(openssl_error_cb, NULL);
}

static char *test_title = NULL;

void set_test_title(const char *title)
{
    free(test_title);
    test_title = title == NULL ? NULL : strdup(title);
}

PRINTF_FORMAT(2, 3) static void test_verdict(int verdict,
                                             const char *description, ...)
{
    va_list ap;

    test_flush_stdout();
    test_flush_stderr();

    if (verdict == 0) {
        if (rand_order)
            test_printf_tapout("# OPENSSL_TEST_RAND_ORDER=%d\n", seed);
        else
            test_printf_tapout("# OPENSSL_TEST_RAND_SEED=%d\n", seed);
    }
    test_printf_tapout("%s ", verdict != 0 ? "ok" : "not ok");
    va_start(ap, description);
    test_vprintf_tapout(description, ap);
    va_end(ap);
    if (verdict == TEST_SKIP_CODE)
        test_printf_tapout(" # skipped");
    test_printf_tapout("\n");
    test_flush_tapout();
}

int run_tests(const char *test_prog_name)
{
    int num_failed = 0;
    int verdict = 1;
    int ii, i, jj, j, jstep;
    int test_case_count = 0;
    int subtest_case_count = 0;
    int permute[OSSL_NELEM(all_tests)];

    i = process_shared_options();
    if (i == 0)
        return EXIT_SUCCESS;
    if (i == -1)
        return EXIT_FAILURE;

    if (num_tests < 1) {
        test_printf_tapout("1..0 # Skipped: %s\n", test_prog_name);
    } else if (show_list == 0 && single_test == -1) {
        if (level > 0) {
            test_printf_stdout("Subtest: %s\n", test_prog_name);
            test_flush_stdout();
        }
        test_printf_tapout("1..%d\n", num_test_cases);
    }

    test_flush_tapout();

    for (i = 0; i < num_tests; i++)
        permute[i] = i;
    if (rand_order != 0)
        for (i = num_tests - 1; i >= 1; i--) {
            j = test_random() % (1 + i);
            ii = permute[j];
            permute[j] = permute[i];
            permute[i] = ii;
        }

    for (ii = 0; ii != num_tests; ++ii) {
        i = permute[ii];

        if (single_test != -1 && ((i+1) != single_test)) {
            continue;
        }
        else if (show_list) {
            if (all_tests[i].num != -1) {
                test_printf_tapout("%d - %s (%d..%d)\n", ii + 1,
                                   all_tests[i].test_case_name, 1,
                                   all_tests[i].num);
            } else {
                test_printf_tapout("%d - %s\n", ii + 1,
                                   all_tests[i].test_case_name);
            }
            test_flush_tapout();
        } else if (all_tests[i].num == -1) {
            set_test_title(all_tests[i].test_case_name);
            ERR_clear_error();
            verdict = all_tests[i].test_fn();
            finalize(verdict != 0);
            test_verdict(verdict, "%d - %s", test_case_count + 1, test_title);
            if (verdict == 0)
                num_failed++;
            test_case_count++;
        } else {
            verdict = TEST_SKIP_CODE;
            set_test_title(all_tests[i].test_case_name);
            if (all_tests[i].subtest) {
                level += 4;
                test_adjust_streams_tap_level(level);
                if (single_iter == -1) {
                    test_printf_stdout("Subtest: %s\n", test_title);
                    test_printf_tapout("%d..%d\n", 1, all_tests[i].num);
                    test_flush_stdout();
                    test_flush_tapout();
                }
            }

            j = -1;
            if (rand_order == 0 || all_tests[i].num < 3)
                jstep = 1;
            else
                do
                    jstep = test_random() % all_tests[i].num;
                while (jstep == 0 || gcd(all_tests[i].num, jstep) != 1);

            for (jj = 0; jj < all_tests[i].num; jj++) {
                int v;

                j = (j + jstep) % all_tests[i].num;
                if (single_iter != -1 && ((jj + 1) != single_iter))
                    continue;
                ERR_clear_error();
                v = all_tests[i].param_test_fn(j);

                if (v == 0) {
                    verdict = 0;
                } else if (v != TEST_SKIP_CODE && verdict != 0) {
                    verdict = 1;
                }

                finalize(v != 0);

                if (all_tests[i].subtest)
                    test_verdict(v, "%d - iteration %d",
                                 subtest_case_count + 1, j + 1);
                else
                    test_verdict(v, "%d - %s - iteration %d",
                                 test_case_count + subtest_case_count + 1,
                                 test_title, j + 1);
                subtest_case_count++;
            }

            if (all_tests[i].subtest) {
                level -= 4;
                test_adjust_streams_tap_level(level);
            }
            if (verdict == 0)
                ++num_failed;
            if (all_tests[i].num == -1 || all_tests[i].subtest)
                test_verdict(verdict, "%d - %s", test_case_count + 1,
                             all_tests[i].test_case_name);
            test_case_count++;
        }
    }
    if (num_failed != 0)
        return EXIT_FAILURE;
    return EXIT_SUCCESS;
}

/*
 * Glue an array of strings together and return it as an allocated string.
 * Optionally return the whole length of this string in |out_len|
 */
char *glue_strings(const char *list[], size_t *out_len)
{
    size_t len = 0;
    char *p, *ret;
    int i;

    for (i = 0; list[i] != NULL; i++)
        len += strlen(list[i]);

    if (out_len != NULL)
        *out_len = len;

    if (!TEST_ptr(ret = p = OPENSSL_malloc(len + 1)))
        return NULL;

    for (i = 0; list[i] != NULL; i++)
        p += strlen(strcpy(p, list[i]));

    return ret;
}

char *test_mk_file_path(const char *dir, const char *file)
{
# ifndef OPENSSL_SYS_VMS
    const char *sep = "/";
# else
    const char *sep = "";
    char *dir_end;
    char dir_end_sep;
# endif
    size_t dirlen = dir != NULL ? strlen(dir) : 0;
    size_t len = dirlen + strlen(sep) + strlen(file) + 1;
    char *full_file = OPENSSL_zalloc(len);

    if (full_file != NULL) {
        if (dir != NULL && dirlen > 0) {
            OPENSSL_strlcpy(full_file, dir, len);
# ifdef OPENSSL_SYS_VMS
            /*
             * If |file| contains a directory spec, we need to do some
             * careful merging.
             * "vol:[dir.dir]" + "[.certs]sm2-root.crt" should become
             * "vol:[dir.dir.certs]sm2-root.crt"
             */
            dir_end = &full_file[strlen(full_file) - 1];
            dir_end_sep = *dir_end;
            if ((dir_end_sep == ']' || dir_end_sep == '>')
                && (file[0] == '[' || file[0] == '<')) {
                file++;
                if (file[0] == '.')
                    *dir_end = '\0';
                else
                    *dir_end = '.';
            }
#else
            OPENSSL_strlcat(full_file, sep, len);
#endif
        }
        OPENSSL_strlcat(full_file, file, len);
    }

    return full_file;
}
