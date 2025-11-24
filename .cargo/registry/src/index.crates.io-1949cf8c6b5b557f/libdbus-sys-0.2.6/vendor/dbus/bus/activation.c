/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* activation.c  Activation of services
 *
 * Copyright (C) 2003  CodeFactory AB
 * Copyright (C) 2003  Red Hat, Inc.
 * Copyright (C) 2004  Imendio HB
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
#include "activation.h"
#include "activation-exit-codes.h"
#include "config-parser.h"
#include "desktop-file.h"
#include "dispatch.h"
#include "services.h"
#include "test.h"
#include "utils.h"
#include <dbus/dbus-internals.h>
#include <dbus/dbus-hash.h>
#include <dbus/dbus-list.h>
#include <dbus/dbus-shell.h>
#ifdef ENABLE_TRADITIONAL_ACTIVATION
#include <dbus/dbus-spawn.h>
#endif
#include <dbus/dbus-timeout.h>
#include <dbus/dbus-sysdeps.h>
#include <dbus/dbus-test-tap.h>
#ifdef HAVE_ERRNO_H
#include <errno.h>
#endif

struct BusActivation
{
  int refcount;
  DBusHashTable *entries;
  DBusHashTable *pending_activations;
  char *server_address;
  BusContext *context;
  int n_pending_activations; /**< This is in fact the number of BusPendingActivationEntry,
                              * i.e. number of pending activation requests, not pending
                              * activations per se
                              */
  DBusList *directories;
  DBusHashTable *environment;
};

typedef struct
{
  int refcount;
  char *dir_c;
  BusServiceDirFlags flags;
  DBusHashTable *entries;
} BusServiceDirectory;

struct BusActivationEntry
{
  int refcount;
  char *name;
  char *exec;
  char *user;
  char *systemd_service;
  char *assumed_apparmor_label;
  unsigned long mtime;
  BusServiceDirectory *s_dir;
  char *filename;
};

typedef struct BusPendingActivationEntry BusPendingActivationEntry;

struct BusPendingActivationEntry
{
  /* Normally a method call, but if connection is NULL, this is a signal
   * instead.
   */
  DBusMessage *activation_message;
  /* NULL if this activation entry is for the dbus-daemon itself,
   * waiting for systemd to start. In this case, auto_activation is always
   * TRUE.
   */
  DBusConnection *connection;

  dbus_bool_t auto_activation;
};

typedef struct
{
  int refcount;
  BusActivation *activation;
  char *service_name;
  char *exec;
  char *systemd_service;
  DBusList *entries;
  int n_entries;
#ifdef ENABLE_TRADITIONAL_ACTIVATION
  DBusBabysitter *babysitter;
#endif
  DBusTimeout *timeout;
  unsigned int timeout_added : 1;
} BusPendingActivation;

#if 0
static BusServiceDirectory *
bus_service_directory_ref (BusServiceDirectory *dir)
{
  _dbus_assert (dir->refcount);

  dir->refcount++;

  return dir;
}
#endif

static void
bus_service_directory_unref (BusServiceDirectory *dir)
{
  if (dir == NULL)
    return;

  _dbus_assert (dir->refcount > 0);
  dir->refcount--;

  if (dir->refcount > 0)
    return;

  if (dir->entries)
    _dbus_hash_table_unref (dir->entries);

  dbus_free (dir->dir_c);
  dbus_free (dir);
}

static void
bus_pending_activation_entry_free (BusPendingActivationEntry *entry)
{
  if (entry->activation_message)
    dbus_message_unref (entry->activation_message);

  if (entry->connection)
    dbus_connection_unref (entry->connection);

  dbus_free (entry);
}

static BusPendingActivation *
bus_pending_activation_ref (BusPendingActivation *pending_activation)
{
  _dbus_assert (pending_activation->refcount > 0);
  pending_activation->refcount += 1;

  return pending_activation;
}

static void
bus_pending_activation_unref (BusPendingActivation *pending_activation)
{
  DBusList *link;

  if (pending_activation == NULL) /* hash table requires this */
    return;

  _dbus_assert (pending_activation->refcount > 0);
  pending_activation->refcount -= 1;

  if (pending_activation->refcount > 0)
    return;

  if (pending_activation->timeout_added)
    {
      _dbus_loop_remove_timeout (bus_context_get_loop (pending_activation->activation->context),
                                 pending_activation->timeout);
      pending_activation->timeout_added = FALSE;
    }

  if (pending_activation->timeout)
    _dbus_timeout_unref (pending_activation->timeout);

#ifdef ENABLE_TRADITIONAL_ACTIVATION
  if (pending_activation->babysitter)
    {
      if (!_dbus_babysitter_set_watch_functions (pending_activation->babysitter,
                                                 NULL, NULL, NULL,
                                                 pending_activation->babysitter,
                                                 NULL))
        _dbus_assert_not_reached ("setting watch functions to NULL failed");

      _dbus_babysitter_unref (pending_activation->babysitter);
    }
#endif

  dbus_free (pending_activation->service_name);
  dbus_free (pending_activation->exec);
  dbus_free (pending_activation->systemd_service);

  link = _dbus_list_get_first_link (&pending_activation->entries);

  while (link != NULL)
    {
      BusPendingActivationEntry *entry = link->data;

      bus_pending_activation_entry_free (entry);

      link = _dbus_list_get_next_link (&pending_activation->entries, link);
    }
  _dbus_list_clear (&pending_activation->entries);

  pending_activation->activation->n_pending_activations -=
    pending_activation->n_entries;

  _dbus_assert (pending_activation->activation->n_pending_activations >= 0);

  dbus_free (pending_activation);
}

static BusActivationEntry *
bus_activation_entry_ref (BusActivationEntry *entry)
{
  _dbus_assert (entry->refcount > 0);
  entry->refcount++;

  return entry;
}

static void
bus_activation_entry_unref (BusActivationEntry *entry)
{
  if (entry == NULL) /* hash table requires this */
    return;

  _dbus_assert (entry->refcount > 0);
  entry->refcount--;

  if (entry->refcount > 0)
    return;

  dbus_free (entry->name);
  dbus_free (entry->exec);
  dbus_free (entry->user);
  dbus_free (entry->filename);
  dbus_free (entry->systemd_service);
  dbus_free (entry->assumed_apparmor_label);

  dbus_free (entry);
}

