/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* config-parser-trivial.c  XML-library-agnostic configuration file parser
 *                  Does not do includes or anything remotely complicated.
 *
 * Copyright (C) 2003, 2004, 2007 Red Hat, Inc.
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
#include "config-parser-common.h"
#include "config-parser-trivial.h"
#include "utils.h"
#include <dbus/dbus-list.h>
#include <dbus/dbus-internals.h>
#include <dbus/dbus-test-tap.h>
#include <string.h>

/**
 * TRIVIAL parser for bus configuration file.
 */
struct BusConfigParser
{
  ElementType type;
  DBusString user;                  /**< User the dbus-daemon runs as */
  DBusString bus_type;              /**< Message bus type */
  DBusString service_helper;        /**< Location of the setuid helper */
  DBusList *service_dirs;           /**< Directories to look for services in */
};

static dbus_bool_t
service_dirs_find_dir (DBusList **service_dirs,
                       const char *dir)
{
  DBusList *link;

  _dbus_assert (dir != NULL);

  for (link = *service_dirs; link; link = _dbus_list_get_next_link(service_dirs, link))
    {
      const char *link_dir;

      link_dir = (const char *)link->data;
      if (strcmp (dir, link_dir) == 0)
        return TRUE;
    }

  return FALSE;
}

static void
service_dirs_append_link_unique_or_free (DBusList **service_dirs,
                                         DBusList *dir_link)
{
  if (!service_dirs_find_dir (service_dirs, dir_link->data))
    {
      _dbus_list_append_link (service_dirs, dir_link);
    }
  else
    {
      dbus_free (dir_link->data);
      _dbus_list_free_link (dir_link);
    }
}

BusConfigParser*
bus_config_parser_new (const DBusString             *basedir,
                       dbus_bool_t                   is_toplevel,
                       const BusConfigParser        *parent)
{
  BusConfigParser *parser;

  parser = dbus_new0 (BusConfigParser, 1);
  if (parser == NULL)
    goto failed;

  parser->type = ELEMENT_NONE;

  /* init the lists */
  parser->service_dirs = NULL;

  /* init the strings */
  if (!_dbus_string_init (&parser->user))
    goto failed_parser;
  if (!_dbus_string_init (&parser->bus_type))
    goto failed_type;
  if (!_dbus_string_init (&parser->service_helper))
    goto failed_helper;

  /* woot! */
  return parser;

/* argh. we have do do this carefully because of OOM */
failed_helper:
  _dbus_string_free (&parser->bus_type);
failed_type:
  _dbus_string_free (&parser->user);
failed_parser:
  dbus_free (parser);
failed:
  return NULL;
}

void
bus_config_parser_unref (BusConfigParser *parser)
{
  _dbus_string_free (&parser->user);
  _dbus_string_free (&parser->service_helper);
  _dbus_string_free (&parser->bus_type);
  _dbus_list_clear_full (&parser->service_dirs, dbus_free);
  dbus_free (parser);
}

dbus_bool_t
bus_config_parser_start_element (BusConfigParser   *parser,
                                 const char        *element_name,
                                 const char       **attribute_names,
                                 const char       **attribute_values,
                                 DBusError         *error)
{
  /* we don't do processing of attribute names, we don't need to */
  parser->type = bus_config_parser_element_name_to_type (element_name);

  switch (parser->type)
    {
    case ELEMENT_SERVICEDIR:
    case ELEMENT_SERVICEHELPER:
    case ELEMENT_USER:
    case ELEMENT_CONFIGTYPE:
      /* content about to be handled */
      break;

    case ELEMENT_STANDARD_SYSTEM_SERVICEDIRS:
      {
        DBusList *link;
        DBusList *dirs;
        dirs = NULL;

        if (!_dbus_get_standard_system_servicedirs (&dirs))
          {
            BUS_SET_OOM (error);
            return FALSE;
          }

          while ((link = _dbus_list_pop_first_link (&dirs)))
            service_dirs_append_link_unique_or_free (&parser->service_dirs, link);
        break;
      }

    case ELEMENT_NONE:
    case ELEMENT_BUSCONFIG:
    case ELEMENT_INCLUDE:
    case ELEMENT_LISTEN:
    case ELEMENT_AUTH:
    case ELEMENT_POLICY:
    case ELEMENT_LIMIT:
    case ELEMENT_ALLOW:
    case ELEMENT_DENY:
    case ELEMENT_FORK:
    case ELEMENT_PIDFILE:
    case ELEMENT_INCLUDEDIR:
    case ELEMENT_SELINUX:
    case ELEMENT_ASSOCIATE:
    case ELEMENT_STANDARD_SESSION_SERVICEDIRS:
    case ELEMENT_KEEP_UMASK:
    case ELEMENT_SYSLOG:
    case ELEMENT_ALLOW_ANONYMOUS:
    case ELEMENT_APPARMOR:
      /* fall through */
    default:
      {
        /* we really don't care about the others... */
        _dbus_verbose (" START We don't care about '%s' type '%i'\n", element_name, parser->type);
        break;
      }
    }
  return TRUE;
}

