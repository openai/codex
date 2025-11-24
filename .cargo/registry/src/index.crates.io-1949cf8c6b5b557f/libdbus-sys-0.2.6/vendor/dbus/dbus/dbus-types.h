/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-types.h  types such as dbus_bool_t
 *
 * Copyright (C) 2002  Red Hat Inc.
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
#if !defined (DBUS_INSIDE_DBUS_H) && !defined (DBUS_COMPILATION)
#error "Only <dbus/dbus.h> can be included directly, this file may disappear or change contents."
#endif

#ifndef DBUS_TYPES_H
#define DBUS_TYPES_H

#include <stddef.h>
#include <dbus/dbus-arch-deps.h>

typedef dbus_uint32_t  dbus_unichar_t;
/* boolean size must be fixed at 4 bytes due to wire protocol! */
typedef dbus_uint32_t  dbus_bool_t;

/* Normally docs are in .c files, but there isn't a .c file for this. */
/**
 * @defgroup DBusTypes Basic types
 * @ingroup  DBus
 * @brief dbus_bool_t, dbus_int32_t, etc.
 *
 * Typedefs for common primitive types.
 *
 * @{
 */

/**
 * @typedef dbus_bool_t
 *
 * A boolean, valid values are #TRUE and #FALSE.
 */

/**
 * @typedef dbus_uint32_t
 *
 * A 32-bit unsigned integer on all platforms.
 */

/**
 * @typedef dbus_int32_t
 *
 * A 32-bit signed integer on all platforms.
 */

/**
 * @typedef dbus_uint16_t
 *
 * A 16-bit unsigned integer on all platforms.
 */

/**
 * @typedef dbus_int16_t
 *
 * A 16-bit signed integer on all platforms.
 */


/**
 * @typedef dbus_uint64_t
 *
 * A 64-bit unsigned integer.
 */

/**
 * @typedef dbus_int64_t
 *
 * A 64-bit signed integer.
 */

/**
 * @def DBUS_HAVE_INT64
 *
 * Always defined.
 *
 * In older libdbus versions, this would be undefined if there was no
 * 64-bit integer type on that platform. libdbus no longer supports
 * such platforms.
 */

/**
 * @def DBUS_INT64_CONSTANT
 *
 * Declare a 64-bit signed integer constant. The macro
 * adds the necessary "LL" or whatever after the integer,
 * giving a literal such as "325145246765LL"
 */

/**
 * @def DBUS_UINT64_CONSTANT
 *
 * Declare a 64-bit unsigned integer constant. The macro
 * adds the necessary "ULL" or whatever after the integer,
 * giving a literal such as "325145246765ULL"
 */

/**
 * An 8-byte struct you could use to access int64 without having
 * int64 support. Use #dbus_int64_t or #dbus_uint64_t instead.
 */
typedef struct
{
  dbus_uint32_t first32;  /**< first 32 bits in the 8 bytes (beware endian issues) */
  dbus_uint32_t second32; /**< second 32 bits in the 8 bytes (beware endian issues) */
} DBus8ByteStruct;

/**
 * A simple value union that lets you access bytes as if they
 * were various types; useful when dealing with basic types via
 * void pointers and varargs.
 *
 * This union also contains a pointer member (which can be used
 * to retrieve a string from dbus_message_iter_get_basic(), for
 * instance), so on future platforms it could conceivably be larger
 * than 8 bytes.
 */
typedef union
{
  unsigned char bytes[8]; /**< as 8 individual bytes */
  dbus_int16_t  i16;   /**< as int16 */
  dbus_uint16_t u16;   /**< as int16 */
  dbus_int32_t  i32;   /**< as int32 */
  dbus_uint32_t u32;   /**< as int32 */
  dbus_bool_t   bool_val; /**< as boolean */
  dbus_int64_t  i64;   /**< as int64 */
  dbus_uint64_t u64;   /**< as int64 */
  DBus8ByteStruct eight; /**< as 8-byte struct */
  double dbl;          /**< as double */
  unsigned char byt;   /**< as byte */
  char *str;           /**< as char* (string, object path or signature) */
  int fd;              /**< as Unix file descriptor */
} DBusBasicValue;

/** @} */

#endif /* DBUS_TYPES_H */
