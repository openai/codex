/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* bus.c  message bus context object
 *
 * Copyright (C) 2003, 2004 Red Hat, Inc.
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

#include <stdio.h>
#include <stdlib.h>

#include "activation.h"
#include "connection.h"
#include "containers.h"
#include "dispatch.h"
#include "services.h"
#include "utils.h"
#include "policy.h"
#include "config-parser.h"
#include "signals.h"
#include "selinux.h"
#include "apparmor.h"
#include "audit.h"
#include "dir-watch.h"
#include <dbus/dbus-auth.h>
#include <dbus/dbus-list.h>
#include <dbus/dbus-hash.h>
#include <dbus/dbus-credentials.h>
#include <dbus/dbus-internals.h>
#include <dbus/dbus-server-protected.h>

#ifdef DBUS_CYGWIN
#include <signal.h>
#endif

struct BusContext
{
  int refcount;
  DBusGUID uuid;
  char *config_file;
  char *type;
  char *servicehelper;
  char *address;
  char *pidfile;
  char *user;
  char *log_prefix;
  DBusLoop *loop;
  DBusList *servers;
  BusConnections *connections;
  BusActivation *activation;
  BusRegistry *registry;
  BusPolicy *policy;
  BusMatchmaker *matchmaker;
  BusLimits limits;
  DBusRLimit *initial_fd_limit;
  BusContainers *containers;
  unsigned int fork : 1;
  unsigned int syslog : 1;
  unsigned int keep_umask : 1;
  unsigned int allow_anonymous : 1;
  unsigned int systemd_activation : 1;
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
  unsigned int quiet_log : 1;
#endif
  dbus_bool_t watches_enabled;
};

static dbus_int32_t server_data_slot = -1;

typedef struct
{
  BusContext *context;
} BusServerData;

#define BUS_SERVER_DATA(server) (dbus_server_get_data ((server), server_data_slot))

static BusContext*
server_get_context (DBusServer *server)
{
  BusContext *context;
  BusServerData *bd;

  /* this data slot was allocated by the BusContext */
  _dbus_assert (server_data_slot >= 0);

  bd = BUS_SERVER_DATA (server);

  /* every DBusServer in the dbus-daemon's main loop has gone through
   * bus_context_setup_server() */
  _dbus_assert (bd != NULL);

  context = bd->context;

  return context;
}

static dbus_bool_t
add_server_watch (DBusWatch  *watch,
                  void       *data)
{
  DBusServer *server = data;
  BusContext *context;

  context = server_get_context (server);

  return _dbus_loop_add_watch (context->loop, watch);
}

static void
remove_server_watch (DBusWatch  *watch,
                     void       *data)
{
  DBusServer *server = data;
  BusContext *context;

  context = server_get_context (server);

  _dbus_loop_remove_watch (context->loop, watch);
}

static void
toggle_server_watch (DBusWatch  *watch,
                     void       *data)
{
  DBusServer *server = data;
  BusContext *context;

  context = server_get_context (server);

  _dbus_loop_toggle_watch (context->loop, watch);
}

static dbus_bool_t
add_server_timeout (DBusTimeout *timeout,
                    void        *data)
{
  DBusServer *server = data;
  BusContext *context;

  context = server_get_context (server);

  return _dbus_loop_add_timeout (context->loop, timeout);
}

static void
remove_server_timeout (DBusTimeout *timeout,
                       void        *data)
{
  DBusServer *server = data;
  BusContext *context;

  context = server_get_context (server);

  _dbus_loop_remove_timeout (context->loop, timeout);
}

static void
new_connection_callback (DBusServer     *server,
                         DBusConnection *new_connection,
                         void           *data)
{
  /* If this fails it logs a warning, so we don't need to do that */
  bus_context_add_incoming_connection (data, new_connection);
}

dbus_bool_t
bus_context_add_incoming_connection (BusContext *context,
                                     DBusConnection *new_connection)
{
  /* If this fails it logs a warning, so we don't need to do that */
  if (!bus_connections_setup_connection (context->connections, new_connection))
    {
      /* if we don't do this, it will get unref'd without
       * being disconnected... kind of strange really
       * that we have to do this, people won't get it right
       * in general.
       */
      dbus_connection_close (new_connection);
      /* on OOM, we won't have ref'd the connection so it will die. */
      return FALSE;
    }

  dbus_connection_set_max_received_size (new_connection,
                                         context->limits.max_incoming_bytes);

  dbus_connection_set_max_message_size (new_connection,
                                        context->limits.max_message_size);

  dbus_connection_set_max_received_unix_fds (new_connection,
                                         context->limits.max_incoming_unix_fds);

  dbus_connection_set_max_message_unix_fds (new_connection,
                                        context->limits.max_message_unix_fds);

  dbus_connection_set_allow_anonymous (new_connection,
                                       context->allow_anonymous);

  return TRUE;
}

static void
free_server_data (void *data)
{
  BusServerData *bd = data;

  dbus_free (bd);
}

static dbus_bool_t
setup_server (BusContext *context,
              DBusServer *server,
              char      **auth_mechanisms,
              DBusError  *error)
{
  if (!bus_context_setup_server (context, server, error))
    return FALSE;

  if (!dbus_server_set_auth_mechanisms (server, (const char**) auth_mechanisms))
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  dbus_server_set_new_connection_function (server, new_connection_callback,
                                           context, NULL);
  return TRUE;
}

dbus_bool_t
bus_context_setup_server (BusContext                 *context,
                          DBusServer                 *server,
                          DBusError                  *error)
{
  BusServerData *bd;

  bd = dbus_new0 (BusServerData, 1);
  if (bd == NULL || !dbus_server_set_data (server,
                                           server_data_slot,
                                           bd, free_server_data))
    {
      dbus_free (bd);
      BUS_SET_OOM (error);
      return FALSE;
    }

  bd->context = context;

  if (!dbus_server_set_watch_functions (server,
                                        add_server_watch,
                                        remove_server_watch,
                                        toggle_server_watch,
                                        server,
                                        NULL))
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  if (!dbus_server_set_timeout_functions (server,
                                          add_server_timeout,
                                          remove_server_timeout,
                                          NULL,
                                          server, NULL))
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  return TRUE;
}

/* This code only gets executed the first time the
 * config files are parsed.  It is not executed
 * when config files are reloaded.
 */
