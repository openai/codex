/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-marshal-basic.h  Marshalling routines for basic (primitive) types
 *
 * Copyright (C) 2002  CodeFactory AB
 * Copyright (C) 2004, 2005  Red Hat, Inc.
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

#ifndef DBUS_MARSHAL_BASIC_H
#define DBUS_MARSHAL_BASIC_H

#ifdef HAVE_BYTESWAP_H
#include <byteswap.h>
#endif

#include <dbus/dbus-protocol.h>
#include <dbus/dbus-types.h>
#include <dbus/dbus-arch-deps.h>
#include <dbus/dbus-string.h>

#ifdef WORDS_BIGENDIAN
#define DBUS_COMPILER_BYTE_ORDER DBUS_BIG_ENDIAN
#else
#define DBUS_COMPILER_BYTE_ORDER DBUS_LITTLE_ENDIAN
#endif

#ifdef HAVE_BYTESWAP_H
#define DBUS_UINT16_SWAP_LE_BE_CONSTANT(val) bswap_16(val)
#define DBUS_UINT32_SWAP_LE_BE_CONSTANT(val) bswap_32(val)
#else /* HAVE_BYTESWAP_H */

#define DBUS_UINT16_SWAP_LE_BE_CONSTANT(val)	((dbus_uint16_t) (      \
    (dbus_uint16_t) ((dbus_uint16_t) (val) >> 8) |                      \
    (dbus_uint16_t) ((dbus_uint16_t) (val) << 8)))

#define DBUS_UINT32_SWAP_LE_BE_CONSTANT(val)	((dbus_uint32_t) (      \
    (((dbus_uint32_t) (val) & (dbus_uint32_t) 0x000000ffU) << 24) |     \
    (((dbus_uint32_t) (val) & (dbus_uint32_t) 0x0000ff00U) <<  8) |     \
    (((dbus_uint32_t) (val) & (dbus_uint32_t) 0x00ff0000U) >>  8) |     \
    (((dbus_uint32_t) (val) & (dbus_uint32_t) 0xff000000U) >> 24)))

#endif /* HAVE_BYTESWAP_H */

#ifdef HAVE_BYTESWAP_H
#define DBUS_UINT64_SWAP_LE_BE_CONSTANT(val) bswap_64(val)
#else /* HAVE_BYTESWAP_H */

#define DBUS_UINT64_SWAP_LE_BE_CONSTANT(val)	((dbus_uint64_t) (              \
      (((dbus_uint64_t) (val) &                                                 \
	(dbus_uint64_t) DBUS_UINT64_CONSTANT (0x00000000000000ff)) << 56) |    \
      (((dbus_uint64_t) (val) &                                                 \
	(dbus_uint64_t) DBUS_UINT64_CONSTANT (0x000000000000ff00)) << 40) |    \
      (((dbus_uint64_t) (val) &                                                 \
	(dbus_uint64_t) DBUS_UINT64_CONSTANT (0x0000000000ff0000)) << 24) |    \
      (((dbus_uint64_t) (val) &                                                 \
	(dbus_uint64_t) DBUS_UINT64_CONSTANT (0x00000000ff000000)) <<  8) |    \
      (((dbus_uint64_t) (val) &                                                 \
	(dbus_uint64_t) DBUS_UINT64_CONSTANT (0x000000ff00000000)) >>  8) |    \
      (((dbus_uint64_t) (val) &                                                 \
	(dbus_uint64_t) DBUS_UINT64_CONSTANT (0x0000ff0000000000)) >> 24) |    \
      (((dbus_uint64_t) (val) &                                                 \
	(dbus_uint64_t) DBUS_UINT64_CONSTANT (0x00ff000000000000)) >> 40) |    \
      (((dbus_uint64_t) (val) &                                                 \
	(dbus_uint64_t) DBUS_UINT64_CONSTANT (0xff00000000000000)) >> 56)))

#endif /* HAVE_BYTESWAP_H */

#define DBUS_UINT16_SWAP_LE_BE(val) (DBUS_UINT16_SWAP_LE_BE_CONSTANT (val))
#define DBUS_INT16_SWAP_LE_BE(val)  ((dbus_int16_t)DBUS_UINT16_SWAP_LE_BE_CONSTANT (val))

