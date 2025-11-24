/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-marshal-header.c  Managing marshaling/demarshaling of message headers
 *
 * Copyright (C) 2005  Red Hat, Inc.
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

#include <config.h>
#include "dbus/dbus-shared.h"
#include "dbus-marshal-header.h"
#include "dbus-marshal-recursive.h"
#include "dbus-marshal-byteswap.h"

/**
 * @addtogroup DBusMarshal
 *
 * @{
 */


/* Not thread locked, but strictly const/read-only so should be OK
 */
/** Static #DBusString containing the signature of a message header */
_DBUS_STRING_DEFINE_STATIC(_dbus_header_signature_str, DBUS_HEADER_SIGNATURE);
/** Static #DBusString containing the local interface */
_DBUS_STRING_DEFINE_STATIC(_dbus_local_interface_str,  DBUS_INTERFACE_LOCAL);
/** Static #DBusString containing the local path */
_DBUS_STRING_DEFINE_STATIC(_dbus_local_path_str,       DBUS_PATH_LOCAL);

/** Offset from start of _dbus_header_signature_str to the signature of the fields array */
#define FIELDS_ARRAY_SIGNATURE_OFFSET 6
/** Offset from start of _dbus_header_signature_str to the signature of an element of the fields array */
#define FIELDS_ARRAY_ELEMENT_SIGNATURE_OFFSET 7


/** Offset to byte order from start of header */
#define BYTE_ORDER_OFFSET    0
/** Offset to type from start of header */
#define TYPE_OFFSET          1
/** Offset to flags from start of header */
#define FLAGS_OFFSET         2
/** Offset to version from start of header */
#define VERSION_OFFSET       3
/** Offset to body length from start of header */
#define BODY_LENGTH_OFFSET 4
/** Offset to client serial from start of header */
#define SERIAL_OFFSET 8
/** Offset to fields array length from start of header */
#define FIELDS_ARRAY_LENGTH_OFFSET 12
/** Offset to first field in header */
#define FIRST_FIELD_OFFSET 16

typedef struct
{
  unsigned char code; /**< the field code */
  unsigned char type; /**< the value type */
} HeaderFieldType;

static const HeaderFieldType
_dbus_header_field_types[DBUS_HEADER_FIELD_LAST+1] = {
  { DBUS_HEADER_FIELD_INVALID, DBUS_TYPE_INVALID },
  { DBUS_HEADER_FIELD_PATH, DBUS_TYPE_OBJECT_PATH },
  { DBUS_HEADER_FIELD_INTERFACE, DBUS_TYPE_STRING },
  { DBUS_HEADER_FIELD_MEMBER, DBUS_TYPE_STRING },
  { DBUS_HEADER_FIELD_ERROR_NAME, DBUS_TYPE_STRING },
  { DBUS_HEADER_FIELD_REPLY_SERIAL, DBUS_TYPE_UINT32 },
  { DBUS_HEADER_FIELD_DESTINATION, DBUS_TYPE_STRING },
  { DBUS_HEADER_FIELD_SENDER, DBUS_TYPE_STRING },
  { DBUS_HEADER_FIELD_SIGNATURE, DBUS_TYPE_SIGNATURE },
  { DBUS_HEADER_FIELD_UNIX_FDS, DBUS_TYPE_UINT32 },
  { DBUS_HEADER_FIELD_CONTAINER_INSTANCE, DBUS_TYPE_OBJECT_PATH }
};

/** Macro to look up the correct type for a field */
#define EXPECTED_TYPE_OF_FIELD(field) (_dbus_header_field_types[field].type)

/** The most padding we could ever need for a header */
#define MAX_POSSIBLE_HEADER_PADDING 7
static dbus_bool_t
reserve_header_padding (DBusHeader *header)
{
  _dbus_assert (header->padding <= MAX_POSSIBLE_HEADER_PADDING);

  if (!_dbus_string_lengthen (&header->data,
                              MAX_POSSIBLE_HEADER_PADDING - header->padding))
    return FALSE;
  header->padding = MAX_POSSIBLE_HEADER_PADDING;
  return TRUE;
}

static void
correct_header_padding (DBusHeader *header)
{
  int unpadded_len;

  _dbus_assert (header->padding == 7);

  _dbus_string_shorten (&header->data, header->padding);
  unpadded_len = _dbus_string_get_length (&header->data);

  if (!_dbus_string_align_length (&header->data, 8))
    _dbus_assert_not_reached ("couldn't pad header though enough padding was preallocated");

  header->padding = _dbus_string_get_length (&header->data) - unpadded_len;
}

/**
 * Compute the end of the header, ignoring padding.
 * In the #DBusHeader diagram, this is the distance from 0 to [B]. */
#define HEADER_END_BEFORE_PADDING(header) \
  (_dbus_string_get_length (&(header)->data) - (header)->padding)

/**
 * Invalidates all fields in the cache. This may be used when the
 * cache is totally uninitialized (contains junk) so should not
 * look at what's in there now.
 *
 * @param header the header
 */
static void
_dbus_header_cache_invalidate_all (DBusHeader *header)
{
  int i;

  i = 0;
  while (i <= DBUS_HEADER_FIELD_LAST)
    {
      header->fields[i].value_pos = _DBUS_HEADER_FIELD_VALUE_UNKNOWN;
      ++i;
    }
}

/**
 * Caches one field
 *
 * @param header the header
 * @param field_code the field
 * @param variant_reader the reader for the variant in the field
 */
static void
_dbus_header_cache_one (DBusHeader     *header,
                        int             field_code,
                        DBusTypeReader *variant_reader)
{
  header->fields[field_code].value_pos =
    _dbus_type_reader_get_value_pos (variant_reader);

#if 0
  _dbus_verbose ("cached value_pos %d for field %d\n",
                 header->fields[field_code].value_pos, field_code)
#endif
}

/**
 * Returns the header's byte order.
 *
 * @param header the header
 * @returns the byte order
 */
char
_dbus_header_get_byte_order (const DBusHeader *header)
{
  _dbus_assert (_dbus_string_get_length (&header->data) > BYTE_ORDER_OFFSET);

  return (char) _dbus_string_get_byte (&header->data, BYTE_ORDER_OFFSET);
}

/**
 * Revalidates the fields cache
 *
 * @param header the header
 */
