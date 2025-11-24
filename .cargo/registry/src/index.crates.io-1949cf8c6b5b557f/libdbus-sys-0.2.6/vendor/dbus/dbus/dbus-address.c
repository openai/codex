/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-address.c  Server address parser.
 *
 * Copyright (C) 2003  CodeFactory AB
 * Copyright (C) 2004-2007  Red Hat, Inc.
 * Copyright (C) 2007  Ralf Habacker
 * Copyright (C) 2013  Chengwei Yang / Intel
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
#include "dbus-address.h"
#include "dbus-internals.h"
#include "dbus-list.h"
#include "dbus-string.h"
#include "dbus-protocol.h"
#include <dbus/dbus-test-tap.h>

/**
 * @defgroup DBusAddressInternals Address parsing
 * @ingroup  DBusInternals
 * @brief Implementation of parsing addresses of D-Bus servers.
 *
 * @{
 */

/**
 * Internals of DBusAddressEntry 
 */
struct DBusAddressEntry
{
  DBusString method; /**< The address type (unix, tcp, etc.) */

  DBusList *keys;    /**< List of keys */
  DBusList *values;  /**< List of values */
};


/**
 *
 * Sets #DBUS_ERROR_BAD_ADDRESS.
 * If address_problem_type and address_problem_field are not #NULL,
 * sets an error message about how the field is no good. Otherwise, sets
 * address_problem_other as the error message.
 * 
 * @param error the error to set
 * @param address_problem_type the address type of the bad address or #NULL
 * @param address_problem_field the missing field of the bad address or #NULL
 * @param address_problem_other any other error message or #NULL
 */
void
_dbus_set_bad_address (DBusError  *error,
                       const char *address_problem_type,
                       const char *address_problem_field,
                       const char *address_problem_other)
{
  if (address_problem_type != NULL)
    dbus_set_error (error, DBUS_ERROR_BAD_ADDRESS,
                    "Server address of type %s was missing argument %s",
                    address_problem_type, address_problem_field);
  else
    dbus_set_error (error, DBUS_ERROR_BAD_ADDRESS,
                    "Could not parse server address: %s",
                    address_problem_other);
}

/**
 * #TRUE if the byte need not be escaped when found in a dbus address.
 * All other bytes are required to be escaped in a valid address.
 */
#define _DBUS_ADDRESS_OPTIONALLY_ESCAPED_BYTE(b)        \
         (((b) >= 'a' && (b) <= 'z') ||                 \
          ((b) >= 'A' && (b) <= 'Z') ||                 \
          ((b) >= '0' && (b) <= '9') ||                 \
          (b) == '-' ||                                 \
          (b) == '_' ||                                 \
          (b) == '/' ||                                 \
          (b) == '\\' ||                                \
          (b) == '*' ||                                \
          (b) == '.')

/**
 * Appends an escaped version of one string to another string,
 * using the D-Bus address escaping mechanism
 *
 * @param escaped the string to append to
 * @param unescaped the string to escape
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_address_append_escaped (DBusString       *escaped,
                              const DBusString *unescaped)
{
  const unsigned char *p;
  const unsigned char *end;
  dbus_bool_t ret;
  int orig_len;

  ret = FALSE;

  orig_len = _dbus_string_get_length (escaped);
  p = _dbus_string_get_const_udata (unescaped);
  end = p + _dbus_string_get_length (unescaped);
  while (p != end)
    {
      if (_DBUS_ADDRESS_OPTIONALLY_ESCAPED_BYTE (*p))
        {
          if (!_dbus_string_append_byte (escaped, *p))
            goto out;
        }
      else
        {
          if (!_dbus_string_append_byte (escaped, '%'))
            goto out;
          if (!_dbus_string_append_byte_as_hex (escaped, *p))
            goto out;
        }
      
      ++p;
    }

  ret = TRUE;
  
 out:
  if (!ret)
    _dbus_string_set_length (escaped, orig_len);
  return ret;
}

/** @} */ /* End of internals */

