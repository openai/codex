/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-signature.c  Routines for reading recursive type signatures
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

#include "dbus-signature.h"
#include "dbus-marshal-recursive.h"
#include "dbus-marshal-basic.h"
#include "dbus-internals.h"
#include "dbus-test.h"
#include <dbus/dbus-test-tap.h>

/**
 * Implementation details of #DBusSignatureIter, all fields are private
 */
typedef struct
{ 
  const char *pos;           /**< current position in the signature string */
  unsigned int finished : 1; /**< true if we are at the end iter */
  unsigned int in_array : 1; /**< true if we are a subiterator pointing to an array's element type */
} DBusSignatureRealIter;

_DBUS_STATIC_ASSERT (sizeof (DBusSignatureIter) >= sizeof (DBusSignatureRealIter));

/** macro that checks whether a typecode is a container type */
#define TYPE_IS_CONTAINER(typecode)             \
    ((typecode) == DBUS_TYPE_STRUCT ||          \
     (typecode) == DBUS_TYPE_DICT_ENTRY ||      \
     (typecode) == DBUS_TYPE_VARIANT ||         \
     (typecode) == DBUS_TYPE_ARRAY)


/**
 * @defgroup DBusSignature Type signature parsing
 * @ingroup  DBus
 * @brief Parsing D-Bus type signatures
 * @{
 */

/**
 * Initializes a #DBusSignatureIter for reading a type signature.  This
 * function is not safe to use on invalid signatures; be sure to
 * validate potentially invalid signatures with dbus_signature_validate
 * before using this function.
 *
 * @param iter pointer to an iterator to initialize
 * @param signature the type signature
 */
void
dbus_signature_iter_init (DBusSignatureIter *iter,
			  const char        *signature)
{
  DBusSignatureRealIter *real_iter = (DBusSignatureRealIter *) iter;

  real_iter->pos = signature;
  real_iter->finished = FALSE;
  real_iter->in_array = FALSE;
}

/**
 * Returns the current type pointed to by the iterator.
 * If the iterator is pointing at a type code such as 's', then
 * it will be returned directly.
 *
 * However, when the parser encounters a container type start
 * character such as '(' for a structure, the corresponding type for
 * the container will be returned, e.g.  DBUS_TYPE_STRUCT, not '('.
 * In this case, you should initialize a sub-iterator with
 * dbus_signature_iter_recurse() to parse the container type.
 *
 * @param iter pointer to an iterator 
 * @returns current type (e.g. #DBUS_TYPE_STRING, #DBUS_TYPE_ARRAY)
 */
int
dbus_signature_iter_get_current_type (const DBusSignatureIter *iter)
{
  DBusSignatureRealIter *real_iter = (DBusSignatureRealIter *) iter;

  return _dbus_first_type_in_signature_c_str (real_iter->pos, 0);
}

/**
 * Returns the signature of the single complete type starting at the
 * given iterator.
 *
 * For example, if the iterator is pointing at the start of "(ii)ii"
 * (which is "a struct of two ints, followed by an int, followed by an
 * int"), then "(ii)" would be returned. If the iterator is pointing at
 * one of the "i" then just that "i" would be returned.
 *
 * @param iter pointer to an iterator 
 * @returns current signature; or #NULL if no memory.  Should be freed with dbus_free()
 */
char *
dbus_signature_iter_get_signature (const DBusSignatureIter *iter)
{
  DBusSignatureRealIter *real_iter = (DBusSignatureRealIter *) iter;
  DBusString str;
  char *ret;
  int pos;
  
  if (!_dbus_string_init (&str))
    return NULL;

  pos = 0;
  _dbus_type_signature_next (real_iter->pos, &pos);

  if (!_dbus_string_append_len (&str, real_iter->pos, pos))
    return NULL;
  if (!_dbus_string_steal_data (&str, &ret))
    ret = NULL;
  _dbus_string_free (&str);

  return ret; 
}

/**
 * Convenience function for returning the element type of an array;
 * This function allows you to avoid initializing a sub-iterator and
 * getting its current type.
 *
 * Undefined behavior results if you invoke this function when the
 * current type of the iterator is not #DBUS_TYPE_ARRAY.
 *
 * @param iter pointer to an iterator 
 * @returns current array element type
 */
int
dbus_signature_iter_get_element_type (const DBusSignatureIter *iter)
{
  DBusSignatureRealIter *real_iter = (DBusSignatureRealIter *) iter;

  _dbus_return_val_if_fail (dbus_signature_iter_get_current_type (iter) == DBUS_TYPE_ARRAY, DBUS_TYPE_INVALID);

  return _dbus_first_type_in_signature_c_str (real_iter->pos, 1);
}