static dbus_bool_t
process_config_first_time_only (BusContext       *context,
				BusConfigParser  *parser,
                                const DBusString *address,
                                BusContextFlags   flags,
				DBusError        *error)
{
  DBusString log_prefix;
  DBusList *link;
  DBusList **addresses;
  const char *user, *pidfile;
  char **auth_mechanisms;
  DBusList **auth_mechanisms_list;
  int len;
  dbus_bool_t retval;
  DBusLogFlags log_flags = DBUS_LOG_FLAGS_STDERR;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  retval = FALSE;
  auth_mechanisms = NULL;
  pidfile = NULL;

  if (flags & BUS_CONTEXT_FLAG_SYSLOG_ALWAYS)
    {
      context->syslog = TRUE;
      log_flags |= DBUS_LOG_FLAGS_SYSTEM_LOG;

      if (flags & BUS_CONTEXT_FLAG_SYSLOG_ONLY)
        log_flags &= ~DBUS_LOG_FLAGS_STDERR;
    }
  else if (flags & BUS_CONTEXT_FLAG_SYSLOG_NEVER)
    {
      context->syslog = FALSE;
    }
  else
    {
      context->syslog = bus_config_parser_get_syslog (parser);

      if (context->syslog)
        log_flags |= DBUS_LOG_FLAGS_SYSTEM_LOG;
    }

  _dbus_init_system_log ("dbus-daemon", log_flags);

  if (flags & BUS_CONTEXT_FLAG_SYSTEMD_ACTIVATION)
    context->systemd_activation = TRUE;
  else
    context->systemd_activation = FALSE;

  /* Check for an existing pid file. Of course this is a race;
   * we'd have to use fcntl() locks on the pid file to
   * avoid that. But we want to check for the pid file
   * before overwriting any existing sockets, etc.
   */

  if (flags & BUS_CONTEXT_FLAG_WRITE_PID_FILE)
    pidfile = bus_config_parser_get_pidfile (parser);

  if (pidfile != NULL)
    {
      DBusString u;
      DBusStat stbuf;

      _dbus_string_init_const (&u, pidfile);

      if (_dbus_stat (&u, &stbuf, NULL))
        {
#ifdef DBUS_CYGWIN
          DBusString p;
          long /* int */ pid;

          _dbus_string_init (&p);
          _dbus_file_get_contents(&p, &u, NULL);
          _dbus_string_parse_int(&p, 0, &pid, NULL);
          _dbus_string_free(&p);

          if ((kill((int)pid, 0))) {
            dbus_set_error(NULL, DBUS_ERROR_FILE_EXISTS,
                           "pid %ld not running, removing stale pid file\n",
                           pid);
            _dbus_delete_file(&u, NULL);
          } else {
#endif
          dbus_set_error (error, DBUS_ERROR_FAILED,
		                  "The pid file \"%s\" exists, if the message bus is not running, remove this file",
                          pidfile);
	      goto failed;
#ifdef DBUS_CYGWIN
          }
#endif
        }
    }

  /* keep around the pid filename so we can delete it later */
  context->pidfile = _dbus_strdup (pidfile);

  /* note that type may be NULL */
  context->type = _dbus_strdup (bus_config_parser_get_type (parser));
  if (bus_config_parser_get_type (parser) != NULL && context->type == NULL)
    goto oom;

  user = bus_config_parser_get_user (parser);
  if (user != NULL)
    {
      context->user = _dbus_strdup (user);
      if (context->user == NULL)
        goto oom;
    }

  /* Set up the prefix for syslog messages */
  if (!_dbus_string_init (&log_prefix))
    goto oom;
  if (context->type && !strcmp (context->type, "system"))
    {
      if (!_dbus_string_append (&log_prefix, "[system] "))
        goto oom;
    }
  else if (context->type && !strcmp (context->type, "session"))
    {
      DBusCredentials *credentials;

      credentials = _dbus_credentials_new_from_current_process ();
      if (!credentials)
        goto oom;
      if (!_dbus_string_append (&log_prefix, "[session "))
        {
          _dbus_credentials_unref (credentials);
          goto oom;
        }
      if (!_dbus_credentials_to_string_append (credentials, &log_prefix))
        {
          _dbus_credentials_unref (credentials);
          goto oom;
        }
      if (!_dbus_string_append (&log_prefix, "] "))
        {
          _dbus_credentials_unref (credentials);
          goto oom;
        }
      _dbus_credentials_unref (credentials);
    }
  if (!_dbus_string_steal_data (&log_prefix, &context->log_prefix))
    goto oom;
  _dbus_string_free (&log_prefix);

  /* Build an array of auth mechanisms */

  auth_mechanisms_list = bus_config_parser_get_mechanisms (parser);
  len = _dbus_list_get_length (auth_mechanisms_list);

  if (len > 0)
    {
      int i;

      auth_mechanisms = dbus_new0 (char*, len + 1);
      if (auth_mechanisms == NULL)
        goto oom;

      i = 0;
      link = _dbus_list_get_first_link (auth_mechanisms_list);
      while (link != NULL)
        {
          DBusString name;
          _dbus_string_init_const (&name, link->data);
          if (!_dbus_auth_is_supported_mechanism (&name))
            {
              DBusString list;
              if (!_dbus_string_init (&list))
                goto oom;

              if (!_dbus_auth_dump_supported_mechanisms (&list))
                {
                  _dbus_string_free (&list);
                  goto oom;
                }
              dbus_set_error (error, DBUS_ERROR_FAILED,
                              "Unsupported auth mechanism \"%s\" in bus config file detected. Supported mechanisms are \"%s\".",
                              (char*)link->data,
                              _dbus_string_get_const_data (&list));
              _dbus_string_free (&list);
              goto failed;
            }
          auth_mechanisms[i] = _dbus_strdup (link->data);
          if (auth_mechanisms[i] == NULL)
            goto oom;
          link = _dbus_list_get_next_link (auth_mechanisms_list, link);
          i += 1;
        }
    }
  else
    {
      auth_mechanisms = NULL;
    }

  /* Listen on our addresses */

  if (address)
    {
      DBusServer *server;

      server = dbus_server_listen (_dbus_string_get_const_data(address), error);
      if (server == NULL)
        {
          _DBUS_ASSERT_ERROR_IS_SET (error);
          goto failed;
        }
      else if (!setup_server (context, server, auth_mechanisms, error))
        {
          _DBUS_ASSERT_ERROR_IS_SET (error);
          goto failed;
        }

      if (!_dbus_list_append (&context->servers, server))
        goto oom;
    }
  else
    {
      addresses = bus_config_parser_get_addresses (parser);

      link = _dbus_list_get_first_link (addresses);
      while (link != NULL)
        {
          DBusServer *server;

          server = dbus_server_listen (link->data, error);
          if (server == NULL)
            {
              _DBUS_ASSERT_ERROR_IS_SET (error);
              goto failed;
            }
          else if (!setup_server (context, server, auth_mechanisms, error))
            {
              _DBUS_ASSERT_ERROR_IS_SET (error);
              goto failed;
            }

          if (!_dbus_list_append (&context->servers, server))
            goto oom;

          link = _dbus_list_get_next_link (addresses, link);
        }
    }

  context->fork = bus_config_parser_get_fork (parser);
  context->keep_umask = bus_config_parser_get_keep_umask (parser);
  context->allow_anonymous = bus_config_parser_get_allow_anonymous (parser);

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  retval = TRUE;

 failed:
  dbus_free_string_array (auth_mechanisms);
  return retval;

 oom:
  BUS_SET_OOM (error);
  dbus_free_string_array (auth_mechanisms);
  return FALSE;
}