static void
_dbus_header_cache_revalidate (DBusHeader *header)
{
  DBusTypeReader array;
  DBusTypeReader reader;
  int i;

  i = 0;
  while (i <= DBUS_HEADER_FIELD_LAST)
    {
      header->fields[i].value_pos = _DBUS_HEADER_FIELD_VALUE_NONEXISTENT;
      ++i;
    }

  _dbus_type_reader_init (&reader,
                          _dbus_header_get_byte_order (header),
                          &_dbus_header_signature_str,
                          FIELDS_ARRAY_SIGNATURE_OFFSET,
                          &header->data,
                          FIELDS_ARRAY_LENGTH_OFFSET);

  _dbus_type_reader_recurse (&reader, &array);

  while (_dbus_type_reader_get_current_type (&array) != DBUS_TYPE_INVALID)
    {
      DBusTypeReader sub;
      DBusTypeReader variant;
      unsigned char field_code;

      _dbus_type_reader_recurse (&array, &sub);

      _dbus_assert (_dbus_type_reader_get_current_type (&sub) == DBUS_TYPE_BYTE);
      _dbus_type_reader_read_basic (&sub, &field_code);

      /* Unknown fields should be ignored */
      if (field_code > DBUS_HEADER_FIELD_LAST)
        goto next_field;

      _dbus_type_reader_next (&sub);

      _dbus_assert (_dbus_type_reader_get_current_type (&sub) == DBUS_TYPE_VARIANT);
      _dbus_type_reader_recurse (&sub, &variant);

      _dbus_header_cache_one (header, field_code, &variant);

    next_field:
      _dbus_type_reader_next (&array);
    }
}

/**
 * Checks for a field, updating the cache if required.
 *
 * @param header the header
 * @param field the field to check
 * @returns #FALSE if the field doesn't exist
 */
static dbus_bool_t
_dbus_header_cache_check (DBusHeader    *header,
                          int            field)
{
  _dbus_assert (field <= DBUS_HEADER_FIELD_LAST);

  if (header->fields[field].value_pos == _DBUS_HEADER_FIELD_VALUE_UNKNOWN)
    _dbus_header_cache_revalidate (header);

  if (header->fields[field].value_pos == _DBUS_HEADER_FIELD_VALUE_NONEXISTENT)
    return FALSE;

  return TRUE;
}

/**
 * Checks whether a field is known not to exist. It may exist
 * even if it's not known to exist.
 *
 * @param header the header
 * @param field the field to check
 * @returns #FALSE if the field definitely doesn't exist
 */
static dbus_bool_t
_dbus_header_cache_known_nonexistent (DBusHeader    *header,
                                      int            field)
{
  _dbus_assert (field <= DBUS_HEADER_FIELD_LAST);

  return (header->fields[field].value_pos == _DBUS_HEADER_FIELD_VALUE_NONEXISTENT);
}

/**
 * Writes a struct of { byte, variant } with the given basic type.
 *
 * @param writer the writer (should be ready to write a struct)
 * @param field the header field
 * @param type the type of the value
 * @param value the value as for _dbus_marshal_set_basic()
 * @returns #FALSE if no memory
 */
static dbus_bool_t
write_basic_field (DBusTypeWriter *writer,
                   int             field,
                   int             type,
                   const void     *value)
{
  DBusTypeWriter sub;
  DBusTypeWriter variant;
  int start;
  int padding;
  unsigned char field_byte;
  DBusString contained_type;
  char buf[2];

  start = writer->value_pos;
  padding = _dbus_string_get_length (writer->value_str) - start;

  if (!_dbus_type_writer_recurse (writer, DBUS_TYPE_STRUCT,
                                  NULL, 0, &sub))
    goto append_failed;

  field_byte = field;
  if (!_dbus_type_writer_write_basic (&sub, DBUS_TYPE_BYTE,
                                      &field_byte))
    goto append_failed;

  buf[0] = type;
  buf[1] = '\0';
  _dbus_string_init_const_len (&contained_type, buf, 1);

  if (!_dbus_type_writer_recurse (&sub, DBUS_TYPE_VARIANT,
                                  &contained_type, 0, &variant))
    goto append_failed;

  if (!_dbus_type_writer_write_basic (&variant, type, value))
    goto append_failed;

  if (!_dbus_type_writer_unrecurse (&sub, &variant))
    goto append_failed;

  if (!_dbus_type_writer_unrecurse (writer, &sub))
    goto append_failed;

  return TRUE;

 append_failed:
  _dbus_string_delete (writer->value_str,
                       start,
                       _dbus_string_get_length (writer->value_str) - start - padding);
  return FALSE;
}

/**
 * Sets a struct of { byte, variant } with the given basic type.
 *
 * @param reader the reader (should be iterating over the array pointing at the field to set)
 * @param field the header field
 * @param type the type of the value
 * @param value the value as for _dbus_marshal_set_basic()
 * @param realign_root where to realign from
 * @returns #FALSE if no memory
 */
static dbus_bool_t
set_basic_field (DBusTypeReader       *reader,
                 int                   field,
                 int                   type,
                 const void           *value,
                 const DBusTypeReader *realign_root)
{
  DBusTypeReader sub;
  DBusTypeReader variant;

  _dbus_type_reader_recurse (reader, &sub);

  _dbus_assert (_dbus_type_reader_get_current_type (&sub) == DBUS_TYPE_BYTE);
#ifndef DBUS_DISABLE_ASSERT
 {
   unsigned char v_BYTE;
   _dbus_type_reader_read_basic (&sub, &v_BYTE);
   _dbus_assert (((int) v_BYTE) == field);
 }
#endif

  if (!_dbus_type_reader_next (&sub))
    _dbus_assert_not_reached ("no variant field?");

  _dbus_type_reader_recurse (&sub, &variant);
  _dbus_assert (_dbus_type_reader_get_current_type (&variant) == type);

  if (!_dbus_type_reader_set_basic (&variant, value, realign_root))
    return FALSE;

  return TRUE;
}

/**
 * Gets the type of the message.
 *
 * @param header the header
 * @returns the type
 */
int
_dbus_header_get_message_type (DBusHeader *header)
{
  int type;

  type = _dbus_string_get_byte (&header->data, TYPE_OFFSET);
  _dbus_assert (type != DBUS_MESSAGE_TYPE_INVALID);

  return type;
}

