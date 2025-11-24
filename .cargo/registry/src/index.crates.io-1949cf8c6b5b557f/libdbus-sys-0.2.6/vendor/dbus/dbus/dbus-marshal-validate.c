/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-marshal-validate.c Validation routines for marshaled data
 *
 * Copyright (C) 2005 Red Hat, Inc.
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
#include "dbus-internals.h"
#include "dbus-marshal-validate.h"
#include "dbus-marshal-recursive.h"
#include "dbus-marshal-basic.h"
#include "dbus-signature.h"
#include "dbus-string.h"

/**
 * @addtogroup DBusMarshal
 *
 * @{
 */

/**
 * Verifies that the range of type_str from type_pos to type_end is a
 * valid signature.  If this function returns #TRUE, it will be safe
 * to iterate over the signature with a types-only #DBusTypeReader.
 * The range passed in should NOT include the terminating
 * nul/DBUS_TYPE_INVALID.
 *
 * @param type_str the string
 * @param type_pos where the typecodes start
 * @param len length of typecodes
 * @returns #DBUS_VALID if valid, reason why invalid otherwise
 */
DBusValidity
_dbus_validate_signature_with_reason (const DBusString *type_str,
                                      int               type_pos,
                                      int               len)
{
  const unsigned char *p;
  const unsigned char *end;
  int last;
  int struct_depth;
  int array_depth;
  int dict_entry_depth;
  DBusValidity result;

  int element_count;
  DBusList *element_count_stack;
  char opened_brackets[DBUS_MAXIMUM_TYPE_RECURSION_DEPTH * 2 + 1] = { '\0' };
  char last_bracket;

  result = DBUS_VALID;
  element_count_stack = NULL;

  if (!_dbus_list_append (&element_count_stack, _DBUS_INT_TO_POINTER (0)))
    {
      result = DBUS_VALIDITY_UNKNOWN_OOM_ERROR;
      goto out;
    }

  _dbus_assert (type_str != NULL);
  _dbus_assert (type_pos < _DBUS_INT32_MAX - len);
  _dbus_assert (len >= 0);
  _dbus_assert (type_pos >= 0);

  if (len > DBUS_MAXIMUM_SIGNATURE_LENGTH)
    {
      result = DBUS_INVALID_SIGNATURE_TOO_LONG;
      goto out;
    }

  p = _dbus_string_get_const_udata_len (type_str, type_pos, 0);

  end = _dbus_string_get_const_udata_len (type_str, type_pos + len, 0);
  struct_depth = 0;
  array_depth = 0;
  dict_entry_depth = 0;
  last = DBUS_TYPE_INVALID;

  while (p != end)
    {
      _dbus_assert (struct_depth + dict_entry_depth >= 0);
      _dbus_assert (struct_depth + dict_entry_depth < _DBUS_N_ELEMENTS (opened_brackets));
      _dbus_assert (opened_brackets[struct_depth + dict_entry_depth] == '\0');

      switch (*p)
        {
        case DBUS_TYPE_BYTE:
        case DBUS_TYPE_BOOLEAN:
        case DBUS_TYPE_INT16:
        case DBUS_TYPE_UINT16:
        case DBUS_TYPE_INT32:
        case DBUS_TYPE_UINT32:
        case DBUS_TYPE_UNIX_FD:
        case DBUS_TYPE_INT64:
        case DBUS_TYPE_UINT64:
        case DBUS_TYPE_DOUBLE:
        case DBUS_TYPE_STRING:
        case DBUS_TYPE_OBJECT_PATH:
        case DBUS_TYPE_SIGNATURE:
        case DBUS_TYPE_VARIANT:
          break;

        case DBUS_TYPE_ARRAY:
          array_depth += 1;
          if (array_depth > DBUS_MAXIMUM_TYPE_RECURSION_DEPTH)
            {
              result = DBUS_INVALID_EXCEEDED_MAXIMUM_ARRAY_RECURSION;
              goto out;
            }
          break;

        case DBUS_STRUCT_BEGIN_CHAR:
          struct_depth += 1;

          if (struct_depth > DBUS_MAXIMUM_TYPE_RECURSION_DEPTH)
            {
              result = DBUS_INVALID_EXCEEDED_MAXIMUM_STRUCT_RECURSION;
              goto out;
            }
          
          if (!_dbus_list_append (&element_count_stack, 
                             _DBUS_INT_TO_POINTER (0)))
            {
              result = DBUS_VALIDITY_UNKNOWN_OOM_ERROR;
              goto out;
            }

          _dbus_assert (struct_depth + dict_entry_depth >= 1);
          _dbus_assert (struct_depth + dict_entry_depth < _DBUS_N_ELEMENTS (opened_brackets));
          _dbus_assert (opened_brackets[struct_depth + dict_entry_depth - 1] == '\0');
          opened_brackets[struct_depth + dict_entry_depth - 1] = DBUS_STRUCT_BEGIN_CHAR;
          break;

        case DBUS_STRUCT_END_CHAR:
          if (struct_depth == 0)
            {
              result = DBUS_INVALID_STRUCT_ENDED_BUT_NOT_STARTED;
              goto out;
            }

          if (last == DBUS_STRUCT_BEGIN_CHAR)
            {
              result = DBUS_INVALID_STRUCT_HAS_NO_FIELDS;
              goto out;
            }

          _dbus_assert (struct_depth + dict_entry_depth >= 1);
          _dbus_assert (struct_depth + dict_entry_depth < _DBUS_N_ELEMENTS (opened_brackets));
          last_bracket = opened_brackets[struct_depth + dict_entry_depth - 1];

          if (last_bracket != DBUS_STRUCT_BEGIN_CHAR)
            {
              result = DBUS_INVALID_STRUCT_ENDED_BUT_NOT_STARTED;
              goto out;
            }

          _dbus_list_pop_last (&element_count_stack);

          struct_depth -= 1;
          opened_brackets[struct_depth + dict_entry_depth] = '\0';
          break;

        case DBUS_DICT_ENTRY_BEGIN_CHAR:
          if (last != DBUS_TYPE_ARRAY)
            {
              result = DBUS_INVALID_DICT_ENTRY_NOT_INSIDE_ARRAY;
              goto out;
            }
            
          dict_entry_depth += 1;

          if (dict_entry_depth > DBUS_MAXIMUM_TYPE_RECURSION_DEPTH)
            {
              result = DBUS_INVALID_EXCEEDED_MAXIMUM_DICT_ENTRY_RECURSION;
              goto out;
            }

          if (!_dbus_list_append (&element_count_stack, 
                             _DBUS_INT_TO_POINTER (0)))
            {
              result = DBUS_VALIDITY_UNKNOWN_OOM_ERROR;
              goto out;
            }

          _dbus_assert (struct_depth + dict_entry_depth >= 1);
          _dbus_assert (struct_depth + dict_entry_depth < _DBUS_N_ELEMENTS (opened_brackets));
          _dbus_assert (opened_brackets[struct_depth + dict_entry_depth - 1] == '\0');
          opened_brackets[struct_depth + dict_entry_depth - 1] = DBUS_DICT_ENTRY_BEGIN_CHAR;
          break;

        case DBUS_DICT_ENTRY_END_CHAR:
          if (dict_entry_depth == 0)
            {
              result = DBUS_INVALID_DICT_ENTRY_ENDED_BUT_NOT_STARTED;
              goto out;
            }

          _dbus_assert (struct_depth + dict_entry_depth >= 1);
          _dbus_assert (struct_depth + dict_entry_depth < _DBUS_N_ELEMENTS (opened_brackets));
          last_bracket = opened_brackets[struct_depth + dict_entry_depth - 1];

          if (last_bracket != DBUS_DICT_ENTRY_BEGIN_CHAR)
            {
              result = DBUS_INVALID_DICT_ENTRY_ENDED_BUT_NOT_STARTED;
              goto out;
            }

          dict_entry_depth -= 1;
          opened_brackets[struct_depth + dict_entry_depth] = '\0';

          element_count = 
            _DBUS_POINTER_TO_INT (_dbus_list_pop_last (&element_count_stack));

          if (element_count != 2)
            {
              if (element_count == 0)
                result = DBUS_INVALID_DICT_ENTRY_HAS_NO_FIELDS;
              else if (element_count == 1)
                result = DBUS_INVALID_DICT_ENTRY_HAS_ONLY_ONE_FIELD;
              else
                result = DBUS_INVALID_DICT_ENTRY_HAS_TOO_MANY_FIELDS;
              
              goto out;
            }
          break;
          
        case DBUS_TYPE_STRUCT:     /* doesn't appear in signatures */
        case DBUS_TYPE_DICT_ENTRY: /* ditto */
        default:
          result = DBUS_INVALID_UNKNOWN_TYPECODE;
	  goto out;
        }

      if (*p != DBUS_TYPE_ARRAY && 
          *p != DBUS_DICT_ENTRY_BEGIN_CHAR && 
	  *p != DBUS_STRUCT_BEGIN_CHAR) 
        {
          element_count = 
            _DBUS_POINTER_TO_INT (_dbus_list_pop_last (&element_count_stack));

          ++element_count;

          if (!_dbus_list_append (&element_count_stack, 
                             _DBUS_INT_TO_POINTER (element_count)))
            {
              result = DBUS_VALIDITY_UNKNOWN_OOM_ERROR;
              goto out;
            }
        }
      
      if (array_depth > 0)
        {
          if (*p == DBUS_TYPE_ARRAY && p != end)
            {
               const unsigned char *p1;
	       p1 = p + 1;
               if (*p1 == DBUS_STRUCT_END_CHAR ||
                   *p1 == DBUS_DICT_ENTRY_END_CHAR)
                 {
                   result = DBUS_INVALID_MISSING_ARRAY_ELEMENT_TYPE;
                   goto out;
                 }
            }
          else
	    {
              array_depth = 0;
	    }
        }

      if (last == DBUS_DICT_ENTRY_BEGIN_CHAR)
        {
          if (!(dbus_type_is_valid (*p) && dbus_type_is_basic (*p)))
            {
              result = DBUS_INVALID_DICT_KEY_MUST_BE_BASIC_TYPE;
              goto out;
            }
        }

      last = *p;
      ++p;
    }


  if (array_depth > 0)
    {
      result = DBUS_INVALID_MISSING_ARRAY_ELEMENT_TYPE;
      goto out;
    }
    
  if (struct_depth > 0)
    {
       result = DBUS_INVALID_STRUCT_STARTED_BUT_NOT_ENDED;
       goto out;
    }
    
  if (dict_entry_depth > 0)
    {
      result =  DBUS_INVALID_DICT_ENTRY_STARTED_BUT_NOT_ENDED;
      goto out;
    }
    
  _dbus_assert (last != DBUS_TYPE_ARRAY);
  _dbus_assert (last != DBUS_STRUCT_BEGIN_CHAR);
  _dbus_assert (last != DBUS_DICT_ENTRY_BEGIN_CHAR);

  result = DBUS_VALID;

out:
  _dbus_list_clear (&element_count_stack);
  return result;
}