static void
dbus_address_entry_free (DBusAddressEntry *entry)
{
  DBusList *link;
  
  _dbus_string_free (&entry->method);

  link = _dbus_list_get_first_link (&entry->keys);
  while (link != NULL)
    {
      _dbus_string_free (link->data);
      dbus_free (link->data);
      
      link = _dbus_list_get_next_link (&entry->keys, link);
    }
  _dbus_list_clear (&entry->keys);
  
  link = _dbus_list_get_first_link (&entry->values);
  while (link != NULL)
    {
      _dbus_string_free (link->data);
      dbus_free (link->data);
      
      link = _dbus_list_get_next_link (&entry->values, link);
    }
  _dbus_list_clear (&entry->values);
  
  dbus_free (entry);
}

/**
 * @defgroup DBusAddress Address parsing
 * @ingroup  DBus
 * @brief Parsing addresses of D-Bus servers.
 *
 * @{
 */

/**
 * Frees a #NULL-terminated array of address entries.
 *
 * @param entries the array.
 */
void
dbus_address_entries_free (DBusAddressEntry **entries)
{
  int i;
  
  for (i = 0; entries[i] != NULL; i++)
    dbus_address_entry_free (entries[i]);
  dbus_free (entries);
}

static DBusAddressEntry *
create_entry (void)
{
  DBusAddressEntry *entry;

  entry = dbus_new0 (DBusAddressEntry, 1);

  if (entry == NULL)
    return NULL;

  if (!_dbus_string_init (&entry->method))
    {
      dbus_free (entry);
      return NULL;
    }

  return entry;
}

/**
 * Returns the method string of an address entry.  For example, given
 * the address entry "tcp:host=example.com" it would return the string
 * "tcp"
 *
 * @param entry the entry.
 * @returns a string describing the method. This string
 * must not be freed.
 */
const char *
dbus_address_entry_get_method (DBusAddressEntry *entry)
{
  return _dbus_string_get_const_data (&entry->method);
}

/**
 * Returns a value from a key of an entry. For example,
 * given the address "tcp:host=example.com,port=8073" if you asked
 * for the key "host" you would get the value "example.com"
 *
 * The returned value is already unescaped.
 * 
 * @param entry the entry.
 * @param key the key.
 * @returns the key value. This string must not be freed.
 */
const char *
dbus_address_entry_get_value (DBusAddressEntry *entry,
			      const char       *key)
{
  DBusList *values, *keys;

  keys = _dbus_list_get_first_link (&entry->keys);
  values = _dbus_list_get_first_link (&entry->values);

  while (keys != NULL)
    {
      _dbus_assert (values != NULL);

      if (_dbus_string_equal_c_str (keys->data, key))
        return _dbus_string_get_const_data (values->data);

      keys = _dbus_list_get_next_link (&entry->keys, keys);
      values = _dbus_list_get_next_link (&entry->values, values);
    }
  
  return NULL;
}

static dbus_bool_t
append_unescaped_value (DBusString       *unescaped,
                        const DBusString *escaped,
                        int               escaped_start,
                        int               escaped_len,
                        DBusError        *error)
{
  const char *p;
  const char *end;
  dbus_bool_t ret;
  
  ret = FALSE;

  p = _dbus_string_get_const_data (escaped) + escaped_start;
  end = p + escaped_len;
  while (p != end)
    {
      if (_DBUS_ADDRESS_OPTIONALLY_ESCAPED_BYTE (*p))
        {
          if (!_dbus_string_append_byte (unescaped, *p))
            goto out;
        }
      else if (*p == '%')
        {
          /* Efficiency is king */
          char buf[3];
          DBusString hex;
          int hex_end;
          
          ++p;

          if ((p + 2) > end)
            {
              dbus_set_error (error, DBUS_ERROR_BAD_ADDRESS,
                              "In D-Bus address, percent character was not followed by two hex digits");
              goto out;
            }
            
          buf[0] = *p;
          ++p;
          buf[1] = *p;
          buf[2] = '\0';

          _dbus_string_init_const (&hex, buf);

          if (!_dbus_string_hex_decode (&hex, 0, &hex_end,
                                        unescaped,
                                        _dbus_string_get_length (unescaped)))
            goto out;

          if (hex_end != 2)
            {
              dbus_set_error (error, DBUS_ERROR_BAD_ADDRESS,
                              "In D-Bus address, percent character was followed by characters other than hex digits");
              goto out;
            }
        }
      else
        {
          /* Error, should have been escaped */
          dbus_set_error (error, DBUS_ERROR_BAD_ADDRESS,
                          "In D-Bus address, character '%c' should have been escaped\n",
                          *p);
          goto out;
        }
      
      ++p;
    }

  ret = TRUE;
  
 out:
  if (!ret && error && !dbus_error_is_set (error))
    _DBUS_SET_OOM (error);

  _dbus_assert (ret || error == NULL || dbus_error_is_set (error));
  
  return ret;
}