/**
 * Skip to the next value on this "level". e.g. the next field in a
 * struct, the next value in an array. Returns #FALSE at the end of the
 * current container.
 *
 * @param iter the iterator
 * @returns FALSE if nothing more to read at or below this level
 */
dbus_bool_t
dbus_signature_iter_next (DBusSignatureIter *iter)
{
  DBusSignatureRealIter *real_iter = (DBusSignatureRealIter *) iter;

  if (real_iter->finished)
    return FALSE;
  else
    {
      int pos;

      if (real_iter->in_array)
	{
	  real_iter->finished = TRUE;
	  return FALSE;
	}

      pos = 0;
      _dbus_type_signature_next (real_iter->pos, &pos);
      real_iter->pos += pos;

      if (*real_iter->pos == DBUS_STRUCT_END_CHAR
	  || *real_iter->pos == DBUS_DICT_ENTRY_END_CHAR)
	{
	  real_iter->finished = TRUE;
	  return FALSE;
	}

      return *real_iter->pos != DBUS_TYPE_INVALID;
    }
}

/**
 * Initialize a new iterator pointing to the first type in the current
 * container.
 * 
 * The results are undefined when calling this if the current type is
 * a non-container (i.e. if dbus_type_is_container() returns #FALSE
 * for the result of dbus_signature_iter_get_current_type()).
 *
 * @param iter the current interator
 * @param subiter an iterator to initialize pointing to the first child
 */
void
dbus_signature_iter_recurse (const DBusSignatureIter *iter,
			     DBusSignatureIter       *subiter)
{
  DBusSignatureRealIter *real_iter = (DBusSignatureRealIter *) iter;
  DBusSignatureRealIter *real_sub_iter = (DBusSignatureRealIter *) subiter;

  _dbus_return_if_fail (dbus_type_is_container (dbus_signature_iter_get_current_type (iter)));

  *real_sub_iter = *real_iter;
  real_sub_iter->in_array = FALSE;
  real_sub_iter->pos++;

  if (dbus_signature_iter_get_current_type (iter) == DBUS_TYPE_ARRAY)
    real_sub_iter->in_array = TRUE;
}

/**
 * Check a type signature for validity. Remember that #NULL can always
 * be passed instead of a DBusError*, if you don't care about having
 * an error name and message.
 *
 * @param signature a potentially invalid type signature
 * @param error error return
 * @returns #TRUE if signature is valid or #FALSE if an error is set
 */
dbus_bool_t
dbus_signature_validate (const char       *signature,
			 DBusError        *error)
			 
{
  DBusString str;
  DBusValidity reason;

  _dbus_string_init_const (&str, signature);
  reason = _dbus_validate_signature_with_reason (&str, 0, _dbus_string_get_length (&str));

  if (reason == DBUS_VALID)
    return TRUE;
  else
    {
      dbus_set_error (error, DBUS_ERROR_INVALID_SIGNATURE, "%s",
          _dbus_validity_to_error_message (reason));
      return FALSE;
    }
}

/**
 * Check that a type signature is both valid and contains exactly one
 * complete type. "One complete type" means a single basic type,
 * array, struct, or dictionary, though the struct or array may be
 * arbitrarily recursive and complex. More than one complete type
 * would mean for example "ii" or two integers in sequence.
 *
 * @param signature a potentially invalid type signature
 * @param error error return
 * @returns #TRUE if signature is valid and has exactly one complete type
 */
dbus_bool_t
dbus_signature_validate_single (const char       *signature,
				DBusError        *error)
{
  DBusSignatureIter iter;

  if (!dbus_signature_validate (signature, error))
    return FALSE;

  dbus_signature_iter_init (&iter, signature);
  if (dbus_signature_iter_get_current_type (&iter) == DBUS_TYPE_INVALID)
    goto lose;
  if (!dbus_signature_iter_next (&iter))
    return TRUE;
 lose:
  dbus_set_error (error, DBUS_ERROR_INVALID_SIGNATURE, "Exactly one complete type required in signature");
  return FALSE;
}

/**
 * A "container type" can contain basic types, or nested
 * container types. #DBUS_TYPE_INVALID is not a container type.
 *
 * It is an error to pass an invalid type-code, other than DBUS_TYPE_INVALID,
 * to this function. The valid type-codes are defined by dbus-protocol.h
 * and can be checked with dbus_type_is_valid().
 *
 * @param typecode either a valid type-code or DBUS_TYPE_INVALID
 * @returns #TRUE if type is a container
 */