/**
 * Sets the serial number of a header.  This can only be done once on
 * a header.
 *
 * @param header the header
 * @param serial the serial
 */
void
_dbus_header_set_serial (DBusHeader    *header,
                         dbus_uint32_t  serial)
{
  /* we use this function to set the serial on outgoing
   * messages, and to reset the serial in dbus_message_copy;
   * this assertion should catch a double-set on outgoing.
   */
  _dbus_assert (_dbus_header_get_serial (header) == 0 ||
                serial == 0);

  _dbus_marshal_set_uint32 (&header->data,
                            SERIAL_OFFSET,
			    serial,
                            _dbus_header_get_byte_order (header));
}

/**
 * See dbus_message_get_serial()
 *
 * @param header the header
 * @returns the client serial
 */
dbus_uint32_t
_dbus_header_get_serial (DBusHeader *header)
{
  return _dbus_marshal_read_uint32 (&header->data,
                                    SERIAL_OFFSET,
                                    _dbus_header_get_byte_order (header),
                                    NULL);
}

/**
 * Re-initializes a header that was previously initialized and never
 * freed.  After this, to make the header valid you have to call
 * _dbus_header_create().
 *
 * @param header header to re-initialize
 */
void
_dbus_header_reinit (DBusHeader *header)
{
  _dbus_string_set_length (&header->data, 0);

  header->padding = 0;

  _dbus_header_cache_invalidate_all (header);
}

/**
 * Initializes a header, but doesn't prepare it for use;
 * to make the header valid, you have to call _dbus_header_create().
 *
 * @param header header to initialize
 * @returns #FALSE if not enough memory
 */
dbus_bool_t
_dbus_header_init (DBusHeader *header)
{
  if (!_dbus_string_init_preallocated (&header->data, 32))
    return FALSE;

  _dbus_header_reinit (header);

  return TRUE;
}

/**
 * Frees a header.
 *
 * @param header the header
 */
void
_dbus_header_free (DBusHeader *header)
{
  _dbus_string_free (&header->data);
}

/**
 * Initializes dest with a copy of the given header.
 * Resets the message serial to 0 on the copy.
 *
 * @param header header to copy
 * @param dest destination for copy
 * @returns #FALSE if not enough memory
 */
dbus_bool_t
_dbus_header_copy (const DBusHeader *header,
                   DBusHeader       *dest)
{
  *dest = *header;

  if (!_dbus_string_init_preallocated (&dest->data,
                                       _dbus_string_get_length (&header->data)))
    return FALSE;

  if (!_dbus_string_copy (&header->data, 0, &dest->data, 0))
    {
      _dbus_string_free (&dest->data);
      return FALSE;
    }

  /* Reset the serial */
  _dbus_header_set_serial (dest, 0);

  return TRUE;
}

/**
 * Fills in the primary fields of the header, so the header is ready
 * for use. #NULL may be specified for some or all of the fields to
 * avoid adding those fields. Some combinations of fields don't make
 * sense, and passing them in will trigger an assertion failure.
 *
 * @param header the header
 * @param byte_order byte order of the header
 * @param message_type the message type
 * @param destination destination field or #NULL
 * @param path path field or #NULL
 * @param interface interface field or #NULL
 * @param member member field or #NULL
 * @param error_name error name or #NULL
 * @returns #FALSE if not enough memory
 */
dbus_bool_t
_dbus_header_create (DBusHeader  *header,
                     int          byte_order,
                     int          message_type,
                     const char  *destination,
                     const char  *path,
                     const char  *interface,
                     const char  *member,
                     const char  *error_name)
{
  unsigned char v_BYTE;
  dbus_uint32_t v_UINT32;
  DBusTypeWriter writer;
  DBusTypeWriter array;

  _dbus_assert (byte_order == DBUS_LITTLE_ENDIAN ||
                byte_order == DBUS_BIG_ENDIAN);
  _dbus_assert (((interface || message_type != DBUS_MESSAGE_TYPE_SIGNAL) && member) ||
                (error_name) ||
                !(interface || member || error_name));
  _dbus_assert (_dbus_string_get_length (&header->data) == 0);

  if (!reserve_header_padding (header))
    return FALSE;

  _dbus_type_writer_init_values_only (&writer, byte_order,
                                      &_dbus_header_signature_str, 0,
                                      &header->data,
                                      HEADER_END_BEFORE_PADDING (header));

  v_BYTE = byte_order;
  if (!_dbus_type_writer_write_basic (&writer, DBUS_TYPE_BYTE,
                                      &v_BYTE))
    goto oom;

  v_BYTE = message_type;
  if (!_dbus_type_writer_write_basic (&writer, DBUS_TYPE_BYTE,
                                      &v_BYTE))
    goto oom;

  v_BYTE = 0; /* flags */
  if (!_dbus_type_writer_write_basic (&writer, DBUS_TYPE_BYTE,
                                      &v_BYTE))
    goto oom;

  v_BYTE = DBUS_MAJOR_PROTOCOL_VERSION;
  if (!_dbus_type_writer_write_basic (&writer, DBUS_TYPE_BYTE,
                                      &v_BYTE))
    goto oom;

  v_UINT32 = 0; /* body length */
  if (!_dbus_type_writer_write_basic (&writer, DBUS_TYPE_UINT32,
                                      &v_UINT32))
    goto oom;

  v_UINT32 = 0; /* serial */
  if (!_dbus_type_writer_write_basic (&writer, DBUS_TYPE_UINT32,
                                      &v_UINT32))
    goto oom;

  if (!_dbus_type_writer_recurse (&writer, DBUS_TYPE_ARRAY,
                                  &_dbus_header_signature_str,
                                  FIELDS_ARRAY_SIGNATURE_OFFSET,
                                  &array))
    goto oom;

  /* Marshal all the fields (Marshall Fields?) */

  if (path != NULL)
    {
      if (!write_basic_field (&array,
                              DBUS_HEADER_FIELD_PATH,
                              DBUS_TYPE_OBJECT_PATH,
                              &path))
        goto oom;
    }

  if (destination != NULL)
    {
      if (!write_basic_field (&array,
                              DBUS_HEADER_FIELD_DESTINATION,
                              DBUS_TYPE_STRING,
                              &destination))
        goto oom;
    }

  /* Note that test/message.c relies on this being in the middle of the
   * message: if you change the order of serialization here (but why
   * would you?), please find some other way to retain test coverage. */
  if (interface != NULL)
    {
      if (!write_basic_field (&array,
                              DBUS_HEADER_FIELD_INTERFACE,
                              DBUS_TYPE_STRING,
                              &interface))
        goto oom;
    }

  if (member != NULL)
    {
      if (!write_basic_field (&array,
                              DBUS_HEADER_FIELD_MEMBER,
                              DBUS_TYPE_STRING,
                              &member))
        goto oom;
    }

  if (error_name != NULL)
    {
      if (!write_basic_field (&array,
                              DBUS_HEADER_FIELD_ERROR_NAME,
                              DBUS_TYPE_STRING,
                              &error_name))
        goto oom;
    }

  if (!_dbus_type_writer_unrecurse (&writer, &array))
    goto oom;

  correct_header_padding (header);

  return TRUE;

 oom:
  _dbus_string_delete (&header->data, 0,
                       _dbus_string_get_length (&header->data) - header->padding);
  correct_header_padding (header);

  return FALSE;
}