/* note: this function is also used to validate the header's values,
 * since the header is a valid body with a particular signature.
 */
static DBusValidity
validate_body_helper (DBusTypeReader       *reader,
                      int                   byte_order,
                      dbus_bool_t           walk_reader_to_end,
                      int                   total_depth,
                      const unsigned char  *p,
                      const unsigned char  *end,
                      const unsigned char **new_p)
{
  int current_type;

  /* The spec allows arrays and structs to each nest 32, for total
   * nesting of 2*32. We want to impose the same limit on "dynamic"
   * value nesting (not visible in the signature) which is introduced
   * by DBUS_TYPE_VARIANT.
   */
  if (total_depth > (DBUS_MAXIMUM_TYPE_RECURSION_DEPTH * 2))
    {
      return DBUS_INVALID_NESTED_TOO_DEEPLY;
    }

  while ((current_type = _dbus_type_reader_get_current_type (reader)) != DBUS_TYPE_INVALID)
    {
      const unsigned char *a;
      int alignment;

#if 0
      _dbus_verbose ("   validating value of type %s type reader %p type_pos %d p %p end %p %d remain\n",
                     _dbus_type_to_string (current_type), reader, reader->type_pos, p, end,
                     (int) (end - p));
#endif

      /* Guarantee that p has one byte to look at */
      if (p == end)
        return DBUS_INVALID_NOT_ENOUGH_DATA;

      switch (current_type)
        {
        case DBUS_TYPE_BYTE:
          ++p;
          break;

        case DBUS_TYPE_BOOLEAN:
        case DBUS_TYPE_INT16:
        case DBUS_TYPE_UINT16:
        case DBUS_TYPE_INT32:
        case DBUS_TYPE_UINT32:
        case DBUS_TYPE_UNIX_FD:
        case DBUS_TYPE_INT64:
        case DBUS_TYPE_UINT64:
        case DBUS_TYPE_DOUBLE:
          alignment = _dbus_type_get_alignment (current_type);
          a = _DBUS_ALIGN_ADDRESS (p, alignment);
          if (a >= end)
            return DBUS_INVALID_NOT_ENOUGH_DATA;
          while (p != a)
            {
              if (*p != '\0')
                return DBUS_INVALID_ALIGNMENT_PADDING_NOT_NUL;
              ++p;
            }
          
          if (current_type == DBUS_TYPE_BOOLEAN)
            {
              dbus_uint32_t v;

              if (p + 4 > end)
                return DBUS_INVALID_NOT_ENOUGH_DATA;

              v = _dbus_unpack_uint32 (byte_order, p);

              if (!(v == 0 || v == 1))
                return DBUS_INVALID_BOOLEAN_NOT_ZERO_OR_ONE;
            }
          
          p += alignment;
          break;

        case DBUS_TYPE_ARRAY:
        case DBUS_TYPE_STRING:
        case DBUS_TYPE_OBJECT_PATH:
          {
            dbus_uint32_t claimed_len;

            a = _DBUS_ALIGN_ADDRESS (p, 4);
            if (a + 4 > end)
              return DBUS_INVALID_NOT_ENOUGH_DATA;
            while (p != a)
              {
                if (*p != '\0')
                  return DBUS_INVALID_ALIGNMENT_PADDING_NOT_NUL;
                ++p;
              }

            claimed_len = _dbus_unpack_uint32 (byte_order, p);
            p += 4;

            /* p may now be == end */
            _dbus_assert (p <= end);

            if (current_type == DBUS_TYPE_ARRAY)
              {
                int array_elem_type = _dbus_type_reader_get_element_type (reader);

                if (!dbus_type_is_valid (array_elem_type))
                  {
                    return DBUS_INVALID_UNKNOWN_TYPECODE;
                  }

                alignment = _dbus_type_get_alignment (array_elem_type);

                a = _DBUS_ALIGN_ADDRESS (p, alignment);

                /* a may now be == end */
                if (a > end)
                  return DBUS_INVALID_NOT_ENOUGH_DATA;

                while (p != a)
                  {
                    if (*p != '\0')
                      return DBUS_INVALID_ALIGNMENT_PADDING_NOT_NUL;
                    ++p;
                  }
              }

            if (claimed_len > (unsigned long) (end - p))
              return DBUS_INVALID_LENGTH_OUT_OF_BOUNDS;

            if (current_type == DBUS_TYPE_OBJECT_PATH)
              {
                DBusString str;
                _dbus_string_init_const_len (&str, (const char *) p, claimed_len);
                if (!_dbus_validate_path (&str, 0,
                                          _dbus_string_get_length (&str)))
                  return DBUS_INVALID_BAD_PATH;

                p += claimed_len;
              }
            else if (current_type == DBUS_TYPE_STRING)
              {
                DBusString str;
                _dbus_string_init_const_len (&str, (const char *) p, claimed_len);
                if (!_dbus_string_validate_utf8 (&str, 0,
                                                 _dbus_string_get_length (&str)))
                  return DBUS_INVALID_BAD_UTF8_IN_STRING;

                p += claimed_len;
              }
            else if (current_type == DBUS_TYPE_ARRAY && claimed_len > 0)
              {
                DBusTypeReader sub;
                DBusValidity validity;
                const unsigned char *array_end;
                int array_elem_type;

                if (claimed_len > DBUS_MAXIMUM_ARRAY_LENGTH)
                  return DBUS_INVALID_ARRAY_LENGTH_EXCEEDS_MAXIMUM;
                
                /* Remember that the reader is types only, so we can't
                 * use it to iterate over elements. It stays the same
                 * for all elements.
                 */
                _dbus_type_reader_recurse (reader, &sub);

                array_end = p + claimed_len;

                array_elem_type = _dbus_type_reader_get_element_type (reader);

                /* avoid recursive call to validate_body_helper if this is an array
                 * of fixed-size elements
                 */ 
                if (dbus_type_is_fixed (array_elem_type))
                  {
                    /* Note that fixed-size types all have sizes equal to
                     * their alignments, so this is really the item size. */
                    alignment = _dbus_type_get_alignment (array_elem_type);
                    _dbus_assert (alignment == 1 || alignment == 2 ||
                                  alignment == 4 || alignment == 8);

                    /* Because the alignment is a power of 2, this is
                     * equivalent to: (claimed_len % alignment) != 0,
                     * but avoids slower integer division */
                    if ((claimed_len & (alignment - 1)) != 0)
                      return DBUS_INVALID_ARRAY_LENGTH_INCORRECT;

                    /* bools need to be handled differently, because they can
                     * have an invalid value
                     */
                    if (array_elem_type == DBUS_TYPE_BOOLEAN)
                      {
                        dbus_uint32_t v;

                        while (p < array_end)
                          {
                            v = _dbus_unpack_uint32 (byte_order, p);

                            if (!(v == 0 || v == 1))
                              return DBUS_INVALID_BOOLEAN_NOT_ZERO_OR_ONE;

                            p += alignment;
                          }
                      }

                    else
                      {
                        p = array_end;
                      }
                  }

                else
                  {
                    while (p < array_end)
                      {
                        validity = validate_body_helper (&sub, byte_order, FALSE,
                                                         total_depth + 1,
                                                         p, end, &p);
                        if (validity != DBUS_VALID)
                          return validity;
                      }
                  }

                if (p != array_end)
                  return DBUS_INVALID_ARRAY_LENGTH_INCORRECT;
              }

            /* check nul termination */
            if (current_type != DBUS_TYPE_ARRAY)
              {
                if (p == end)
                  return DBUS_INVALID_NOT_ENOUGH_DATA;

                if (*p != '\0')
                  return DBUS_INVALID_STRING_MISSING_NUL;
                ++p;
              }
          }
          break;

        case DBUS_TYPE_SIGNATURE:
          {
            dbus_uint32_t claimed_len;
            DBusString str;
            DBusValidity validity;

            claimed_len = *p;
            ++p;

            /* 1 is for nul termination */
            if (claimed_len + 1 > (unsigned long) (end - p))
              return DBUS_INVALID_SIGNATURE_LENGTH_OUT_OF_BOUNDS;

            _dbus_string_init_const_len (&str, (const char *) p, claimed_len);
            validity =
              _dbus_validate_signature_with_reason (&str, 0,
                                                    _dbus_string_get_length (&str));

            if (validity != DBUS_VALID)
              return validity;

            p += claimed_len;

            _dbus_assert (p < end);
            if (*p != DBUS_TYPE_INVALID)
              return DBUS_INVALID_SIGNATURE_MISSING_NUL;

            ++p;

            _dbus_verbose ("p = %p end = %p claimed_len %u\n", p, end, claimed_len);
          }
          break;

        case DBUS_TYPE_VARIANT:
          {
            /* 1 byte sig len, sig typecodes, align to
             * contained-type-boundary, values.
             */

            /* In addition to normal signature validation, we need to be sure
             * the signature contains only a single (possibly container) type.
             */
            dbus_uint32_t claimed_len;
            DBusString sig;
            DBusTypeReader sub;
            DBusValidity validity;
            int contained_alignment;
            int contained_type;
            DBusValidity reason;

            claimed_len = *p;
            ++p;

            /* + 1 for nul */
            if (claimed_len + 1 > (unsigned long) (end - p))
              return DBUS_INVALID_VARIANT_SIGNATURE_LENGTH_OUT_OF_BOUNDS;

            _dbus_string_init_const_len (&sig, (const char *) p, claimed_len);
            reason = _dbus_validate_signature_with_reason (&sig, 0,
                                           _dbus_string_get_length (&sig));
            if (!(reason == DBUS_VALID))
              {
                if (reason == DBUS_VALIDITY_UNKNOWN_OOM_ERROR)
                  return reason;
                else 
                  return DBUS_INVALID_VARIANT_SIGNATURE_BAD;
              }

            p += claimed_len;
            
            if (*p != DBUS_TYPE_INVALID)
              return DBUS_INVALID_VARIANT_SIGNATURE_MISSING_NUL;
            ++p;

            contained_type = _dbus_first_type_in_signature (&sig, 0);
            if (contained_type == DBUS_TYPE_INVALID)
              return DBUS_INVALID_VARIANT_SIGNATURE_EMPTY;
            
            contained_alignment = _dbus_type_get_alignment (contained_type);
            
            a = _DBUS_ALIGN_ADDRESS (p, contained_alignment);
            if (a > end)
              return DBUS_INVALID_NOT_ENOUGH_DATA;
            while (p != a)
              {
                if (*p != '\0')
                  return DBUS_INVALID_ALIGNMENT_PADDING_NOT_NUL;
                ++p;
              }

            _dbus_type_reader_init_types_only (&sub, &sig, 0);

            _dbus_assert (_dbus_type_reader_get_current_type (&sub) != DBUS_TYPE_INVALID);

            validity = validate_body_helper (&sub, byte_order, FALSE,
                                             total_depth + 1,
                                             p, end, &p);
            if (validity != DBUS_VALID)
              return validity;

            if (_dbus_type_reader_next (&sub))
              return DBUS_INVALID_VARIANT_SIGNATURE_SPECIFIES_MULTIPLE_VALUES;

            _dbus_assert (_dbus_type_reader_get_current_type (&sub) == DBUS_TYPE_INVALID);
          }
          break;

        case DBUS_TYPE_DICT_ENTRY:
        case DBUS_TYPE_STRUCT:
          {
            DBusTypeReader sub;
            DBusValidity validity;

            a = _DBUS_ALIGN_ADDRESS (p, 8);
            if (a > end)
              return DBUS_INVALID_NOT_ENOUGH_DATA;
            while (p != a)
              {
                if (*p != '\0')
                  return DBUS_INVALID_ALIGNMENT_PADDING_NOT_NUL;
                ++p;
              }

            _dbus_type_reader_recurse (reader, &sub);

            validity = validate_body_helper (&sub, byte_order, TRUE,
                                             total_depth + 1,
                                             p, end, &p);
            if (validity != DBUS_VALID)
              return validity;
          }
          break;

        default:
          _dbus_assert_not_reached ("invalid typecode in supposedly-validated signature");
          break;
        }

#if 0
      _dbus_verbose ("   validated value of type %s type reader %p type_pos %d p %p end %p %d remain\n",
                     _dbus_type_to_string (current_type), reader, reader->type_pos, p, end,
                     (int) (end - p));
#endif

      if (p > end)
        {
          _dbus_verbose ("not enough data!!! p = %p end = %p end-p = %d\n",
                         p, end, (int) (end - p));
          return DBUS_INVALID_NOT_ENOUGH_DATA;
        }

      if (walk_reader_to_end)
        _dbus_type_reader_next (reader);
      else
        break;
    }

  if (new_p)
    *new_p = p;

  return DBUS_VALID;
}

