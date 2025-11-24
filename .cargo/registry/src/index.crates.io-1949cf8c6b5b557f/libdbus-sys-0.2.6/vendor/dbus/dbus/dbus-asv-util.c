/* dbus-asv-util.c - utility functions for a{sv}
 *
 * Copyright © 2011-2012 Nokia Corporation
 * Copyright © 2012-2013 Collabora Ltd.
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
 * Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA
 * 02110-1301 USA
 */

#include <config.h>

#include <dbus/dbus.h>

#include "dbus/dbus-asv-util.h"

/**
 * Convenience function to create a method-call reply whose type is a{sv}
 * (map from string to variant).
 *
 * Append values with 0 or more sequences of _dbus_asv_open_entry(),
 * appending a value to var_iter, and _dbus_asv_close_entry(),
 * then close the a{sv} with _dbus_asv_close() or _dbus_asv_abandon().
 *
 * This must be paired with a call to _dbus_asv_close() or _dbus_asv_abandon().
 *
 * @param message a method call message
 * @param iter an iterator which will be initialized to append to the message
 * @param arr_iter an iterator which will be initialized to append to the array
 * @returns a new message, or #NULL if not enough memory
 */
DBusMessage *
_dbus_asv_new_method_return (DBusMessage      *message,
                             DBusMessageIter  *iter,
                             DBusMessageIter  *arr_iter)
{
  DBusMessage *reply = dbus_message_new_method_return (message);

  if (reply == NULL)
    return NULL;

  dbus_message_iter_init_append (reply, iter);

  if (!dbus_message_iter_open_container (iter, DBUS_TYPE_ARRAY, "{sv}",
                                         arr_iter))
    {
      dbus_message_unref (reply);
      return NULL;
    }

  return reply;
}

/*
 * Open a new entry in an a{sv} (map from string to variant).
 *
 * This must be paired with a call to either _dbus_asv_close_entry()
 * or _dbus_asv_abandon_entry().
 *
 * If this function fails, the a{sv} must be abandoned, for instance
 * with _dbus_asv_abandon().
 *
 * @param arr_iter the iterator which is appending to the array
 * @param entry_iter will be initialized to append to the dict-entry
 * @param key a UTF-8 key for the map
 * @param type the type of the variant value, e.g. DBUS_TYPE_STRING_AS_STRING
 * @param var_iter will be initialized to append (i.e. write) to the variant
 * @returns #TRUE on success, or #FALSE if not enough memory
 */
dbus_bool_t
_dbus_asv_open_entry (DBusMessageIter *arr_iter,
                      DBusMessageIter *entry_iter,
                      const char      *key,
                      const char      *type,
                      DBusMessageIter *var_iter)
{
  if (!dbus_message_iter_open_container (arr_iter, DBUS_TYPE_DICT_ENTRY,
                                         NULL, entry_iter))
    return FALSE;

  if (!dbus_message_iter_append_basic (entry_iter, DBUS_TYPE_STRING, &key))
    {
      dbus_message_iter_abandon_container (arr_iter, entry_iter);
      return FALSE;
    }

  if (!dbus_message_iter_open_container (entry_iter, DBUS_TYPE_VARIANT,
                                         type, var_iter))
    {
      dbus_message_iter_abandon_container (arr_iter, entry_iter);
      return FALSE;
    }

  return TRUE;
}

/*
 * Closes an a{sv} entry after successfully appending the value.
 *
 * If this function fails, the a{sv} must be abandoned, for instance
 * with _dbus_asv_abandon().
 *
 * @param arr_iter the iterator which is appending to the array
 * @param entry_iter the iterator appending to the dict-entry, will be closed
 * @param var_iter the iterator appending to the variant, will be closed
 * @returns #TRUE on success, or #FALSE if not enough memory
 */
dbus_bool_t
_dbus_asv_close_entry (DBusMessageIter *arr_iter,
                       DBusMessageIter *entry_iter,
                       DBusMessageIter *var_iter)
{
  if (!dbus_message_iter_close_container (entry_iter, var_iter))
    {
      dbus_message_iter_abandon_container (arr_iter, entry_iter);
      return FALSE;
    }

  if (!dbus_message_iter_close_container (arr_iter, entry_iter))
    return FALSE;

  return TRUE;
}