/**
 * Parses an address string of the form:
 *
 * method:key=value,key=value;method:key=value
 *
 * See the D-Bus specification for complete docs on the format.
 *
 * When connecting to an address, the first address entries
 * in the semicolon-separated list should be tried first.
 * 
 * @param address the address.
 * @param entry_result return location to an array of entries.
 * @param array_len return location for array length.
 * @param error address where an error can be returned.
 * @returns #TRUE on success, #FALSE otherwise.
 */
dbus_bool_t
dbus_parse_address (const char         *address,
		    DBusAddressEntry ***entry_result,
		    int                *array_len,
                    DBusError          *error)
{
  DBusString str;
  int pos, end_pos, len, i;
  DBusList *entries, *link;
  DBusAddressEntry **entry_array;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  
  _dbus_string_init_const (&str, address);

  entries = NULL;
  pos = 0;
  len = _dbus_string_get_length (&str);

  if (len == 0)
  {
    dbus_set_error (error, DBUS_ERROR_BAD_ADDRESS,
                    "Empty address '%s'", address);
    goto error;
  }
  
  while (pos < len)
    {
      DBusAddressEntry *entry;

      int found_pos;

      entry = create_entry ();
      if (!entry)
	{
	  dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);

	  goto error;
	}
      
      /* Append the entry */
      if (!_dbus_list_append (&entries, entry))
	{
	  dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
	  dbus_address_entry_free (entry);
	  goto error;
	}
      
      /* Look for a semi-colon */
      if (!_dbus_string_find (&str, pos, ";", &end_pos))
	end_pos = len;
      
      /* Look for the colon : */
      if (!_dbus_string_find_to (&str, pos, end_pos, ":", &found_pos))
	{
	  dbus_set_error (error, DBUS_ERROR_BAD_ADDRESS, "Address does not contain a colon");
	  goto error;
	}

      if (!_dbus_string_copy_len (&str, pos, found_pos - pos, &entry->method, 0))
	{
	  dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
	  goto error;
	}
	  
      pos = found_pos + 1;

      while (pos < end_pos)
	{
	  int comma_pos, equals_pos;

	  if (!_dbus_string_find_to (&str, pos, end_pos, ",", &comma_pos))
	    comma_pos = end_pos;
	  
	  if (!_dbus_string_find_to (&str, pos, comma_pos, "=", &equals_pos) ||
	      equals_pos == pos || equals_pos + 1 == comma_pos)
	    {
	      dbus_set_error (error, DBUS_ERROR_BAD_ADDRESS,
                              "'=' character not found or has no value following it");
              goto error;
	    }
	  else
	    {
	      DBusString *key;
	      DBusString *value;

	      key = dbus_new0 (DBusString, 1);

	      if (!key)
		{
		  dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);		  
		  goto error;
		}

	      value = dbus_new0 (DBusString, 1);
	      if (!value)
		{
		  dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
		  dbus_free (key);
		  goto error;
		}
	      
	      if (!_dbus_string_init (key))
		{
		  dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
		  dbus_free (key);
		  dbus_free (value);
		  
		  goto error;
		}
	      
	      if (!_dbus_string_init (value))
		{
		  dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
		  _dbus_string_free (key);

		  dbus_free (key);
		  dbus_free (value);		  
		  goto error;
		}

	      if (!_dbus_string_copy_len (&str, pos, equals_pos - pos, key, 0))
		{
		  dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
		  _dbus_string_free (key);
		  _dbus_string_free (value);

		  dbus_free (key);
		  dbus_free (value);		  
		  goto error;
		}

	      if (!append_unescaped_value (value, &str, equals_pos + 1,
                                           comma_pos - equals_pos - 1, error))
		{
                  _dbus_assert (error == NULL || dbus_error_is_set (error));
		  _dbus_string_free (key);
		  _dbus_string_free (value);

		  dbus_free (key);
		  dbus_free (value);		  
		  goto error;
		}

	      if (!_dbus_list_append (&entry->keys, key))
		{
		  dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);		  
		  _dbus_string_free (key);
		  _dbus_string_free (value);

		  dbus_free (key);
		  dbus_free (value);		  
		  goto error;
		}

	      if (!_dbus_list_append (&entry->values, value))
		{
		  dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);		  
		  _dbus_string_free (value);

		  dbus_free (value);
		  goto error;		  
		}
	    }

	  pos = comma_pos + 1;
	}

      pos = end_pos + 1;
    }

  *array_len = _dbus_list_get_length (&entries);
  
  entry_array = dbus_new (DBusAddressEntry *, *array_len + 1);

  if (!entry_array)
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      
      goto error;
    }
  
  entry_array [*array_len] = NULL;

  link = _dbus_list_get_first_link (&entries);
  i = 0;
  while (link != NULL)
    {
      entry_array[i] = link->data;
      i++;
      link = _dbus_list_get_next_link (&entries, link);
    }

  _dbus_list_clear (&entries);
  *entry_result = entry_array;

  return TRUE;

 error:
  
  link = _dbus_list_get_first_link (&entries);
  while (link != NULL)
    {
      dbus_address_entry_free (link->data);
      link = _dbus_list_get_next_link (&entries, link);
    }

  _dbus_list_clear (&entries);
  
  return FALSE;
  
}