dbus_bool_t
bus_config_parser_end_element (BusConfigParser   *parser,
                               const char               *element_name,
                               DBusError                *error)
{
  /* we don't care */
  return TRUE;
}

dbus_bool_t
bus_config_parser_content (BusConfigParser   *parser,
                           const DBusString  *content,
                           DBusError         *error)
{
  DBusString content_sane;
  dbus_bool_t retval;

  retval = FALSE;

  if (!_dbus_string_init (&content_sane))
    {
      BUS_SET_OOM (error);
      goto out;
    }
  if (!_dbus_string_copy (content, 0, &content_sane, 0))
    {
      BUS_SET_OOM (error);
      goto out_content;
    }

  /* rip out white space */
  _dbus_string_chop_white (&content_sane);
  if (_dbus_string_get_length (&content_sane) == 0)
    {
      /* optimise, there is no content */
      retval = TRUE;
      goto out_content;
    }

  switch (parser->type)
    {
    case ELEMENT_SERVICEDIR:
      {
      	char *cpath;

        /* copy the sane data into a char array */
        if (!_dbus_string_copy_data(&content_sane, &cpath))
          {
            BUS_SET_OOM (error);
            goto out_content;
          }

        /* append the dynamic char string to service dirs */
        if (!_dbus_list_append (&parser->service_dirs, cpath))
          {
            dbus_free (cpath);
            BUS_SET_OOM (error);
            goto out_content;
          }
      }
      break;

    case ELEMENT_SERVICEHELPER:
      {
        if (!_dbus_string_copy (&content_sane, 0, &parser->service_helper, 0))
          {
            BUS_SET_OOM (error);
            goto out_content;
          }
      }
      break;

    case ELEMENT_USER:
      {
        if (!_dbus_string_copy (&content_sane, 0, &parser->user, 0))
          {
            BUS_SET_OOM (error);
            goto out_content;
          }
      }
      break;

    case ELEMENT_CONFIGTYPE:
      {
        if (!_dbus_string_copy (&content_sane, 0, &parser->bus_type, 0))
          {
            BUS_SET_OOM (error);
            goto out_content;
          }
      }
      break;

    case ELEMENT_NONE:
    case ELEMENT_BUSCONFIG:
    case ELEMENT_INCLUDE:
    case ELEMENT_LISTEN:
    case ELEMENT_AUTH:
    case ELEMENT_POLICY:
    case ELEMENT_LIMIT:
    case ELEMENT_ALLOW:
    case ELEMENT_DENY:
    case ELEMENT_FORK:
    case ELEMENT_PIDFILE:
    case ELEMENT_INCLUDEDIR:
    case ELEMENT_SELINUX:
    case ELEMENT_ASSOCIATE:
    case ELEMENT_STANDARD_SESSION_SERVICEDIRS:
    case ELEMENT_STANDARD_SYSTEM_SERVICEDIRS:
    case ELEMENT_KEEP_UMASK:
    case ELEMENT_SYSLOG:
    case ELEMENT_ALLOW_ANONYMOUS:
    case ELEMENT_APPARMOR:
      /* fall through */
    default:
      {
        /* we don't care about the others... really */
        _dbus_verbose (" CONTENTS We don't care about '%s' type '%i'\n", _dbus_string_get_const_data (&content_sane), parser->type);
        break;
      }
    }

  /* woot! */
  retval = TRUE;

out_content:
  _dbus_string_free (&content_sane);
out:
  return retval;
}