/**
 * Given data long enough to contain the length of the message body
 * and the fields array, check whether the data is long enough to
 * contain the entire message (assuming the claimed lengths are
 * accurate). Also checks that the lengths are in sanity parameters.
 *
 * @param max_message_length maximum length of a valid message
 * @param validity return location for why the data is invalid if it is
 * @param byte_order return location for byte order
 * @param fields_array_len return location for claimed fields array length
 * @param header_len return location for claimed header length
 * @param body_len return location for claimed body length
 * @param str the data
 * @param start start of data, 8-aligned
 * @param len length of data
 * @returns #TRUE if the data is long enough for the claimed length, and the lengths were valid
 */
dbus_bool_t
_dbus_header_have_message_untrusted (int                max_message_length,
                                     DBusValidity      *validity,
                                     int               *byte_order,
                                     int               *fields_array_len,
                                     int               *header_len,
                                     int               *body_len,
                                     const DBusString  *str,
                                     int                start,
                                     int                len)

{
  dbus_uint32_t header_len_unsigned;
  dbus_uint32_t fields_array_len_unsigned;
  dbus_uint32_t body_len_unsigned;

  _dbus_assert (start >= 0);
  _dbus_assert (start < _DBUS_INT32_MAX / 2);
  _dbus_assert (len >= 0);

  _dbus_assert (start == (int) _DBUS_ALIGN_VALUE (start, 8));

  *byte_order = _dbus_string_get_byte (str, start + BYTE_ORDER_OFFSET);

  if (*byte_order != DBUS_LITTLE_ENDIAN && *byte_order != DBUS_BIG_ENDIAN)
    {
      *validity = DBUS_INVALID_BAD_BYTE_ORDER;
      return FALSE;
    }

  _dbus_assert (FIELDS_ARRAY_LENGTH_OFFSET + 4 <= len);
  fields_array_len_unsigned = _dbus_marshal_read_uint32 (str, start + FIELDS_ARRAY_LENGTH_OFFSET,
                                                         *byte_order, NULL);

  if (fields_array_len_unsigned > (unsigned) max_message_length)
    {
      *validity = DBUS_INVALID_INSANE_FIELDS_ARRAY_LENGTH;
      return FALSE;
    }

  _dbus_assert (BODY_LENGTH_OFFSET + 4 < len);
  body_len_unsigned = _dbus_marshal_read_uint32 (str, start + BODY_LENGTH_OFFSET,
                                                 *byte_order, NULL);

  if (body_len_unsigned > (unsigned) max_message_length)
    {
      *validity = DBUS_INVALID_INSANE_BODY_LENGTH;
      return FALSE;
    }

  header_len_unsigned = FIRST_FIELD_OFFSET + fields_array_len_unsigned;
  header_len_unsigned = _DBUS_ALIGN_VALUE (header_len_unsigned, 8);

  /* overflow should be impossible since the lengths aren't allowed to
   * be huge.
   */
  _dbus_assert (max_message_length < _DBUS_INT32_MAX / 2);
  if (body_len_unsigned + header_len_unsigned > (unsigned) max_message_length)
    {
      *validity = DBUS_INVALID_MESSAGE_TOO_LONG;
      return FALSE;
    }

  _dbus_assert (body_len_unsigned < (unsigned) _DBUS_INT32_MAX);
  _dbus_assert (fields_array_len_unsigned < (unsigned) _DBUS_INT32_MAX);
  _dbus_assert (header_len_unsigned < (unsigned) _DBUS_INT32_MAX);

  *body_len = body_len_unsigned;
  *fields_array_len = fields_array_len_unsigned;
  *header_len = header_len_unsigned;

  *validity = DBUS_VALID;

  _dbus_verbose ("have %d bytes, need body %u + header %u = %u\n",
                 len, body_len_unsigned, header_len_unsigned,
                 body_len_unsigned + header_len_unsigned);

  return (body_len_unsigned + header_len_unsigned) <= (unsigned) len;
}

static DBusValidity
check_mandatory_fields (DBusHeader *header)
{
#define REQUIRE_FIELD(name) do { if (header->fields[DBUS_HEADER_FIELD_##name].value_pos < 0) return DBUS_INVALID_MISSING_##name; } while (0)

  switch (_dbus_header_get_message_type (header))
    {
    case DBUS_MESSAGE_TYPE_SIGNAL:
      REQUIRE_FIELD (INTERFACE);
      /* FALL THRU - signals also require the path and member */
    case DBUS_MESSAGE_TYPE_METHOD_CALL:
      REQUIRE_FIELD (PATH);
      REQUIRE_FIELD (MEMBER);
      break;
    case DBUS_MESSAGE_TYPE_ERROR:
      REQUIRE_FIELD (ERROR_NAME);
      REQUIRE_FIELD (REPLY_SERIAL);
      break;
    case DBUS_MESSAGE_TYPE_METHOD_RETURN:
      REQUIRE_FIELD (REPLY_SERIAL);
      break;
    default:
      /* other message types allowed but ignored */
      break;
    }

  return DBUS_VALID;
}

