/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-marshal-recursive.h  Marshalling routines for recursive types
 *
 * Copyright (C) 2004, 2005 Red Hat, Inc.
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

#ifndef DBUS_MARSHAL_RECURSIVE_H
#define DBUS_MARSHAL_RECURSIVE_H

#include <dbus/dbus-protocol.h>
#include <dbus/dbus-list.h>

typedef struct DBusTypeReader      DBusTypeReader;
typedef struct DBusTypeWriter      DBusTypeWriter;
typedef struct DBusTypeReaderClass DBusTypeReaderClass;
typedef struct DBusArrayLenFixup   DBusArrayLenFixup;

/**
 * The type reader is an iterator for reading values from a block of
 * values.
 */
struct DBusTypeReader
{
  dbus_uint32_t byte_order : 8; /**< byte order of the block */

  dbus_uint32_t finished : 1;   /**< marks we're at end iterator for cases
                                 * where we don't have another way to tell
                                 */
  dbus_uint32_t array_len_offset : 3; /**< bytes back from start_pos that len ends */
  const DBusString *type_str;   /**< string containing signature of block */
  int type_pos;                 /**< current position in signature */
  const DBusString *value_str;  /**< string containing values of block */
  int value_pos;                /**< current position in values */

  const DBusTypeReaderClass *klass; /**< the vtable for the reader */
  union
  {
    struct {
      int start_pos;                /**< for array readers, the start of the array values */
    } array;
  } u; /**< class-specific data */
};

/**
 * The type writer is an iterator for writing to a block of values.
 */
struct DBusTypeWriter
{
  dbus_uint32_t byte_order : 8;            /**< byte order to write values with */

  dbus_uint32_t container_type : 8;        /**< what are we inside? (e.g. struct, variant, array) */

  dbus_uint32_t type_pos_is_expectation : 1; /**< type_pos can be either an insertion point for or an expected next type */

  dbus_uint32_t enabled : 1; /**< whether to write values */

  DBusString *type_str; /**< where to write typecodes (or read type expectations) */
  int type_pos;         /**< current pos in type_str */
  DBusString *value_str; /**< where to write values */
  int value_pos;         /**< next position to write */

  union
  {
    struct {
      int start_pos; /**< position of first element in the array */
      int len_pos;   /**< position of length of the array */
      int element_type_pos; /**< position of array element type in type_str */
    } array;
  } u; /**< class-specific data */
};

/**
 * When modifying an existing block of values, array lengths may need
 * to be adjusted; those adjustments are described by this struct.
 */
struct DBusArrayLenFixup
{
  int len_pos_in_reader; /**< where the length was in the original block */
  int new_len;           /**< the new value of the length in the written-out block */
};

DBUS_PRIVATE_EXPORT
void        _dbus_type_reader_init                      (DBusTypeReader        *reader,
                                                         int                    byte_order,
                                                         const DBusString      *type_str,
                                                         int                    type_pos,
                                                         const DBusString      *value_str,
                                                         int                    value_pos);
DBUS_PRIVATE_EXPORT
void        _dbus_type_reader_init_types_only           (DBusTypeReader        *reader,
                                                         const DBusString      *type_str,
                                                         int                    type_pos);
DBUS_PRIVATE_EXPORT
int         _dbus_type_reader_get_current_type          (const DBusTypeReader  *reader);
DBUS_PRIVATE_EXPORT
int         _dbus_type_reader_get_element_type          (const DBusTypeReader  *reader);
DBUS_PRIVATE_EXPORT
int         _dbus_type_reader_get_value_pos             (const DBusTypeReader  *reader);
DBUS_PRIVATE_EXPORT
void        _dbus_type_reader_read_basic                (const DBusTypeReader  *reader,
                                                         void                  *value);
int         _dbus_type_reader_get_array_length          (const DBusTypeReader  *reader);
DBUS_PRIVATE_EXPORT
void        _dbus_type_reader_read_fixed_multi          (const DBusTypeReader  *reader,
                                                         const void           **value,
                                                         int                   *n_elements);
void        _dbus_type_reader_read_raw                  (const DBusTypeReader  *reader,
                                                         const unsigned char  **value_location);
DBUS_PRIVATE_EXPORT
void        _dbus_type_reader_recurse                   (DBusTypeReader        *reader,
                                                         DBusTypeReader        *subreader);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_type_reader_next                      (DBusTypeReader        *reader);
dbus_bool_t _dbus_type_reader_has_next                  (const DBusTypeReader  *reader);
DBUS_PRIVATE_EXPORT
void        _dbus_type_reader_get_signature             (const DBusTypeReader  *reader,
                                                         const DBusString     **str_p,
                                                         int                   *start_p,
                                                         int                   *len_p);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_type_reader_set_basic                 (DBusTypeReader        *reader,
                                                         const void            *value,
                                                         const DBusTypeReader  *realign_root);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_type_reader_delete                    (DBusTypeReader        *reader,
                                                         const DBusTypeReader  *realign_root);

dbus_bool_t _dbus_type_reader_equal_values              (const DBusTypeReader *lhs,
                                                         const DBusTypeReader *rhs);

void        _dbus_type_signature_next                   (const char            *signature,
							 int                   *type_pos);

DBUS_PRIVATE_EXPORT
void        _dbus_type_writer_init                 (DBusTypeWriter        *writer,
                                                    int                    byte_order,
                                                    DBusString            *type_str,
                                                    int                    type_pos,
                                                    DBusString            *value_str,
                                                    int                    value_pos);
void        _dbus_type_writer_init_types_delayed   (DBusTypeWriter        *writer,
                                                    int                    byte_order,
                                                    DBusString            *value_str,
                                                    int                    value_pos);
void        _dbus_type_writer_add_types            (DBusTypeWriter        *writer,
                                                    DBusString            *type_str,
                                                    int                    type_pos);
void        _dbus_type_writer_remove_types         (DBusTypeWriter        *writer);
DBUS_PRIVATE_EXPORT
void        _dbus_type_writer_init_values_only     (DBusTypeWriter        *writer,
                                                    int                    byte_order,
                                                    const DBusString      *type_str,
                                                    int                    type_pos,
                                                    DBusString            *value_str,
                                                    int                    value_pos);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_type_writer_write_basic          (DBusTypeWriter        *writer,
                                                    int                    type,
                                                    const void            *value);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_type_writer_write_fixed_multi    (DBusTypeWriter        *writer,
                                                    int                    element_type,
                                                    const void            *value,
                                                    int                    n_elements);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_type_writer_recurse              (DBusTypeWriter        *writer,
                                                    int                    container_type,
                                                    const DBusString      *contained_type,
                                                    int                    contained_type_start,
                                                    DBusTypeWriter        *sub);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_type_writer_unrecurse            (DBusTypeWriter        *writer,
                                                    DBusTypeWriter        *sub);
dbus_bool_t _dbus_type_writer_append_array         (DBusTypeWriter        *writer,
                                                    const DBusString      *contained_type,
                                                    int                    contained_type_start,
                                                    DBusTypeWriter        *sub);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_type_writer_write_reader         (DBusTypeWriter        *writer,
                                                    DBusTypeReader        *reader);


#endif /* DBUS_MARSHAL_RECURSIVE_H */