static dbus_bool_t
update_desktop_file_entry (BusActivation       *activation,
                           BusServiceDirectory *s_dir,
                           DBusString          *filename,
                           BusDesktopFile      *desktop_file,
                           DBusError           *error)
{
  char *name, *exec, *user, *exec_tmp, *systemd_service;
  char *assumed_apparmor_label;
  BusActivationEntry *entry;
  DBusStat stat_buf;
  DBusString file_path;
  DBusError tmp_error;
  dbus_bool_t retval;
  DBusString str;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  retval = FALSE;
  name = NULL;
  exec = NULL;
  user = NULL;
  exec_tmp = NULL;
  entry = NULL;
  systemd_service = NULL;
  assumed_apparmor_label = NULL;

  dbus_error_init (&tmp_error);

  if (!_dbus_string_init (&file_path))
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  if (!_dbus_string_append (&file_path, s_dir->dir_c) ||
      !_dbus_concat_dir_and_file (&file_path, filename))
    {
      BUS_SET_OOM (error);
      goto out;
    }

  if (!_dbus_stat (&file_path, &stat_buf, NULL))
    {
      dbus_set_error (error, DBUS_ERROR_FAILED,
                      "Can't stat the service file\n");
      goto out;
    }

  if (!bus_desktop_file_get_string (desktop_file,
                                    DBUS_SERVICE_SECTION,
                                    DBUS_SERVICE_NAME,
                                    &name,
                                    error))
    goto out;

  if (!bus_desktop_file_get_string (desktop_file,
                                    DBUS_SERVICE_SECTION,
                                    DBUS_SERVICE_EXEC,
                                    &exec_tmp,
                                    error))
    goto out;

  if (!_dbus_string_init (&str))
    goto out;

  if (!_dbus_string_append (&str, exec_tmp) ||
      !_dbus_replace_install_prefix (&str) ||
      !_dbus_string_steal_data (&str, &exec))
    {
      _dbus_string_free (&str);
      goto out;
    }

  _dbus_string_free (&str);

  /* user is not _required_ unless we are using system activation */
  if (!bus_desktop_file_get_string (desktop_file,
                                    DBUS_SERVICE_SECTION,
                                    DBUS_SERVICE_USER,
                                    &user, &tmp_error))
    {
      _DBUS_ASSERT_ERROR_IS_SET (&tmp_error);
      /* if we got OOM, then exit */
      if (dbus_error_has_name (&tmp_error, DBUS_ERROR_NO_MEMORY))
        {
          dbus_move_error (&tmp_error, error);
          goto out;
        }
      else
        {
          /* if we have error because we didn't find anything then continue */
          dbus_error_free (&tmp_error);
          dbus_free (user);
          user = NULL;
        }
    }
  _DBUS_ASSERT_ERROR_IS_CLEAR (&tmp_error);

  /* systemd service is never required */
  if (!bus_desktop_file_get_string (desktop_file,
                                    DBUS_SERVICE_SECTION,
                                    DBUS_SERVICE_SYSTEMD_SERVICE,
                                    &systemd_service, &tmp_error))
    {
      _DBUS_ASSERT_ERROR_IS_SET (&tmp_error);
      /* if we got OOM, then exit */
      if (dbus_error_has_name (&tmp_error, DBUS_ERROR_NO_MEMORY))
        {
          dbus_move_error (&tmp_error, error);
          goto out;
        }
      else
        {
          /* if we have error because we didn't find anything then continue */
          dbus_error_free (&tmp_error);
          dbus_free (systemd_service);
          systemd_service = NULL;
        }
    }

  /* assumed AppArmor label is never required */
  if (!bus_desktop_file_get_string (desktop_file,
                                    DBUS_SERVICE_SECTION,
                                    DBUS_SERVICE_ASSUMED_APPARMOR_LABEL,
                                    &assumed_apparmor_label, &tmp_error))
    {
      _DBUS_ASSERT_ERROR_IS_SET (&tmp_error);
      /* if we got OOM, then exit */
      if (dbus_error_has_name (&tmp_error, DBUS_ERROR_NO_MEMORY))
        {
          dbus_move_error (&tmp_error, error);
          goto out;
        }
      else
        {
          /* if we have error because we didn't find anything then continue */
          dbus_error_free (&tmp_error);
          dbus_free (assumed_apparmor_label);
          assumed_apparmor_label = NULL;
        }
    }

  _DBUS_ASSERT_ERROR_IS_CLEAR (&tmp_error);

  entry = _dbus_hash_table_lookup_string (s_dir->entries,
                                          _dbus_string_get_const_data (filename));

  if (entry == NULL) /* New file */
    {
      DBusString expected_name;

      if (!_dbus_string_init (&expected_name))
        {
          BUS_SET_OOM (error);
          goto out;
        }

      if (!_dbus_string_append (&expected_name, name) ||
          !_dbus_string_append (&expected_name, ".service"))
        {
          _dbus_string_free (&expected_name);
          BUS_SET_OOM (error);
          goto out;
        }

      if (_dbus_string_equal (&expected_name, filename))
        {
          _dbus_verbose ("Name of \"%s\" is as expected\n",
                         _dbus_string_get_const_data (&file_path));
        }
      else if (s_dir->flags & BUS_SERVICE_DIR_FLAGS_STRICT_NAMING)
        {
          bus_context_log_and_set_error (activation->context,
                                         DBUS_SYSTEM_LOG_WARNING, error,
                                         DBUS_ERROR_FAILED,
                                         "Service file \"%s\" should have "
                                         "been named \"%s\": not loading it",
                                         _dbus_string_get_const_data (&file_path),
                                         _dbus_string_get_const_data (&expected_name));
          _dbus_string_free (&expected_name);
          goto out;
        }
      else if (bus_context_get_servicehelper (activation->context) != NULL)
        {
          bus_context_log (activation->context, DBUS_SYSTEM_LOG_WARNING,
                           "Service file \"%s\" should have been named \"%s\" "
                           "and will not work with system bus activation",
                           _dbus_string_get_const_data (&file_path),
                           _dbus_string_get_const_data (&expected_name));
          /* We don't actually error out here, because *technically* it could
           * still work on systemd systems, where we tell systemd to start the
           * SystemdService instead of launching dbus-daemon-launch-helper
           * ourselves. But maybe we should:
           * https://bugs.freedesktop.org/show_bug.cgi?id=99874 */
        }
      else
        {
          /* We could maybe log mismatched names for session services in
           * a user-visible way too, but not until
           * https://lintian.debian.org/tags/dbus-session-service-wrong-name.html
           * is a bit shorter.
           * https://bugs.freedesktop.org/show_bug.cgi?id=99873 */
          _dbus_verbose ("Name of \"%s\" should canonically be \"%s\"\n",
                         _dbus_string_get_const_data (&file_path),
                         _dbus_string_get_const_data (&expected_name));
        }

      _dbus_string_free (&expected_name);

      /* FIXME we need a better-defined algorithm for which service file to
       * pick than "whichever one is first in the directory listing"
       * See also https://bugs.freedesktop.org/show_bug.cgi?id=99874
       */
      if (_dbus_hash_table_lookup_string (activation->entries, name))
        {
          dbus_set_error (error, DBUS_ERROR_FAILED,
                          "Service %s already exists in activation entry list\n", name);
          goto out;
        }

      entry = dbus_new0 (BusActivationEntry, 1);
      if (entry == NULL)
        {
          BUS_SET_OOM (error);
          goto out;
        }

      entry->name = name;
      entry->exec = exec;
      entry->user = user;
      entry->systemd_service = systemd_service;
      entry->assumed_apparmor_label = assumed_apparmor_label;
      entry->refcount = 1;

      /* ownership has been transferred to entry, do not free separately */
      name = NULL;
      exec = NULL;
      user = NULL;
      systemd_service = NULL;
      assumed_apparmor_label = NULL;

      entry->s_dir = s_dir;
      entry->filename = _dbus_strdup (_dbus_string_get_const_data (filename));
      if (!entry->filename)
        {
          BUS_SET_OOM (error);
          goto out;
        }

      if (!_dbus_hash_table_insert_string (activation->entries, entry->name, bus_activation_entry_ref (entry)))
        {
          BUS_SET_OOM (error);
          goto out;
        }

      if (!_dbus_hash_table_insert_string (s_dir->entries, entry->filename, bus_activation_entry_ref (entry)))
        {
          /* Revert the insertion in the entries table */
          _dbus_hash_table_remove_string (activation->entries, entry->name);
          BUS_SET_OOM (error);
          goto out;
        }

      _dbus_verbose ("Added \"%s\" to list of services\n", entry->name);
    }
  else /* Just update the entry */
    {
      bus_activation_entry_ref (entry);
      _dbus_hash_table_remove_string (activation->entries, entry->name);

      if (_dbus_hash_table_lookup_string (activation->entries, name))
        {
          _dbus_verbose ("The new service name \"%s\" of service file \"%s\" is already in cache, ignoring\n",
                         name, _dbus_string_get_const_data (&file_path));
          dbus_set_error (error, DBUS_ERROR_FAILED,
                          "The new service name \"%s\" of service file \"%s\" is already in cache, ignoring\n",
                          name, _dbus_string_get_const_data (&file_path));
          goto out;
        }

      /* ownership has been transferred to entry, do not free separately */
      dbus_free (entry->name);
      entry->name = name;
      name = NULL;

      dbus_free (entry->exec);
      entry->exec = exec;
      exec = NULL;

      dbus_free (entry->user);
      entry->user = user;
      user = NULL;

      dbus_free (entry->systemd_service);
      entry->systemd_service = systemd_service;
      systemd_service = NULL;

      dbus_free (entry->assumed_apparmor_label);
      entry->assumed_apparmor_label = assumed_apparmor_label;
      assumed_apparmor_label = NULL;

      if (!_dbus_hash_table_insert_string (activation->entries,
                                           entry->name, bus_activation_entry_ref(entry)))
        {
          BUS_SET_OOM (error);
          /* Also remove path to entries hash since we want this in sync with
           * the entries hash table */
          _dbus_hash_table_remove_string (entry->s_dir->entries,
                                          entry->filename);
          goto out;
        }
    }

  entry->mtime = stat_buf.mtime;
  retval = TRUE;

out:
  /* if these have been transferred into entry, the variables will be NULL */
  dbus_free (exec_tmp);
  dbus_free (name);
  dbus_free (exec);
  dbus_free (user);
  dbus_free (systemd_service);
  dbus_free (assumed_apparmor_label);
  _dbus_string_free (&file_path);

  if (entry)
    bus_activation_entry_unref (entry);

  return retval;
}

static dbus_bool_t
check_service_file (BusActivation       *activation,
                    BusActivationEntry  *entry,
                    BusActivationEntry **updated_entry,
                    DBusError           *error)
{
  DBusStat stat_buf;
  dbus_bool_t retval;
  BusActivationEntry *tmp_entry;
  DBusString file_path;
  DBusString filename;

  retval = TRUE;
  tmp_entry = entry;

  _dbus_string_init_const (&filename, entry->filename);

  if (!_dbus_string_init (&file_path))
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  if (!_dbus_string_append (&file_path, entry->s_dir->dir_c) ||
      !_dbus_concat_dir_and_file (&file_path, &filename))
    {
      BUS_SET_OOM (error);
      retval = FALSE;
      goto out;
    }

  if (!_dbus_stat (&file_path, &stat_buf, NULL))
    {
      _dbus_verbose ("****** Can't stat file \"%s\", removing from cache\n",
                     _dbus_string_get_const_data (&file_path));

      _dbus_hash_table_remove_string (activation->entries, entry->name);
      _dbus_hash_table_remove_string (entry->s_dir->entries, entry->filename);

      tmp_entry = NULL;
      retval = TRUE;
      goto out;
    }
  else
    {
      if (stat_buf.mtime > entry->mtime)
        {
          BusDesktopFile *desktop_file;
          DBusError tmp_error;

          dbus_error_init (&tmp_error);

          desktop_file = bus_desktop_file_load (&file_path, &tmp_error);
          if (desktop_file == NULL)
            {
              _dbus_verbose ("Could not load %s: %s\n",
                             _dbus_string_get_const_data (&file_path),
                             tmp_error.message);
              if (dbus_error_has_name (&tmp_error, DBUS_ERROR_NO_MEMORY))
                {
                  dbus_move_error (&tmp_error, error);
                  retval = FALSE;
                  goto out;
                }
              dbus_error_free (&tmp_error);
              retval = TRUE;
              goto out;
            }

          /* @todo We can return OOM or a DBUS_ERROR_FAILED error
           *       Handle these both better
           */
          if (!update_desktop_file_entry (activation, entry->s_dir, &filename, desktop_file, &tmp_error))
            {
              bus_desktop_file_free (desktop_file);
              if (dbus_error_has_name (&tmp_error, DBUS_ERROR_NO_MEMORY))
                {
                  dbus_move_error (&tmp_error, error);
                  retval = FALSE;
                  goto out;
                }
              dbus_error_free (&tmp_error);
              retval = TRUE;
              goto out;
            }

          bus_desktop_file_free (desktop_file);
          retval = TRUE;
        }
    }

out:
  _dbus_string_free (&file_path);

  if (updated_entry != NULL)
    *updated_entry = tmp_entry;
  return retval;
}


/* warning: this doesn't fully "undo" itself on failure, i.e. doesn't strip
 * hash entries it already added.
 */