/**
 * Verifies that the range of value_str from value_pos to value_end is
 * a legitimate value of type expected_signature.  If this function
 * returns #TRUE, it will be safe to iterate over the values with
 * #DBusTypeReader. The signature is assumed to be already valid.
 *
 * If bytes_remaining is not #NULL, then leftover bytes will be stored
 * there and #DBUS_VALID returned. If it is #NULL, then
 * #DBUS_INVALID_TOO_MUCH_DATA will be returned if bytes are left
 * over.
 *
 * @param expected_signature the expected types in the value_str
 * @param expected_signature_start where in expected_signature is the signature
 * @param byte_order the byte order
 * @param bytes_remaining place to store leftover bytes
 * @param value_str the string containing the body
 * @param value_pos where the values start
 * @param len length of values after value_pos
 * @returns #DBUS_VALID if valid, reason why invalid otherwise
 */
DBusValidity
_dbus_validate_body_with_reason (const DBusString *expected_signature,
                                 int               expected_signature_start,
                                 int               byte_order,
                                 int              *bytes_remaining,
                                 const DBusString *value_str,
                                 int               value_pos,
                                 int               len)
{
  DBusTypeReader reader;
  const unsigned char *p;
  const unsigned char *end;
  DBusValidity validity;

  _dbus_assert (len >= 0);
  _dbus_assert (value_pos >= 0);
  _dbus_assert (value_pos <= _dbus_string_get_length (value_str) - len);

  _dbus_verbose ("validating body from pos %d len %d sig '%s'\n",
                 value_pos, len, _dbus_string_get_const_data_len (expected_signature,
                                                                  expected_signature_start,
                                                                  0));

  _dbus_type_reader_init_types_only (&reader,
                                     expected_signature, expected_signature_start);

  p = _dbus_string_get_const_udata_len (value_str, value_pos, len);
  end = p + len;

  validity = validate_body_helper (&reader, byte_order, TRUE, 0, p, end, &p);
  if (validity != DBUS_VALID)
    return validity;
  
  if (bytes_remaining)
    {
      *bytes_remaining = end - p;
      return DBUS_VALID;
    }
  else if (p < end)
    return DBUS_INVALID_TOO_MUCH_DATA;
  else
    {
      _dbus_assert (p == end);
      return DBUS_VALID;
    }
}