#define DBUS_UINT32_SWAP_LE_BE(val) (DBUS_UINT32_SWAP_LE_BE_CONSTANT (val))
#define DBUS_INT32_SWAP_LE_BE(val)  ((dbus_int32_t)DBUS_UINT32_SWAP_LE_BE_CONSTANT (val))

#define DBUS_UINT64_SWAP_LE_BE(val) (DBUS_UINT64_SWAP_LE_BE_CONSTANT (val))
#define DBUS_INT64_SWAP_LE_BE(val)  ((dbus_int64_t)DBUS_UINT64_SWAP_LE_BE_CONSTANT (val))

#ifdef WORDS_BIGENDIAN

#  define DBUS_INT16_TO_BE(val)	((dbus_int16_t) (val))
#  define DBUS_UINT16_TO_BE(val)	((dbus_uint16_t) (val))
#  define DBUS_INT16_TO_LE(val)	(DBUS_INT16_SWAP_LE_BE (val))
#  define DBUS_UINT16_TO_LE(val)	(DBUS_UINT16_SWAP_LE_BE (val))
#  define DBUS_INT32_TO_BE(val)	((dbus_int32_t) (val))
#  define DBUS_UINT32_TO_BE(val)	((dbus_uint32_t) (val))
#  define DBUS_INT32_TO_LE(val)	(DBUS_INT32_SWAP_LE_BE (val))
#  define DBUS_UINT32_TO_LE(val)	(DBUS_UINT32_SWAP_LE_BE (val))
#  define DBUS_INT64_TO_BE(val)	((dbus_int64_t) (val))
#  define DBUS_UINT64_TO_BE(val)	((dbus_uint64_t) (val))
#  define DBUS_INT64_TO_LE(val)	(DBUS_INT64_SWAP_LE_BE (val))
#  define DBUS_UINT64_TO_LE(val)	(DBUS_UINT64_SWAP_LE_BE (val))

#else /* WORDS_BIGENDIAN */

#  define DBUS_INT16_TO_LE(val)	((dbus_int16_t) (val))
#  define DBUS_UINT16_TO_LE(val)	((dbus_uint16_t) (val))
#  define DBUS_INT16_TO_BE(val)	((dbus_int16_t) DBUS_UINT16_SWAP_LE_BE (val))
#  define DBUS_UINT16_TO_BE(val)	(DBUS_UINT16_SWAP_LE_BE (val))
#  define DBUS_INT32_TO_LE(val)	((dbus_int32_t) (val))
#  define DBUS_UINT32_TO_LE(val)	((dbus_uint32_t) (val))
#  define DBUS_INT32_TO_BE(val)	((dbus_int32_t) DBUS_UINT32_SWAP_LE_BE (val))
#  define DBUS_UINT32_TO_BE(val)	(DBUS_UINT32_SWAP_LE_BE (val))
#  define DBUS_INT64_TO_LE(val)	((dbus_int64_t) (val))
#  define DBUS_UINT64_TO_LE(val)	((dbus_uint64_t) (val))
#  define DBUS_INT64_TO_BE(val)	((dbus_int64_t) DBUS_UINT64_SWAP_LE_BE (val))
#  define DBUS_UINT64_TO_BE(val)	(DBUS_UINT64_SWAP_LE_BE (val))
#endif

/* The transformation is symmetric, so the FROM just maps to the TO. */
#define DBUS_INT16_FROM_LE(val)	 (DBUS_INT16_TO_LE (val))
#define DBUS_UINT16_FROM_LE(val) (DBUS_UINT16_TO_LE (val))
#define DBUS_INT16_FROM_BE(val)	 (DBUS_INT16_TO_BE (val))
#define DBUS_UINT16_FROM_BE(val) (DBUS_UINT16_TO_BE (val))
#define DBUS_INT32_FROM_LE(val)	 (DBUS_INT32_TO_LE (val))
#define DBUS_UINT32_FROM_LE(val) (DBUS_UINT32_TO_LE (val))
#define DBUS_INT32_FROM_BE(val)	 (DBUS_INT32_TO_BE (val))
#define DBUS_UINT32_FROM_BE(val) (DBUS_UINT32_TO_BE (val))
#define DBUS_INT64_FROM_LE(val)	 (DBUS_INT64_TO_LE (val))
#define DBUS_UINT64_FROM_LE(val) (DBUS_UINT64_TO_LE (val))
#define DBUS_INT64_FROM_BE(val)	 (DBUS_INT64_TO_BE (val))
#define DBUS_UINT64_FROM_BE(val) (DBUS_UINT64_TO_BE (val))