static dbus_bool_t
update_directory (BusActivation       *activation,
                  BusServiceDirectory *s_dir,
                  DBusError           *error)
{
  DBusDirIter *iter;
  DBusString dir, filename;
  BusDesktopFile *desktop_file;
  DBusError tmp_error;
  dbus_bool_t retval;
  BusActivationEntry *entry;
  DBusString full_path;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  iter = NULL;
  desktop_file = NULL;

  _dbus_string_init_const (&dir, s_dir->dir_c);

  if (!_dbus_string_init (&filename))
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  if (!_dbus_string_init (&full_path))
    {
      BUS_SET_OOM (error);
      _dbus_string_free (&filename);
      return FALSE;
    }

  retval = FALSE;

  /* from this point it's safe to "goto out" */

  iter = _dbus_directory_open (&dir, error);
  if (iter == NULL)
    {
      _dbus_verbose ("Failed to open directory %s: %s\n",
                     s_dir->dir_c,
                     error ? error->message : "unknown");
      goto out;
    }

  /* Now read the files */
  dbus_error_init (&tmp_error);
  while (_dbus_directory_get_next_file (iter, &filename, &tmp_error))
    {
      _dbus_assert (!dbus_error_is_set (&tmp_error));

      _dbus_string_set_length (&full_path, 0);

      if (!_dbus_string_ends_with_c_str (&filename, ".service"))
        {
          _dbus_verbose ("Skipping non-.service file '%s'\n",
                         _dbus_string_get_const_data (&filename));
          continue;
        }

      entry = _dbus_hash_table_lookup_string (s_dir->entries, _dbus_string_get_const_data (&filename));
      if (entry) /* Already has this service file in the cache */
        {
          if (!check_service_file (activation, entry, NULL, error))
            goto out;

          continue;
        }

      if (!_dbus_string_append (&full_path, s_dir->dir_c) ||
          !_dbus_concat_dir_and_file (&full_path, &filename))
        {
          BUS_SET_OOM (error);
          goto out;
        }

      /* New file */
      desktop_file = bus_desktop_file_load (&full_path, &tmp_error);
      if (desktop_file == NULL)
        {
          _dbus_verbose ("Could not load %s: %s\n",
                         _dbus_string_get_const_data (&full_path),
                         tmp_error.message);

          if (dbus_error_has_name (&tmp_error, DBUS_ERROR_NO_MEMORY))
            {
              dbus_move_error (&tmp_error, error);
              goto out;
            }

          dbus_error_free (&tmp_error);
          continue;
        }

      /* @todo We can return OOM or a DBUS_ERROR_FAILED error
       *       Handle these both better
       */
      if (!update_desktop_file_entry (activation, s_dir, &filename, desktop_file, &tmp_error))
        {
          bus_desktop_file_free (desktop_file);
          desktop_file = NULL;

          _dbus_verbose ("Could not add %s to activation entry list: %s\n",
                         _dbus_string_get_const_data (&full_path), tmp_error.message);

          if (dbus_error_has_name (&tmp_error, DBUS_ERROR_NO_MEMORY))
            {
              dbus_move_error (&tmp_error, error);
              goto out;
            }

          dbus_error_free (&tmp_error);
          continue;
        }
      else
        {
          bus_desktop_file_free (desktop_file);
          desktop_file = NULL;
          continue;
        }
    }

  if (dbus_error_is_set (&tmp_error))
    {
      dbus_move_error (&tmp_error, error);
      goto out;
    }

  retval = TRUE;

 out:
  _DBUS_ASSERT_ERROR_XOR_BOOL (error, retval);

  if (iter != NULL)
    _dbus_directory_close (iter);
  _dbus_string_free (&filename);
  _dbus_string_free (&full_path);

  return retval;
}

static dbus_bool_t
populate_environment (BusActivation *activation)
{
  char       **environment;
  dbus_bool_t  retval = FALSE;

  environment = _dbus_get_environment ();

  if (environment == NULL)
    return FALSE;

  retval = _dbus_hash_table_from_array (activation->environment, environment, '=');
  dbus_free_string_array (environment);

  /*
   * These environment variables are set by systemd for the dbus-daemon
   * itself, and are not applicable to our child processes.
   *
   * Of the other environment variables listed in systemd.exec(5):
   *
   * - XDG_RUNTIME_DIR, XDG_SESSION_ID, XDG_SEAT, XDG_VTNR: Properties of
   *   the session and equally true for the activated service, should not
   *   be reset
   * - PATH, LANG, USER, LOGNAME, HOME, SHELL, MANAGERPID: Equally true for
   *   the activated service, should not be reset
   * - TERM, WATCHDOG_*: Should not be set for dbus-daemon, so not applicable
   * - MAINPID, SERVICE_RESULT, EXIT_CODE, EXIT_STATUS: Not set for ExecStart,
   *   so not applicable
   */

  /* We give activated services their own Journal stream to avoid their
   * logging being attributed to dbus-daemon */
  _dbus_hash_table_remove_string (activation->environment, "JOURNAL_STREAM");

  /* This is dbus-daemon's listening socket, not the activatable service's */
  _dbus_hash_table_remove_string (activation->environment, "LISTEN_FDNAMES");
  _dbus_hash_table_remove_string (activation->environment, "LISTEN_FDS");
  _dbus_hash_table_remove_string (activation->environment, "LISTEN_PID");

  /* This is dbus-daemon's status notification, not the activatable service's
   * (and NotifyAccess wouldn't let it write here anyway) */
  _dbus_hash_table_remove_string (activation->environment, "NOTIFY_SOCKET");

  /* This identifies the dbus-daemon invocation. Whether it should be
   * inherited by "smaller" services isn't entirely clear-cut, but not
   * inheriting it makes traditional D-Bus activation under systemd a
   * little more consistent with systemd activation.
   * https://lists.freedesktop.org/archives/systemd-devel/2018-March/040467.html */
  _dbus_hash_table_remove_string (activation->environment, "INVOCATION_ID");

  return retval;
}

dbus_bool_t
bus_activation_reload (BusActivation     *activation,
                       const DBusString  *address,
                       DBusList         **directories,
                       DBusError         *error)
{
  DBusList      *link;
  char          *dir;
  DBusError local_error = DBUS_ERROR_INIT;

  if (activation->server_address != NULL)
    dbus_free (activation->server_address);
  if (!_dbus_string_copy_data (address, &activation->server_address))
    {
      BUS_SET_OOM (error);
      goto failed;
    }

  if (activation->entries != NULL)
    _dbus_hash_table_unref (activation->entries);
  activation->entries = _dbus_hash_table_new (DBUS_HASH_STRING, NULL,
                                             (DBusFreeFunction)bus_activation_entry_unref);
  if (activation->entries == NULL)
    {
      BUS_SET_OOM (error);
      goto failed;
    }

  _dbus_list_clear_full (&activation->directories,
                         (DBusFreeFunction) bus_service_directory_unref);

  link = _dbus_list_get_first_link (directories);
  while (link != NULL)
    {
      BusConfigServiceDir *config = link->data;
      BusServiceDirectory *s_dir;

      _dbus_assert (config->path != NULL);

      dir = _dbus_strdup (config->path);
      if (!dir)
        {
          BUS_SET_OOM (error);
          goto failed;
        }

      s_dir = dbus_new0 (BusServiceDirectory, 1);
      if (!s_dir)
        {
          dbus_free (dir);
          BUS_SET_OOM (error);
          goto failed;
        }

      s_dir->refcount = 1;
      s_dir->dir_c = dir;
      s_dir->flags = config->flags;

      s_dir->entries = _dbus_hash_table_new (DBUS_HASH_STRING, NULL,
                                             (DBusFreeFunction)bus_activation_entry_unref);

      if (!s_dir->entries)
        {
          bus_service_directory_unref (s_dir);
          BUS_SET_OOM (error);
          goto failed;
        }

      if (!_dbus_list_append (&activation->directories, s_dir))
        {
          bus_service_directory_unref (s_dir);
          BUS_SET_OOM (error);
          goto failed;
        }

      /* only fail on OOM, it is ok if we can't read the directory */
      if (!update_directory (activation, s_dir, &local_error))
       {
         if (dbus_error_has_name (&local_error, DBUS_ERROR_NO_MEMORY))
           {
             dbus_move_error (&local_error, error);
             goto failed;
           }
         else
           {
             dbus_error_free (&local_error);
           }
       }

      link = _dbus_list_get_next_link (directories, link);
    }

  return TRUE;
 failed:
  return FALSE;
}

BusActivation*
bus_activation_new (BusContext        *context,
                    const DBusString  *address,
                    DBusList         **directories,
                    DBusError         *error)
{
  BusActivation *activation;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  activation = dbus_new0 (BusActivation, 1);
  if (activation == NULL)
    {
      BUS_SET_OOM (error);
      return NULL;
    }

  activation->refcount = 1;
  activation->context = context;
  activation->n_pending_activations = 0;

  if (!bus_activation_reload (activation, address, directories, error))
    goto failed;

   /* Initialize this hash table once, we don't want to lose pending
   * activations on reload. */
  activation->pending_activations = _dbus_hash_table_new (DBUS_HASH_STRING, NULL,
                                                          (DBusFreeFunction)bus_pending_activation_unref);

  if (activation->pending_activations == NULL)
    {
      BUS_SET_OOM (error);
      goto failed;
    }

  activation->environment = _dbus_hash_table_new (DBUS_HASH_STRING,
                                                  (DBusFreeFunction) dbus_free,
                                                  (DBusFreeFunction) dbus_free);

  if (activation->environment == NULL)
    {
      BUS_SET_OOM (error);
      goto failed;
    }

  if (!populate_environment (activation))
    {
      BUS_SET_OOM (error);
      goto failed;
    }

  return activation;

 failed:
  bus_activation_unref (activation);
  return NULL;
}

BusActivation *
bus_activation_ref (BusActivation *activation)
{
  _dbus_assert (activation->refcount > 0);

  activation->refcount += 1;

  return activation;
}

void
bus_activation_unref (BusActivation *activation)
{
  _dbus_assert (activation->refcount > 0);

  activation->refcount -= 1;

  if (activation->refcount > 0)
    return;

  dbus_free (activation->server_address);
  if (activation->entries)
    _dbus_hash_table_unref (activation->entries);
  if (activation->pending_activations)
    _dbus_hash_table_unref (activation->pending_activations);

  _dbus_list_clear_full (&activation->directories,
                         (DBusFreeFunction) bus_service_directory_unref);

  if (activation->environment)
    _dbus_hash_table_unref (activation->environment);

  dbus_free (activation);
}