static DBusValidity
load_and_validate_field (DBusHeader     *header,
                         int             field,
                         DBusTypeReader *variant_reader)
{
  int type;
  int expected_type;
  const DBusString *value_str;
  int value_pos;
  int str_data_pos;
  dbus_uint32_t v_UINT32;
  int bad_string_code;
  dbus_bool_t (* string_validation_func) (const DBusString *str,
                                          int start, int len);

  /* Supposed to have been checked already */
  _dbus_assert (field <= DBUS_HEADER_FIELD_LAST);
  _dbus_assert (field != DBUS_HEADER_FIELD_INVALID);

  /* Before we can cache a field, we need to know it has the right type */
  type = _dbus_type_reader_get_current_type (variant_reader);

  _dbus_assert (_dbus_header_field_types[field].code == field);

  expected_type = EXPECTED_TYPE_OF_FIELD (field);
  if (type != expected_type)
    {
      _dbus_verbose ("Field %d should have type %d but has %d\n",
                     field, expected_type, type);
      return DBUS_INVALID_HEADER_FIELD_HAS_WRONG_TYPE;
    }

  /* If the field was provided twice, we aren't happy */
  if (header->fields[field].value_pos >= 0)
    {
      _dbus_verbose ("Header field %d seen a second time\n", field);
      return DBUS_INVALID_HEADER_FIELD_APPEARS_TWICE;
    }

  /* Now we can cache and look at the field content */
  _dbus_verbose ("initially caching field %d\n", field);
  _dbus_header_cache_one (header, field, variant_reader);

  string_validation_func = NULL;

  /* make compiler happy that all this is initialized */
  v_UINT32 = 0;
  value_str = NULL;
  value_pos = -1;
  str_data_pos = -1;
  bad_string_code = DBUS_VALID;

  if (expected_type == DBUS_TYPE_UINT32)
    {
      _dbus_header_get_field_basic (header, field, expected_type,
                                    &v_UINT32);
    }
  else if (expected_type == DBUS_TYPE_STRING ||
           expected_type == DBUS_TYPE_OBJECT_PATH ||
           expected_type == DBUS_TYPE_SIGNATURE)
    {
      _dbus_header_get_field_raw (header, field,
                                  &value_str, &value_pos);
      str_data_pos = _DBUS_ALIGN_VALUE (value_pos, 4) + 4;
    }
  else
    {
      _dbus_assert_not_reached ("none of the known fields should have this type");
    }

  switch (field)
    {
    case DBUS_HEADER_FIELD_DESTINATION:
      string_validation_func = _dbus_validate_bus_name;
      bad_string_code = DBUS_INVALID_BAD_DESTINATION;
      break;
    case DBUS_HEADER_FIELD_INTERFACE:
      string_validation_func = _dbus_validate_interface;
      bad_string_code = DBUS_INVALID_BAD_INTERFACE;

      if (_dbus_string_equal_substring (&_dbus_local_interface_str,
                                        0,
                                        _dbus_string_get_length (&_dbus_local_interface_str),
                                        value_str, str_data_pos))
        {
          _dbus_verbose ("Message is on the local interface\n");
          return DBUS_INVALID_USES_LOCAL_INTERFACE;
        }
      break;

    case DBUS_HEADER_FIELD_MEMBER:
      string_validation_func = _dbus_validate_member;
      bad_string_code = DBUS_INVALID_BAD_MEMBER;
      break;

    case DBUS_HEADER_FIELD_ERROR_NAME:
      string_validation_func = _dbus_validate_error_name;
      bad_string_code = DBUS_INVALID_BAD_ERROR_NAME;
      break;

    case DBUS_HEADER_FIELD_SENDER:
      string_validation_func = _dbus_validate_bus_name;
      bad_string_code = DBUS_INVALID_BAD_SENDER;
      break;

    case DBUS_HEADER_FIELD_PATH:
      /* OBJECT_PATH was validated generically due to its type */
      string_validation_func = NULL;

      if (_dbus_string_equal_substring (&_dbus_local_path_str,
                                        0,
                                        _dbus_string_get_length (&_dbus_local_path_str),
                                        value_str, str_data_pos))
        {
          _dbus_verbose ("Message is from the local path\n");
          return DBUS_INVALID_USES_LOCAL_PATH;
        }
      break;

    case DBUS_HEADER_FIELD_REPLY_SERIAL:
      /* Can't be 0 */
      if (v_UINT32 == 0)
        {
          return DBUS_INVALID_BAD_SERIAL;
        }
      break;

    case DBUS_HEADER_FIELD_UNIX_FDS:
      /* Every value makes sense */
      break;

    case DBUS_HEADER_FIELD_SIGNATURE:
      /* SIGNATURE validated generically due to its type */
      string_validation_func = NULL;
      break;

    case DBUS_HEADER_FIELD_CONTAINER_INSTANCE:
      /* OBJECT_PATH was validated generically due to its type */
      string_validation_func = NULL;
      break;

    default:
      _dbus_assert_not_reached ("unknown field shouldn't be seen here");
      break;
    }

  if (string_validation_func)
    {
      dbus_uint32_t len;

      _dbus_assert (bad_string_code != DBUS_VALID);

      len = _dbus_marshal_read_uint32 (value_str, value_pos,
                                       _dbus_header_get_byte_order (header),
                                       NULL);

#if 0
      _dbus_verbose ("Validating string header field; code %d if fails\n",
                     bad_string_code);
#endif
      if (!(*string_validation_func) (value_str, str_data_pos, len))
        return bad_string_code;
    }

  return DBUS_VALID;
}

/**
 * Creates a message header from potentially-untrusted data. The
 * return value is #TRUE if there was enough memory and the data was
 * valid. If it returns #TRUE, the header will be created. If it
 * returns #FALSE and *validity == #DBUS_VALIDITY_UNKNOWN_OOM_ERROR, 
 * then there wasn't enough memory.  If it returns #FALSE 
 * and *validity != #DBUS_VALIDITY_UNKNOWN_OOM_ERROR then the data was 
 * invalid.
 *
 * The byte_order, fields_array_len, and body_len args should be from
 * _dbus_header_have_message_untrusted(). Validation performed in
 * _dbus_header_have_message_untrusted() is assumed to have been
 * already done.
 *
 * @param header the header (must be initialized)
 * @param mode whether to do validation
 * @param validity return location for invalidity reason
 * @param byte_order byte order from header
 * @param fields_array_len claimed length of fields array
 * @param body_len claimed length of body
 * @param header_len claimed length of header
 * @param str a string starting with the header
 * @returns #FALSE if no memory or data was invalid, #TRUE otherwise
 */