/**
 * Determine wether the given character is valid as the first character
 * in a name.
 */
#define VALID_INITIAL_NAME_CHARACTER(c)         \
  ( ((c) >= 'A' && (c) <= 'Z') ||               \
    ((c) >= 'a' && (c) <= 'z') ||               \
    ((c) == '_') )

/**
 * Determine wether the given character is valid as a second or later
 * character in a name
 */
#define VALID_NAME_CHARACTER(c)                 \
  ( ((c) >= '0' && (c) <= '9') ||               \
    ((c) >= 'A' && (c) <= 'Z') ||               \
    ((c) >= 'a' && (c) <= 'z') ||               \
    ((c) == '_') )

/**
 * Checks that the given range of the string is a valid object path
 * name in the D-Bus protocol. Part of the validation ensures that
 * the object path contains only ASCII.
 *
 * @todo this is inconsistent with most of DBusString in that
 * it allows a start,len range that extends past the string end.
 *
 * @todo change spec to disallow more things, such as spaces in the
 * path name
 *
 * @param str the string
 * @param start first byte index to check
 * @param len number of bytes to check
 * @returns #TRUE if the byte range exists and is a valid name
 */
dbus_bool_t
_dbus_validate_path (const DBusString  *str,
                     int                start,
                     int                len)
{
  const unsigned char *s;
  const unsigned char *end;
  const unsigned char *last_slash;

  _dbus_assert (start >= 0);
  _dbus_assert (len >= 0);
  _dbus_assert (start <= _dbus_string_get_length (str));
  
  if (len > _dbus_string_get_length (str) - start)
    return FALSE;

  if (len == 0)
    return FALSE;

  s = _dbus_string_get_const_udata (str) + start;
  end = s + len;

  if (*s != '/')
    return FALSE;
  last_slash = s;
  ++s;

  while (s != end)
    {
      if (*s == '/')
        {
          if ((s - last_slash) < 2)
            return FALSE; /* no empty path components allowed */

          last_slash = s;
        }
      else
        {
          if (_DBUS_UNLIKELY (!VALID_NAME_CHARACTER (*s)))
            return FALSE;
        }

      ++s;
    }

  if ((end - last_slash) < 2 &&
      len > 1)
    return FALSE; /* trailing slash not allowed unless the string is "/" */

  return TRUE;
}