#ifdef ENABLE_TRADITIONAL_ACTIVATION
static dbus_bool_t
add_bus_environment (BusActivation *activation,
                     DBusError     *error)
{
  const char *type;

  if (!bus_activation_set_environment_variable (activation,
                                                "DBUS_STARTER_ADDRESS",
                                                activation->server_address,
                                                error))
    return FALSE;

  type = bus_context_get_type (activation->context);
  if (type != NULL)
    {
      if (!bus_activation_set_environment_variable (activation,
                                                    "DBUS_STARTER_BUS_TYPE", type,
                                                    error))
        return FALSE;

      if (strcmp (type, "session") == 0)
        {
          if (!bus_activation_set_environment_variable (activation,
                                                        "DBUS_SESSION_BUS_ADDRESS",
                                                        activation->server_address,
                                                        error))
            return FALSE;
        }
      else if (strcmp (type, "system") == 0)
        {
          if (!bus_activation_set_environment_variable (activation,
                                                        "DBUS_SYSTEM_BUS_ADDRESS",
                                                        activation->server_address,
                                                        error))
            return FALSE;
        }
    }

  return TRUE;
}
#endif

typedef struct
{
  BusPendingActivation *pending_activation;
  DBusPreallocatedHash *hash_entry;
} RestorePendingData;

static void
restore_pending (void *data)
{
  RestorePendingData *d = data;

  _dbus_assert (d->pending_activation != NULL);
  _dbus_assert (d->hash_entry != NULL);

  _dbus_verbose ("Restoring pending activation for service %s, has timeout = %d\n",
                 d->pending_activation->service_name,
                 d->pending_activation->timeout_added);

  _dbus_hash_table_insert_string_preallocated (d->pending_activation->activation->pending_activations,
                                               d->hash_entry,
                                               d->pending_activation->service_name, d->pending_activation);

  bus_pending_activation_ref (d->pending_activation);

  d->hash_entry = NULL;
}

static void
free_restore_pending_data (void *data)
{
  RestorePendingData *d = data;

  if (d->hash_entry)
    _dbus_hash_table_free_preallocated_entry (d->pending_activation->activation->pending_activations,
                                              d->hash_entry);

  bus_pending_activation_unref (d->pending_activation);

  dbus_free (d);
}

static dbus_bool_t
add_restore_pending_to_transaction (BusTransaction       *transaction,
                                    BusPendingActivation *pending_activation)
{
  RestorePendingData *d;

  d = dbus_new (RestorePendingData, 1);
  if (d == NULL)
    return FALSE;

  d->pending_activation = pending_activation;
  d->hash_entry = _dbus_hash_table_preallocate_entry (d->pending_activation->activation->pending_activations);

  bus_pending_activation_ref (d->pending_activation);

  if (d->hash_entry == NULL ||
      !bus_transaction_add_cancel_hook (transaction, restore_pending, d,
                                        free_restore_pending_data))
    {
      free_restore_pending_data (d);
      return FALSE;
    }

  _dbus_verbose ("Saved pending activation to be restored if the transaction fails\n");

  return TRUE;
}

dbus_bool_t
bus_activation_service_created (BusActivation  *activation,
                                const char     *service_name,
                                BusTransaction *transaction,
                                DBusError      *error)
{
  BusPendingActivation *pending_activation;
  DBusMessage *message;
  DBusList *link;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  /* Check if it's a pending activation */
  pending_activation = _dbus_hash_table_lookup_string (activation->pending_activations, service_name);

  if (!pending_activation)
    return TRUE;

  bus_context_log (activation->context,
                   DBUS_SYSTEM_LOG_INFO, "Successfully activated service '%s'",
                   service_name);

  link = _dbus_list_get_first_link (&pending_activation->entries);
  while (link != NULL)
    {
      BusPendingActivationEntry *entry = link->data;
      DBusList *next = _dbus_list_get_next_link (&pending_activation->entries, link);

      /* entry->connection is NULL for activating systemd */
      if (entry->connection && dbus_connection_get_is_connected (entry->connection))
        {
          /* Only send activation replies to regular activation requests. */
          if (!entry->auto_activation)
            {
              dbus_uint32_t result;

              message = dbus_message_new_method_return (entry->activation_message);
              if (!message)
                {
                  BUS_SET_OOM (error);
                  goto error;
                }

              result = DBUS_START_REPLY_SUCCESS;

              if (!dbus_message_append_args (message,
                                             DBUS_TYPE_UINT32, &result,
                                             DBUS_TYPE_INVALID))
                {
                  dbus_message_unref (message);
                  BUS_SET_OOM (error);
                  goto error;
                }

              if (!bus_transaction_send_from_driver (transaction, entry->connection, message))
                {
                  dbus_message_unref (message);
                  BUS_SET_OOM (error);
                  goto error;
                }

              dbus_message_unref (message);
            }
        }

      link = next;
    }

  return TRUE;

 error:
  return FALSE;
}

dbus_bool_t
bus_activation_send_pending_auto_activation_messages (BusActivation  *activation,
                                                      BusService     *service,
                                                      BusTransaction *transaction)
{
  BusPendingActivation *pending_activation;
  DBusList *link;

  /* Check if it's a pending activation */
  pending_activation = _dbus_hash_table_lookup_string (activation->pending_activations,
                                                       bus_service_get_name (service));

  if (!pending_activation)
    return TRUE;

  link = _dbus_list_get_first_link (&pending_activation->entries);
  while (link != NULL)
    {
      BusPendingActivationEntry *entry = link->data;
      DBusList *next = _dbus_list_get_next_link (&pending_activation->entries, link);

      if (entry->auto_activation && (entry->connection == NULL || dbus_connection_get_is_connected (entry->connection)))
        {
          DBusConnection *addressed_recipient;
          DBusError error;

          dbus_error_init (&error);

          addressed_recipient = bus_service_get_primary_owners_connection (service);

          /* Resume dispatching where we left off in bus_dispatch() */
          if (!bus_dispatch_matches (transaction,
                                     entry->connection,
                                     addressed_recipient,
                                     entry->activation_message, &error))
            {
              /* If permission is denied, we just want to return the error
               * to the original method invoker; in particular, we don't
               * want to make the RequestName call fail with that error
               * (see fd.o #78979, CVE-2014-3477). */
              if (!bus_transaction_send_error_reply (transaction, entry->connection,
                                                     &error, entry->activation_message))
                {
                  bus_connection_send_oom_error (entry->connection,
                                                 entry->activation_message);
                }

              dbus_error_free (&error);
              link = next;
              continue;
            }
        }

      link = next;
    }

  if (!add_restore_pending_to_transaction (transaction, pending_activation))
    {
      _dbus_verbose ("Could not add cancel hook to transaction to revert removing pending activation\n");
      goto error;
    }

  _dbus_hash_table_remove_string (activation->pending_activations, bus_service_get_name (service));

  return TRUE;

 error:
  return FALSE;
}

/**
 * FIXME @todo the error messages here would ideally be preallocated
 * so we don't need to allocate memory to send them.
 * Using the usual tactic, prealloc an OOM message, then
 * if we can't alloc the real error send the OOM error instead.
 */
static dbus_bool_t
try_send_activation_failure (BusPendingActivation *pending_activation,
                             const DBusError      *how)
{
  BusActivation *activation;
  DBusList *link;
  BusTransaction *transaction;

  activation = pending_activation->activation;

  transaction = bus_transaction_new (activation->context);
  if (transaction == NULL)
    return FALSE;

  link = _dbus_list_get_first_link (&pending_activation->entries);
  while (link != NULL)
    {
      BusPendingActivationEntry *entry = link->data;
      DBusList *next = _dbus_list_get_next_link (&pending_activation->entries, link);

      if (entry->connection && dbus_connection_get_is_connected (entry->connection))
        {
          if (!bus_transaction_send_error_reply (transaction,
                                                 entry->connection,
                                                 how,
                                                 entry->activation_message))
            goto error;
        }

      link = next;
    }

  bus_transaction_execute_and_free (transaction);

  return TRUE;

 error:
  if (transaction)
    bus_transaction_cancel_and_free (transaction);
  return FALSE;
}

/**
 * Free the pending activation and send an error message to all the
 * connections that were waiting for it.
 */
static void
pending_activation_failed (BusPendingActivation *pending_activation,
                           const DBusError      *how)
{
  /* FIXME use preallocated OOM messages instead of bus_wait_for_memory() */
  while (!try_send_activation_failure (pending_activation, how))
    _dbus_wait_for_memory ();

  /* Destroy this pending activation */
  _dbus_hash_table_remove_string (pending_activation->activation->pending_activations,
                                  pending_activation->service_name);
}

#ifdef ENABLE_TRADITIONAL_ACTIVATION
/**
 * Depending on the exit code of the helper, set the error accordingly
 */