dbus_bool_t
_dbus_header_load (DBusHeader        *header,
                   DBusValidationMode mode,
                   DBusValidity      *validity,
                   int                byte_order,
                   int                fields_array_len,
                   int                header_len,
                   int                body_len,
                   const DBusString  *str)
{
  int leftover;
  DBusValidity v;
  DBusTypeReader reader;
  DBusTypeReader array_reader;
  unsigned char v_byte;
  dbus_uint32_t v_uint32;
  dbus_uint32_t serial;
  int padding_start;
  int padding_len;
  int i;
  int len;

  len = _dbus_string_get_length (str);

  _dbus_assert (header_len <= len);
  _dbus_assert (_dbus_string_get_length (&header->data) == 0);

  if (!_dbus_string_copy_len (str, 0, header_len, &header->data, 0))
    {
      _dbus_verbose ("Failed to copy buffer into new header\n");
      *validity = DBUS_VALIDITY_UNKNOWN_OOM_ERROR;
      return FALSE;
    }

  if (mode == DBUS_VALIDATION_MODE_WE_TRUST_THIS_DATA_ABSOLUTELY)
    {
      leftover = len - header_len - body_len;
    }
  else
    {
      v = _dbus_validate_body_with_reason (&_dbus_header_signature_str, 0,
                                           byte_order,
                                           &leftover,
                                           str, 0, len);
      
      if (v != DBUS_VALID)
        {
          *validity = v;
          goto invalid;
        }
    }

  _dbus_assert (leftover < len);

  padding_len = header_len - (FIRST_FIELD_OFFSET + fields_array_len);
  padding_start = FIRST_FIELD_OFFSET + fields_array_len;
  _dbus_assert (header_len == (int) _DBUS_ALIGN_VALUE (padding_start, 8));
  _dbus_assert (header_len == padding_start + padding_len);

  if (mode != DBUS_VALIDATION_MODE_WE_TRUST_THIS_DATA_ABSOLUTELY)
    {
      if (!_dbus_string_validate_nul (str, padding_start, padding_len))
        {
          *validity = DBUS_INVALID_ALIGNMENT_PADDING_NOT_NUL;
          goto invalid;
        }
    }

  header->padding = padding_len;

  if (mode == DBUS_VALIDATION_MODE_WE_TRUST_THIS_DATA_ABSOLUTELY)
    {
      *validity = DBUS_VALID;
      return TRUE;
    }

  /* We now know the data is well-formed, but we have to check that
   * it's valid.
   */

  _dbus_type_reader_init (&reader,
                          byte_order,
                          &_dbus_header_signature_str, 0,
                          str, 0);

  /* BYTE ORDER */
  _dbus_assert (_dbus_type_reader_get_current_type (&reader) == DBUS_TYPE_BYTE);
  _dbus_assert (_dbus_type_reader_get_value_pos (&reader) == BYTE_ORDER_OFFSET);
  _dbus_type_reader_read_basic (&reader, &v_byte);
  _dbus_type_reader_next (&reader);

  _dbus_assert (v_byte == byte_order);

  /* MESSAGE TYPE */
  _dbus_assert (_dbus_type_reader_get_current_type (&reader) == DBUS_TYPE_BYTE);
  _dbus_assert (_dbus_type_reader_get_value_pos (&reader) == TYPE_OFFSET);
  _dbus_type_reader_read_basic (&reader, &v_byte);
  _dbus_type_reader_next (&reader);

  /* unknown message types are supposed to be ignored, so only validation here is
   * that it isn't invalid
   */
  if (v_byte == DBUS_MESSAGE_TYPE_INVALID)
    {
      *validity = DBUS_INVALID_BAD_MESSAGE_TYPE;
      goto invalid;
    }

  /* FLAGS */
  _dbus_assert (_dbus_type_reader_get_current_type (&reader) == DBUS_TYPE_BYTE);
  _dbus_assert (_dbus_type_reader_get_value_pos (&reader) == FLAGS_OFFSET);
  _dbus_type_reader_read_basic (&reader, &v_byte);
  _dbus_type_reader_next (&reader);

  /* unknown flags should be ignored */

  /* PROTOCOL VERSION */
  _dbus_assert (_dbus_type_reader_get_current_type (&reader) == DBUS_TYPE_BYTE);
  _dbus_assert (_dbus_type_reader_get_value_pos (&reader) == VERSION_OFFSET);
  _dbus_type_reader_read_basic (&reader, &v_byte);
  _dbus_type_reader_next (&reader);

  if (v_byte != DBUS_MAJOR_PROTOCOL_VERSION)
    {
      *validity = DBUS_INVALID_BAD_PROTOCOL_VERSION;
      goto invalid;
    }

  /* BODY LENGTH */
  _dbus_assert (_dbus_type_reader_get_current_type (&reader) == DBUS_TYPE_UINT32);
  _dbus_assert (_dbus_type_reader_get_value_pos (&reader) == BODY_LENGTH_OFFSET);
  _dbus_type_reader_read_basic (&reader, &v_uint32);
  _dbus_type_reader_next (&reader);

  _dbus_assert (body_len == (signed) v_uint32);

  /* SERIAL */
  _dbus_assert (_dbus_type_reader_get_current_type (&reader) == DBUS_TYPE_UINT32);
  _dbus_assert (_dbus_type_reader_get_value_pos (&reader) == SERIAL_OFFSET);
  _dbus_type_reader_read_basic (&reader, &serial);
  _dbus_type_reader_next (&reader);

  if (serial == 0)
    {
      *validity = DBUS_INVALID_BAD_SERIAL;
      goto invalid;
    }

  _dbus_assert (_dbus_type_reader_get_current_type (&reader) == DBUS_TYPE_ARRAY);
  _dbus_assert (_dbus_type_reader_get_value_pos (&reader) == FIELDS_ARRAY_LENGTH_OFFSET);

  _dbus_type_reader_recurse (&reader, &array_reader);
  while (_dbus_type_reader_get_current_type (&array_reader) != DBUS_TYPE_INVALID)
    {
      DBusTypeReader struct_reader;
      DBusTypeReader variant_reader;
      unsigned char field_code;

      _dbus_assert (_dbus_type_reader_get_current_type (&array_reader) == DBUS_TYPE_STRUCT);

      _dbus_type_reader_recurse (&array_reader, &struct_reader);

      _dbus_assert (_dbus_type_reader_get_current_type (&struct_reader) == DBUS_TYPE_BYTE);
      _dbus_type_reader_read_basic (&struct_reader, &field_code);
      _dbus_type_reader_next (&struct_reader);

      if (field_code == DBUS_HEADER_FIELD_INVALID)
        {
          _dbus_verbose ("invalid header field code\n");
          *validity = DBUS_INVALID_HEADER_FIELD_CODE;
          goto invalid;
        }

      if (field_code > DBUS_HEADER_FIELD_LAST)
        {
          _dbus_verbose ("unknown header field code %d, skipping\n",
                         field_code);
          goto next_field;
        }

      _dbus_assert (_dbus_type_reader_get_current_type (&struct_reader) == DBUS_TYPE_VARIANT);
      _dbus_type_reader_recurse (&struct_reader, &variant_reader);

      v = load_and_validate_field (header, field_code, &variant_reader);
      if (v != DBUS_VALID)
        {
          _dbus_verbose ("Field %d was invalid\n", field_code);
          *validity = v;
          goto invalid;
        }

    next_field:
      _dbus_type_reader_next (&array_reader);
    }

  /* Anything we didn't fill in is now known not to exist */
  i = 0;
  while (i <= DBUS_HEADER_FIELD_LAST)
    {
      if (header->fields[i].value_pos == _DBUS_HEADER_FIELD_VALUE_UNKNOWN)
        header->fields[i].value_pos = _DBUS_HEADER_FIELD_VALUE_NONEXISTENT;
      ++i;
    }

  v = check_mandatory_fields (header);
  if (v != DBUS_VALID)
    {
      _dbus_verbose ("Mandatory fields were missing, code %d\n", v);
      *validity = v;
      goto invalid;
    }

  *validity = DBUS_VALID;
  return TRUE;

 invalid:
  _dbus_string_set_length (&header->data, 0);
  return FALSE;
}

