/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* activation-helper.c  Setuid helper for launching programs as a custom
 *                      user. This file is security sensitive.
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

#include "bus.h"
#include "driver.h"
#include "utils.h"
#include "desktop-file.h"
#include "config-parser-trivial.h"
#include "activation-helper.h"
#include "activation-exit-codes.h"

#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/types.h>
#include <pwd.h>
#include <grp.h>

#include <dbus/dbus-misc.h>
#include <dbus/dbus-shell.h>
#include <dbus/dbus-marshal-validate.h>
#include <dbus/dbus-sysdeps-unix.h>

static BusDesktopFile *
desktop_file_for_name (BusConfigParser *parser,
                       const char *name,
                       DBusError  *error)
{
  BusDesktopFile *desktop_file;
  DBusList **service_dirs;
  DBusList *link;
  DBusError tmp_error;
  DBusString full_path;
  DBusString filename;
  const char *dir;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  desktop_file = NULL;

  if (!_dbus_string_init (&filename))
    {
      BUS_SET_OOM (error);
      goto out_all;
    }

  if (!_dbus_string_init (&full_path))
    {
      BUS_SET_OOM (error);
      goto out_filename;
    }

  if (!_dbus_string_append (&filename, name) ||
      !_dbus_string_append (&filename, ".service"))
    {
      BUS_SET_OOM (error);
      goto out;
    }

  service_dirs = bus_config_parser_get_service_paths (parser);
  for (link = _dbus_list_get_first_link (service_dirs);
       link != NULL;
       link = _dbus_list_get_next_link (service_dirs, link))
    {
      dir = link->data;
      _dbus_verbose ("Looking at '%s'\n", dir);

      dbus_error_init (&tmp_error);

      /* clear the path from last time */
      _dbus_string_set_length (&full_path, 0);

      /* build the full path */
      if (!_dbus_string_append (&full_path, dir) ||
          !_dbus_concat_dir_and_file (&full_path, &filename))
        {
          BUS_SET_OOM (error);
          goto out;
        }

      _dbus_verbose ("Trying to load file '%s'\n", _dbus_string_get_data (&full_path));
      desktop_file = bus_desktop_file_load (&full_path, &tmp_error);
      if (desktop_file == NULL)
        {
          _DBUS_ASSERT_ERROR_IS_SET (&tmp_error);
          _dbus_verbose ("Could not load %s: %s: %s\n",
                         _dbus_string_get_const_data (&full_path),
                         tmp_error.name, tmp_error.message);

          /* we may have failed if the file is not found; this is not fatal */
          if (dbus_error_has_name (&tmp_error, DBUS_ERROR_NO_MEMORY))
            {
              dbus_move_error (&tmp_error, error);
              /* we only bail out on OOM */
              goto out;
            }
          dbus_error_free (&tmp_error);
        }

      /* did we find the desktop file we want? */
      if (desktop_file != NULL)
        break;
    }

  /* Didn't find desktop file; set error */
  if (desktop_file == NULL)
    {
      dbus_set_error (error, DBUS_ERROR_SPAWN_SERVICE_NOT_FOUND,
                      "The name %s was not provided by any .service files",
                      name);
    }

out:
  _dbus_string_free (&full_path);
out_filename:
  _dbus_string_free (&filename);
out_all:
  return desktop_file;
}

/* Clears the environment, except for DBUS_STARTER_x,
 * which we hardcode to the system bus.
 */
static dbus_bool_t
clear_environment (DBusError *error)
{
#ifndef ACTIVATION_LAUNCHER_TEST
  /* totally clear the environment */
  if (!_dbus_clearenv ())
    {
      dbus_set_error (error, DBUS_ERROR_SPAWN_SETUP_FAILED,
                      "could not clear environment\n");
      return FALSE;
    }

  /* Ensure the bus is set to system */
  dbus_setenv ("DBUS_STARTER_ADDRESS", DBUS_SYSTEM_BUS_DEFAULT_ADDRESS);
  dbus_setenv ("DBUS_STARTER_BUS_TYPE", "system");
#endif

  return TRUE;
}

static dbus_bool_t
check_permissions (const char *dbus_user, DBusError *error)
{
#ifndef ACTIVATION_LAUNCHER_TEST
  uid_t uid, euid;
  struct passwd *pw;

  pw = NULL;
  uid = 0;
  euid = 0;

  /* bail out unless the dbus user is invoking the helper */
  pw = getpwnam(dbus_user);
  if (!pw)
    {
      dbus_set_error (error, DBUS_ERROR_SPAWN_PERMISSIONS_INVALID,
                      "cannot find user '%s'", dbus_user);
      return FALSE;
    }
  uid = getuid();
  if (pw->pw_uid != uid)
    {
      dbus_set_error (error, DBUS_ERROR_SPAWN_PERMISSIONS_INVALID,
                      "not invoked from user '%s'", dbus_user);
      return FALSE;
    }

  /* bail out unless we are setuid to user root */
  euid = geteuid();
  if (euid != 0)
    {
      dbus_set_error (error, DBUS_ERROR_SPAWN_PERMISSIONS_INVALID,
                      "not setuid root");
      return FALSE;
    }
#endif

  return TRUE;
}