static void
handle_servicehelper_exit_error (int        exit_code,
                                 DBusError *error)
{
  switch (exit_code)
    {
    case BUS_SPAWN_EXIT_CODE_CONFIG_INVALID:
      dbus_set_error (error, DBUS_ERROR_SPAWN_CONFIG_INVALID,
		      "Invalid configuration (missing or empty <user>?)");
      break;
    case BUS_SPAWN_EXIT_CODE_NO_MEMORY:
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY,
                      "Launcher could not run (out of memory)");
      break;
    case BUS_SPAWN_EXIT_CODE_SETUP_FAILED:
      dbus_set_error (error, DBUS_ERROR_SPAWN_SETUP_FAILED,
                      "Failed to setup environment correctly");
      break;
    case BUS_SPAWN_EXIT_CODE_NAME_INVALID:
      dbus_set_error (error, DBUS_ERROR_SPAWN_SERVICE_INVALID,
                      "Bus name is not valid or missing");
      break;
    case BUS_SPAWN_EXIT_CODE_SERVICE_NOT_FOUND:
      dbus_set_error (error, DBUS_ERROR_SPAWN_SERVICE_NOT_FOUND,
                      "Bus name not found in system service directory");
      break;
    case BUS_SPAWN_EXIT_CODE_PERMISSIONS_INVALID:
      dbus_set_error (error, DBUS_ERROR_SPAWN_PERMISSIONS_INVALID,
                      "The permission of the setuid helper is not correct");
      break;
    case BUS_SPAWN_EXIT_CODE_FILE_INVALID:
      dbus_set_error (error, DBUS_ERROR_SPAWN_PERMISSIONS_INVALID,
                      "The service file is incorrect or does not have all required attributes");
      break;
    case BUS_SPAWN_EXIT_CODE_EXEC_FAILED:
      dbus_set_error (error, DBUS_ERROR_SPAWN_EXEC_FAILED,
                      "Cannot launch daemon, file not found or permissions invalid");
      break;
    case BUS_SPAWN_EXIT_CODE_INVALID_ARGS:
      dbus_set_error (error, DBUS_ERROR_INVALID_ARGS,
                      "Invalid arguments to command line");
      break;
    case BUS_SPAWN_EXIT_CODE_CHILD_SIGNALED:
      dbus_set_error (error, DBUS_ERROR_SPAWN_CHILD_SIGNALED,
                      "Launched child was signaled, it probably crashed");
      break;
    case BUS_SPAWN_EXIT_CODE_GENERIC_FAILURE:
    default:
      dbus_set_error (error, DBUS_ERROR_SPAWN_CHILD_EXITED,
                      "Launch helper exited with unknown return code %i", exit_code);
      break;
    }
}

static void
pending_activation_finished_cb (DBusBabysitter *babysitter,
                                void           *data)
{
  BusPendingActivation *pending_activation = data;
  dbus_bool_t uses_servicehelper;

  _dbus_assert (babysitter == pending_activation->babysitter);
  _dbus_babysitter_ref (babysitter);

  /* There are two major cases here; are we the system bus or the session?  Here this
   * is distinguished by whether or not we use a setuid helper launcher.  With the launch helper,
   * some process exit codes are meaningful, processed by handle_servicehelper_exit_error.
   *
   * In both cases though, just ignore when a process exits with status 0; it's possible for
   * a program to (misguidedly) "daemonize", and that appears to us as an exit.  This closes a race
   * condition between this code and the child process claiming the bus name.
   */
  uses_servicehelper = bus_context_get_servicehelper (pending_activation->activation->context) != NULL;

  /* strictly speaking this is redundant with the check in dbus-spawn now */
  if (_dbus_babysitter_get_child_exited (babysitter))
    {
      DBusError error;
      DBusHashIter iter;
      dbus_bool_t activation_failed;
      int exit_code = 0;

      dbus_error_init (&error);

      _dbus_babysitter_set_child_exit_error (babysitter, &error);

      /* Explicitly check for SPAWN_CHILD_EXITED to avoid overwriting an
       * exec error */
      if (dbus_error_has_name (&error, DBUS_ERROR_SPAWN_CHILD_EXITED)
          && _dbus_babysitter_get_child_exit_status (babysitter, &exit_code))
        {
          activation_failed = exit_code != 0;

          dbus_error_free(&error);

          if (activation_failed)
            {
              if (uses_servicehelper)
                handle_servicehelper_exit_error (exit_code, &error);
              else
                _dbus_babysitter_set_child_exit_error (babysitter, &error);
            }
        }
      else
        {
          activation_failed = TRUE;
        }

      if (activation_failed)
        {
          bus_context_log (pending_activation->activation->context,
                           DBUS_SYSTEM_LOG_INFO, "Activated service '%s' failed: %s",
                           pending_activation->service_name,
                           error.message);

          /* Destroy all pending activations with the same exec */
          _dbus_hash_iter_init (pending_activation->activation->pending_activations,
                                &iter);
          while (_dbus_hash_iter_next (&iter))
            {
              BusPendingActivation *p = _dbus_hash_iter_get_value (&iter);

              if (p != pending_activation && p->exec != NULL &&
                  strcmp (p->exec, pending_activation->exec) == 0)
                pending_activation_failed (p, &error);
            }

          /* Destroys the pending activation */
          pending_activation_failed (pending_activation, &error);

          dbus_error_free (&error);
        }
    }

  _dbus_babysitter_unref (babysitter);
}

static dbus_bool_t
add_babysitter_watch (DBusWatch      *watch,
                      void           *data)
{
  BusPendingActivation *pending_activation = data;

  return _dbus_loop_add_watch (
      bus_context_get_loop (pending_activation->activation->context),
      watch);
}

static void
remove_babysitter_watch (DBusWatch      *watch,
                         void           *data)
{
  BusPendingActivation *pending_activation = data;

  _dbus_loop_remove_watch (bus_context_get_loop (pending_activation->activation->context),
                           watch);
}

static void
toggle_babysitter_watch (DBusWatch      *watch,
                         void           *data)
{
  BusPendingActivation *pending_activation = data;

  _dbus_loop_toggle_watch (bus_context_get_loop (pending_activation->activation->context),
                           watch);
}
#endif

static dbus_bool_t
pending_activation_timed_out (void *data)
{
  BusPendingActivation *pending_activation = data;
  BusContext *context;
  DBusError error;
  int timeout;

  context = pending_activation->activation->context;
  timeout = bus_context_get_activation_timeout (context);

#ifdef ENABLE_TRADITIONAL_ACTIVATION
  /* Kill the spawned process, since it sucks
   * (not sure this is what we want to do, but
   * may as well try it for now)
   */
  if (pending_activation->babysitter)
    _dbus_babysitter_kill_child (pending_activation->babysitter);
#endif

  dbus_error_init (&error);

  bus_context_log_and_set_error (context, DBUS_SYSTEM_LOG_WARNING, &error,
                   DBUS_ERROR_TIMED_OUT,
                   "Failed to activate service '%s': timed out "
                   "(service_start_timeout=%dms)",
                   pending_activation->service_name, timeout);

  pending_activation_failed (pending_activation, &error);

  dbus_error_free (&error);

  return TRUE;
}

static void
cancel_pending (void *data)
{
  BusPendingActivation *pending_activation = data;

  _dbus_verbose ("Canceling pending activation of %s\n",
                 pending_activation->service_name);

#ifdef ENABLE_TRADITIONAL_ACTIVATION
  if (pending_activation->babysitter)
    _dbus_babysitter_kill_child (pending_activation->babysitter);
#endif

  _dbus_hash_table_remove_string (pending_activation->activation->pending_activations,
                                  pending_activation->service_name);
}

static void
free_pending_cancel_data (void *data)
{
  BusPendingActivation *pending_activation = data;

  bus_pending_activation_unref (pending_activation);
}

static dbus_bool_t
add_cancel_pending_to_transaction (BusTransaction       *transaction,
                                   BusPendingActivation *pending_activation)
{
  if (!bus_transaction_add_cancel_hook (transaction, cancel_pending,
                                        pending_activation,
                                        free_pending_cancel_data))
    return FALSE;

  bus_pending_activation_ref (pending_activation);

  _dbus_verbose ("Saved pending activation to be canceled if the transaction fails\n");

  return TRUE;
}

static dbus_bool_t
update_service_cache (BusActivation *activation, DBusError *error)
{
  DBusList *iter;

  for (iter = _dbus_list_get_first_link (&activation->directories);
       iter != NULL;
       iter = _dbus_list_get_next_link (&activation->directories, iter))
    {
      DBusError tmp_error;
      BusServiceDirectory *s_dir = iter->data;

      dbus_error_init (&tmp_error);
      if (!update_directory (activation, s_dir, &tmp_error))
        {
          if (dbus_error_has_name (&tmp_error, DBUS_ERROR_NO_MEMORY))
            {
              dbus_move_error (&tmp_error, error);
              return FALSE;
            }

          dbus_error_free (&tmp_error);
          continue;
        }
    }

  return TRUE;
}

static BusActivationEntry *
activation_find_entry (BusActivation *activation,
                       const char    *service_name,
                       DBusError     *error)
{
  BusActivationEntry *entry;

  entry = _dbus_hash_table_lookup_string (activation->entries, service_name);
  if (!entry)
    {
      if (!update_service_cache (activation, error))
        return NULL;

      entry = _dbus_hash_table_lookup_string (activation->entries,
                                              service_name);
    }
  else
    {
      BusActivationEntry *updated_entry;

      if (!check_service_file (activation, entry, &updated_entry, error))
        return NULL;

      entry = updated_entry;
    }

  if (!entry)
    {
      dbus_set_error (error, DBUS_ERROR_SERVICE_UNKNOWN,
                      "The name %s was not provided by any .service files",
                      service_name);
      return NULL;
    }

  return entry;
}

#ifdef ENABLE_TRADITIONAL_ACTIVATION
static char **
bus_activation_get_environment (BusActivation *activation)
{
  return _dbus_hash_table_to_array (activation->environment, '=');
}
#endif

dbus_bool_t
bus_activation_set_environment_variable (BusActivation     *activation,
                                         const char        *key,
                                         const char        *value,
                                         DBusError         *error)
{
  char        *hash_key;
  char        *hash_value;
  dbus_bool_t  retval;

  retval = FALSE;
  hash_key = NULL;
  hash_value = NULL;
  hash_key = _dbus_strdup (key);

  if (hash_key == NULL)
    goto out;

  hash_value = _dbus_strdup (value);

  if (hash_value == NULL)
    goto out;

  if (!_dbus_hash_table_insert_string (activation->environment,
                                       hash_key, hash_value))
    goto out;

  retval = TRUE;
out:
  if (retval == FALSE)
    {
      dbus_free (hash_key);
      dbus_free (hash_value);
      BUS_SET_OOM (error);
    }

  return retval;
}

#ifdef ENABLE_TRADITIONAL_ACTIVATION
static void
child_setup (void *user_data)
{
#ifdef DBUS_UNIX
  BusActivation *activation = user_data;
  DBusRLimit *initial_fd_limit;
  DBusError error;

  dbus_error_init (&error);
  initial_fd_limit = bus_context_get_initial_fd_limit (activation->context);

  if (initial_fd_limit != NULL &&
      !_dbus_rlimit_restore_fd_limit (initial_fd_limit, &error))
    {
      /* unfortunately we don't actually know the service name here */
      bus_context_log (activation->context,
                       DBUS_SYSTEM_LOG_WARNING,
                       "Failed to reset fd limit before activating "
                       "service: %s: %s",
                       error.name, error.message);
    }
#endif
}
#endif