/**
 * Closes an a{sv} after successfully appending all values.
 *
 * If this function fails, you must abandon iter and whatever
 * larger data structure (message, etc.) the a{sv} was embedded in.
 *
 * @param iter the iterator which is appending to the message or other data structure containing the a{sv}
 * @param arr_iter the iterator appending to the array, will be closed
 * @returns #TRUE on success, or #FALSE if not enough memory
 */
dbus_bool_t
_dbus_asv_close (DBusMessageIter *iter,
                 DBusMessageIter *arr_iter)
{
  return dbus_message_iter_close_container (iter, arr_iter);
}

/*
 * Closes an a{sv} entry after unsuccessfully appending a value.
 * You must also abandon the a{sv} itself (for instance with
 * _dbus_asv_abandon()), and abandon whatever larger data structure
 * the a{sv} was embedded in.
 *
 * @param iter the iterator which is appending to the message or other data structure containing the a{sv}
 * @param arr_iter the iterator appending to the array, will be closed
 * @returns #TRUE on success, or #FALSE if not enough memory
 */
void
_dbus_asv_abandon_entry (DBusMessageIter *arr_iter,
                         DBusMessageIter *entry_iter,
                         DBusMessageIter *var_iter)
{
  dbus_message_iter_abandon_container (entry_iter, var_iter);
  dbus_message_iter_abandon_container (arr_iter, entry_iter);
}

/**
 * Closes an a{sv} after unsuccessfully appending a value.
 *
 * You must also abandon whatever larger data structure (message, etc.)
 * the a{sv} was embedded in.
 *
 * @param iter the iterator which is appending to the message or other data structure containing the a{sv}
 * @param arr_iter the iterator appending to the array, will be closed
 */
void
_dbus_asv_abandon (DBusMessageIter *iter,
                   DBusMessageIter *arr_iter)
{
  dbus_message_iter_abandon_container (iter, arr_iter);
}

/**
 * Create a new entry in an a{sv} (map from string to variant)
 * with a 32-bit unsigned integer value.
 *
 * If this function fails, the a{sv} must be abandoned, for instance
 * with _dbus_asv_abandon().
 *
 * @param arr_iter the iterator which is appending to the array
 * @param key a UTF-8 key for the map
 * @param value the value
 * @returns #TRUE on success, or #FALSE if not enough memory
 */
dbus_bool_t
_dbus_asv_add_uint32 (DBusMessageIter *arr_iter,
                      const char *key,
                      dbus_uint32_t value)
{
  DBusMessageIter entry_iter, var_iter;

  if (!_dbus_asv_open_entry (arr_iter, &entry_iter, key,
                             DBUS_TYPE_UINT32_AS_STRING, &var_iter))
    return FALSE;

  if (!dbus_message_iter_append_basic (&var_iter, DBUS_TYPE_UINT32,
                                       &value))
    {
      _dbus_asv_abandon_entry (arr_iter, &entry_iter, &var_iter);
      return FALSE;
    }

  if (!_dbus_asv_close_entry (arr_iter, &entry_iter, &var_iter))
    return FALSE;

  return TRUE;
}

/**
 * Create a new entry in an a{sv} (map from string to variant)
 * with a UTF-8 string value.
 *
 * If this function fails, the a{sv} must be abandoned, for instance
 * with _dbus_asv_abandon().
 *
 * @param arr_iter the iterator which is appending to the array
 * @param key a UTF-8 key for the map
 * @param value the value
 * @returns #TRUE on success, or #FALSE if not enough memory
 */
dbus_bool_t
_dbus_asv_add_string (DBusMessageIter *arr_iter,
                      const char *key,
                      const char *value)
{
  DBusMessageIter entry_iter, var_iter;

  if (!_dbus_asv_open_entry (arr_iter, &entry_iter, key,
                             DBUS_TYPE_STRING_AS_STRING, &var_iter))
    return FALSE;

  if (!dbus_message_iter_append_basic (&var_iter, DBUS_TYPE_STRING,
                                       &value))
    {
      _dbus_asv_abandon_entry (arr_iter, &entry_iter, &var_iter);
      return FALSE;
    }

  if (!_dbus_asv_close_entry (arr_iter, &entry_iter, &var_iter))
    return FALSE;

  return TRUE;
}