/**
 * Escapes the given string as a value in a key=value pair
 * for a D-Bus address.
 *
 * @param value the unescaped value
 * @returns newly-allocated escaped value or #NULL if no memory
 */
char*
dbus_address_escape_value (const char *value)
{
  DBusString escaped;
  DBusString unescaped;
  char *ret;

  ret = NULL;

  _dbus_string_init_const (&unescaped, value);
  
  if (!_dbus_string_init (&escaped))
    return NULL;

  if (!_dbus_address_append_escaped (&escaped, &unescaped))
    goto out;
  
  if (!_dbus_string_steal_data (&escaped, &ret))
    goto out;

 out:
  _dbus_string_free (&escaped);
  return ret;
}

/**
 * Unescapes the given string as a value in a key=value pair
 * for a D-Bus address. Note that dbus_address_entry_get_value()
 * returns an already-unescaped value.
 *
 * @param value the escaped value
 * @param error error to set if the unescaping fails
 * @returns newly-allocated unescaped value or #NULL if no memory
 */
char*
dbus_address_unescape_value (const char *value,
                             DBusError  *error)
{
  DBusString unescaped;
  DBusString escaped;
  char *ret;
  
  ret = NULL;

  _dbus_string_init_const (&escaped, value);
  
  if (!_dbus_string_init (&unescaped))
    return NULL;

  if (!append_unescaped_value (&unescaped, &escaped,
                               0, _dbus_string_get_length (&escaped),
                               error))
    goto out;
  
  if (!_dbus_string_steal_data (&unescaped, &ret))
    goto out;

 out:
  if (ret == NULL && error && !dbus_error_is_set (error))
    _DBUS_SET_OOM (error);

  _dbus_assert (ret != NULL || error == NULL || dbus_error_is_set (error));
  
  _dbus_string_free (&unescaped);
  return ret;
}

/** @} */ /* End of public API */