const char *
_dbus_validity_to_error_message (DBusValidity validity)
{
  switch (validity)
    {
    case DBUS_VALIDITY_UNKNOWN_OOM_ERROR:                          return "Out of memory";
    case DBUS_INVALID_FOR_UNKNOWN_REASON:                          return "Unknown reason";
    case DBUS_VALID_BUT_INCOMPLETE:                                return "Valid but incomplete";
    case DBUS_VALIDITY_UNKNOWN:                                    return "Validity unknown";
    case DBUS_VALID:                                               return "Valid";
    case DBUS_INVALID_UNKNOWN_TYPECODE:                            return "Unknown typecode";
    case DBUS_INVALID_MISSING_ARRAY_ELEMENT_TYPE:                  return "Missing array element type";
    case DBUS_INVALID_SIGNATURE_TOO_LONG:                          return "Signature is too long";
    case DBUS_INVALID_EXCEEDED_MAXIMUM_ARRAY_RECURSION:            return "Exceeded maximum array recursion";
    case DBUS_INVALID_EXCEEDED_MAXIMUM_STRUCT_RECURSION:           return "Exceeded maximum struct recursion";
    case DBUS_INVALID_STRUCT_ENDED_BUT_NOT_STARTED:                return "Struct ended but not started";
    case DBUS_INVALID_STRUCT_STARTED_BUT_NOT_ENDED:                return "Struct started but not ended";
    case DBUS_INVALID_STRUCT_HAS_NO_FIELDS:                        return "Struct has no fields";
    case DBUS_INVALID_ALIGNMENT_PADDING_NOT_NUL:                   return "Alignment padding not null";
    case DBUS_INVALID_BOOLEAN_NOT_ZERO_OR_ONE:                     return "Boolean is not zero or one";
    case DBUS_INVALID_NOT_ENOUGH_DATA:                             return "Not enough data";
    case DBUS_INVALID_TOO_MUCH_DATA:                               return "Too much data";
    case DBUS_INVALID_BAD_BYTE_ORDER:                              return "Bad byte order";
    case DBUS_INVALID_BAD_PROTOCOL_VERSION:                        return "Bad protocol version";
    case DBUS_INVALID_BAD_MESSAGE_TYPE:                            return "Bad message type";
    case DBUS_INVALID_BAD_SERIAL:                                  return "Bad serial";
    case DBUS_INVALID_INSANE_FIELDS_ARRAY_LENGTH:                  return "Insane fields array length";
    case DBUS_INVALID_INSANE_BODY_LENGTH:                          return "Insane body length";
    case DBUS_INVALID_MESSAGE_TOO_LONG:                            return "Message too long";
    case DBUS_INVALID_HEADER_FIELD_CODE:                           return "Header field code";
    case DBUS_INVALID_HEADER_FIELD_HAS_WRONG_TYPE:                 return "Header field has wrong type";
    case DBUS_INVALID_USES_LOCAL_INTERFACE:                        return "Uses local interface";
    case DBUS_INVALID_USES_LOCAL_PATH:                             return "Uses local path";
    case DBUS_INVALID_HEADER_FIELD_APPEARS_TWICE:                  return "Header field appears twice";
    case DBUS_INVALID_BAD_DESTINATION:                             return "Bad destination";
    case DBUS_INVALID_BAD_INTERFACE:                               return "Bad interface";
    case DBUS_INVALID_BAD_MEMBER:                                  return "Bad member";
    case DBUS_INVALID_BAD_ERROR_NAME:                              return "Bad error name";
    case DBUS_INVALID_BAD_SENDER:                                  return "Bad sender";
    case DBUS_INVALID_MISSING_PATH:                                return "Missing path";
    case DBUS_INVALID_MISSING_INTERFACE:                           return "Missing interface";
    case DBUS_INVALID_MISSING_MEMBER:                              return "Missing member";
    case DBUS_INVALID_MISSING_ERROR_NAME:                          return "Missing error name";
    case DBUS_INVALID_MISSING_REPLY_SERIAL:                        return "Missing reply serial";
    case DBUS_INVALID_LENGTH_OUT_OF_BOUNDS:                        return "Length out of bounds";
    case DBUS_INVALID_ARRAY_LENGTH_EXCEEDS_MAXIMUM:                return "Array length exceeds maximum";
    case DBUS_INVALID_BAD_PATH:                                    return "Bad path";
    case DBUS_INVALID_SIGNATURE_LENGTH_OUT_OF_BOUNDS:              return "Signature length out of bounds";
    case DBUS_INVALID_BAD_UTF8_IN_STRING:                          return "Bad utf8 in string";
    case DBUS_INVALID_ARRAY_LENGTH_INCORRECT:                      return "Array length incorrect";
    case DBUS_INVALID_VARIANT_SIGNATURE_LENGTH_OUT_OF_BOUNDS:      return "Variant signature length out of bounds";
    case DBUS_INVALID_VARIANT_SIGNATURE_BAD:                       return "Variant signature bad";
    case DBUS_INVALID_VARIANT_SIGNATURE_EMPTY:                     return "Variant signature empty";
    case DBUS_INVALID_VARIANT_SIGNATURE_SPECIFIES_MULTIPLE_VALUES: return "Variant signature specifies multiple values";
    case DBUS_INVALID_VARIANT_SIGNATURE_MISSING_NUL:               return "Variant signature missing nul";
    case DBUS_INVALID_STRING_MISSING_NUL:                          return "String missing nul";
    case DBUS_INVALID_SIGNATURE_MISSING_NUL:                       return "Signature missing nul";
    case DBUS_INVALID_EXCEEDED_MAXIMUM_DICT_ENTRY_RECURSION:       return "Exceeded maximum dict entry recursion";
    case DBUS_INVALID_DICT_ENTRY_ENDED_BUT_NOT_STARTED:            return "Dict entry ended but not started";
    case DBUS_INVALID_DICT_ENTRY_STARTED_BUT_NOT_ENDED:            return "Dict entry started but not ended";
    case DBUS_INVALID_DICT_ENTRY_HAS_NO_FIELDS:                    return "Dict entry has no fields";
    case DBUS_INVALID_DICT_ENTRY_HAS_ONLY_ONE_FIELD:               return "Dict entry has only one field";
    case DBUS_INVALID_DICT_ENTRY_HAS_TOO_MANY_FIELDS:              return "Dict entry has too many fields";
    case DBUS_INVALID_DICT_ENTRY_NOT_INSIDE_ARRAY:                 return "Dict entry not inside array";
    case DBUS_INVALID_DICT_KEY_MUST_BE_BASIC_TYPE:                 return "Dict key must be basic type";
    case DBUS_INVALID_MISSING_UNIX_FDS:                            return "Unix file descriptor missing";
    case DBUS_INVALID_NESTED_TOO_DEEPLY:                           return "Variants cannot be used to create a hugely recursive tree of values";
    case DBUS_VALIDITY_LAST:
    default:
      return "Invalid";
    }
}