dbus_bool_t
dbus_type_is_container (int typecode)
{
  /* only reasonable (non-line-noise) typecodes are allowed */
  _dbus_return_val_if_fail (dbus_type_is_valid (typecode) || typecode == DBUS_TYPE_INVALID,
			    FALSE);
  return TYPE_IS_CONTAINER (typecode);
}

/**
 * A "basic type" is a somewhat arbitrary concept, but the intent is
 * to include those types that are fully-specified by a single
 * typecode, with no additional type information or nested values. So
 * all numbers and strings are basic types and structs, arrays, and
 * variants are not basic types.  #DBUS_TYPE_INVALID is not a basic
 * type.
 *
 * It is an error to pass an invalid type-code, other than DBUS_TYPE_INVALID,
 * to this function. The valid type-codes are defined by dbus-protocol.h
 * and can be checked with dbus_type_is_valid().
 *
 * @param typecode either a valid type-code or DBUS_TYPE_INVALID
 * @returns #TRUE if type is basic
 */
dbus_bool_t
dbus_type_is_basic (int typecode)
{
  /* only reasonable (non-line-noise) typecodes are allowed */
  _dbus_return_val_if_fail (dbus_type_is_valid (typecode) || typecode == DBUS_TYPE_INVALID,
			    FALSE);

  /* everything that isn't invalid or a container */
  return !(typecode == DBUS_TYPE_INVALID || TYPE_IS_CONTAINER (typecode));
}

/**
 * Tells you whether values of this type can change length if you set
 * them to some other value. For this purpose, you assume that the
 * first byte of the old and new value would be in the same location,
 * so alignment padding is not a factor.
 *
 * This function is useful to determine whether
 * dbus_message_iter_get_fixed_array() may be used.
 *
 * Some structs are fixed-size (if they contain only fixed-size types)
 * but struct is not considered a fixed type for purposes of this
 * function.
 *
 * It is an error to pass an invalid type-code, other than DBUS_TYPE_INVALID,
 * to this function. The valid type-codes are defined by dbus-protocol.h
 * and can be checked with dbus_type_is_valid().
 *
 * @param typecode either a valid type-code or DBUS_TYPE_INVALID
 * @returns #FALSE if the type can occupy different lengths
 */
dbus_bool_t
dbus_type_is_fixed (int typecode)
{
  /* only reasonable (non-line-noise) typecodes are allowed */
  _dbus_return_val_if_fail (dbus_type_is_valid (typecode) || typecode == DBUS_TYPE_INVALID,
			    FALSE);
  
  switch (typecode)
    {
    case DBUS_TYPE_BYTE:
    case DBUS_TYPE_BOOLEAN:
    case DBUS_TYPE_INT16:
    case DBUS_TYPE_UINT16:
    case DBUS_TYPE_INT32:
    case DBUS_TYPE_UINT32:
    case DBUS_TYPE_INT64:
    case DBUS_TYPE_UINT64:
    case DBUS_TYPE_DOUBLE:
    case DBUS_TYPE_UNIX_FD:
      return TRUE;
    default:
      return FALSE;
    }
}

/**
 * Return #TRUE if the argument is a valid typecode.
 * #DBUS_TYPE_INVALID surprisingly enough is not considered valid, and
 * random unknown bytes aren't either. This function is safe with
 * untrusted data.
 *
 * @param typecode a potential type-code
 * @returns #TRUE if valid
 */
dbus_bool_t
dbus_type_is_valid (int typecode)
{
  switch (typecode)
    {
    case DBUS_TYPE_BYTE:
    case DBUS_TYPE_BOOLEAN:
    case DBUS_TYPE_INT16:
    case DBUS_TYPE_UINT16:
    case DBUS_TYPE_INT32:
    case DBUS_TYPE_UINT32:
    case DBUS_TYPE_INT64:
    case DBUS_TYPE_UINT64:
    case DBUS_TYPE_DOUBLE:
    case DBUS_TYPE_STRING:
    case DBUS_TYPE_OBJECT_PATH:
    case DBUS_TYPE_SIGNATURE:
    case DBUS_TYPE_ARRAY:
    case DBUS_TYPE_STRUCT:
    case DBUS_TYPE_DICT_ENTRY:
    case DBUS_TYPE_VARIANT:
    case DBUS_TYPE_UNIX_FD:
      return TRUE;

    default:
      return FALSE;
    }
}

/** @} */ /* end of DBusSignature group */