/* This code gets executed every time the config files
 * are parsed: both during BusContext construction
 * and on reloads. This function is slightly screwy
 * since it can do a "half reload" in out-of-memory
 * situations. Realistically, unlikely to ever matter.
 */
static dbus_bool_t
process_config_every_time (BusContext      *context,
			   BusConfigParser *parser,
			   dbus_bool_t      is_reload,
			   DBusError       *error)
{
  DBusString full_address;
  DBusList *link;
  DBusList **dirs;
  char *addr;
  const char *servicehelper;
  char *s;

  dbus_bool_t retval;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  addr = NULL;
  retval = FALSE;

  if (!_dbus_string_init (&full_address))
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  /* get our limits and timeout lengths */
  bus_config_parser_get_limits (parser, &context->limits);

  if (context->policy)
    bus_policy_unref (context->policy);
  context->policy = bus_config_parser_steal_policy (parser);
  _dbus_assert (context->policy != NULL);

  /* context->connections is NULL when creating new BusContext */
  if (context->connections)
    {
      _dbus_verbose ("Reload policy rules for completed connections\n");
      retval = bus_connections_reload_policy (context->connections, error);
      if (!retval)
        {
          _DBUS_ASSERT_ERROR_IS_SET (error);
          goto failed;
        }
    }

  /* We have to build the address backward, so that
   * <listen> later in the config file have priority
   */
  link = _dbus_list_get_last_link (&context->servers);
  while (link != NULL)
    {
      addr = dbus_server_get_address (link->data);
      if (addr == NULL)
        {
          BUS_SET_OOM (error);
          goto failed;
        }

      if (_dbus_string_get_length (&full_address) > 0)
        {
          if (!_dbus_string_append (&full_address, ";"))
            {
              BUS_SET_OOM (error);
              goto failed;
            }
        }

      if (!_dbus_string_append (&full_address, addr))
        {
          BUS_SET_OOM (error);
          goto failed;
        }

      dbus_free (addr);
      addr = NULL;

      link = _dbus_list_get_prev_link (&context->servers, link);
    }

  if (is_reload)
    dbus_free (context->address);

  if (!_dbus_string_copy_data (&full_address, &context->address))
    {
      BUS_SET_OOM (error);
      goto failed;
    }

  /* get the service directories */
  dirs = bus_config_parser_get_service_dirs (parser);

  /* and the service helper */
  servicehelper = bus_config_parser_get_servicehelper (parser);

  s = _dbus_strdup(servicehelper);
  if (s == NULL && servicehelper != NULL)
    {
      BUS_SET_OOM (error);
      goto failed;
    }
  else
    {
      dbus_free(context->servicehelper);
      context->servicehelper = s;
    }

  /* Create activation subsystem */
  if (context->activation)
    {
      if (!bus_activation_reload (context->activation, &full_address, dirs, error))
        goto failed;
    }
  else
    {
      context->activation = bus_activation_new (context, &full_address, dirs, error);
    }

  if (context->activation == NULL)
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      goto failed;
    }

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  retval = TRUE;

 failed:
  _dbus_string_free (&full_address);

  if (addr)
    dbus_free (addr);

  return retval;
}

static void
raise_file_descriptor_limit (BusContext      *context)
{
#ifdef DBUS_UNIX
  DBusError error = DBUS_ERROR_INIT;

  /* we only do this once */
  if (context->initial_fd_limit != NULL)
    return;

  context->initial_fd_limit = _dbus_rlimit_save_fd_limit (&error);

  if (context->initial_fd_limit == NULL)
    {
      bus_context_log (context, DBUS_SYSTEM_LOG_WARNING,
                       "%s: %s", error.name, error.message);
      dbus_error_free (&error);
      return;
    }

  /* We used to compute a suitable rlimit based on the configured number
   * of connections, but that breaks down as soon as we allow fd-passing,
   * because each connection is allowed to pass 64 fds to us, and if
   * they all did, we'd hit kernel limits. We now hard-code a good
   * limit that is enough to avoid DoS from anything short of multiple
   * uids conspiring against us, much like systemd does.
   */
  if (!_dbus_rlimit_raise_fd_limit (&error))
    {
      bus_context_log (context, DBUS_SYSTEM_LOG_WARNING,
                       "%s: %s", error.name, error.message);
      dbus_error_free (&error);
      return;
    }
#endif
}

static dbus_bool_t
process_config_postinit (BusContext      *context,
			 BusConfigParser *parser,
			 DBusError       *error)
{
  DBusHashTable *service_context_table;
  DBusList *watched_dirs = NULL;

  service_context_table = bus_config_parser_steal_service_context_table (parser);
  if (!bus_registry_set_service_context_table (context->registry,
					       service_context_table))
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  _dbus_hash_table_unref (service_context_table);

  /* We need to monitor both the configuration directories and directories
   * containing .service files.
   */
  if (!bus_config_parser_get_watched_dirs (parser, &watched_dirs))
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  bus_set_watched_dirs (context, &watched_dirs);

  _dbus_list_clear (&watched_dirs);

  return TRUE;
}