/**
 * Fills in the correct body length.
 *
 * @param header the header
 * @param body_len the length of the body
 */
void
_dbus_header_update_lengths (DBusHeader *header,
                             int         body_len)
{
  _dbus_marshal_set_uint32 (&header->data,
                            BODY_LENGTH_OFFSET,
                            body_len,
                            _dbus_header_get_byte_order (header));
}

/**
 * Try to find the given field.
 *
 * @param header the header
 * @param field the field code
 * @param reader a type reader; on success this is left pointing at the struct
 *  (uv) for the field, while on failure it is left pointing into empty space
 *  at the end of the header fields
 * @param realign_root another type reader; on success or failure it is left
 *  pointing to the beginning of the array of fields (i.e. the thing that might
 *  need realigning)
 * @returns #TRUE on success
 */
static dbus_bool_t
find_field_for_modification (DBusHeader     *header,
                             int             field,
                             DBusTypeReader *reader,
                             DBusTypeReader *realign_root)
{
  dbus_bool_t retval;

  retval = FALSE;

  _dbus_type_reader_init (realign_root,
                          _dbus_header_get_byte_order (header),
                          &_dbus_header_signature_str,
                          FIELDS_ARRAY_SIGNATURE_OFFSET,
                          &header->data,
                          FIELDS_ARRAY_LENGTH_OFFSET);

  _dbus_type_reader_recurse (realign_root, reader);

  while (_dbus_type_reader_get_current_type (reader) != DBUS_TYPE_INVALID)
    {
      DBusTypeReader sub;
      unsigned char field_code;

      _dbus_type_reader_recurse (reader, &sub);

      _dbus_assert (_dbus_type_reader_get_current_type (&sub) == DBUS_TYPE_BYTE);
      _dbus_type_reader_read_basic (&sub, &field_code);

      if (field_code == (unsigned) field)
        {
          _dbus_assert (_dbus_type_reader_get_current_type (reader) == DBUS_TYPE_STRUCT);
          retval = TRUE;
          goto done;
        }

      _dbus_type_reader_next (reader);
    }

 done:
  return retval;
}

/**
 * Sets the value of a field with basic type. If the value is a string
 * value, it isn't allowed to be #NULL. If the field doesn't exist,
 * it will be created.
 *
 * @param header the header
 * @param field the field to set
 * @param type the type of the value
 * @param value the value as for _dbus_marshal_set_basic()
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_header_set_field_basic (DBusHeader       *header,
                              int               field,
                              int               type,
                              const void       *value)
{
  _dbus_assert (field <= DBUS_HEADER_FIELD_LAST);

  if (!reserve_header_padding (header))
    return FALSE;

  /* If the field exists we set, otherwise we append */
  if (_dbus_header_cache_check (header, field))
    {
      DBusTypeReader reader;
      DBusTypeReader realign_root;

      if (!find_field_for_modification (header, field,
                                        &reader, &realign_root))
        _dbus_assert_not_reached ("field was marked present in cache but wasn't found");

      if (!set_basic_field (&reader, field, type, value, &realign_root))
        return FALSE;
    }
  else
    {
      DBusTypeWriter writer;
      DBusTypeWriter array;

      _dbus_type_writer_init_values_only (&writer,
                                          _dbus_header_get_byte_order (header),
                                          &_dbus_header_signature_str,
                                          FIELDS_ARRAY_SIGNATURE_OFFSET,
                                          &header->data,
                                          FIELDS_ARRAY_LENGTH_OFFSET);

      /* recurse into array without creating a new length, and jump to
       * end of array.
       */
      if (!_dbus_type_writer_append_array (&writer,
                                           &_dbus_header_signature_str,
                                           FIELDS_ARRAY_ELEMENT_SIGNATURE_OFFSET,
                                           &array))
        _dbus_assert_not_reached ("recurse into ARRAY should not have used memory");

      _dbus_assert (array.u.array.len_pos == FIELDS_ARRAY_LENGTH_OFFSET);
      _dbus_assert (array.u.array.start_pos == FIRST_FIELD_OFFSET);
      _dbus_assert (array.value_pos == HEADER_END_BEFORE_PADDING (header));

      if (!write_basic_field (&array,
                              field, type, value))
        return FALSE;

      if (!_dbus_type_writer_unrecurse (&writer, &array))
        _dbus_assert_not_reached ("unrecurse from ARRAY should not have used memory");
    }

  correct_header_padding (header);

  /* We could be smarter about this (only invalidate fields after the
   * one we modified, or even only if the one we modified changed
   * length). But this hack is a start.
   */
  _dbus_header_cache_invalidate_all (header);

  return TRUE;
}