dbus_bool_t
bus_config_parser_finished (BusConfigParser   *parser,
                            DBusError         *error)
{
  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  _dbus_verbose ("finished scanning!\n");
  return TRUE;
}

const char*
bus_config_parser_get_user (BusConfigParser *parser)
{
  return _dbus_string_get_const_data (&parser->user);
}

const char*
bus_config_parser_get_type (BusConfigParser *parser)
{
  return _dbus_string_get_const_data (&parser->bus_type);
}

/*
 * @returns A list of strings, owned by the BusConfigParser
 */
DBusList**
bus_config_parser_get_service_paths (BusConfigParser *parser)
{
  return &parser->service_dirs;
}

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
#include <stdio.h>
#include "test.h"

typedef enum
{
  VALID,
  INVALID,
  UNKNOWN
} Validity;

static dbus_bool_t
check_return_values (const DBusString *full_path)
{
  BusConfigParser *parser;
  DBusError error;
  dbus_bool_t retval;
  const char *user;
  const char *type;
  DBusList **dirs;

  dbus_error_init (&error);
  retval = FALSE;

  _dbus_test_diag ("Testing values from: %s", _dbus_string_get_const_data (full_path));

  parser = bus_config_load (full_path, TRUE, NULL, &error);
  if (parser == NULL)
    {
      _DBUS_ASSERT_ERROR_IS_SET (&error);
      if (dbus_error_has_name (&error, DBUS_ERROR_NO_MEMORY))
        _dbus_verbose ("Failed to load valid file due to OOM\n");
      goto finish;
    }
  _DBUS_ASSERT_ERROR_IS_CLEAR (&error);

  /* check user return value is okay */
  user = bus_config_parser_get_user (parser);
  if (user == NULL)
    {
      _dbus_warn ("User was NULL!");
      goto finish;
    }
#if 0
  /* the username can be configured in configure.in so this test doesn't work */
  if (strcmp (user, "dbus") != 0)
    {
      _dbus_warn ("User was invalid; '%s'!", user);
      goto finish;
    }
  _dbus_test_diag ("    <user>dbus</user> OKAY!");
#endif
  
  /* check type return value is okay */
  type = bus_config_parser_get_type (parser);
  if (type == NULL)
    {
      _dbus_warn ("Type was NULL!");
      goto finish;
    }
  if (strcmp (type, "system") != 0)
    {
      _dbus_warn ("Type was invalid; '%s'!", user);
      goto finish;
    }
  _dbus_test_diag ("    <type>system</type> OKAY!");

  /* check dirs return value is okay */
  dirs = bus_config_parser_get_service_paths (parser);
  if (dirs == NULL)
    {
      _dbus_warn ("Service dirs are NULL!");
      goto finish;
    }
  _dbus_test_diag ("    <standard_system_service_dirs/> OKAY!");
  /* NOTE: We tested the specific return values in the config-parser tests */

  /* woohoo! */
  retval = TRUE;
finish:
  if (parser != NULL)
    bus_config_parser_unref (parser);
  dbus_error_free (&error);
  return retval;
}

static dbus_bool_t
do_load (const DBusString *full_path,
         Validity          validity,
         dbus_bool_t       oom_possible)
{
  BusConfigParser *parser;
  DBusError error;

  dbus_error_init (&error);

  parser = bus_config_load (full_path, TRUE, NULL, &error);
  if (parser == NULL)
    {
      _DBUS_ASSERT_ERROR_IS_SET (&error);

      if (oom_possible &&
          dbus_error_has_name (&error, DBUS_ERROR_NO_MEMORY))
        {
          _dbus_verbose ("Failed to load valid file due to OOM\n");
          dbus_error_free (&error);
          return TRUE;
        }
      else if (validity == VALID)
        {
          _dbus_warn ("Failed to load valid file but still had memory: %s",
                      error.message);

          dbus_error_free (&error);
          return FALSE;
        }
      else
        {
          dbus_error_free (&error);
          return TRUE;
        }
    }
  else
    {
      _DBUS_ASSERT_ERROR_IS_CLEAR (&error);

      bus_config_parser_unref (parser);

      if (validity == INVALID)
        {
          _dbus_warn ("Accepted invalid file");
          return FALSE;
        }

      return TRUE;
    }
}