/* Takes ownership of print_addr_pipe fds, print_pid_pipe fds and ready_event_handle */
BusContext*
bus_context_new (const DBusString *config_file,
                 BusContextFlags   flags,
                 DBusPipe         *print_addr_pipe,
                 DBusPipe         *print_pid_pipe,
                 void             *ready_event_handle,
                 const DBusString *address,
                 DBusError        *error)
{
  BusContext *context;
  BusConfigParser *parser;

  _dbus_assert ((flags & BUS_CONTEXT_FLAG_FORK_NEVER) == 0 ||
                (flags & BUS_CONTEXT_FLAG_FORK_ALWAYS) == 0);

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  context = NULL;
  parser = NULL;

  if (!dbus_server_allocate_data_slot (&server_data_slot))
    {
      BUS_SET_OOM (error);
      return NULL;
    }

  context = dbus_new0 (BusContext, 1);
  if (context == NULL)
    {
      BUS_SET_OOM (error);
      goto failed;
    }
  context->refcount = 1;

  if (!_dbus_generate_uuid (&context->uuid, error))
    goto failed;

  if (!_dbus_string_copy_data (config_file, &context->config_file))
    {
      BUS_SET_OOM (error);
      goto failed;
    }

  context->loop = _dbus_loop_new ();
  if (context->loop == NULL)
    {
      BUS_SET_OOM (error);
      goto failed;
    }

  context->watches_enabled = TRUE;

  context->registry = bus_registry_new (context);
  if (context->registry == NULL)
    {
      BUS_SET_OOM (error);
      goto failed;
    }

  parser = bus_config_load (config_file, TRUE, NULL, error);
  if (parser == NULL)
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      goto failed;
    }

  if (!process_config_first_time_only (context, parser, address, flags, error))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      goto failed;
    }
  if (!process_config_every_time (context, parser, FALSE, error))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      goto failed;
    }

  /* we need another ref of the server data slot for the context
   * to own
   */
  if (!dbus_server_allocate_data_slot (&server_data_slot))
    _dbus_assert_not_reached ("second ref of server data slot failed");

  /* Note that we don't know whether the print_addr_pipe is
   * one of the sockets we're using to listen on, or some
   * other random thing. But I think the answer is "don't do
   * that then"
   */
  if (print_addr_pipe != NULL && _dbus_pipe_is_valid (print_addr_pipe))
    {
      DBusString addr;
      const char *a = bus_context_get_address (context);
      int bytes;

      _dbus_assert (a != NULL);
      if (!_dbus_string_init (&addr))
        {
          BUS_SET_OOM (error);
          goto failed;
        }

      if (!_dbus_string_append (&addr, a) ||
          !_dbus_string_append (&addr, "\n"))
        {
          _dbus_string_free (&addr);
          BUS_SET_OOM (error);
          goto failed;
        }

      bytes = _dbus_string_get_length (&addr);
      if (_dbus_pipe_write (print_addr_pipe, &addr, 0, bytes, error) != bytes)
        {
          /* pipe write returns an error on failure but not short write */
          if (error != NULL && !dbus_error_is_set (error))
            {
              dbus_set_error (error, DBUS_ERROR_FAILED,
                              "Printing message bus address: did not write all bytes\n");
            }
          _dbus_string_free (&addr);
          goto failed;
        }

      if (!_dbus_pipe_is_stdout_or_stderr (print_addr_pipe))
        _dbus_pipe_close (print_addr_pipe, NULL);

      _dbus_string_free (&addr);
    }

#ifdef DBUS_WIN
  if (ready_event_handle != NULL)
    {
      _dbus_verbose ("Notifying that we are ready to receive connections (event handle=%p)\n", ready_event_handle);
      if (!_dbus_win_event_set (ready_event_handle, error))
        goto failed;
      if (!_dbus_win_event_free (ready_event_handle, error))
        goto failed;
    }
#endif

  context->connections = bus_connections_new (context);
  if (context->connections == NULL)
    {
      BUS_SET_OOM (error);
      goto failed;
    }

  context->matchmaker = bus_matchmaker_new ();
  if (context->matchmaker == NULL)
    {
      BUS_SET_OOM (error);
      goto failed;
    }

  context->containers = bus_containers_new ();

  if (context->containers == NULL)
    {
      BUS_SET_OOM (error);
      goto failed;
    }

  /* check user before we fork */
  if (context->user != NULL)
    {
      if (!_dbus_verify_daemon_user (context->user))
        {
          dbus_set_error (error, DBUS_ERROR_FAILED,
                          "Could not get UID and GID for username \"%s\"",
                          context->user);
          goto failed;
        }
    }

  /* Now become a daemon if appropriate and write out pid file in any case */
  {
    DBusString u;

    if (context->pidfile)
      _dbus_string_init_const (&u, context->pidfile);

    if (((flags & BUS_CONTEXT_FLAG_FORK_NEVER) == 0 && context->fork) ||
        (flags & BUS_CONTEXT_FLAG_FORK_ALWAYS))
      {
        _dbus_verbose ("Forking and becoming daemon\n");

        if (!_dbus_become_daemon (context->pidfile ? &u : NULL,
                                  print_pid_pipe,
                                  error,
                                  context->keep_umask))
          {
            _DBUS_ASSERT_ERROR_IS_SET (error);
            goto failed;
          }
      }
    else
      {
        _dbus_verbose ("Fork not requested\n");

        /* Need to write PID file and to PID pipe for ourselves,
         * not for the child process. This is a no-op if the pidfile
         * is NULL and print_pid_pipe is NULL.
         */
        if (!_dbus_write_pid_to_file_and_pipe (context->pidfile ? &u : NULL,
                                               print_pid_pipe,
                                               _dbus_getpid (),
                                               error))
          {
            _DBUS_ASSERT_ERROR_IS_SET (error);
            goto failed;
          }
      }
  }

  if (print_pid_pipe && _dbus_pipe_is_valid (print_pid_pipe) &&
      !_dbus_pipe_is_stdout_or_stderr (print_pid_pipe))
    _dbus_pipe_close (print_pid_pipe, NULL);

  /* Raise the file descriptor limits before dropping the privileges
   * required to do so.
   */
  raise_file_descriptor_limit (context);

  /* Here we change our credentials if required,
   * as soon as we've set up our sockets and pidfile.
   * This must be done before initializing LSMs, so that the netlink
   * monitoring thread started by avc_init() will not lose CAP_AUDIT_WRITE
   * when the main thread calls setuid().
   * https://bugs.freedesktop.org/show_bug.cgi?id=92832
   */
  if (context->user != NULL)
    {
      if (!_dbus_change_to_daemon_user (context->user, error))
	{
	  _DBUS_ASSERT_ERROR_IS_SET (error);
	  goto failed;
	}
    }

  /* Auditing should be initialized before LSMs, so that the LSMs are able
   * to log audit-events that happen during their initialization.
   */
  bus_audit_init (context);

  if (!bus_selinux_full_init (context, error))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      goto failed;
    }

  if (!bus_apparmor_full_init (error))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      goto failed;
    }

  if (bus_selinux_enabled ())
    {
      if (context->syslog)
        bus_context_log (context, DBUS_SYSTEM_LOG_INFO,
                         "SELinux support is enabled\n");
    }

  if (bus_apparmor_enabled ())
    {
      /* Only print AppArmor mediation message when syslog support is enabled */
      if (context->syslog)
        bus_context_log (context, DBUS_SYSTEM_LOG_INFO,
                         "AppArmor D-Bus mediation is enabled\n");
    }

  /* When SELinux is used, this must happen after bus_selinux_full_init()
   * so that it has access to the access vector cache, which is required
   * to process <associate/> elements.
   * http://lists.freedesktop.org/archives/dbus/2008-October/010491.html
   */
  if (!process_config_postinit (context, parser, error))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      goto failed;
    }

  if (parser != NULL)
    {
      bus_config_parser_unref (parser);
      parser = NULL;
    }

  dbus_server_free_data_slot (&server_data_slot);

  return context;

 failed:
  if (parser != NULL)
    bus_config_parser_unref (parser);
  if (context != NULL)
    bus_context_unref (context);

  if (server_data_slot >= 0)
    dbus_server_free_data_slot (&server_data_slot);

  return NULL;
}