static dbus_bool_t
check_service_name (BusDesktopFile *desktop_file,
                    const char     *service_name,
                    DBusError      *error)
{
  char *name_tmp;
  dbus_bool_t retval;

  retval = FALSE;

  /* try to get Name */
  if (!bus_desktop_file_get_string (desktop_file,
                                    DBUS_SERVICE_SECTION,
                                    DBUS_SERVICE_NAME,
                                    &name_tmp,
                                    error))
    goto failed;

  /* verify that the name is the same as the file service name */
  if (strcmp (service_name, name_tmp) != 0)
    {
      dbus_set_error (error, DBUS_ERROR_SPAWN_FILE_INVALID,
                      "Service '%s' does not match expected value", name_tmp);
      goto failed_free;
    }

  retval = TRUE;

failed_free:
  /* we don't return the name, so free it here */
  dbus_free (name_tmp);
failed:
  return retval;
}

static dbus_bool_t
get_parameters_for_service (BusDesktopFile *desktop_file,
                            const char     *service_name,
                            char          **exec,
                            char          **user,
                            DBusError      *error)
{
  char *exec_tmp;
  char *user_tmp;

  exec_tmp = NULL;
  user_tmp = NULL;

  /* check the name of the service */
  if (!check_service_name (desktop_file, service_name, error))
    goto failed;

  /* get the complete path of the executable */
  if (!bus_desktop_file_get_string (desktop_file,
                                    DBUS_SERVICE_SECTION,
                                    DBUS_SERVICE_EXEC,
                                    &exec_tmp,
                                    error))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      goto failed;
    }

  /* get the user that should run this service - user is compulsary for system activation */
  if (!bus_desktop_file_get_string (desktop_file,
                                    DBUS_SERVICE_SECTION,
                                    DBUS_SERVICE_USER,
                                    &user_tmp,
                                    error))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      goto failed;
    }

  /* only assign if all the checks passed */
  *exec = exec_tmp;
  *user = user_tmp;
  return TRUE;

failed:
  dbus_free (exec_tmp);
  dbus_free (user_tmp);
  return FALSE;
}

static dbus_bool_t
switch_user (char *user, DBusError *error)
{
#ifndef ACTIVATION_LAUNCHER_TEST
  struct passwd *pw;

  /* find user */
  pw = getpwnam (user);
  if (!pw)
    {
      dbus_set_error (error, DBUS_ERROR_SPAWN_SETUP_FAILED,
                      "cannot find user '%s'\n", user);
      return FALSE;
    }

  /* initialize the group access list */
  if (initgroups (user, pw->pw_gid))
    {
      dbus_set_error (error, DBUS_ERROR_SPAWN_SETUP_FAILED,
                      "could not initialize groups");
      return FALSE;
    }

  /* change to the primary group for the user */
  if (setgid (pw->pw_gid))
    {
      dbus_set_error (error, DBUS_ERROR_SPAWN_SETUP_FAILED,
                      "cannot setgid group %i", pw->pw_gid);
      return FALSE;
    }

  /* change to the user specified */
  if (setuid (pw->pw_uid) < 0)
    {
      dbus_set_error (error, DBUS_ERROR_SPAWN_SETUP_FAILED,
                      "cannot setuid user %i", pw->pw_uid);
      return FALSE;
    }
#endif
  return TRUE;
}

static dbus_bool_t
exec_for_correct_user (char *exec, char *user, DBusError *error)
{
  char **argv;
  int argc;
  dbus_bool_t retval;
  const char *error_str = NULL;

  argc = 0;
  retval = TRUE;
  argv = NULL;

  /* Resetting the OOM score adjustment is best-effort, so we don't
   * treat a failure to do so as fatal. */
  if (!_dbus_reset_oom_score_adj (&error_str))
    _dbus_log (DBUS_SYSTEM_LOG_WARNING, "%s: %s", error_str, strerror (errno));

  if (!switch_user (user, error))
    return FALSE;

  /* convert command into arguments */
  if (!_dbus_shell_parse_argv (exec, &argc, &argv, error))
    return FALSE;

#ifndef ACTIVATION_LAUNCHER_DO_OOM
  /* replace with new binary, with no environment */
  if (execv (argv[0], argv) < 0)
    {
      dbus_set_error (error, DBUS_ERROR_SPAWN_EXEC_FAILED,
                      "Failed to exec: %s", argv[0]);
      retval = FALSE;
    }
#endif

  dbus_free_string_array (argv);
  return retval;
}