/*
 * Try to activate the given service.
 *
 * connection is the connection requesting that the service be started,
 * or NULL if the activation was caused by the dbus-daemon itself (when
 * systemd activation waits for systemd to connect to us, or when calling
 * SetEnvironment on systemd).
 *
 * auto_activation is TRUE if we are carrying out auto-starting (we are
 * activating a service automatically in order to deliver a message to it)
 * or FALSE if we are starting the service explicitly (as for
 * StartServiceByName).
 *
 * activation_message is the message that caused this activation.
 */
dbus_bool_t
bus_activation_activate_service (BusActivation  *activation,
                                 DBusConnection *connection,
                                 BusTransaction *transaction,
                                 dbus_bool_t     auto_activation,
                                 DBusMessage    *activation_message,
                                 const char     *service_name,
                                 DBusError      *error)
{
  BusActivationEntry *entry;
  BusPendingActivation *pending_activation;
  BusPendingActivationEntry *pending_activation_entry;
  DBusMessage *message;
  DBusString service_str;
  dbus_bool_t retval;
  dbus_bool_t was_pending_activation;
  int limit;
#ifdef ENABLE_TRADITIONAL_ACTIVATION
  DBusError tmp_error;
  DBusString command;
  char **argv;
  char **envp = NULL;
  int argc;
  const char *servicehelper;
  DBusSpawnFlags flags = DBUS_SPAWN_NONE;
#endif

  _dbus_assert (activation != NULL);
  _dbus_assert (transaction != NULL);
  _dbus_assert (activation_message != NULL);
  _dbus_assert (service_name != NULL);
  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  limit = bus_context_get_max_pending_activations (activation->context);

  if (activation->n_pending_activations >= limit)
    {
      dbus_set_error (error, DBUS_ERROR_LIMITS_EXCEEDED,
                      "The maximum number of pending activations has been "
                      "reached, activation of %s failed "
                      "(max_pending_service_starts=%d)",
                      service_name, limit);
      return FALSE;
    }

  if (bus_context_get_systemd_activation (activation->context) &&
      strcmp (service_name, "org.freedesktop.systemd1") == 0)
    {
      /* if we're doing systemd activation, we assume systemd will connect
       * eventually, and it does not need a .service file */
      entry = NULL;
    }
  else
    {
      entry = activation_find_entry (activation, service_name, error);
      if (!entry)
        return FALSE;
    }

  if (auto_activation &&
      entry != NULL &&
      !bus_context_check_security_policy (activation->context,
        transaction,
        connection, /* sender */
        NULL, /* addressed recipient */
        NULL, /* proposed recipient */
        activation_message,
        entry,
        error))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      _dbus_verbose ("activation not authorized: %s: %s\n",
          error != NULL ? error->name : "(error ignored)",
          error != NULL ? error->message : "(error ignored)");
      return FALSE;
    }

  /* Bypass the registry lookup if we're auto-activating, bus_dispatch would not
   * call us if the service is already active.
   */
  if (!auto_activation)
    {
      /* Check if the service is active */
      _dbus_string_init_const (&service_str, service_name);
      if (bus_registry_lookup (bus_context_get_registry (activation->context), &service_str) != NULL)
        {
          dbus_uint32_t result;

          _dbus_verbose ("Service \"%s\" is already active\n", service_name);

          message = dbus_message_new_method_return (activation_message);

          if (!message)
            {
              _dbus_verbose ("No memory to create reply to activate message\n");
              BUS_SET_OOM (error);
              return FALSE;
            }

          result = DBUS_START_REPLY_ALREADY_RUNNING;

          if (!dbus_message_append_args (message,
                                         DBUS_TYPE_UINT32, &result,
                                         DBUS_TYPE_INVALID))
            {
              _dbus_verbose ("No memory to set args of reply to activate message\n");
              BUS_SET_OOM (error);
              dbus_message_unref (message);
              return FALSE;
            }

          retval = bus_transaction_send_from_driver (transaction, connection, message);
          dbus_message_unref (message);
          if (!retval)
            {
              _dbus_verbose ("Failed to send reply\n");
              BUS_SET_OOM (error);
            }

          return retval;
        }
    }

  pending_activation_entry = dbus_new0 (BusPendingActivationEntry, 1);
  if (!pending_activation_entry)
    {
      _dbus_verbose ("Failed to create pending activation entry\n");
      BUS_SET_OOM (error);
      return FALSE;
    }

  pending_activation_entry->auto_activation = auto_activation;

  pending_activation_entry->activation_message = activation_message;
  dbus_message_ref (activation_message);
  pending_activation_entry->connection = connection;
  if (connection)
    dbus_connection_ref (connection);

  /* Check if the service is being activated */
  pending_activation = _dbus_hash_table_lookup_string (activation->pending_activations, service_name);
  was_pending_activation = (pending_activation != NULL);
  if (was_pending_activation)
    {
      if (!_dbus_list_append (&pending_activation->entries, pending_activation_entry))
        {
          _dbus_verbose ("Failed to append a new entry to pending activation\n");

          BUS_SET_OOM (error);
          bus_pending_activation_entry_free (pending_activation_entry);
          return FALSE;
        }

      pending_activation->n_entries += 1;
      pending_activation->activation->n_pending_activations += 1;
    }
  else
    {
      pending_activation = dbus_new0 (BusPendingActivation, 1);
      if (!pending_activation)
        {
          _dbus_verbose ("Failed to create pending activation\n");

          BUS_SET_OOM (error);
          bus_pending_activation_entry_free (pending_activation_entry);
          return FALSE;
        }

      pending_activation->activation = activation;
      pending_activation->refcount = 1;

      pending_activation->service_name = _dbus_strdup (service_name);
      if (!pending_activation->service_name)
        {
          _dbus_verbose ("Failed to copy service name for pending activation\n");

          BUS_SET_OOM (error);
          bus_pending_activation_unref (pending_activation);
          bus_pending_activation_entry_free (pending_activation_entry);
          return FALSE;
        }

      if (entry != NULL)
        {
          pending_activation->exec = _dbus_strdup (entry->exec);
          if (!pending_activation->exec)
            {
              _dbus_verbose ("Failed to copy service exec for pending activation\n");
              BUS_SET_OOM (error);
              bus_pending_activation_unref (pending_activation);
              bus_pending_activation_entry_free (pending_activation_entry);
              return FALSE;
            }
        }

      if (entry != NULL && entry->systemd_service != NULL)
        {
          pending_activation->systemd_service = _dbus_strdup (entry->systemd_service);
          if (!pending_activation->systemd_service)
            {
              _dbus_verbose ("Failed to copy systemd service for pending activation\n");
              BUS_SET_OOM (error);
              bus_pending_activation_unref (pending_activation);
              bus_pending_activation_entry_free (pending_activation_entry);
              return FALSE;
            }
        }

      pending_activation->timeout =
        _dbus_timeout_new (bus_context_get_activation_timeout (activation->context),
                           pending_activation_timed_out,
                           pending_activation,
                           NULL);
      if (!pending_activation->timeout)
        {
          _dbus_verbose ("Failed to create timeout for pending activation\n");

          BUS_SET_OOM (error);
          bus_pending_activation_unref (pending_activation);
          bus_pending_activation_entry_free (pending_activation_entry);
          return FALSE;
        }

      if (!_dbus_loop_add_timeout (bus_context_get_loop (activation->context),
                                   pending_activation->timeout))
        {
          _dbus_verbose ("Failed to add timeout for pending activation\n");

          BUS_SET_OOM (error);
          bus_pending_activation_unref (pending_activation);
          bus_pending_activation_entry_free (pending_activation_entry);
          return FALSE;
        }

      pending_activation->timeout_added = TRUE;

      if (!_dbus_list_append (&pending_activation->entries, pending_activation_entry))
        {
          _dbus_verbose ("Failed to add entry to just-created pending activation\n");

          BUS_SET_OOM (error);
          bus_pending_activation_unref (pending_activation);
          bus_pending_activation_entry_free (pending_activation_entry);
          return FALSE;
        }

      pending_activation->n_entries += 1;
      pending_activation->activation->n_pending_activations += 1;

      if (!_dbus_hash_table_insert_string (activation->pending_activations,
                                           pending_activation->service_name,
                                           pending_activation))
        {
          _dbus_verbose ("Failed to put pending activation in hash table\n");

          BUS_SET_OOM (error);
          bus_pending_activation_unref (pending_activation);
          return FALSE;
        }
    }

  if (!add_cancel_pending_to_transaction (transaction, pending_activation))
    {
      _dbus_verbose ("Failed to add pending activation cancel hook to transaction\n");
      BUS_SET_OOM (error);
      goto cancel_pending_activation;
    }

  if (was_pending_activation)
    return TRUE;

  if (bus_context_get_systemd_activation (activation->context))
    {
      if (strcmp (service_name, "org.freedesktop.systemd1") == 0)
          /* systemd itself is missing apparently. That can happen
             only during early startup. Let's just wait until systemd
             connects to us and do nothing. */
        return TRUE;

      if (entry->systemd_service)
        {
          BusTransaction *activation_transaction;
          DBusString service_string;
          BusService *service;
          BusRegistry *registry;
          DBusConnection *systemd = NULL;

          /* OK, we have a systemd service configured for this entry,
             hence let's enqueue an activation request message. This
             is implemented as a directed signal, not a method call,
             for three reasons: 1) we don't expect a response on
             success, where we just expect a name appearing on the
             bus; 2) at this time the systemd service might not yet
             have connected, so we wouldn't know the message serial at
             this point to set up a pending call; 3) it is ugly if the
             bus suddenly becomes the caller of a remote method. */

          message = dbus_message_new_signal (DBUS_PATH_DBUS,
                                             "org.freedesktop.systemd1.Activator",
                                             "ActivationRequest");
          if (!message)
            {
              _dbus_verbose ("No memory to create activation message\n");
              BUS_SET_OOM (error);
              goto cancel_pending_activation;
            }

          if (!dbus_message_set_sender (message, DBUS_SERVICE_DBUS) ||
              !dbus_message_set_destination (message, "org.freedesktop.systemd1") ||
              !dbus_message_append_args (message,
                                         DBUS_TYPE_STRING, &entry->systemd_service,
                                         DBUS_TYPE_INVALID))
            {
              _dbus_verbose ("No memory to set args of activation message\n");
              dbus_message_unref (message);
              BUS_SET_OOM (error);
              goto cancel_pending_activation;
            }

          /* Create our transaction */
          activation_transaction = bus_transaction_new (activation->context);
          if (activation_transaction == NULL)
            {
              _dbus_verbose ("No memory to create activation transaction\n");
              dbus_message_unref (message);
              BUS_SET_OOM (error);
              goto cancel_pending_activation;
            }

          /* Check whether systemd is already connected */
          registry = bus_connection_get_registry (connection);
          _dbus_string_init_const (&service_string, "org.freedesktop.systemd1");
          service = bus_registry_lookup (registry, &service_string);

          if (service)
            systemd = bus_service_get_primary_owners_connection (service);

          /* Following the general principle of "log early and often",
           * we capture that we *want* to send the activation message, even if
           * systemd is not actually there to receive it yet */
          if (!bus_transaction_capture (activation_transaction,
                                        NULL, systemd, message))
            {
              dbus_message_unref (message);
              BUS_SET_OOM (error);
              goto cancel_pending_activation;
            }

          if (service != NULL)
            {
              bus_context_log (activation->context,
                               DBUS_SYSTEM_LOG_INFO, "Activating via systemd: service name='%s' unit='%s' requested by '%s' (%s)",
                               service_name,
                               entry->systemd_service,
                               bus_connection_get_name (connection),
                               bus_connection_get_loginfo (connection));
              /* Wonderful, systemd is connected, let's just send the msg */
              retval = bus_dispatch_matches (activation_transaction, NULL,
                                             systemd, message, error);
            }
          else
            {
              bus_context_log (activation->context,
                               DBUS_SYSTEM_LOG_INFO, "Activating systemd to hand-off: service name='%s' unit='%s' requested by '%s' (%s)",
                               service_name,
                               entry->systemd_service,
                               bus_connection_get_name (connection),
                               bus_connection_get_loginfo (connection));
              /* systemd is not around, let's "activate" it. */
              retval = bus_activation_activate_service (activation, NULL, activation_transaction, TRUE,
                                                        message, "org.freedesktop.systemd1", error);
            }

          dbus_message_unref (message);

          if (!retval)
            {
              bus_context_log (activation->context,
                               DBUS_SYSTEM_LOG_INFO, "Failed to activate via systemd: service name='%s' unit='%s'",
                               service_name,
                               entry->systemd_service);
              _DBUS_ASSERT_ERROR_IS_SET (error);
              _dbus_verbose ("failed to send activation message: %s\n", error->name);
              bus_transaction_cancel_and_free (activation_transaction);
              goto cancel_pending_activation;
            }

          bus_transaction_execute_and_free (activation_transaction);
          return TRUE;
        }

      /* OK, we have no configured systemd service, hence let's
         proceed with traditional activation. */
    }