typedef struct
{
  const DBusString *full_path;
  Validity          validity;
} LoaderOomData;

static dbus_bool_t
check_loader_oom_func (void *data)
{
  LoaderOomData *d = data;

  return do_load (d->full_path, d->validity, TRUE);
}

static dbus_bool_t
process_test_valid_subdir (const DBusString *test_base_dir,
                           const char       *subdir,
                           Validity          validity)
{
  DBusString test_directory;
  DBusString filename;
  DBusDirIter *dir;
  dbus_bool_t retval;
  DBusError error;

  retval = FALSE;
  dir = NULL;

  if (!_dbus_string_init (&test_directory))
    _dbus_test_fatal ("didn't allocate test_directory");

  _dbus_string_init_const (&filename, subdir);

  if (!_dbus_string_copy (test_base_dir, 0,
                          &test_directory, 0))
    _dbus_test_fatal ("couldn't copy test_base_dir to test_directory");

  if (!_dbus_concat_dir_and_file (&test_directory, &filename))
    _dbus_test_fatal ("couldn't allocate full path");

  _dbus_string_free (&filename);
  if (!_dbus_string_init (&filename))
    _dbus_test_fatal ("didn't allocate filename string");

  dbus_error_init (&error);
  dir = _dbus_directory_open (&test_directory, &error);
  if (dir == NULL)
    {
      _dbus_warn ("Could not open %s: %s",
                  _dbus_string_get_const_data (&test_directory),
                  error.message);
      dbus_error_free (&error);
      goto failed;
    }

  if (validity == VALID)
    _dbus_test_diag ("Testing valid files:");
  else if (validity == INVALID)
    _dbus_test_diag ("Testing invalid files:");
  else
    _dbus_test_diag ("Testing unknown files:");

 next:
  while (_dbus_directory_get_next_file (dir, &filename, &error))
    {
      DBusString full_path;
      LoaderOomData d;

      if (!_dbus_string_init (&full_path))
        _dbus_test_fatal ("couldn't init string");

      if (!_dbus_string_copy (&test_directory, 0, &full_path, 0))
        _dbus_test_fatal ("couldn't copy dir to full_path");

      if (!_dbus_concat_dir_and_file (&full_path, &filename))
        _dbus_test_fatal ("couldn't concat file to dir");

      if (!_dbus_string_ends_with_c_str (&full_path, ".conf"))
        {
          _dbus_verbose ("Skipping non-.conf file %s\n",
                         _dbus_string_get_const_data (&filename));
          _dbus_string_free (&full_path);
          goto next;
        }

      _dbus_test_diag ("    %s", _dbus_string_get_const_data (&filename));

      _dbus_verbose (" expecting %s\n",
                     validity == VALID ? "valid" :
                     (validity == INVALID ? "invalid" :
                      (validity == UNKNOWN ? "unknown" : "???")));

      d.full_path = &full_path;
      d.validity = validity;

      /* FIXME hackaround for an expat problem, see
       * https://bugzilla.redhat.com/bugzilla/show_bug.cgi?id=124747
       * http://freedesktop.org/pipermail/dbus/2004-May/001153.html
       */
      /* if (!_dbus_test_oom_handling ("config-loader", check_loader_oom_func, &d)) */
      if (!check_loader_oom_func (&d))
        _dbus_test_fatal ("test failed");

      _dbus_string_free (&full_path);
    }

  if (dbus_error_is_set (&error))
    {
      _dbus_warn ("Could not get next file in %s: %s",
                  _dbus_string_get_const_data (&test_directory),
                  error.message);
      dbus_error_free (&error);
      goto failed;
    }

  retval = TRUE;

 failed:

  if (dir)
    _dbus_directory_close (dir);
  _dbus_string_free (&test_directory);
  _dbus_string_free (&filename);

  return retval;
}