/**
 * Checks that the given range of the string is a valid interface name
 * in the D-Bus protocol. This includes a length restriction and an
 * ASCII subset, see the specification.
 *
 * @todo this is inconsistent with most of DBusString in that
 * it allows a start,len range that extends past the string end.
 *
 * @param str the string
 * @param start first byte index to check
 * @param len number of bytes to check
 * @returns #TRUE if the byte range exists and is a valid name
 */
dbus_bool_t
_dbus_validate_interface (const DBusString  *str,
                          int                start,
                          int                len)
{
  const unsigned char *s;
  const unsigned char *end;
  const unsigned char *iface;
  const unsigned char *last_dot;

  _dbus_assert (start >= 0);
  _dbus_assert (len >= 0);
  _dbus_assert (start <= _dbus_string_get_length (str));

  if (len > _dbus_string_get_length (str) - start)
    return FALSE;

  if (len > DBUS_MAXIMUM_NAME_LENGTH)
    return FALSE;

  if (len == 0)
    return FALSE;

  last_dot = NULL;
  iface = _dbus_string_get_const_udata (str) + start;
  end = iface + len;
  s = iface;

  /* check special cases of first char so it doesn't have to be done
   * in the loop. Note we know len > 0
   */
  if (_DBUS_UNLIKELY (*s == '.')) /* disallow starting with a . */
    return FALSE;
  else if (_DBUS_UNLIKELY (!VALID_INITIAL_NAME_CHARACTER (*s)))
    return FALSE;
  else
    ++s;

  while (s != end)
    {
      if (*s == '.')
        {
          if (_DBUS_UNLIKELY ((s + 1) == end))
            return FALSE;
          else if (_DBUS_UNLIKELY (!VALID_INITIAL_NAME_CHARACTER (*(s + 1))))
            return FALSE;
          last_dot = s;
          ++s; /* we just validated the next char, so skip two */
        }
      else if (_DBUS_UNLIKELY (!VALID_NAME_CHARACTER (*s)))
        {
          return FALSE;
        }

      ++s;
    }

  if (_DBUS_UNLIKELY (last_dot == NULL))
    return FALSE;

  return TRUE;
}