#ifdef ENABLE_TRADITIONAL_ACTIVATION
  /* If entry was NULL, it would be because we were doing systemd activation
   * and activating systemd itself; but we already handled that case with
   * an early-return */
  _dbus_assert (entry != NULL);

  /* use command as system and session different */
  if (!_dbus_string_init (&command))
    {
      BUS_SET_OOM (error);
      goto cancel_pending_activation;
    }

  /* does the bus use a helper? */
  servicehelper = bus_context_get_servicehelper (activation->context);
  if (servicehelper != NULL)
    {
      if (entry->user == NULL)
        {
          _dbus_string_free (&command);
          dbus_set_error (error, DBUS_ERROR_SPAWN_FILE_INVALID,
                          "Cannot do system-bus activation with no user\n");
          goto cancel_pending_activation;
        }

      /* join the helper path and the service name */
      if (!_dbus_string_append (&command, servicehelper))
        {
          _dbus_string_free (&command);
          BUS_SET_OOM (error);
          goto cancel_pending_activation;
        }
      if (!_dbus_string_append (&command, " "))
        {
          _dbus_string_free (&command);
          BUS_SET_OOM (error);
          goto cancel_pending_activation;
        }
      if (!_dbus_string_append (&command, service_name))
        {
          _dbus_string_free (&command);
          BUS_SET_OOM (error);
          goto cancel_pending_activation;
        }
    }
  else
    {
      /* the bus does not use a helper, so we can append arguments with the exec line */
      if (!_dbus_string_append (&command, entry->exec))
        {
          _dbus_string_free (&command);
          BUS_SET_OOM (error);
          goto cancel_pending_activation;
        }
    }

  /* convert command into arguments */
  if (!_dbus_shell_parse_argv (_dbus_string_get_const_data (&command), &argc, &argv, error))
    {
      _dbus_verbose ("Failed to parse command line: %s\n", entry->exec);
      _DBUS_ASSERT_ERROR_IS_SET (error);
      _dbus_string_free (&command);
      goto cancel_pending_activation;
    }
  _dbus_string_free (&command);

  if (!add_bus_environment (activation, error))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      dbus_free_string_array (argv);
      goto cancel_pending_activation;
    }

  envp = bus_activation_get_environment (activation);

  if (envp == NULL)
    {
      BUS_SET_OOM (error);
      dbus_free_string_array (argv);
      goto cancel_pending_activation;
    }

  _dbus_verbose ("Spawning %s ...\n", argv[0]);
  if (servicehelper != NULL)
    bus_context_log (activation->context,
                     DBUS_SYSTEM_LOG_INFO, "Activating service name='%s' requested by '%s' (%s) (using servicehelper)",
                     service_name,
                     bus_connection_get_name (connection),
                     bus_connection_get_loginfo (connection));
  else
    bus_context_log (activation->context,
                     DBUS_SYSTEM_LOG_INFO, "Activating service name='%s' requested by '%s' (%s)",
                     service_name,
                     bus_connection_get_name (connection),
                     bus_connection_get_loginfo (connection));

  dbus_error_init (&tmp_error);

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
  if (bus_context_get_quiet_log (activation->context))
    flags |= DBUS_SPAWN_SILENCE_OUTPUT;
#endif

  if (bus_context_get_using_syslog (activation->context))
    flags |= DBUS_SPAWN_REDIRECT_OUTPUT;

  if (!_dbus_spawn_async_with_babysitter (&pending_activation->babysitter,
                                          service_name,
                                          argv,
                                          envp,
                                          flags,
                                          child_setup,
                                          activation,
                                          &tmp_error))
    {
      _dbus_verbose ("Failed to spawn child\n");
      bus_context_log (activation->context,
                       DBUS_SYSTEM_LOG_INFO, "Failed to activate service %s: %s",
                       service_name,
                       tmp_error.message);
      _DBUS_ASSERT_ERROR_IS_SET (&tmp_error);
      dbus_move_error (&tmp_error, error);
      dbus_free_string_array (argv);
      dbus_free_string_array (envp);
      goto cancel_pending_activation;
    }

  dbus_free_string_array (argv);
  dbus_free_string_array (envp);
  envp = NULL;

  _dbus_assert (pending_activation->babysitter != NULL);

  _dbus_babysitter_set_result_function (pending_activation->babysitter,
                                        pending_activation_finished_cb,
                                        pending_activation);

  if (!_dbus_babysitter_set_watch_functions (pending_activation->babysitter,
                                             add_babysitter_watch,
                                             remove_babysitter_watch,
                                             toggle_babysitter_watch,
                                             pending_activation,
                                             NULL))
    {
      BUS_SET_OOM (error);
      _dbus_verbose ("Failed to set babysitter watch functions\n");
      goto cancel_pending_activation;
    }

  return TRUE;
#else /* !TRADITIONAL_ACTIVATION */
    bus_context_log (activation->context,
                     DBUS_SYSTEM_LOG_INFO, "Cannot activate service name='%s' requested by '%s' (%s): SystemdService not configured and dbus was compiled with --disable-traditional-activation",
                     service_name,
                     bus_connection_get_name (connection),
                     bus_connection_get_loginfo (connection));
#endif
cancel_pending_activation:
  _DBUS_ASSERT_ERROR_IS_SET (error);
  _dbus_hash_table_remove_string (activation->pending_activations,
                                  pending_activation->service_name);
  return FALSE;
}

dbus_bool_t
bus_activation_list_services (BusActivation *activation,
			      char        ***listp,
			      int           *array_len)
{
  int i, j, len;
  char **retval;
  DBusHashIter iter;

  len = _dbus_hash_table_get_n_entries (activation->entries);
  retval = dbus_new (char *, len + 1);

  if (retval == NULL)
    return FALSE;

  _dbus_hash_iter_init (activation->entries, &iter);
  i = 0;
  while (_dbus_hash_iter_next (&iter))
    {
      BusActivationEntry *entry = _dbus_hash_iter_get_value (&iter);

      retval[i] = _dbus_strdup (entry->name);
      if (retval[i] == NULL)
	goto error;

      i++;
    }

  retval[i] = NULL;

  if (array_len)
    *array_len = len;

  *listp = retval;
  return TRUE;

 error:
  for (j = 0; j < i; j++)
    dbus_free (retval[j]);
  dbus_free (retval);

  return FALSE;
}