static dbus_bool_t
check_bus_name (const char *bus_name,
                DBusError  *error)
{
  DBusString str;

  _dbus_string_init_const (&str, bus_name);
  if (!_dbus_validate_bus_name (&str, 0, _dbus_string_get_length (&str)))
    {
      dbus_set_error (error, DBUS_ERROR_SPAWN_SERVICE_INVALID,
                      "bus name '%s' is not a valid bus name\n",
                      bus_name);
      return FALSE;
    }
  
  return TRUE;
}

static dbus_bool_t
get_correct_parser (BusConfigParser **parser, DBusError *error)
{
  DBusString config_file;
  dbus_bool_t retval;
#ifdef ACTIVATION_LAUNCHER_TEST
  const char *test_config_file;
#endif

  retval = FALSE;

#ifdef ACTIVATION_LAUNCHER_TEST
  test_config_file = NULL;

  /* there is no _way_ we should be setuid if this define is set.
   * but we should be doubly paranoid and check... */
  if (getuid() != geteuid())
    _dbus_assert_not_reached ("dbus-daemon-launch-helper-test binary is setuid!");

  /* this is not a security hole. The environment variable is only passed in the
   * dbus-daemon-lauch-helper-test NON-SETUID launcher */
  test_config_file = _dbus_getenv ("TEST_LAUNCH_HELPER_CONFIG");
  if (test_config_file == NULL)
    {
      dbus_set_error (error, DBUS_ERROR_SPAWN_SETUP_FAILED,
                      "the TEST_LAUNCH_HELPER_CONFIG env variable is not set");
      goto out;
    }
#endif

  /* we _only_ use the predefined system config file */
  if (!_dbus_string_init (&config_file))
    {
      BUS_SET_OOM (error);
      goto out;
    }
#ifndef ACTIVATION_LAUNCHER_TEST
  if (!_dbus_string_append (&config_file, DBUS_SYSTEM_CONFIG_FILE))
    {
      BUS_SET_OOM (error);
      goto out_free_config;
    }
#else
  if (!_dbus_string_append (&config_file, test_config_file))
    {
      BUS_SET_OOM (error);
      goto out_free_config;
    }
#endif

  /* where are we pointing.... */
  _dbus_verbose ("dbus-daemon-activation-helper: using config file: %s\n",
                 _dbus_string_get_const_data (&config_file));

  /* get the dbus user */
  *parser = bus_config_load (&config_file, TRUE, NULL, error);
  if (*parser == NULL)
    {
      goto out_free_config;
    }

  /* woot */
  retval = TRUE;

out_free_config:
  _dbus_string_free (&config_file);
out:
  return retval;
}

static dbus_bool_t
launch_bus_name (const char *bus_name, BusConfigParser *parser, DBusError *error)
{
  BusDesktopFile *desktop_file;
  char *exec, *user;
  dbus_bool_t retval;

  exec = NULL;
  user = NULL;
  retval = FALSE;

  /* get the correct service file for the name we are trying to activate */
  desktop_file = desktop_file_for_name (parser, bus_name, error);
  if (desktop_file == NULL)
    return FALSE;

  /* get exec and user for service name */
  if (!get_parameters_for_service (desktop_file, bus_name, &exec, &user, error))
    goto finish;

  _dbus_verbose ("dbus-daemon-activation-helper: Name='%s'\n", bus_name);
  _dbus_verbose ("dbus-daemon-activation-helper: Exec='%s'\n", exec);
  _dbus_verbose ("dbus-daemon-activation-helper: User='%s'\n", user);

  /* actually execute */
  if (!exec_for_correct_user (exec, user, error))
    goto finish;

  retval = TRUE;

finish:
  dbus_free (exec);
  dbus_free (user);
  bus_desktop_file_free (desktop_file);
  return retval;
}

static dbus_bool_t
check_dbus_user (BusConfigParser *parser, DBusError *error)
{
  const char *dbus_user;

  dbus_user = bus_config_parser_get_user (parser);
  if (dbus_user == NULL)
    {
      dbus_set_error (error, DBUS_ERROR_SPAWN_CONFIG_INVALID,
                      "could not get user from config file\n");
      return FALSE;
    }

  /* check to see if permissions are correct */
  if (!check_permissions (dbus_user, error))
    return FALSE;

  return TRUE;
}

dbus_bool_t
run_launch_helper (const char *bus_name,
                   DBusError  *error)
{
  BusConfigParser *parser;
  dbus_bool_t retval;

  parser = NULL;
  retval = FALSE;

  /* clear the environment, apart from a few select settings */
  if (!clear_environment (error))
    goto error;

  /* check to see if we have a valid bus name */
  if (!check_bus_name (bus_name, error))
    goto error;

  /* get the correct parser, either the test or default parser */
  if (!get_correct_parser (&parser, error))
    goto error;

  /* check we are being invoked by the correct dbus user */
  if (!check_dbus_user (parser, error))
    goto error_free_parser;

  /* launch the bus with the service defined user */
  if (!launch_bus_name (bus_name, parser, error))
    goto error_free_parser;

  /* woohoo! */
  retval = TRUE;

error_free_parser:
  bus_config_parser_unref (parser);
error:
  return retval;
}
