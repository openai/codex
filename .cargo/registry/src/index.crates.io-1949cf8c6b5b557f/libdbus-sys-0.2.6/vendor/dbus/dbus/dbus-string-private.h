/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-string-private.h String utility class (internal to D-Bus implementation)
 *
 * Copyright (C) 2002, 2003  Red Hat, Inc.
 *
 * Licensed under the Academic Free License version 2.1
 *
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; either version 2 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program; if not, write to the Free Software
 * Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301  USA
 *
 */

#ifndef DBUS_STRING_PRIVATE_H
#define DBUS_STRING_PRIVATE_H

#include <dbus/dbus-internals.h>
#include <dbus/dbus-memory.h>
#include <dbus/dbus-types.h>

#ifndef DBUS_CAN_USE_DBUS_STRING_PRIVATE
#error "Don't go including dbus-string-private.h for no good reason"
#endif

DBUS_BEGIN_DECLS

/**
 * @brief Internals of DBusString.
 * 
 * DBusString internals. DBusString is an opaque objects, it must be
 * used via accessor functions.
 */
typedef struct
{
  unsigned char *str;            /**< String data, plus nul termination */
  int            len;            /**< Length without nul */
  int            allocated;      /**< Allocated size of data */
  unsigned int   constant : 1;   /**< String data is not owned by DBusString */
  unsigned int   locked : 1;     /**< DBusString has been locked and can't be changed */
  unsigned int   valid : 1;      /**< DBusString is valid (initialized and not freed) */
  unsigned int   align_offset : 3; /**< str - align_offset is the actual malloc block */
} DBusRealString;

_DBUS_STATIC_ASSERT (sizeof (DBusRealString) == sizeof (DBusString));

/**
 * @defgroup DBusStringInternals DBusString implementation details
 * @ingroup  DBusInternals
 * @brief DBusString implementation details
 *
 * The guts of DBusString.
 *
 * @{
 */

/**
 * The maximum length of a DBusString
 */
#define _DBUS_STRING_MAX_LENGTH (_DBUS_INT32_MAX - _DBUS_STRING_ALLOCATION_PADDING)

/**
 * Checks a bunch of assertions about a string object
 *
 * @param real the DBusRealString
 */
#define DBUS_GENERIC_STRING_PREAMBLE(real) \
  do { \
      (void) real; /* might be unused unless asserting */ \
      _dbus_assert ((real) != NULL); \
      _dbus_assert ((real)->valid); \
      _dbus_assert ((real)->len >= 0); \
      _dbus_assert ((real)->allocated >= 0); \
      _dbus_assert ((real)->len <= ((real)->allocated - _DBUS_STRING_ALLOCATION_PADDING)); \
      _dbus_assert ((real)->len <= _DBUS_STRING_MAX_LENGTH); \
  } while (0)

/**
 * Checks assertions about a string object that needs to be
 * modifiable - may not be locked or const. Also declares
 * the "real" variable pointing to DBusRealString. 
 * @param str the string
 */
#define DBUS_STRING_PREAMBLE(str) DBusRealString *real = (DBusRealString*) str; \
  DBUS_GENERIC_STRING_PREAMBLE (real);                                          \
  _dbus_assert (!(real)->constant);                                             \
  _dbus_assert (!(real)->locked)

/**
 * Checks assertions about a string object that may be locked but
 * can't be const. i.e. a string object that we can free.  Also
 * declares the "real" variable pointing to DBusRealString.
 *
 * @param str the string
 */
#define DBUS_LOCKED_STRING_PREAMBLE(str) DBusRealString *real = (DBusRealString*) str; \
  DBUS_GENERIC_STRING_PREAMBLE (real);                                                 \
  _dbus_assert (!(real)->constant)

/**
 * Checks assertions about a string that may be const or locked.  Also
 * declares the "real" variable pointing to DBusRealString.
 * @param str the string.
 */
#define DBUS_CONST_STRING_PREAMBLE(str) const DBusRealString *real = (DBusRealString*) str; \
  DBUS_GENERIC_STRING_PREAMBLE (real)

/**
 * Checks for ASCII blank byte
 * @param c the byte
 */
#define DBUS_IS_ASCII_BLANK(c) ((c) == ' ' || (c) == '\t')

/**
 * Checks for ASCII whitespace byte
 * @param c the byte
 */
#define DBUS_IS_ASCII_WHITE(c) ((c) == ' ' || (c) == '\t' || (c) == '\n' || (c) == '\r')

/** @} */

DBUS_END_DECLS

#endif /* DBUS_STRING_PRIVATE_H */
