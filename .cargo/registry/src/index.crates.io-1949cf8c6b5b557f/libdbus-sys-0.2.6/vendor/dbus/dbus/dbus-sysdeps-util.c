/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-sysdeps-util.c Tests for dbus-sysdeps.h API
 *
 * Copyright 2002-2008 Red Hat, Inc.
 * Copyright 2003 CodeFactory AB
 * Copyright 2006 Ralf Habacker
 * Copyright 2006 Sjoerd Simons
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
#include "dbus-sysdeps.h"
#include "dbus-internals.h"
#include "dbus-string.h"

#include <stdlib.h>

#ifdef DBUS_WIN
  /* do nothing, it's in stdlib.h */
#elif (defined __APPLE__)
# include <crt_externs.h>
# define environ (*_NSGetEnviron())
#elif HAVE_DECL_ENVIRON && defined(HAVE_UNISTD_H)
# include <unistd.h>
#else
extern char **environ;
#endif

/**
 * Gets a #NULL-terminated list of key=value pairs from the
 * environment. Use dbus_free_string_array to free it.
 *
 * @returns the environment or #NULL on OOM
 */
char **
_dbus_get_environment (void)
{
  int i, length;
  char **environment;

  _dbus_assert (environ != NULL);

  for (length = 0; environ[length] != NULL; length++);

  /* Add one for NULL */
  length++;

  environment = dbus_new0 (char *, length);

  if (environment == NULL)
    return NULL;

  for (i = 0; environ[i] != NULL; i++)
    {
      environment[i] = _dbus_strdup (environ[i]);

      if (environment[i] == NULL)
        break;
    }

  if (environ[i] != NULL)
    {
      dbus_free_string_array (environment);
      environment = NULL;
    }

  return environment;
}