/**
 * Checks that the given range of the string is a valid member name
 * in the D-Bus protocol. This includes a length restriction, etc.,
 * see the specification.
 *
 * @todo this is inconsistent with most of DBusString in that
 * it allows a start,len range that extends past the string end.
 *
 * @param str the string
 * @param start first byte index to check
 * @param len number of bytes to check
 * @returns #TRUE if the byte range exists and is a valid name
 */
dbus_bool_t
_dbus_validate_member (const DBusString  *str,
                       int                start,
                       int                len)
{
  const unsigned char *s;
  const unsigned char *end;
  const unsigned char *member;

  _dbus_assert (start >= 0);
  _dbus_assert (len >= 0);
  _dbus_assert (start <= _dbus_string_get_length (str));

  if (len > _dbus_string_get_length (str) - start)
    return FALSE;

  if (len > DBUS_MAXIMUM_NAME_LENGTH)
    return FALSE;

  if (len == 0)
    return FALSE;

  member = _dbus_string_get_const_udata (str) + start;
  end = member + len;
  s = member;

  /* check special cases of first char so it doesn't have to be done
   * in the loop. Note we know len > 0
   */

  if (_DBUS_UNLIKELY (!VALID_INITIAL_NAME_CHARACTER (*s)))
    return FALSE;
  else
    ++s;

  while (s != end)
    {
      if (_DBUS_UNLIKELY (!VALID_NAME_CHARACTER (*s)))
        {
          return FALSE;
        }

      ++s;
    }

  return TRUE;
}

