/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-test-tap — TAP helpers for "embedded tests"
 *
 * Copyright © 2017 Collabora Ltd.
 *
 * Permission is hereby granted, free of charge, to any person
 * obtaining a copy of this software and associated documentation files
 * (the "Software"), to deal in the Software without restriction,
 * including without limitation the rights to use, copy, modify, merge,
 * publish, distribute, sublicense, and/or sell copies of the Software,
 * and to permit persons to whom the Software is furnished to do so,
 * subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be
 * included in all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
 * EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
 * MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
 * NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS
 * BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN
 * ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
 * CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

#include <config.h>
#include "dbus/dbus-test-tap.h"

/*
 * TAP, the Test Anything Protocol, is a text-based syntax for test-cases
 * to report results to test harnesses.
 *
 * See <http://testanything.org/> for details of the syntax, which
 * will not be explained here.
 */

#include <stdio.h>
#include <stdlib.h>

static unsigned int failures = 0;
static unsigned int skipped = 0;
static unsigned int tap_test_counter = 0;

/*
 * Output TAP indicating a fatal error, and exit unsuccessfully.
 */
void
_dbus_test_fatal (const char *format,
    ...)
{
  va_list ap;

  printf ("Bail out! ");
  va_start (ap, format);
  vprintf (format, ap);
  va_end (ap);
  printf ("\n");
  fflush (stdout);
  exit (1);
}

/*
 * Output TAP indicating a diagnostic (informational message).
 */
void
_dbus_test_diag (const char *format,
    ...)
{
  va_list ap;

  printf ("# ");
  va_start (ap, format);
  vprintf (format, ap);
  va_end (ap);
  printf ("\n");
  fflush (stdout);
}

/*
 * Output TAP indicating that all tests have been skipped, and exit
 * successfully.
 *
 * This is only valid if _dbus_test_ok(), _dbus_test_not_ok(),
 * etc. have not yet been called.
 */
void
_dbus_test_skip_all (const char *format,
    ...)
{
  va_list ap;

  _dbus_assert (tap_test_counter == 0);

  printf ("1..0 # SKIP - ");
  va_start (ap, format);
  vprintf (format, ap);
  va_end (ap);
  printf ("\n");
  fflush (stdout);
  exit (0);
}

/*
 * Output TAP indicating that a test has passed, and increment the
 * test counter.
 */
void
_dbus_test_ok (const char *format,
    ...)
{
  va_list ap;

  printf ("ok %u - ", ++tap_test_counter);
  va_start (ap, format);
  vprintf (format, ap);
  va_end (ap);
  printf ("\n");
  fflush (stdout);
}

/*
 * Output TAP indicating that a test has failed (in a way that is not
 * fatal to the test executable), and increment the test counter.
 */
void
_dbus_test_not_ok (const char *format,
    ...)
{
  va_list ap;

  printf ("not ok %u - ", ++tap_test_counter);
  va_start (ap, format);
  vprintf (format, ap);
  va_end (ap);
  printf ("\n");
  failures++;
  fflush (stdout);
}

/*
 * Output TAP indicating that a test has been skipped (in a way that is
 * not fatal to the test executable), and increment the test counter.
 */
void
_dbus_test_skip (const char *format,
    ...)
{
  va_list ap;

  printf ("ok %u # SKIP ", ++tap_test_counter);
  ++skipped;
  va_start (ap, format);
  vprintf (format, ap);
  va_end (ap);
  printf ("\n");
  fflush (stdout);
}

/*
 * Shut down libdbus, check that exactly previously_allocated memory
 * blocks are allocated, and output TAP indicating a test pass or failure.
 *
 * Return TRUE if no leaks were detected.
 */
void
_dbus_test_check_memleaks (const char *test_name)
{
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
  dbus_shutdown ();

  if (_dbus_get_malloc_blocks_outstanding () == 0)
    {
      printf ("ok %u - %s did not leak memory\n", ++tap_test_counter,
          test_name);
    }
  else
    {
      printf ("not ok %u - %s leaked %d blocks\n",
          ++tap_test_counter, test_name,
          _dbus_get_malloc_blocks_outstanding ());
      failures++;
    }
#else
  _dbus_test_skip (
      "unable to determine whether %s leaked memory (not compiled "
      "with memory instrumentation)",
      test_name);
#endif
}

/*
 * Output TAP indicating that testing has finished and no more tests
 * are going to be run. Return the Unix-style exit code.
 */
int
_dbus_test_done_testing (void)
{
  _dbus_assert (skipped <= tap_test_counter);

  if (failures == 0)
    _dbus_test_diag ("%u tests passed (%d skipped)",
                     tap_test_counter - skipped, skipped);
  else
    _dbus_test_diag ("%u/%u tests failed (%d skipped)",
                     failures, tap_test_counter - skipped, skipped);

  printf ("1..%u\n", tap_test_counter);
  fflush (stdout);

  if (failures == 0)
    return 0;

  return 1;
}