/**
 * Create a new entry in an a{sv} (map from string to variant)
 * with an object-path value.
 *
 * If this function fails, the a{sv} must be abandoned, for instance
 * with _dbus_asv_abandon().
 *
 * @param arr_iter the iterator which is appending to the array
 * @param key a UTF-8 key for the map
 * @param value the value
 * @returns #TRUE on success, or #FALSE if not enough memory
 */
dbus_bool_t
_dbus_asv_add_object_path (DBusMessageIter *arr_iter,
                           const char *key,
                           const char *value)
{
  DBusMessageIter entry_iter, var_iter;

  if (!_dbus_asv_open_entry (arr_iter, &entry_iter, key,
                             DBUS_TYPE_OBJECT_PATH_AS_STRING, &var_iter))
    return FALSE;

  if (!dbus_message_iter_append_basic (&var_iter, DBUS_TYPE_OBJECT_PATH,
                                       &value))
    {
      _dbus_asv_abandon_entry (arr_iter, &entry_iter, &var_iter);
      return FALSE;
    }

  if (!_dbus_asv_close_entry (arr_iter, &entry_iter, &var_iter))
    return FALSE;

  return TRUE;
}

/**
 * Create a new entry in an a{sv} (map from string to variant)
 * with an array of a fixed-length basic type (excluding unix fd).
 *
 * If this function fails, the a{sv} must be abandoned, for instance
 * with _dbus_asv_abandon().
 *
 * @param arr_iter the iterator which is appending to the array
 * @param key a UTF-8 key for the map
 * @param value the value
 * @param n_elements the number of elements to append
 * @returns #TRUE on success, or #FALSE if not enough memory
 */
dbus_bool_t
_dbus_asv_add_fixed_array (DBusMessageIter *arr_iter,
                           const char      *key,
                           char            element_type,
                           const void      *value,
                           int              n_elements)
{
  const char type[] = { DBUS_TYPE_ARRAY, element_type, 0 };
  DBusMessageIter entry_iter;
  DBusMessageIter var_iter;
  DBusMessageIter array_iter;

  _dbus_assert (dbus_type_is_fixed (element_type) && element_type != DBUS_TYPE_UNIX_FD);

  if (!_dbus_asv_open_entry (arr_iter, &entry_iter, key, type, &var_iter))
    return FALSE;

  if (!dbus_message_iter_open_container (&var_iter, DBUS_TYPE_ARRAY, type + 1,
                                         &array_iter))
    {
      _dbus_asv_abandon_entry (arr_iter, &entry_iter, &var_iter);
      return FALSE;
    }

  if (!dbus_message_iter_append_fixed_array (&array_iter, element_type,
                                             &value, n_elements))
    {
      dbus_message_iter_abandon_container (&var_iter, &array_iter);
      _dbus_asv_abandon_entry (arr_iter, &entry_iter, &var_iter);
      return FALSE;
    }

  if (!dbus_message_iter_close_container (&var_iter, &array_iter))
    {
      _dbus_asv_abandon_entry (arr_iter, &entry_iter, &var_iter);
      return FALSE;
    }

  if (!_dbus_asv_close_entry (arr_iter, &entry_iter, &var_iter))
    return FALSE;

  return TRUE;
}

/**
 * Create a new entry in an a{sv} (map from string to variant)
 * with a byte array value.
 *
 * If this function fails, the a{sv} must be abandoned, for instance
 * with _dbus_asv_abandon().
 *
 * @param arr_iter the iterator which is appending to the array
 * @param key a UTF-8 key for the map
 * @param value the value
 * @param n_elements the number of elements to append
 * @returns #TRUE on success, or #FALSE if not enough memory
 */
dbus_bool_t
_dbus_asv_add_byte_array (DBusMessageIter *arr_iter,
                          const char      *key,
                          const void      *value,
                          int              n_elements)
{
  return _dbus_asv_add_fixed_array (arr_iter, key, DBUS_TYPE_BYTE, value,
                                    n_elements);
}