/**
 * Checks that the given range of the string is a valid error name
 * in the D-Bus protocol. This includes a length restriction, etc.,
 * see the specification.
 *
 * @todo this is inconsistent with most of DBusString in that
 * it allows a start,len range that extends past the string end.
 *
 * @param str the string
 * @param start first byte index to check
 * @param len number of bytes to check
 * @returns #TRUE if the byte range exists and is a valid name
 */
dbus_bool_t
_dbus_validate_error_name (const DBusString  *str,
                           int                start,
                           int                len)
{
  /* Same restrictions as interface name at the moment */
  return _dbus_validate_interface (str, start, len);
}

/**
 * Determine wether the given character is valid as the first character
 * in a bus name.
 */
#define VALID_INITIAL_BUS_NAME_CHARACTER(c)         \
  ( ((c) >= 'A' && (c) <= 'Z') ||               \
    ((c) >= 'a' && (c) <= 'z') ||               \
    ((c) == '_') || ((c) == '-'))

/**
 * Determine wether the given character is valid as a second or later
 * character in a bus name
 */
#define VALID_BUS_NAME_CHARACTER(c)                 \
  ( ((c) >= '0' && (c) <= '9') ||               \
    ((c) >= 'A' && (c) <= 'Z') ||               \
    ((c) >= 'a' && (c) <= 'z') ||               \
    ((c) == '_') || ((c) == '-'))

static dbus_bool_t
_dbus_validate_bus_name_full (const DBusString  *str,
                              int                start,
                              int                len,
                              dbus_bool_t        is_namespace)
{
  const unsigned char *s;
  const unsigned char *end;
  const unsigned char *iface;
  const unsigned char *last_dot;

  _dbus_assert (start >= 0);
  _dbus_assert (len >= 0);
  _dbus_assert (start <= _dbus_string_get_length (str));

  if (len > _dbus_string_get_length (str) - start)
    return FALSE;

  if (len > DBUS_MAXIMUM_NAME_LENGTH)
    return FALSE;

  if (len == 0)
    return FALSE;

  last_dot = NULL;
  iface = _dbus_string_get_const_udata (str) + start;
  end = iface + len;
  s = iface;

  /* check special cases of first char so it doesn't have to be done
   * in the loop. Note we know len > 0
   */
  if (*s == ':')
  {
    /* unique name */
    ++s;
    while (s != end)
      {
        if (*s == '.')
          {
            if (_DBUS_UNLIKELY ((s + 1) == end))
              return FALSE;
            if (_DBUS_UNLIKELY (!VALID_BUS_NAME_CHARACTER (*(s + 1))))
              return FALSE;
            ++s; /* we just validated the next char, so skip two */
          }
        else if (_DBUS_UNLIKELY (!VALID_BUS_NAME_CHARACTER (*s)))
          {
            return FALSE;
          }

        ++s;
      }

    return TRUE;
  }
  else if (_DBUS_UNLIKELY (*s == '.')) /* disallow starting with a . */
    return FALSE;
  else if (_DBUS_UNLIKELY (!VALID_INITIAL_BUS_NAME_CHARACTER (*s)))
    return FALSE;
  else
    ++s;

  while (s != end)
    {
      if (*s == '.')
        {
          if (_DBUS_UNLIKELY ((s + 1) == end))
            return FALSE;
          else if (_DBUS_UNLIKELY (!VALID_INITIAL_BUS_NAME_CHARACTER (*(s + 1))))
            return FALSE;
          last_dot = s;
          ++s; /* we just validated the next char, so skip two */
        }
      else if (_DBUS_UNLIKELY (!VALID_BUS_NAME_CHARACTER (*s)))
        {
          return FALSE;
        }

      ++s;
    }

  if (!is_namespace && _DBUS_UNLIKELY (last_dot == NULL))
    return FALSE;

  return TRUE;
}

/**
 * Checks that the given range of the string is a valid bus name in
 * the D-Bus protocol. This includes a length restriction, etc., see
 * the specification.
 *
 * @todo this is inconsistent with most of DBusString in that
 * it allows a start,len range that extends past the string end.
 *
 * @param str the string
 * @param start first byte index to check
 * @param len number of bytes to check
 * @returns #TRUE if the byte range exists and is a valid name
 */
dbus_bool_t
_dbus_validate_bus_name (const DBusString  *str,
                         int                start,
                         int                len)
{
  return _dbus_validate_bus_name_full (str, start, len, FALSE);
}

/**
 * Checks that the given range of the string is a prefix of a valid bus name in
 * the D-Bus protocol. Unlike _dbus_validate_bus_name(), this accepts strings
 * with only one period-separated component.
 *
 * @todo this is inconsistent with most of DBusString in that
 * it allows a start,len range that extends past the string end.
 *
 * @param str the string
 * @param start first byte index to check
 * @param len number of bytes to check
 * @returns #TRUE if the byte range exists and is a valid name
 */
dbus_bool_t
_dbus_validate_bus_namespace (const DBusString  *str,
                              int                start,
                              int                len)
{
  return _dbus_validate_bus_name_full (str, start, len, TRUE);
}

/** define _dbus_check_is_valid_path() */
DEFINE_DBUS_NAME_CHECK(path)
/** define _dbus_check_is_valid_interface() */
DEFINE_DBUS_NAME_CHECK(interface)
/** define _dbus_check_is_valid_member() */
DEFINE_DBUS_NAME_CHECK(member)
/** define _dbus_check_is_valid_error_name() */
DEFINE_DBUS_NAME_CHECK(error_name)
/** define _dbus_check_is_valid_bus_name() */
DEFINE_DBUS_NAME_CHECK(bus_name)
/** define _dbus_check_is_valid_utf8() */
DEFINE_DBUS_NAME_CHECK(utf8)

/** @} */

/* tests in dbus-marshal-validate-util.c */