/**
 * Gets the value of a field with basic type. If the field
 * doesn't exist, returns #FALSE, otherwise returns #TRUE.
 *
 * @param header the header
 * @param field the field to get
 * @param type the type of the value
 * @param value the value as for _dbus_marshal_read_basic()
 * @returns #FALSE if the field doesn't exist
 */
dbus_bool_t
_dbus_header_get_field_basic (DBusHeader    *header,
                              int            field,
                              int            type,
                              void          *value)
{
  _dbus_assert (field != DBUS_HEADER_FIELD_INVALID);
  _dbus_assert (field <= DBUS_HEADER_FIELD_LAST);
  _dbus_assert (_dbus_header_field_types[field].code == field);
  /* in light of this you might ask why the type is passed in;
   * the only rationale I can think of is so the caller has
   * to specify its expectation and breaks if we change it
   */
  _dbus_assert (type == EXPECTED_TYPE_OF_FIELD (field));

  if (!_dbus_header_cache_check (header, field))
    return FALSE;

  _dbus_assert (header->fields[field].value_pos >= 0);

  _dbus_marshal_read_basic (&header->data,
                            header->fields[field].value_pos,
                            type, value, _dbus_header_get_byte_order (header),
                            NULL);

  return TRUE;
}

/**
 * Gets the raw marshaled data for a field. If the field doesn't
 * exist, returns #FALSE, otherwise returns #TRUE.  Returns the start
 * of the marshaled data, i.e. usually the byte where the length
 * starts (for strings and arrays) or for basic types just the value
 * itself.
 *
 * @param header the header
 * @param field the field to get
 * @param str return location for the data string
 * @param pos return location for start of field value
 * @returns #FALSE if the field doesn't exist
 */
dbus_bool_t
_dbus_header_get_field_raw (DBusHeader        *header,
                            int                field,
                            const DBusString **str,
                            int               *pos)
{
  if (!_dbus_header_cache_check (header, field))
    return FALSE;

  if (str)
    *str = &header->data;
  if (pos)
    *pos = header->fields[field].value_pos;

  return TRUE;
}

/**
 * Deletes a field, if it exists.
 *
 * @param header the header
 * @param field the field to delete
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_header_delete_field (DBusHeader *header,
                           int         field)
{
  DBusTypeReader reader;
  DBusTypeReader realign_root;

  if (_dbus_header_cache_known_nonexistent (header, field))
    return TRUE; /* nothing to do */

  /* Scan to the field we want, delete and realign, reappend
   * padding. Field may turn out not to exist.
   */
  if (!find_field_for_modification (header, field,
                                    &reader, &realign_root))
    return TRUE; /* nothing to do */

  if (!reserve_header_padding (header))
    return FALSE;

  if (!_dbus_type_reader_delete (&reader,
                                 &realign_root))
    return FALSE;

  correct_header_padding (header);

  _dbus_header_cache_invalidate_all (header);

  _dbus_assert (!_dbus_header_cache_check (header, field)); /* Expensive assertion ... */

  return TRUE;
}

/**
 * Toggles a message flag bit, turning on the bit if value = TRUE and
 * flipping it off if value = FALSE.
 *
 * @param header the header
 * @param flag the message flag to toggle
 * @param value toggle on or off
 */
void
_dbus_header_toggle_flag (DBusHeader   *header,
                          dbus_uint32_t flag,
                          dbus_bool_t   value)
{
  unsigned char *flags_p;

  flags_p = _dbus_string_get_udata_len (&header->data, FLAGS_OFFSET, 1);

  if (value)
    *flags_p |= flag;
  else
    *flags_p &= ~flag;
}

/**
 * Gets a message flag bit, returning TRUE if the bit is set.
 *
 * @param header the header
 * @param flag the message flag to get
 * @returns #TRUE if the flag is set
 */
dbus_bool_t
_dbus_header_get_flag (DBusHeader   *header,
                       dbus_uint32_t flag)
{
  const unsigned char *flags_p;

  flags_p = _dbus_string_get_const_udata_len (&header->data, FLAGS_OFFSET, 1);

  return (*flags_p & flag) != 0;
}

/**
 * Swaps the header into the given order if required.
 *
 * @param header the header
 * @param new_order the new byte order
 */
void
_dbus_header_byteswap (DBusHeader *header,
                       int         new_order)
{
  char byte_order;

  byte_order = _dbus_header_get_byte_order (header);

  if (byte_order == new_order)
    return;

  _dbus_marshal_byteswap (&_dbus_header_signature_str,
                          0, byte_order,
                          new_order,
                          &header->data, 0);

  _dbus_string_set_byte (&header->data, BYTE_ORDER_OFFSET, new_order);
}

/**
 * Remove every header field not known to this version of dbus.
 *
 * @param header the header
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_header_remove_unknown_fields (DBusHeader *header)
{
  DBusTypeReader array;
  DBusTypeReader fields_reader;

  _dbus_type_reader_init (&fields_reader,
                          _dbus_header_get_byte_order (header),
                          &_dbus_header_signature_str,
                          FIELDS_ARRAY_SIGNATURE_OFFSET,
                          &header->data,
                          FIELDS_ARRAY_LENGTH_OFFSET);

  _dbus_type_reader_recurse (&fields_reader, &array);

  while (_dbus_type_reader_get_current_type (&array) != DBUS_TYPE_INVALID)
    {
      DBusTypeReader sub;
      unsigned char field_code;

      _dbus_type_reader_recurse (&array, &sub);

      _dbus_assert (_dbus_type_reader_get_current_type (&sub) == DBUS_TYPE_BYTE);
      _dbus_type_reader_read_basic (&sub, &field_code);

      if (field_code > DBUS_HEADER_FIELD_LAST)
        {
          if (!reserve_header_padding (header))
            return FALSE;

          if (!_dbus_type_reader_delete (&array, &fields_reader))
            return FALSE;

          correct_header_padding (header);
          _dbus_header_cache_invalidate_all (header);
        }
      else
        {
          _dbus_type_reader_next (&array);
        }
    }

  return TRUE;
}

/** @} */