dbus_bool_t
bus_context_get_id (BusContext       *context,
                    DBusString       *uuid)
{
  return _dbus_uuid_encode (&context->uuid, uuid);
}

/**
 * Send signal to the buses that the activatable services may be changed
 *
 * @param context bus context to use
 * @param error the error to set, if NULL no error will be set
 * @return #FALSE if an error occurred, the reason is returned in \p error
 */
static dbus_bool_t
bus_context_send_activatable_services_changed (BusContext *context,
                                               DBusError  *error)
{
  DBusMessage *message;
  BusTransaction *transaction;
  dbus_bool_t retval = FALSE;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  transaction = bus_transaction_new (context);
  if (transaction == NULL)
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  message = dbus_message_new_signal (DBUS_PATH_DBUS,
                                     DBUS_INTERFACE_DBUS,
                                     "ActivatableServicesChanged");

  if (message == NULL)
    {
      BUS_SET_OOM (error);
      goto out;
    }

  if (!dbus_message_set_sender (message, DBUS_SERVICE_DBUS))
    {
      BUS_SET_OOM (error);
      goto out;
    }

  if (!bus_transaction_capture (transaction, NULL, NULL, message))
    {
      BUS_SET_OOM (error);
      goto out;
    }

  retval = bus_dispatch_matches (transaction, NULL, NULL, message, error);

out:
  if (transaction != NULL)
    {
      if (retval)
        bus_transaction_execute_and_free (transaction);
      else
        bus_transaction_cancel_and_free (transaction);
    }
  dbus_clear_message (&message);
  return retval;
}

dbus_bool_t
bus_context_reload_config (BusContext *context,
			   DBusError  *error)
{
  BusConfigParser *parser;
  DBusString config_file;
  dbus_bool_t ret;

  _dbus_daemon_report_reloading ();

  /* Flush the user database cache */
  _dbus_flush_caches ();

  ret = FALSE;
  _dbus_string_init_const (&config_file, context->config_file);
  parser = bus_config_load (&config_file, TRUE, NULL, error);
  if (parser == NULL)
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      goto failed;
    }

  if (!process_config_every_time (context, parser, TRUE, error))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      goto failed;
    }
  if (!process_config_postinit (context, parser, error))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      goto failed;
    }
  ret = TRUE;

  bus_context_log (context, DBUS_SYSTEM_LOG_INFO, "Reloaded configuration");
 failed:
  if (!ret)
    bus_context_log (context, DBUS_SYSTEM_LOG_INFO, "Unable to reload configuration: %s", error->message);
  if (parser != NULL)
    bus_config_parser_unref (parser);

  {
    DBusError local_error = DBUS_ERROR_INIT;

    if (!bus_context_send_activatable_services_changed (context, &local_error))
      bus_context_log (context, DBUS_SYSTEM_LOG_INFO, "Unable to send signal that configuration has been reloaded: %s", local_error.message);

    dbus_error_free (&local_error);
  }

  _dbus_daemon_report_reloaded ();
  return ret;
}

static void
shutdown_server (BusContext *context,
                 DBusServer *server)
{
  if (server == NULL ||
      !dbus_server_get_is_connected (server))
    return;

  if (!dbus_server_set_watch_functions (server,
                                        NULL, NULL, NULL,
                                        context,
                                        NULL))
    _dbus_assert_not_reached ("setting watch functions to NULL failed");

  if (!dbus_server_set_timeout_functions (server,
                                          NULL, NULL, NULL,
                                          context,
                                          NULL))
    _dbus_assert_not_reached ("setting timeout functions to NULL failed");

  dbus_server_disconnect (server);
}

void
bus_context_shutdown (BusContext  *context)
{
  DBusList *link;

  link = _dbus_list_get_first_link (&context->servers);
  while (link != NULL)
    {
      shutdown_server (context, link->data);

      link = _dbus_list_get_next_link (&context->servers, link);
    }

  if (context->containers != NULL)
    bus_containers_stop_listening (context->containers);
}

BusContext *
bus_context_ref (BusContext *context)
{
  _dbus_assert (context->refcount > 0);
  context->refcount += 1;

  return context;
}

void
bus_context_unref (BusContext *context)
{
  _dbus_assert (context->refcount > 0);
  context->refcount -= 1;

  if (context->refcount == 0)
    {
      DBusList *link;

      _dbus_verbose ("Finalizing bus context %p\n", context);

      bus_context_shutdown (context);

      if (context->connections)
        {
          bus_connections_unref (context->connections);
          context->connections = NULL;
        }

      if (context->registry)
        {
          bus_registry_unref (context->registry);
          context->registry = NULL;
        }

      if (context->activation)
        {
          bus_activation_unref (context->activation);
          context->activation = NULL;
        }

      link = _dbus_list_get_first_link (&context->servers);
      while (link != NULL)
        {
          dbus_server_unref (link->data);

          link = _dbus_list_get_next_link (&context->servers, link);
        }
      _dbus_list_clear (&context->servers);

      if (context->policy)
        {
          bus_policy_unref (context->policy);
          context->policy = NULL;
        }

      if (context->loop)
        {
          _dbus_loop_unref (context->loop);
          context->loop = NULL;
        }

      if (context->matchmaker)
        {
          bus_matchmaker_unref (context->matchmaker);
          context->matchmaker = NULL;
        }

      bus_clear_containers (&context->containers);
      dbus_free (context->config_file);
      dbus_free (context->log_prefix);
      dbus_free (context->type);
      dbus_free (context->address);
      dbus_free (context->user);
      dbus_free (context->servicehelper);

      if (context->pidfile)
	{
          DBusString u;
          _dbus_string_init_const (&u, context->pidfile);

          /* Deliberately ignore errors here, since there's not much
	   * we can do about it, and we're exiting anyways.
	   */
	  _dbus_delete_file (&u, NULL);

          dbus_free (context->pidfile);
	}

      if (context->initial_fd_limit)
        _dbus_rlimit_free (context->initial_fd_limit);

      dbus_free (context);

      dbus_server_free_data_slot (&server_data_slot);
    }
}

