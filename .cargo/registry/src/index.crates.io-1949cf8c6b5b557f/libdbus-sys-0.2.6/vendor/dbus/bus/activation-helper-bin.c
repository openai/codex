/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* activation-helper-bin.c  Setuid helper for launching programs as a custom
 *                          user. This file is security sensitive.
 *
 * Copyright (C) 2007 Red Hat, Inc.
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

#include "utils.h"
#include "activation-helper.h"
#include "activation-exit-codes.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int
convert_error_to_exit_code (DBusError *error)
{
  if (dbus_error_has_name (error, DBUS_ERROR_NO_MEMORY))
    return BUS_SPAWN_EXIT_CODE_NO_MEMORY;

  if (dbus_error_has_name (error, DBUS_ERROR_SPAWN_CONFIG_INVALID))
    return BUS_SPAWN_EXIT_CODE_CONFIG_INVALID;

  if (dbus_error_has_name (error, DBUS_ERROR_SPAWN_SETUP_FAILED))
    return BUS_SPAWN_EXIT_CODE_SETUP_FAILED;

  if (dbus_error_has_name (error, DBUS_ERROR_SPAWN_SERVICE_INVALID))
    return BUS_SPAWN_EXIT_CODE_NAME_INVALID;

  if (dbus_error_has_name (error, DBUS_ERROR_SPAWN_SERVICE_NOT_FOUND))
    return BUS_SPAWN_EXIT_CODE_SERVICE_NOT_FOUND;

  if (dbus_error_has_name (error, DBUS_ERROR_SPAWN_PERMISSIONS_INVALID))
    return BUS_SPAWN_EXIT_CODE_PERMISSIONS_INVALID;

  if (dbus_error_has_name (error, DBUS_ERROR_SPAWN_FILE_INVALID))
    return BUS_SPAWN_EXIT_CODE_FILE_INVALID;

  if (dbus_error_has_name (error, DBUS_ERROR_SPAWN_EXEC_FAILED))
    return BUS_SPAWN_EXIT_CODE_EXEC_FAILED;

  if (dbus_error_has_name (error, DBUS_ERROR_INVALID_ARGS))
    return BUS_SPAWN_EXIT_CODE_INVALID_ARGS;

  if (dbus_error_has_name (error, DBUS_ERROR_SPAWN_CHILD_SIGNALED))
    return BUS_SPAWN_EXIT_CODE_CHILD_SIGNALED;
  
  /* should we assert? */
  fprintf(stderr, "%s: %s\n", error->name, error->message);
  
  return BUS_SPAWN_EXIT_CODE_GENERIC_FAILURE;
}

int
main (int argc, char **argv)
{
  DBusError error;
  int retval;

  /* default is all okay */
  retval = 0;

  /* have we used a help option or not specified the correct arguments? */
  if (argc != 2 ||
      strcmp (argv[1], "--help") == 0 ||
      strcmp (argv[1], "-h") == 0 ||
      strcmp (argv[1], "-?") == 0)
    {
        fprintf (stderr, "dbus-daemon-activation-helper service.to.activate\n");
        exit (0);
    }

  dbus_error_init (&error);
  if (!run_launch_helper (argv[1], &error))
    {
      /* convert error to an exit code */
      retval = convert_error_to_exit_code (&error);
      dbus_error_free (&error);
    }

  return retval;
}