/* convenience function, do not reuse outside of TEST */
static dbus_bool_t
make_full_path (const DBusString *test_data_dir,
			          const char       *subdir,
			          const char       *file,
			          DBusString       *full_path)
{
  DBusString filename;
  dbus_bool_t retval;

  retval = FALSE;

  if (!_dbus_string_init (full_path))
    {
      _dbus_warn ("couldn't allocate full path");
      goto finish;
    }

  if (!_dbus_string_copy (test_data_dir, 0, full_path, 0))
    {
      _dbus_warn ("couldn't allocate full path");
      goto finish;
    }

  _dbus_string_init_const (&filename, subdir);
  if (!_dbus_concat_dir_and_file (full_path, &filename))
    {
      _dbus_warn ("couldn't allocate full path");
      goto finish;
    }
  _dbus_string_free (&filename);

  _dbus_string_init_const (&filename, file);
  if (!_dbus_concat_dir_and_file (full_path, &filename))
    {
      _dbus_warn ("couldn't allocate full path");
      goto finish;
    }

  /* woot! */
  retval = TRUE;

finish:
  _dbus_string_free (&filename);
  return retval;
}

static dbus_bool_t
check_file_valid (DBusString *full_path,
			            Validity    validity)
{
  dbus_bool_t retval;

  if (validity == VALID)
    _dbus_test_diag ("Testing valid file:");
  else if (validity == INVALID)
    _dbus_test_diag ("Testing invalid file:");
  else
    _dbus_test_diag ("Testing unknown file:");

  /* print the filename, just so we match the other output */
  _dbus_test_diag ("    %s", _dbus_string_get_const_data (full_path));

  /* only test one file */
  retval = do_load (full_path, validity, TRUE);

  return retval;
}

dbus_bool_t
bus_config_parser_trivial_test (const char *test_data_dir_cstr)
{
  DBusString test_data_dir;
  DBusString full_path;
  dbus_bool_t retval;

  retval = FALSE;

  if (test_data_dir_cstr == NULL || test_data_dir_cstr[0] == '\0')
    {
      _dbus_test_diag ("No test data");
      return TRUE;
    }

  _dbus_string_init_const (&test_data_dir, test_data_dir_cstr);

  /* We already test default_session_servicedirs and default_system_servicedirs
   * in bus_config_parser_test() */
  if (!process_test_valid_subdir (&test_data_dir, "valid-config-files",
                                  VALID))
    goto finish;

#ifndef DBUS_WIN
  /* We already test default_session_servicedirs and default_system_servicedirs
   * in bus_config_parser_test() */
  if (!process_test_valid_subdir (&test_data_dir,
                                  "valid-config-files-system", VALID))
    goto finish;
#endif

  /* we don't process all the invalid files, as the trivial parser can't hope
   * to validate them all for all different syntaxes. We just check one broken
   * file to see if junk is received */
  if (!make_full_path (&test_data_dir, "invalid-config-files",
                       "not-well-formed.conf", &full_path))
    goto finish;
  if (!check_file_valid (&full_path, INVALID))
    goto finish;
  _dbus_string_free (&full_path);

#ifndef DBUS_WIN
  /* just test if the check_file_valid works okay and we got sane values */
  if (!make_full_path (&test_data_dir, "valid-config-files-system",
                       "system.conf", &full_path))
    goto finish;
  if (!check_file_valid (&full_path, VALID))
    goto finish;
  /* check to see if we got the correct values from the parser */
  if (!check_return_values (&full_path))
    goto finish;
#endif

  /* woot! */
  retval = TRUE;

finish:
  _dbus_string_free (&full_path);

  /* we don't process equiv-config-files as we don't handle <include> */
  return retval;
}

#endif /* DBUS_ENABLE_EMBEDDED_TESTS */