/* type may be NULL */
const char*
bus_context_get_type (BusContext *context)
{
  return context->type;
}

const char*
bus_context_get_address (BusContext *context)
{
  return context->address;
}

const char*
bus_context_get_servicehelper (BusContext *context)
{
  return context->servicehelper;
}

dbus_bool_t
bus_context_get_systemd_activation (BusContext *context)
{
  return context->systemd_activation;
}

BusRegistry*
bus_context_get_registry (BusContext  *context)
{
  return context->registry;
}

BusConnections*
bus_context_get_connections (BusContext  *context)
{
  return context->connections;
}

BusActivation*
bus_context_get_activation (BusContext  *context)
{
  return context->activation;
}

BusMatchmaker*
bus_context_get_matchmaker (BusContext  *context)
{
  return context->matchmaker;
}

DBusLoop*
bus_context_get_loop (BusContext *context)
{
  return context->loop;
}

dbus_bool_t
bus_context_allow_unix_user (BusContext   *context,
                             unsigned long uid)
{
  return bus_policy_allow_unix_user (context->policy,
                                     uid);
}

/* For now this is never actually called because the default
 * DBusConnection behavior of 'same user that owns the bus can connect'
 * is all it would do.
 */
dbus_bool_t
bus_context_allow_windows_user (BusContext       *context,
                                const char       *windows_sid)
{
  return bus_policy_allow_windows_user (context->policy,
                                        windows_sid);
}

BusContainers *
bus_context_get_containers (BusContext *context)
{
  return context->containers;
}

BusClientPolicy*
bus_context_create_client_policy (BusContext      *context,
                                  DBusConnection  *connection,
                                  DBusError       *error)
{
  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  return bus_policy_create_client_policy (context->policy, connection,
                                          error);
}

int
bus_context_get_activation_timeout (BusContext *context)
{

  return context->limits.activation_timeout;
}

int
bus_context_get_auth_timeout (BusContext *context)
{
  return context->limits.auth_timeout;
}

int
bus_context_get_pending_fd_timeout (BusContext *context)
{
  return context->limits.pending_fd_timeout;
}

int
bus_context_get_max_completed_connections (BusContext *context)
{
  return context->limits.max_completed_connections;
}

int
bus_context_get_max_incomplete_connections (BusContext *context)
{
  return context->limits.max_incomplete_connections;
}

int
bus_context_get_max_connections_per_user (BusContext *context)
{
  return context->limits.max_connections_per_user;
}

int
bus_context_get_max_pending_activations (BusContext *context)
{
  return context->limits.max_pending_activations;
}

int
bus_context_get_max_services_per_connection (BusContext *context)
{
  return context->limits.max_services_per_connection;
}

int
bus_context_get_max_match_rules_per_connection (BusContext *context)
{
  return context->limits.max_match_rules_per_connection;
}

int
bus_context_get_max_replies_per_connection (BusContext *context)
{
  return context->limits.max_replies_per_connection;
}

int
bus_context_get_reply_timeout (BusContext *context)
{
  return context->limits.reply_timeout;
}

int bus_context_get_max_containers (BusContext *context)
{
  return context->limits.max_containers;
}

int bus_context_get_max_containers_per_user (BusContext *context)
{
  return context->limits.max_containers_per_user;
}

int bus_context_get_max_container_metadata_bytes (BusContext *context)
{
  return context->limits.max_container_metadata_bytes;
}

int bus_context_get_max_connections_per_container (BusContext *context)
{
  return context->limits.max_connections_per_container;
}

DBusRLimit *
bus_context_get_initial_fd_limit (BusContext *context)
{
  return context->initial_fd_limit;
}

dbus_bool_t
bus_context_get_using_syslog (BusContext *context)
{
  return context->syslog;
}

void
bus_context_log (BusContext *context, DBusSystemLogSeverity severity, const char *msg, ...)
{
  va_list args;

  va_start (args, msg);

  if (context->log_prefix)
    {
      DBusString full_msg;

      if (!_dbus_string_init (&full_msg))
        goto out;
      if (!_dbus_string_append (&full_msg, context->log_prefix))
        goto oom_out;
      if (!_dbus_string_append_printf_valist (&full_msg, msg, args))
        goto oom_out;

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
      if (severity > DBUS_SYSTEM_LOG_WARNING || !context->quiet_log)
#endif
        _dbus_log (severity, "%s", _dbus_string_get_const_data (&full_msg));

    oom_out:
      _dbus_string_free (&full_msg);
    }
  else
    _dbus_logv (severity, msg, args);

out:
  va_end (args);
}

static inline const char *
nonnull (const char *maybe_null,
         const char *if_null)
{
  return (maybe_null ? maybe_null : if_null);
}

void
bus_context_log_literal (BusContext            *context,
                         DBusSystemLogSeverity  severity,
                         const char            *msg)
{
  _dbus_log (severity, "%s%s", nonnull (context->log_prefix, ""), msg);
}

void
bus_context_log_and_set_error (BusContext            *context,
                               DBusSystemLogSeverity  severity,
                               DBusError             *error,
                               const char            *name,
                               const char            *msg,
                               ...)
{
  DBusError stack_error = DBUS_ERROR_INIT;
  va_list args;

  va_start (args, msg);
  _dbus_set_error_valist (&stack_error, name, msg, args);
  va_end (args);

  /* If we hit OOM while setting the error, this will syslog "out of memory"
   * which is itself an indication that something is seriously wrong */
  bus_context_log_literal (context, DBUS_SYSTEM_LOG_SECURITY,
                           stack_error.message);

  dbus_move_error (&stack_error, error);
}

/*
 * Log something about a message, usually that it was rejected.
 */
