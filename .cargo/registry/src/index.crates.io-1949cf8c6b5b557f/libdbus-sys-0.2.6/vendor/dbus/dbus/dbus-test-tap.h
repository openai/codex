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

#ifndef DBUS_TEST_TAP_H
#define DBUS_TEST_TAP_H

#include <dbus/dbus-internals.h>

DBUS_EMBEDDED_TESTS_EXPORT
void _dbus_test_fatal (const char *format,
    ...) _DBUS_GNUC_NORETURN _DBUS_GNUC_PRINTF (1, 2);

DBUS_EMBEDDED_TESTS_EXPORT
void _dbus_test_diag (const char *format,
    ...) _DBUS_GNUC_PRINTF (1, 2);

DBUS_EMBEDDED_TESTS_EXPORT
void _dbus_test_skip_all (const char *format,
    ...) _DBUS_GNUC_NORETURN _DBUS_GNUC_PRINTF (1, 2);

DBUS_EMBEDDED_TESTS_EXPORT
void _dbus_test_ok (const char *format,
    ...) _DBUS_GNUC_PRINTF (1, 2);
DBUS_EMBEDDED_TESTS_EXPORT
void _dbus_test_not_ok (const char *format,
    ...) _DBUS_GNUC_PRINTF (1, 2);
DBUS_EMBEDDED_TESTS_EXPORT
void _dbus_test_skip (const char *format,
    ...) _DBUS_GNUC_PRINTF (1, 2);

DBUS_EMBEDDED_TESTS_EXPORT
void _dbus_test_check_memleaks (const char *test_name);

DBUS_EMBEDDED_TESTS_EXPORT
int _dbus_test_done_testing (void);

#define _dbus_test_check(a) do { \
    if (!(a)) \
      _dbus_test_not_ok ("%s:%d - '%s' failed\n", __FILE__, __LINE__, #a); \
  } while (0)

#endif