#ifdef DBUS_DISABLE_ASSERT
#define _dbus_unpack_uint16(byte_order, data)           \
   (((byte_order) == DBUS_LITTLE_ENDIAN) ?              \
     DBUS_UINT16_FROM_LE (*(dbus_uint16_t*)(data)) :    \
     DBUS_UINT16_FROM_BE (*(dbus_uint16_t*)(data)))

#define _dbus_unpack_uint32(byte_order, data)           \
   (((byte_order) == DBUS_LITTLE_ENDIAN) ?              \
     DBUS_UINT32_FROM_LE (*(dbus_uint32_t*)(data)) :    \
     DBUS_UINT32_FROM_BE (*(dbus_uint32_t*)(data)))
#endif

#ifndef _dbus_unpack_uint16
DBUS_PRIVATE_EXPORT
dbus_uint16_t _dbus_unpack_uint16 (int                  byte_order,
                                   const unsigned char *data);
#endif

void          _dbus_pack_uint32   (dbus_uint32_t        value,
                                   int                  byte_order,
                                   unsigned char       *data);
#ifndef _dbus_unpack_uint32
DBUS_PRIVATE_EXPORT
dbus_uint32_t _dbus_unpack_uint32 (int                  byte_order,
                                   const unsigned char *data);
#endif

dbus_bool_t   _dbus_marshal_set_basic         (DBusString       *str,
                                               int               pos,
                                               int               type,
                                               const void       *value,
                                               int               byte_order,
                                               int              *old_end_pos,
                                               int              *new_end_pos);
dbus_bool_t   _dbus_marshal_write_basic       (DBusString       *str,
                                               int               insert_at,
                                               int               type,
                                               const void       *value,
                                               int               byte_order,
                                               int              *pos_after);
dbus_bool_t   _dbus_marshal_write_fixed_multi (DBusString       *str,
                                               int               insert_at,
                                               int               element_type,
                                               const void       *value,
                                               int               n_elements,
                                               int               byte_order,
                                               int              *pos_after);
void          _dbus_marshal_read_basic        (const DBusString *str,
                                               int               pos,
                                               int               type,
                                               void             *value,
                                               int               byte_order,
                                               int              *new_pos);
void          _dbus_marshal_read_fixed_multi  (const DBusString *str,
                                               int               pos,
                                               int               element_type,
                                               const void      **value,
                                               int               n_elements,
                                               int               byte_order,
                                               int              *new_pos);
void          _dbus_marshal_skip_basic        (const DBusString *str,
                                               int               type,
                                               int               byte_order,
                                               int              *pos);
void          _dbus_marshal_skip_array        (const DBusString *str,
                                               int               element_type,
                                               int               byte_order,
                                               int              *pos);
DBUS_PRIVATE_EXPORT
void          _dbus_marshal_set_uint32        (DBusString       *str,
                                               int               pos,
                                               dbus_uint32_t     value,
                                               int               byte_order);
DBUS_PRIVATE_EXPORT
dbus_uint32_t _dbus_marshal_read_uint32       (const DBusString *str,
                                               int               pos,
                                               int               byte_order,
                                               int              *new_pos);
int           _dbus_type_get_alignment        (int               typecode);
DBUS_PRIVATE_EXPORT
const char*   _dbus_type_to_string            (int               typecode);

DBUS_PRIVATE_EXPORT
int           _dbus_first_type_in_signature   (const DBusString *str,
                                               int               pos);

int           _dbus_first_type_in_signature_c_str   (const char       *str,
						     int               pos);

void _dbus_swap_array (unsigned char *data,
                       int            n_elements,
                       int            alignment);

#endif /* DBUS_MARSHAL_BASIC_H */