static void
complain_about_message (BusContext     *context,
                        const char     *error_name,
                        const char     *complaint,
                        int             matched_rules,
                        DBusMessage    *message,
                        DBusConnection *sender,
                        DBusConnection *proposed_recipient,
                        dbus_bool_t     requested_reply,
                        dbus_bool_t     log,
                        DBusError      *error)
{
  DBusError stack_error = DBUS_ERROR_INIT;
  const char *sender_name;
  const char *sender_loginfo;
  const char *proposed_recipient_loginfo;

  if (error == NULL && !log)
    return;

  if (sender != NULL)
    {
      sender_name = bus_connection_get_name (sender);
      sender_loginfo = bus_connection_get_loginfo (sender);
    }
  else
    {
      sender_name = "(unset)";
      sender_loginfo = "(bus)";
    }

  if (proposed_recipient != NULL)
    proposed_recipient_loginfo = bus_connection_get_loginfo (proposed_recipient);
  else
    proposed_recipient_loginfo = "bus";

  dbus_set_error (&stack_error, error_name,
      "%s, %d matched rules; type=\"%s\", sender=\"%s\" (%s) "
      "interface=\"%s\" member=\"%s\" error name=\"%s\" "
      "requested_reply=\"%d\" destination=\"%s\" (%s)",
      complaint,
      matched_rules,
      dbus_message_type_to_string (dbus_message_get_type (message)),
      sender_name,
      sender_loginfo,
      nonnull (dbus_message_get_interface (message), "(unset)"),
      nonnull (dbus_message_get_member (message), "(unset)"),
      nonnull (dbus_message_get_error_name (message), "(unset)"),
      requested_reply,
      nonnull (dbus_message_get_destination (message), DBUS_SERVICE_DBUS),
      proposed_recipient_loginfo);

  /* If we hit OOM while setting the error, this will syslog "out of memory"
   * which is itself an indication that something is seriously wrong */
  if (log)
    bus_context_log_literal (context, DBUS_SYSTEM_LOG_SECURITY,
        stack_error.message);

  dbus_move_error (&stack_error, error);
}

/*
 * addressed_recipient is the recipient specified in the message.
 *
 * proposed_recipient is the recipient we're considering sending
 * to right this second, and may be an eavesdropper.
 *
 * sender is the sender of the message.
 *
 * NULL for sender definitely means the bus driver.
 *
 * NULL for proposed_recipient may mean the bus driver, or may mean
 * we are checking whether service-activation is allowed as a first
 * pass before all details of the activated service are known.
 *
 * NULL for addressed_recipient may mean the bus driver, or may mean
 * no destination was specified in the message (e.g. a signal).
 */
