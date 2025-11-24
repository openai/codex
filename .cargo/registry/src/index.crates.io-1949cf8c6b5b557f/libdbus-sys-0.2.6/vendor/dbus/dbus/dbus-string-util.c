/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-string-util.c Would be in dbus-string.c, but not used in libdbus
 *
 * Copyright 2002-2008 Red Hat, Inc.
 * Copyright 2003 CodeFactory AB
 * Copyright 2003 Mark McLoughlin
 * Copyright 2004 Michael Meeks
 * Copyright 2006-2015 Ralf Habacker <ralf.habacker@freenet.de>
 * Copyright 2007 Allison Lortie
 * Copyright 2011 Roberto Guido
 * Copyright 2016-2018 Collabora Ltd.
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
#include "dbus-string.h"
#define DBUS_CAN_USE_DBUS_STRING_PRIVATE 1
#include "dbus-string-private.h"

/**
 * @addtogroup DBusString
 * @{
 */

/**
 * Returns whether a string ends with the given suffix
 *
 * @todo memcmp might make this faster.
 * 
 * @param a the string
 * @param c_str the C-style string
 * @returns #TRUE if the string ends with the suffix
 */
dbus_bool_t
_dbus_string_ends_with_c_str (const DBusString *a,
                              const char       *c_str)
{
  const unsigned char *ap;
  const unsigned char *bp;
  const unsigned char *a_end;
  unsigned long c_str_len;
  const DBusRealString *real_a = (const DBusRealString*) a;
  DBUS_GENERIC_STRING_PREAMBLE (real_a);
  _dbus_assert (c_str != NULL);
  
  c_str_len = strlen (c_str);
  if (((unsigned long)real_a->len) < c_str_len)
    return FALSE;
  
  ap = real_a->str + (real_a->len - c_str_len);
  bp = (const unsigned char*) c_str;
  a_end = real_a->str + real_a->len;
  while (ap != a_end)
    {
      if (*ap != *bp)
        return FALSE;
      
      ++ap;
      ++bp;
    }

  _dbus_assert (*ap == '\0');
  _dbus_assert (*bp == '\0');
  
  return TRUE;
}

/**
 * Find the given byte scanning backward from the given start.
 * Sets *found to -1 if the byte is not found.
 *
 * @param str the string
 * @param start the place to start scanning (will not find the byte at this point)
 * @param byte the byte to find
 * @param found return location for where it was found
 * @returns #TRUE if found
 */
dbus_bool_t
_dbus_string_find_byte_backward (const DBusString  *str,
                                 int                start,
                                 unsigned char      byte,
                                 int               *found)
{
  int i;
  DBUS_CONST_STRING_PREAMBLE (str);
  _dbus_assert (start <= real->len);
  _dbus_assert (start >= 0);
  _dbus_assert (found != NULL);

  i = start - 1;
  while (i >= 0)
    {
      if (real->str[i] == byte)
        break;
      
      --i;
    }

  if (found)
    *found = i;

  return i >= 0;
}

/** @} */