dbus_bool_t
dbus_activation_systemd_failure (BusActivation *activation,
                                 DBusMessage   *message)
{
  DBusError error;
  const char *code, *str, *unit = NULL;

  dbus_error_init(&error);

  /* This is called whenever the systemd activator sent us a
     response. We'll invalidate all pending activations that match the
     unit name. */

  if (dbus_message_get_args (message, &error,
                             DBUS_TYPE_STRING, &unit,
                             DBUS_TYPE_STRING, &code,
                             DBUS_TYPE_STRING, &str,
                             DBUS_TYPE_INVALID))
    dbus_set_error (&error, code, "%s", str);


  if (unit)
    {
      DBusHashIter iter;

      bus_context_log (activation->context,
                       DBUS_SYSTEM_LOG_INFO, "Activation via systemd failed for unit '%s': %s",
                       unit,
                       str);

      _dbus_hash_iter_init (activation->pending_activations,
                            &iter);

      while (_dbus_hash_iter_next (&iter))
        {
          BusPendingActivation *p = _dbus_hash_iter_get_value (&iter);

          if (p->systemd_service && strcmp (p->systemd_service, unit) == 0)
            pending_activation_failed(p, &error);
        }
    }

  dbus_error_free(&error);

  return TRUE;
}

const char *
bus_activation_entry_get_assumed_apparmor_label (BusActivationEntry *entry)
{
  return entry->assumed_apparmor_label;
}

#ifdef DBUS_ENABLE_EMBEDDED_TESTS

#include <stdio.h>

#define SERVICE_NAME_1 "com.example.MyService1"
#define SERVICE_NAME_2 "org.example.MyService2"
#define SERVICE_NAME_3 "net.example.MyService3"

#define SERVICE_FILE_1 "service-1.service"
#define SERVICE_FILE_2 "service-2.service"
#define SERVICE_FILE_3 "service-3.service"

static dbus_bool_t
test_create_service_file (DBusString *dir,
                          const char *filename,
                          const char *name,
                          const char *exec)
{
  DBusString  file_name, full_path;
  FILE        *file;
  dbus_bool_t  ret_val;

  ret_val = TRUE;
  _dbus_string_init_const (&file_name, filename);

  if (!_dbus_string_init (&full_path))
    return FALSE;

  if (!_dbus_string_append (&full_path, _dbus_string_get_const_data (dir)) ||
      !_dbus_concat_dir_and_file (&full_path, &file_name))
    {
      ret_val = FALSE;
      goto out;
    }

  file = fopen (_dbus_string_get_const_data (&full_path), "w");
  if (!file)
    {
      ret_val = FALSE;
      goto out;
    }

  fprintf (file, "[D-BUS Service]\nName=%s\nExec=%s\n", name, exec);
  fclose (file);

out:
  _dbus_string_free (&full_path);
  return ret_val;
}

static dbus_bool_t
test_remove_service_file (DBusString *dir, const char *filename)
{
  DBusString  file_name, full_path;
  dbus_bool_t ret_val;

  ret_val = TRUE;

  _dbus_string_init_const (&file_name, filename);

  if (!_dbus_string_init (&full_path))
    return FALSE;

  if (!_dbus_string_append (&full_path, _dbus_string_get_const_data (dir)) ||
      !_dbus_concat_dir_and_file (&full_path, &file_name))
    {
      ret_val = FALSE;
      goto out;
    }

  if (!_dbus_delete_file (&full_path, NULL))
    {
      ret_val = FALSE;
      goto out;
    }

out:
  _dbus_string_free (&full_path);
  return ret_val;
}

static dbus_bool_t
test_remove_directory (DBusString *dir)
{
  DBusDirIter *iter;
  DBusString   filename, full_path;
  dbus_bool_t  ret_val;

  ret_val = TRUE;

  if (!_dbus_string_init (&filename))
    return FALSE;

  if (!_dbus_string_init (&full_path))
    {
      _dbus_string_free (&filename);
      return FALSE;
    }

  iter = _dbus_directory_open (dir, NULL);
  if (iter == NULL)
    {
      ret_val = FALSE;
      goto out;
    }

  while (_dbus_directory_get_next_file (iter, &filename, NULL))
    {
      if (!test_remove_service_file (dir, _dbus_string_get_const_data (&filename)))
        {
          ret_val = FALSE;
          _dbus_directory_close (iter);
          goto out;
        }
    }
  _dbus_directory_close (iter);

  if (!_dbus_delete_directory (dir, NULL))
    {
      ret_val = FALSE;
      goto out;
    }

out:
  _dbus_string_free (&filename);
  _dbus_string_free (&full_path);

  return ret_val;
}

static dbus_bool_t
init_service_reload_test (DBusString *dir)
{
  if (!_dbus_create_directory (dir, NULL))
    return FALSE;

  /* Create one initial file */
  if (!test_create_service_file (dir, SERVICE_FILE_1, SERVICE_NAME_1, "exec-1"))
    return FALSE;

  return TRUE;
}

static dbus_bool_t
cleanup_service_reload_test (DBusString *dir)
{
  if (!test_remove_directory (dir))
    return FALSE;

  return TRUE;
}

typedef struct
{
  BusActivation *activation;
  const char    *service_name;
  dbus_bool_t    expecting_find;
} CheckData;

static dbus_bool_t
check_func (void        *data,
            dbus_bool_t  have_memory)
{
  CheckData          *d;
  BusActivationEntry *entry;
  DBusError           error;
  dbus_bool_t         ret_val;

  ret_val = TRUE;
  d = data;

  dbus_error_init (&error);

  entry = activation_find_entry (d->activation, d->service_name, &error);
  if (entry == NULL)
    {
      if (dbus_error_has_name (&error, DBUS_ERROR_NO_MEMORY))
        {
          ret_val = TRUE;
        }
      else
        {
          if (d->expecting_find)
            ret_val = FALSE;
        }

      dbus_error_free (&error);
    }
  else
    {
      if (!d->expecting_find)
        ret_val = FALSE;
    }

  return ret_val;
}

static dbus_bool_t
do_test (const char *description, dbus_bool_t oom_test, CheckData *data)
{
  dbus_bool_t err;

  if (oom_test)
    err = !_dbus_test_oom_handling (description, check_func, data);
  else
    err = !check_func (data, TRUE);

  if (err)
    _dbus_test_fatal ("Test failed");

  return TRUE;
}

static dbus_bool_t
do_service_reload_test (const DBusString *test_data_dir,
                        DBusString       *dir,
                        dbus_bool_t       oom_test)
{
  BusActivation *activation;
  BusConfigServiceDir config;
  BusContext    *context;
  DBusString     address;
  DBusList      *directories;
  CheckData      d;

  directories = NULL;
  _dbus_string_init_const (&address, "");

  config.path = _dbus_string_get_data (dir);
  config.flags = BUS_SERVICE_DIR_FLAGS_NONE;

  if (!_dbus_list_append (&directories, &config))
    return FALSE;

  context = bus_context_new_test (test_data_dir,
                                  "valid-config-files/debug-allow-all.conf");
  if (context == NULL)
    return FALSE;

  activation = bus_activation_new (context, &address, &directories, NULL);
  if (!activation)
    return FALSE;

  d.activation = activation;

  /* Check for existing service file */
  d.expecting_find = TRUE;
  d.service_name = SERVICE_NAME_1;

  if (!do_test ("Existing service file", oom_test, &d))
    return FALSE;

  /* Check for non-existing service file */
  d.expecting_find = FALSE;
  d.service_name = SERVICE_NAME_3;

  if (!do_test ("Nonexisting service file", oom_test, &d))
    return FALSE;

  /* Check for added service file */
  if (!test_create_service_file (dir, SERVICE_FILE_2, SERVICE_NAME_2, "exec-2"))
    return FALSE;

  d.expecting_find = TRUE;
  d.service_name = SERVICE_NAME_2;

  if (!do_test ("Added service file", oom_test, &d))
    return FALSE;

  /* Check for removed service file */
  if (!test_remove_service_file (dir, SERVICE_FILE_2))
    return FALSE;

  d.expecting_find = FALSE;
  d.service_name = SERVICE_FILE_2;

  if (!do_test ("Removed service file", oom_test, &d))
    return FALSE;

  /* Check for updated service file */

  _dbus_sleep_milliseconds (1000); /* Sleep a second to make sure the mtime is updated */

  if (!test_create_service_file (dir, SERVICE_FILE_1, SERVICE_NAME_3, "exec-3"))
    return FALSE;

  d.expecting_find = TRUE;
  d.service_name = SERVICE_NAME_3;

  if (!do_test ("Updated service file, part 1", oom_test, &d))
    return FALSE;

  d.expecting_find = FALSE;
  d.service_name = SERVICE_NAME_1;

  if (!do_test ("Updated service file, part 2", oom_test, &d))
    return FALSE;

  bus_activation_unref (activation);
  _dbus_list_clear (&directories);
  bus_context_unref (context);

  return TRUE;
}

dbus_bool_t
bus_activation_service_reload_test (const char *test_data_dir_cstr)
{
  DBusString test_data_dir;
  DBusString directory;
  const char *tmp;
  dbus_bool_t ret = FALSE;

  _dbus_string_init_const (&test_data_dir, test_data_dir_cstr);

  if (!_dbus_string_init (&directory))
    return FALSE;

  tmp = _dbus_get_tmpdir ();

  if (tmp == NULL)
    goto out;

  if (!_dbus_string_append (&directory, tmp))
    goto out;

  if (!_dbus_string_append (&directory, "/dbus-reload-test-") ||
      !_dbus_generate_random_ascii (&directory, 6, NULL))
    goto out;

  /* Do normal tests */
  if (!init_service_reload_test (&directory))
    _dbus_test_fatal ("could not initiate service reload test");

  if (!do_service_reload_test (&test_data_dir, &directory, FALSE))
    {
      /* Do nothing? */
    }

  if (!cleanup_service_reload_test (&directory))
    goto out;

  /* Do OOM tests */
  if (!init_service_reload_test (&directory))
    _dbus_test_fatal ("could not initiate service reload test");

  if (!do_service_reload_test (&test_data_dir, &directory, TRUE))
    {
      /* Do nothing? */
    }

  /* Cleanup test directory */
  if (!cleanup_service_reload_test (&directory))
    goto out;

  ret = TRUE;

out:
  _dbus_string_free (&directory);
  return ret;
}

#endif /* DBUS_ENABLE_EMBEDDED_TESTS */