dbus_bool_t
bus_context_check_security_policy (BusContext     *context,
                                   BusTransaction *transaction,
                                   DBusConnection *sender,
                                   DBusConnection *addressed_recipient,
                                   DBusConnection *proposed_recipient,
                                   DBusMessage    *message,
                                   BusActivationEntry *activation_entry,
                                   DBusError      *error)
{
  const char *src, *dest;
  BusClientPolicy *sender_policy;
  BusClientPolicy *recipient_policy;
  dbus_int32_t toggles;
  dbus_bool_t log;
  int type;
  dbus_bool_t requested_reply;

  type = dbus_message_get_type (message);
  src = dbus_message_get_sender (message);
  dest = dbus_message_get_destination (message);

  /* dispatch.c was supposed to ensure these invariants */
  _dbus_assert (dest != NULL ||
                type == DBUS_MESSAGE_TYPE_SIGNAL ||
                (sender == NULL && !bus_connection_is_active (proposed_recipient)));
  _dbus_assert (type == DBUS_MESSAGE_TYPE_SIGNAL ||
                addressed_recipient != NULL ||
                activation_entry != NULL ||
                strcmp (dest, DBUS_SERVICE_DBUS) == 0);

  switch (type)
    {
    case DBUS_MESSAGE_TYPE_METHOD_CALL:
    case DBUS_MESSAGE_TYPE_SIGNAL:
    case DBUS_MESSAGE_TYPE_METHOD_RETURN:
    case DBUS_MESSAGE_TYPE_ERROR:
      break;

    default:
      _dbus_verbose ("security check disallowing message of unknown type %d\n",
                     type);

      dbus_set_error (error, DBUS_ERROR_ACCESS_DENIED,
                      "Message bus will not accept messages of unknown type\n");

      return FALSE;
    }

  requested_reply = FALSE;

  if (sender != NULL)
    {
      if (bus_connection_is_active (sender))
        {
          sender_policy = bus_connection_get_policy (sender);
          _dbus_assert (sender_policy != NULL);

          /* Fill in requested_reply variable with TRUE if this is a
           * reply and the reply was pending.
           */
          if (dbus_message_get_reply_serial (message) != 0)
            {
              if (proposed_recipient != NULL /* not to the bus driver */ &&
                  addressed_recipient == proposed_recipient /* not eavesdropping */)
                {
                  DBusError error2;

                  dbus_error_init (&error2);
                  requested_reply = bus_connections_check_reply (bus_connection_get_connections (sender),
                                                                 transaction,
                                                                 sender, addressed_recipient, message,
                                                                 &error2);
                  if (dbus_error_is_set (&error2))
                    {
                      dbus_move_error (&error2, error);
                      return FALSE;
                    }
                }
            }
        }
      else
        {
          sender_policy = NULL;
        }

      /* First verify the SELinux access controls.  If allowed then
       * go on with the standard checks.
       */
      if (!bus_selinux_allows_send (sender, proposed_recipient,
                                    dbus_message_type_to_string (dbus_message_get_type (message)),
                                    dbus_message_get_interface (message),
                                    dbus_message_get_member (message),
                                    dbus_message_get_error_name (message),
                                    dest ? dest : DBUS_SERVICE_DBUS,
                                    activation_entry,
                                    error))
        {
          if (error != NULL && !dbus_error_is_set (error))
            {
              /* don't syslog this, just set the error: avc_has_perm should
               * have already written to either the audit log or syslog */
              complain_about_message (context, DBUS_ERROR_ACCESS_DENIED,
                  "An SELinux policy prevents this sender from sending this "
                  "message to this recipient",
                  0, message, sender, proposed_recipient, FALSE, FALSE, error);
              _dbus_verbose ("SELinux security check denying send to service\n");
            }

          return FALSE;
        }

      /* next verify AppArmor access controls.  If allowed then
       * go on with the standard checks.
       */
      if (!bus_apparmor_allows_send (sender, proposed_recipient,
                                     requested_reply,
                                     bus_context_get_type (context),
                                     dbus_message_get_type (message),
                                     dbus_message_get_path (message),
                                     dbus_message_get_interface (message),
                                     dbus_message_get_member (message),
                                     dbus_message_get_error_name (message),
                                     dest ? dest : DBUS_SERVICE_DBUS,
                                     src ? src : DBUS_SERVICE_DBUS,
                                     activation_entry,
                                     error))
        return FALSE;

      if (!bus_connection_is_active (sender))
        {
          /* Policy for inactive connections is that they can only send
           * the hello message to the bus driver
           */
          if (proposed_recipient == NULL &&
              dbus_message_is_method_call (message,
                                           DBUS_INTERFACE_DBUS,
                                           "Hello"))
            {
              _dbus_verbose ("security check allowing %s message\n",
                             "Hello");
              return TRUE;
            }
          else
            {
              _dbus_verbose ("security check disallowing non-%s message\n",
                             "Hello");

              dbus_set_error (error, DBUS_ERROR_ACCESS_DENIED,
                              "Client tried to send a message other than %s without being registered",
                              "Hello");

              return FALSE;
            }
        }
    }
  else
    {
      sender_policy = NULL;

      /* If the sender is the bus driver, we assume any reply was a
       * requested reply as bus driver won't send bogus ones
       */
      if (addressed_recipient == proposed_recipient /* not eavesdropping */ &&
          dbus_message_get_reply_serial (message) != 0)
        requested_reply = TRUE;
    }

  _dbus_assert ((sender != NULL && sender_policy != NULL) ||
                (sender == NULL && sender_policy == NULL));

  if (proposed_recipient != NULL)
    {
      /* only the bus driver can send to an inactive recipient (as it
       * owns no services, so other apps can't address it). Inactive
       * recipients can receive any message.
       */
      if (bus_connection_is_active (proposed_recipient))
        {
          recipient_policy = bus_connection_get_policy (proposed_recipient);
          _dbus_assert (recipient_policy != NULL);
        }
      else if (sender == NULL)
        {
          _dbus_verbose ("security check using NULL recipient policy for message from bus\n");
          recipient_policy = NULL;
        }
      else
        {
          _dbus_assert_not_reached ("a message was somehow sent to an inactive recipient from a source other than the message bus");
          recipient_policy = NULL;
        }
    }
  else
    recipient_policy = NULL;

  _dbus_assert ((proposed_recipient != NULL && recipient_policy != NULL) ||
                (proposed_recipient != NULL && sender == NULL && recipient_policy == NULL) ||
                (proposed_recipient == NULL && recipient_policy == NULL));

  log = FALSE;
  if (sender_policy &&
      !bus_client_policy_check_can_send (sender_policy,
                                         context->registry,
                                         requested_reply,
                                         proposed_recipient,
                                         message, &toggles, &log))
    {
      complain_about_message (context, DBUS_ERROR_ACCESS_DENIED,
          "Rejected send message", toggles,
          message, sender, proposed_recipient, requested_reply,
          (addressed_recipient == proposed_recipient), error);
      _dbus_verbose ("security policy disallowing message due to sender policy\n");
      return FALSE;
    }

  if (log)
    {
      /* We want to drop this message, and are only not doing so for backwards
       * compatibility. */
      complain_about_message (context, DBUS_ERROR_ACCESS_DENIED,
          "Would reject message", toggles,
          message, sender, proposed_recipient, requested_reply,
          TRUE, NULL);
    }

  if (recipient_policy &&
      !bus_client_policy_check_can_receive (recipient_policy,
                                            context->registry,
                                            requested_reply,
                                            sender,
                                            addressed_recipient, proposed_recipient,
                                            message, &toggles))
    {
      complain_about_message (context, DBUS_ERROR_ACCESS_DENIED,
          "Rejected receive message", toggles,
          message, sender, proposed_recipient, requested_reply,
          (addressed_recipient == proposed_recipient), error);
      _dbus_verbose ("security policy disallowing message due to recipient policy\n");
      return FALSE;
    }

  /* See if limits on size have been exceeded */
  if (proposed_recipient &&
      ((dbus_connection_get_outgoing_size (proposed_recipient) > context->limits.max_outgoing_bytes) ||
       (dbus_connection_get_outgoing_unix_fds (proposed_recipient) > context->limits.max_outgoing_unix_fds)))
    {
      complain_about_message (context, DBUS_ERROR_LIMITS_EXCEEDED,
          "Rejected: destination has a full message queue",
          0, message, sender, proposed_recipient, requested_reply, TRUE,
          error);
      _dbus_verbose ("security policy disallowing message due to full message queue\n");
      return FALSE;
    }

  /* Record that we will allow a reply here in the future (don't
   * bother if the recipient is the bus or this is an eavesdropping
   * connection). Only the addressed recipient may reply.
   *
   * This isn't done for activation attempts because they have no addressed
   * or proposed recipient; when we check whether to actually deliver the
   * message, later, we'll record the reply expectation at that point.
   */
  if (type == DBUS_MESSAGE_TYPE_METHOD_CALL &&
      sender &&
      addressed_recipient &&
      addressed_recipient == proposed_recipient && /* not eavesdropping */
      !bus_connections_expect_reply (bus_connection_get_connections (sender),
                                     transaction,
                                     sender, addressed_recipient,
                                     message, error))
    {
      _dbus_verbose ("Failed to record reply expectation or problem with the message expecting a reply\n");
      return FALSE;
    }

  _dbus_verbose ("security policy allowing message\n");
  return TRUE;
}

void
bus_context_check_all_watches (BusContext *context)
{
  DBusList *link;
  dbus_bool_t enabled = TRUE;

  if (bus_connections_get_n_incomplete (context->connections) >=
      bus_context_get_max_incomplete_connections (context))
    {
      enabled = FALSE;
    }

  if (context->watches_enabled == enabled)
    return;

  context->watches_enabled = enabled;

  for (link = _dbus_list_get_first_link (&context->servers);
       link != NULL;
       link = _dbus_list_get_next_link (&context->servers, link))
    {
      /* A BusContext might contains several DBusServer (if there are
       * several <listen> configuration items) and a DBusServer might
       * contain several DBusWatch in its DBusWatchList (if getaddrinfo
       * returns several addresses on a dual IPv4-IPv6 stack or if
       * systemd passes several fds).
       * We want to enable/disable them all.
       */
      DBusServer *server = link->data;
      _dbus_server_toggle_all_watches (server, enabled);
    }
}

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
void
bus_context_quiet_log_begin (BusContext *context)
{
  context->quiet_log = TRUE;
}

void
bus_context_quiet_log_end (BusContext *context)
{
  context->quiet_log = FALSE;
}

dbus_bool_t
bus_context_get_quiet_log (BusContext *context)
{
  return context->quiet_log;
}
#endif
