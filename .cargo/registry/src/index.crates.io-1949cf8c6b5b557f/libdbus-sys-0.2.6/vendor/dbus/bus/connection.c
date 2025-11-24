/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* connection.c  Client connections
 *
 * Copyright (C) 2003  Red Hat, Inc.
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
#include "connection.h"

#include "containers.h"
#include "dispatch.h"
#include "policy.h"
#include "services.h"
#include "utils.h"
#include "signals.h"
#include "expirelist.h"
#include "selinux.h"
#include "apparmor.h"
#include <dbus/dbus-list.h>
#include <dbus/dbus-hash.h>
#include <dbus/dbus-timeout.h>
#include <dbus/dbus-connection-internal.h>
#include <dbus/dbus-internals.h>

/* Trim executed commands to this length; we want to keep logs readable */
#define MAX_LOG_COMMAND_LEN 50

static void bus_connection_remove_transactions (DBusConnection *connection);

typedef struct
{
  BusExpireItem expire_item;

  DBusConnection *will_get_reply;
  DBusConnection *will_send_reply;

  dbus_uint32_t reply_serial;
  
} BusPendingReply;

struct BusConnections
{
  int refcount;
  DBusList *completed;  /**< List of all completed connections */
  int n_completed;      /**< Length of completed list */
  DBusList *incomplete; /**< List of all not-yet-active connections */
  int n_incomplete;     /**< Length of incomplete list */
  BusContext *context;
  DBusHashTable *completed_by_user; /**< Number of completed connections for each UID */
  DBusTimeout *expire_timeout; /**< Timeout for expiring incomplete connections. */
  int stamp;                   /**< Incrementing number */
  BusExpireList *pending_replies; /**< List of pending replies */

  /** List of all monitoring connections, a subset of completed.
   * Each member is a #DBusConnection. */
  DBusList *monitors;
  BusMatchmaker *monitor_matchmaker;

#ifdef DBUS_ENABLE_STATS
  int total_match_rules;
  int peak_match_rules;
  int peak_match_rules_per_conn;

  int total_bus_names;
  int peak_bus_names;
  int peak_bus_names_per_conn;
#endif
};

static dbus_int32_t connection_data_slot = -1;

typedef struct
{
  BusConnections *connections;
  DBusList *link_in_connection_list;
  DBusConnection *connection;
  DBusList *services_owned;
  int n_services_owned;
  DBusList *match_rules;
  int n_match_rules;
  char *name;
  DBusList *transaction_messages; /**< Stuff we need to send as part of a transaction */
  DBusMessage *oom_message;
  DBusPreallocatedSend *oom_preallocated;
  BusClientPolicy *policy;

  char *cached_loginfo_string;
  BusSELinuxID *selinux_id;
  BusAppArmorConfinement *apparmor_confinement;

  long connection_tv_sec;  /**< Time when we connected (seconds component) */
  long connection_tv_usec; /**< Time when we connected (microsec component) */
  int stamp;               /**< connections->stamp last time we were traversed */
  BusExtraHeaders want_headers;

#ifdef DBUS_ENABLE_STATS
  int peak_match_rules;
  int peak_bus_names;
#endif
  int n_pending_unix_fds;
  DBusTimeout *pending_unix_fds_timeout;

  /** non-NULL if and only if this is a monitor */
  DBusList *link_in_monitors;
} BusConnectionData;

static dbus_bool_t bus_pending_reply_expired (BusExpireList *list,
                                              DBusList      *link,
                                              void          *data);

static void bus_connection_drop_pending_replies (BusConnections  *connections,
                                                 DBusConnection  *connection);

static dbus_bool_t expire_incomplete_timeout (void *data);

#define BUS_CONNECTION_DATA(connection) (dbus_connection_get_data ((connection), connection_data_slot))

static DBusLoop*
connection_get_loop (DBusConnection *connection)
{
  BusConnectionData *d;

  d = BUS_CONNECTION_DATA (connection);
  _dbus_assert(d != NULL);

  return bus_context_get_loop (d->connections->context);
}


static int
get_connections_for_uid (BusConnections *connections,
                         dbus_uid_t      uid)
{
  void *val;
  int current_count;

  /* val is NULL is 0 when it isn't in the hash yet */
  
  val = _dbus_hash_table_lookup_uintptr (connections->completed_by_user,
                                       uid);

  current_count = _DBUS_POINTER_TO_INT (val);

  return current_count;
}

static dbus_bool_t
adjust_connections_for_uid (BusConnections *connections,
                            dbus_uid_t      uid,
                            int             adjustment)
{
  int current_count;

  current_count = get_connections_for_uid (connections, uid);

  _dbus_verbose ("Adjusting connection count for UID " DBUS_UID_FORMAT
                 ": was %d adjustment %d making %d\n",
                 uid, current_count, adjustment, current_count + adjustment);
  
  _dbus_assert (current_count >= 0);
  
  current_count += adjustment;

  _dbus_assert (current_count >= 0);

  if (current_count == 0)
    {
      _dbus_hash_table_remove_uintptr (connections->completed_by_user, uid);
      return TRUE;
    }
  else
    {
      dbus_bool_t retval;
      
      retval = _dbus_hash_table_insert_uintptr (connections->completed_by_user,
                                              uid, _DBUS_INT_TO_POINTER (current_count));

      /* only positive adjustment can fail as otherwise
       * a hash entry should already exist
       */
      _dbus_assert (adjustment > 0 ||
                    (adjustment <= 0 && retval));

      return retval;
    }
}

void
bus_connection_disconnected (DBusConnection *connection)
{
  BusConnectionData *d;
  BusService *service;
  BusMatchmaker *matchmaker;
  
  d = BUS_CONNECTION_DATA (connection);
  _dbus_assert (d != NULL);

  _dbus_verbose ("%s disconnected, dropping all service ownership and releasing\n",
                 d->name ? d->name : "(inactive)");

  /* Delete our match rules */
  if (d->n_match_rules > 0)
    {
      matchmaker = bus_context_get_matchmaker (d->connections->context);
      bus_matchmaker_disconnected (matchmaker, connection);
    }
  
  /* Drop any service ownership. Unfortunately, this requires
   * memory allocation and there doesn't seem to be a good way to
   * handle it other than sleeping; we can't "fail" the operation of
   * disconnecting a client, and preallocating a broadcast "service is
   * now gone" message for every client-service pair seems kind of
   * involved.
   */
  while ((service = _dbus_list_get_last (&d->services_owned)))
    {
      BusTransaction *transaction;
      DBusError error;

    retry:
      
      dbus_error_init (&error);
        
      while ((transaction = bus_transaction_new (d->connections->context)) == NULL)
        _dbus_wait_for_memory ();
        
      if (!bus_service_remove_owner (service, connection,
                                     transaction, &error))
        {
          _DBUS_ASSERT_ERROR_IS_SET (&error);
          
          if (dbus_error_has_name (&error, DBUS_ERROR_NO_MEMORY))
            {
              dbus_error_free (&error);
              bus_transaction_cancel_and_free (transaction);
              _dbus_wait_for_memory ();
              goto retry;
            }
          else
            {
              _dbus_verbose ("Failed to remove service owner: %s %s\n",
                             error.name, error.message);
              _dbus_assert_not_reached ("Removing service owner failed for non-memory-related reason");
            }
        }
        
      bus_transaction_execute_and_free (transaction);
    }

  bus_dispatch_remove_connection (connection);
  
  /* no more watching */
  if (!dbus_connection_set_watch_functions (connection,
                                            NULL, NULL, NULL,
                                            connection,
                                            NULL))
    _dbus_assert_not_reached ("setting watch functions to NULL failed");

  if (!dbus_connection_set_timeout_functions (connection,
                                              NULL, NULL, NULL,
                                              connection,
                                              NULL))
    _dbus_assert_not_reached ("setting timeout functions to NULL failed");
  
  dbus_connection_set_unix_user_function (connection,
                                          NULL, NULL, NULL);
  dbus_connection_set_windows_user_function (connection,
                                             NULL, NULL, NULL);
  
  dbus_connection_set_dispatch_status_function (connection,
                                                NULL, NULL, NULL);

  if (d->pending_unix_fds_timeout)
    {
      _dbus_loop_remove_timeout (bus_context_get_loop (d->connections->context),
                                 d->pending_unix_fds_timeout);
      _dbus_timeout_unref (d->pending_unix_fds_timeout);
    }
  d->pending_unix_fds_timeout = NULL;
  _dbus_connection_set_pending_fds_function (connection, NULL, NULL);
  
  bus_connection_remove_transactions (connection);

  if (d->link_in_monitors != NULL)
    {
      BusMatchmaker *mm = d->connections->monitor_matchmaker;

      if (mm != NULL)
        bus_matchmaker_disconnected (mm, connection);

      _dbus_list_remove_link (&d->connections->monitors, d->link_in_monitors);
      d->link_in_monitors = NULL;
    }

  bus_containers_remove_connection (bus_context_get_containers (d->connections->context),
                                    connection);

  if (d->link_in_connection_list != NULL)
    {
      if (d->name != NULL)
        {
          unsigned long uid;
          
          _dbus_list_remove_link (&d->connections->completed, d->link_in_connection_list);
          d->link_in_connection_list = NULL;
          d->connections->n_completed -= 1;

          if (dbus_connection_get_unix_user (connection, &uid))
            {
              if (!adjust_connections_for_uid (d->connections,
                                               uid, -1))
                _dbus_assert_not_reached ("adjusting downward should never fail");
            }
        }
      else
        {
          _dbus_list_remove_link (&d->connections->incomplete, d->link_in_connection_list);
          d->link_in_connection_list = NULL;
          d->connections->n_incomplete -= 1;

          /* If we have dropped below the max. number of incomplete
           * connections, start accept()ing again */
          bus_context_check_all_watches (d->connections->context);
        }
      
      _dbus_assert (d->connections->n_incomplete >= 0);
      _dbus_assert (d->connections->n_completed >= 0);
    }

  bus_connection_drop_pending_replies (d->connections, connection);
  
  /* frees "d" as side effect */
  dbus_connection_set_data (connection,
                            connection_data_slot,
                            NULL, NULL);
  
  dbus_connection_unref (connection);
}

static dbus_bool_t
add_connection_watch (DBusWatch      *watch,
                      void           *data)
{
  DBusConnection *connection = data;

  return _dbus_loop_add_watch (connection_get_loop (connection), watch);
}

static void
remove_connection_watch (DBusWatch      *watch,
                         void           *data)
{
  DBusConnection *connection = data;
  
  _dbus_loop_remove_watch (connection_get_loop (connection), watch);
}

static void
toggle_connection_watch (DBusWatch      *watch,
                         void           *data)
{
  DBusConnection *connection = data;

  _dbus_loop_toggle_watch (connection_get_loop (connection), watch);
}

static dbus_bool_t
add_connection_timeout (DBusTimeout    *timeout,
                        void           *data)
{
  DBusConnection *connection = data;
  
  return _dbus_loop_add_timeout (connection_get_loop (connection), timeout);
}

static void
remove_connection_timeout (DBusTimeout    *timeout,
                           void           *data)
{
  DBusConnection *connection = data;
  
  _dbus_loop_remove_timeout (connection_get_loop (connection), timeout);
}

static void
dispatch_status_function (DBusConnection    *connection,
                          DBusDispatchStatus new_status,
                          void              *data)
{
  DBusLoop *loop = data;
  
  if (new_status != DBUS_DISPATCH_COMPLETE)
    {
      while (!_dbus_loop_queue_dispatch (loop, connection))
        _dbus_wait_for_memory ();
    }
}

static dbus_bool_t
allow_unix_user_function (DBusConnection *connection,
                          unsigned long   uid,
                          void           *data)
{
  BusConnectionData *d;
    
  d = BUS_CONNECTION_DATA (connection);

  _dbus_assert (d != NULL);
  
  return bus_context_allow_unix_user (d->connections->context, uid);
}

static void
free_connection_data (void *data)
{
  BusConnectionData *d = data;

  /* services_owned should be NULL since we should be disconnected */
  _dbus_assert (d->services_owned == NULL);
  _dbus_assert (d->n_services_owned == 0);
  /* similarly */
  _dbus_assert (d->transaction_messages == NULL);

  if (d->oom_preallocated)
    dbus_connection_free_preallocated_send (d->connection, d->oom_preallocated);

  if (d->oom_message)
    dbus_message_unref (d->oom_message);

  if (d->policy)
    bus_client_policy_unref (d->policy);

  if (d->apparmor_confinement)
    bus_apparmor_confinement_unref (d->apparmor_confinement);
  
  dbus_free (d->cached_loginfo_string);
  
  dbus_free (d->name);
  
  dbus_free (d);
}

BusConnections*
bus_connections_new (BusContext *context)
{
  BusConnections *connections;

  if (!dbus_connection_allocate_data_slot (&connection_data_slot))
    goto failed_0;

  connections = dbus_new0 (BusConnections, 1);
  if (connections == NULL)
    goto failed_1;

  connections->completed_by_user = _dbus_hash_table_new (DBUS_HASH_UINTPTR,
                                                         NULL, NULL);
  if (connections->completed_by_user == NULL)
    goto failed_2;

  connections->expire_timeout = _dbus_timeout_new (100, /* irrelevant */
                                                   expire_incomplete_timeout,
                                                   connections, NULL);
  if (connections->expire_timeout == NULL)
    goto failed_3;

  _dbus_timeout_disable (connections->expire_timeout);

  connections->pending_replies = bus_expire_list_new (bus_context_get_loop (context),
                                                      bus_context_get_reply_timeout (context),
                                                      bus_pending_reply_expired,
                                                      connections);
  if (connections->pending_replies == NULL)
    goto failed_4;
  
  if (!_dbus_loop_add_timeout (bus_context_get_loop (context),
                               connections->expire_timeout))
    goto failed_5;
  
  connections->refcount = 1;
  connections->context = context;
  
  return connections;

 failed_5:
  bus_expire_list_free (connections->pending_replies);
 failed_4:
  _dbus_timeout_unref (connections->expire_timeout);
 failed_3:
  _dbus_hash_table_unref (connections->completed_by_user);
 failed_2:
  dbus_free (connections);
 failed_1:
  dbus_connection_free_data_slot (&connection_data_slot);
 failed_0:
  return NULL;
}

BusConnections *
bus_connections_ref (BusConnections *connections)
{
  _dbus_assert (connections->refcount > 0);
  connections->refcount += 1;

  return connections;
}

void
bus_connections_unref (BusConnections *connections)
{
  _dbus_assert (connections->refcount > 0);
  connections->refcount -= 1;
  if (connections->refcount == 0)
    {
      /* drop all incomplete */
      while (connections->incomplete != NULL)
        {
          DBusConnection *connection;

          connection = connections->incomplete->data;

          dbus_connection_ref (connection);
          dbus_connection_close (connection);
          bus_connection_disconnected (connection);
          dbus_connection_unref (connection);
        }

      _dbus_assert (connections->n_incomplete == 0);

      /* drop all real connections */
      while (connections->completed != NULL)
        {
          DBusConnection *connection;

          connection = connections->completed->data;

          dbus_connection_ref (connection);
          dbus_connection_close (connection);
          bus_connection_disconnected (connection);
          dbus_connection_unref (connection);
        }

      _dbus_assert (connections->n_completed == 0);

      /* disconnecting all the connections should have emptied the list of
       * monitors (each link is removed in bus_connection_disconnected) */
      _dbus_assert (connections->monitors == NULL);

      bus_expire_list_free (connections->pending_replies);
      
      _dbus_loop_remove_timeout (bus_context_get_loop (connections->context),
                                 connections->expire_timeout);
      
      _dbus_timeout_unref (connections->expire_timeout);
      
      _dbus_hash_table_unref (connections->completed_by_user);

      if (connections->monitor_matchmaker != NULL)
        bus_matchmaker_unref (connections->monitor_matchmaker);

      dbus_free (connections);

      dbus_connection_free_data_slot (&connection_data_slot);
    }
}

/* Used for logging */
static dbus_bool_t
cache_peer_loginfo_string (BusConnectionData *d, 
                           DBusConnection    *connection)
{
  DBusString loginfo_buf;
  dbus_uid_t uid = DBUS_UID_UNSET;
  dbus_pid_t pid = DBUS_PID_UNSET;
  const char *windows_sid = NULL;
  const char *security_label = NULL;
  dbus_bool_t prev_added;
  const char *container = NULL;
  const char *container_type = NULL;
  const char *container_name = NULL;
  DBusCredentials *credentials;

  if (!_dbus_string_init (&loginfo_buf))
    return FALSE;

  credentials = _dbus_connection_get_credentials (connection);

  prev_added = FALSE;

  if (credentials != NULL)
    {
      uid = _dbus_credentials_get_unix_uid (credentials);
      pid = _dbus_credentials_get_pid (credentials);
      windows_sid = _dbus_credentials_get_windows_sid (credentials);
      security_label = _dbus_credentials_get_linux_security_label (credentials);
    }

  if (uid != DBUS_UID_UNSET)
    {
      if (!_dbus_string_append_printf (&loginfo_buf, "uid=" DBUS_UID_FORMAT, uid))
        goto oom;
      else
        prev_added = TRUE;
    }

  if (pid != DBUS_PID_UNSET)
    {
      if (prev_added)
        {
          if (!_dbus_string_append_byte (&loginfo_buf, ' '))
            goto oom;
        }
      if (!_dbus_string_append_printf (&loginfo_buf,
                                       "pid=" DBUS_PID_FORMAT " comm=\"", pid))
        goto oom;
      /* Ignore errors here; we may not have permissions to read the
       * proc file. */
      _dbus_command_for_pid (pid, &loginfo_buf, MAX_LOG_COMMAND_LEN, NULL);
      if (!_dbus_string_append_byte (&loginfo_buf, '"'))
        goto oom;
      else
        prev_added = TRUE;
    }

  if (windows_sid != NULL)
    {
      dbus_bool_t did_append;

      if (prev_added)
        {
          if (!_dbus_string_append_byte (&loginfo_buf, ' '))
            goto oom;
        }

      did_append = _dbus_string_append_printf (&loginfo_buf,
                                               "sid=\"%s\"", windows_sid);
      if (!did_append)
        goto oom;
      else
        prev_added = TRUE;
    }

  if (security_label != NULL)
    {
      dbus_bool_t did_append;

      if (prev_added)
        {
          if (!_dbus_string_append_byte (&loginfo_buf, ' '))
            goto oom;
        }

      did_append = _dbus_string_append_printf (&loginfo_buf,
                                               "label=\"%s\"", security_label);
      if (!did_append)
        goto oom;
      else
        prev_added = TRUE;
    }

  /* This does have to come from the connection, not the credentials */
  if (bus_containers_connection_is_contained (connection, &container,
                                              &container_type,
                                              &container_name))
    {
      dbus_bool_t did_append;

      if (prev_added)
        {
          if (!_dbus_string_append_byte (&loginfo_buf, ' '))
            goto oom;
        }

      did_append = _dbus_string_append_printf (&loginfo_buf,
                                               "container=%s %s=\"%s\")",
                                               container,
                                               container_type,
                                               container_name);
      if (!did_append)
        goto oom;
      else
        prev_added = TRUE;
    }

  if (!_dbus_string_steal_data (&loginfo_buf, &(d->cached_loginfo_string)))
    goto oom;

  _dbus_string_free (&loginfo_buf); 

  return TRUE;
oom:
   _dbus_string_free (&loginfo_buf);
   return FALSE;
}

static void
check_pending_fds_cb (DBusConnection *connection)
{
  BusConnectionData *d = BUS_CONNECTION_DATA (connection);
  int n_pending_unix_fds_old;
  int n_pending_unix_fds_new;

  _dbus_assert(d != NULL);

  n_pending_unix_fds_old = d->n_pending_unix_fds;
  n_pending_unix_fds_new = _dbus_connection_get_pending_fds_count (connection);

  _dbus_verbose ("Pending fds count changed on connection %p: %d -> %d\n",
                 connection, n_pending_unix_fds_old, n_pending_unix_fds_new);

  if (n_pending_unix_fds_old == 0 && n_pending_unix_fds_new > 0)
    {
      _dbus_timeout_restart (d->pending_unix_fds_timeout,
              bus_context_get_pending_fd_timeout (d->connections->context));
    }

  if (n_pending_unix_fds_old > 0 && n_pending_unix_fds_new == 0)
    {
      _dbus_timeout_disable (d->pending_unix_fds_timeout);
    }


  d->n_pending_unix_fds = n_pending_unix_fds_new;
}

static dbus_bool_t
pending_unix_fds_timeout_cb (void *data)
{
  DBusConnection *connection = data;
  BusConnectionData *d = BUS_CONNECTION_DATA (connection);
  int limit;

  _dbus_assert (d != NULL);
  limit = bus_context_get_pending_fd_timeout (d->connections->context);
  bus_context_log (d->connections->context, DBUS_SYSTEM_LOG_WARNING,
      "Connection \"%s\" (%s) has had Unix fds pending for too long, "
      "closing it (pending_fd_timeout=%d ms)",
      d->name != NULL ? d->name : "(null)",
      bus_connection_get_loginfo (connection),
      limit);

  dbus_connection_close (connection);
  return TRUE;
}

dbus_bool_t
bus_connections_setup_connection (BusConnections *connections,
                                  DBusConnection *connection)
{

  BusConnectionData *d = NULL;
  DBusError error;

  d = dbus_new0 (BusConnectionData, 1);
  
  if (d == NULL)
    goto oom;

  d->connections = connections;
  d->connection = connection;
  
  _dbus_get_monotonic_time (&d->connection_tv_sec,
                            &d->connection_tv_usec);
  
  _dbus_assert (connection_data_slot >= 0);
  
  if (!dbus_connection_set_data (connection,
                                 connection_data_slot,
                                 d, free_connection_data))
    {
      /* We have to free d explicitly, because this is the only code
       * path where it's non-NULL but dbus_connection_set_data() hasn't
       * taken responsibility for freeing it. */
      dbus_free (d);
      d = NULL;
      goto oom;
    }

  dbus_connection_set_route_peer_messages (connection, TRUE);

  dbus_error_init (&error);
  d->selinux_id = bus_selinux_init_connection_id (connection,
                                                  &error);
  if (dbus_error_is_set (&error))
    {
      bus_context_log (connections->context, DBUS_SYSTEM_LOG_WARNING,
                       "Unable to set up new connection: %s", error.message);
      dbus_error_free (&error);
      goto error;
    }

  d->apparmor_confinement = bus_apparmor_init_connection_confinement (connection,
                                                                      &error);
  if (dbus_error_is_set (&error))
    {
      bus_context_log (connections->context, DBUS_SYSTEM_LOG_WARNING,
                       "Unable to set up new connection: %s", error.message);
      dbus_error_free (&error);
      goto error;
    }

  if (!dbus_connection_set_watch_functions (connection,
                                            add_connection_watch,
                                            remove_connection_watch,
                                            toggle_connection_watch,
                                            connection,
                                            NULL))
    goto oom;
  
  if (!dbus_connection_set_timeout_functions (connection,
                                              add_connection_timeout,
                                              remove_connection_timeout,
                                              NULL,
                                              connection, NULL))
    goto oom;

  /* For now we don't need to set a Windows user function because
   * there are no policies in the config file controlling what
   * Windows users can connect. The default 'same user that owns the
   * bus can connect' behavior of DBusConnection is fine on Windows.
   */
  dbus_connection_set_unix_user_function (connection,
                                          allow_unix_user_function,
                                          NULL, NULL);

  dbus_connection_set_dispatch_status_function (connection,
                                                dispatch_status_function,
                                                bus_context_get_loop (connections->context),
                                                NULL);

  d->link_in_connection_list = _dbus_list_alloc_link (connection);
  if (d->link_in_connection_list == NULL)
    goto oom;
  
  /* Setup the connection with the dispatcher */
  if (!bus_dispatch_add_connection (connection))
    goto oom;

  if (dbus_connection_get_dispatch_status (connection) != DBUS_DISPATCH_COMPLETE)
    {
      if (!_dbus_loop_queue_dispatch (bus_context_get_loop (connections->context), connection))
        {
          bus_dispatch_remove_connection (connection);
          goto oom;
        }
    }

  /* Setup pending fds timeout (see #80559) */
  d->pending_unix_fds_timeout = _dbus_timeout_new (100, /* irrelevant */
                                                   pending_unix_fds_timeout_cb,
                                                   connection, NULL);
  if (d->pending_unix_fds_timeout == NULL)
    goto oom;

  _dbus_timeout_disable (d->pending_unix_fds_timeout);
  if (!_dbus_loop_add_timeout (bus_context_get_loop (connections->context),
                               d->pending_unix_fds_timeout))
    goto oom;

  _dbus_connection_set_pending_fds_function (connection,
          (DBusPendingFdsChangeFunction) check_pending_fds_cb,
          connection);

  _dbus_list_append_link (&connections->incomplete, d->link_in_connection_list);
  connections->n_incomplete += 1;
  
  dbus_connection_ref (connection);

  bus_connections_expire_incomplete (connections);
  
  /* The listening socket is removed from the main loop,
   * i.e. does not accept(), while n_incomplete is at its
   * maximum value; so we shouldn't get here in that case */
  _dbus_assert (connections->n_incomplete <=
      bus_context_get_max_incomplete_connections (connections->context));

  /* If we have the maximum number of incomplete connections,
   * stop accept()ing any more, to avert a DoS. See fd.o #80919 */
  bus_context_check_all_watches (d->connections->context);
  
  return TRUE;

oom:
  bus_context_log (connections->context, DBUS_SYSTEM_LOG_WARNING,
                   "No memory to set up new connection");
  /* fall through */
error:
  if (d != NULL)
    {
      d->selinux_id = NULL;

      if (d->apparmor_confinement)
        bus_apparmor_confinement_unref (d->apparmor_confinement);
      d->apparmor_confinement = NULL;
      
      if (!dbus_connection_set_watch_functions (connection,
                                                NULL, NULL, NULL,
                                                connection,
                                                NULL))
        _dbus_assert_not_reached ("setting watch functions to NULL failed");
      
      if (!dbus_connection_set_timeout_functions (connection,
                                                  NULL, NULL, NULL,
                                                  connection,
                                                  NULL))
        _dbus_assert_not_reached ("setting timeout functions to NULL failed");

      dbus_connection_set_unix_user_function (connection,
                                              NULL, NULL, NULL);

      dbus_connection_set_windows_user_function (connection,
                                                 NULL, NULL, NULL);
      
      dbus_connection_set_dispatch_status_function (connection,
                                                    NULL, NULL, NULL);

      if (d->pending_unix_fds_timeout)
        _dbus_timeout_unref (d->pending_unix_fds_timeout);

      d->pending_unix_fds_timeout = NULL;

      _dbus_connection_set_pending_fds_function (connection, NULL, NULL);

      if (d->link_in_connection_list != NULL)
        {
          _dbus_assert (d->link_in_connection_list->next == NULL);
          _dbus_assert (d->link_in_connection_list->prev == NULL);
          _dbus_list_free_link (d->link_in_connection_list);
          d->link_in_connection_list = NULL;
        }
      
      if (!dbus_connection_set_data (connection,
                                     connection_data_slot,
                                     NULL, NULL))
        _dbus_assert_not_reached ("failed to set connection data to null");

      /* "d" has now been freed */
    }
  
  return FALSE;
}

void
bus_connections_expire_incomplete (BusConnections *connections)
{    
  int next_interval;

  next_interval = -1;
  
  if (connections->incomplete != NULL)
    {
      long tv_sec, tv_usec;
      DBusList *link;
      int auth_timeout;
      
      _dbus_get_monotonic_time (&tv_sec, &tv_usec);
      auth_timeout = bus_context_get_auth_timeout (connections->context);
  
      link = _dbus_list_get_first_link (&connections->incomplete);
      while (link != NULL)
        {
          DBusList *next = _dbus_list_get_next_link (&connections->incomplete, link);
          DBusConnection *connection;
          BusConnectionData *d;
          double elapsed;
      
          connection = link->data;
      
          d = BUS_CONNECTION_DATA (connection);
      
          _dbus_assert (d != NULL);
      
          elapsed = ELAPSED_MILLISECONDS_SINCE (d->connection_tv_sec,
                                                d->connection_tv_usec,
                                                tv_sec, tv_usec);

          if (elapsed >= (double) auth_timeout)
            {
              /* Unfortunately, we can't identify the connection: it doesn't
               * have a unique name yet, we don't know its uid/pid yet,
               * and so on. */
              bus_context_log (connections->context, DBUS_SYSTEM_LOG_WARNING,
                  "Connection has not authenticated soon enough, closing it "
                  "(auth_timeout=%dms, elapsed: %.0fms)",
                  auth_timeout, elapsed);

              _dbus_verbose ("Timing out authentication for connection %p\n", connection);
              dbus_connection_close (connection);
            }
          else
            {
              /* We can end the loop, since the connections are in oldest-first order */
              next_interval = ((double)auth_timeout) - elapsed;
              _dbus_verbose ("Connection %p authentication expires in %d milliseconds\n",
                             connection, next_interval);
          
              break;
            }
      
          link = next;
        }
    }

  bus_expire_timeout_set_interval (connections->expire_timeout,
                                   next_interval);
}

static dbus_bool_t
expire_incomplete_timeout (void *data)
{
  BusConnections *connections = data;

  _dbus_verbose ("Running\n");
  
  /* note that this may remove the timeout */
  bus_connections_expire_incomplete (connections);

  return TRUE;
}

dbus_bool_t
bus_connection_get_unix_groups  (DBusConnection   *connection,
                                 unsigned long   **groups,
                                 int              *n_groups,
                                 DBusError        *error)
{
  /* Assigning dbus_gid_t to unsigned long is lossless (in fact
   * they are the same type) */
  _DBUS_STATIC_ASSERT (sizeof (unsigned long) == sizeof (dbus_gid_t));

  const dbus_gid_t *groups_borrowed = NULL;
  DBusCredentials *credentials;
  unsigned long uid;
  size_t n = 0;

  *groups = NULL;
  *n_groups = 0;

  credentials = _dbus_connection_get_credentials (connection);

  if (credentials != NULL &&
      _dbus_credentials_get_unix_gids (credentials, &groups_borrowed, &n))
    {
      size_t i;

      /* We got the group IDs from SO_PEERGROUPS or equivalent - no
       * need to ask NSS */

      *n_groups = n;
      *groups = dbus_new (unsigned long, n);

      if (*groups == NULL)
        {
          BUS_SET_OOM (error);
          return FALSE;
        }

      for (i = 0; i < n; i++)
        (*groups)[i] = groups_borrowed[i];

      return TRUE;
    }

  if (dbus_connection_get_unix_user (connection, &uid))
    {
      if (!_dbus_unix_groups_from_uid (uid, groups, n_groups))
        {
          _dbus_verbose ("Did not get any groups for UID %lu\n",
                         uid);
          return FALSE;
        }
      else
        {
          _dbus_verbose ("Got %d groups for UID %lu\n",
                         *n_groups, uid);
          return TRUE;
        }
    }
  else
    return TRUE; /* successfully got 0 groups */
}

dbus_bool_t
bus_connection_is_in_unix_group (DBusConnection *connection,
                                 unsigned long   gid)
{
  int i;
  unsigned long *group_ids;
  int n_group_ids;

  if (!bus_connection_get_unix_groups (connection, &group_ids, &n_group_ids,
                                       NULL))
    return FALSE;

  i = 0;
  while (i < n_group_ids)
    {
      if (group_ids[i] == gid)
        {
          dbus_free (group_ids);
          return TRUE;
        }
      ++i;
    }

  dbus_free (group_ids);
  return FALSE;
}

const char *
bus_connection_get_loginfo (DBusConnection        *connection)
{
  BusConnectionData *d;
    
  d = BUS_CONNECTION_DATA (connection);
  _dbus_assert(d != NULL);

  if (!bus_connection_is_active (connection))
    return "inactive";
  return d->cached_loginfo_string;  
}

BusClientPolicy*
bus_connection_get_policy (DBusConnection *connection)
{
  BusConnectionData *d;
    
  d = BUS_CONNECTION_DATA (connection);

  _dbus_assert (d != NULL);
  _dbus_assert (d->policy != NULL);
  
  return d->policy;
}

/*
 * This is used to avoid covering the same connection twice when
 * traversing connections. Note that it assumes we will
 * bus_connection_mark_stamp() each connection at least once per
 * INT_MAX increments of the global stamp, or wraparound would break
 * things.
 */
void
bus_connections_increment_stamp (BusConnections *connections)
{
  connections->stamp += 1;
}

/* Mark connection with current stamp, return TRUE if it
 * didn't already have that stamp
 */
dbus_bool_t
bus_connection_mark_stamp (DBusConnection *connection)
{
  BusConnectionData *d;
  
  d = BUS_CONNECTION_DATA (connection);
  
  _dbus_assert (d != NULL);

  if (d->stamp == d->connections->stamp)
    return FALSE;
  else
    {
      d->stamp = d->connections->stamp;
      return TRUE;
    }
}

BusContext*
bus_connection_get_context (DBusConnection *connection)
{
  BusConnectionData *d;

  d = BUS_CONNECTION_DATA (connection);

  _dbus_assert (d != NULL);

  return d->connections->context;
}

BusConnections*
bus_connection_get_connections (DBusConnection *connection)
{
  BusConnectionData *d;
    
  d = BUS_CONNECTION_DATA (connection);

  _dbus_assert (d != NULL);

  return d->connections;
}

BusRegistry*
bus_connection_get_registry (DBusConnection *connection)
{
  BusConnectionData *d;

  d = BUS_CONNECTION_DATA (connection);

  _dbus_assert (d != NULL);

  return bus_context_get_registry (d->connections->context);
}

BusActivation*
bus_connection_get_activation (DBusConnection *connection)
{
  BusConnectionData *d;

  d = BUS_CONNECTION_DATA (connection);

  _dbus_assert (d != NULL);

  return bus_context_get_activation (d->connections->context);
}

BusMatchmaker*
bus_connection_get_matchmaker (DBusConnection *connection)
{
  BusConnectionData *d;

  d = BUS_CONNECTION_DATA (connection);

  _dbus_assert (d != NULL);

  return bus_context_get_matchmaker (d->connections->context);
}

BusSELinuxID*
bus_connection_get_selinux_id (DBusConnection *connection)
{
  BusConnectionData *d;

  d = BUS_CONNECTION_DATA (connection);

  _dbus_assert (d != NULL);

  return d->selinux_id;
}

BusAppArmorConfinement*
bus_connection_dup_apparmor_confinement (DBusConnection *connection)
{
  BusConnectionData *d;

  d = BUS_CONNECTION_DATA (connection);

  _dbus_assert (d != NULL);

  bus_apparmor_confinement_ref (d->apparmor_confinement);
  return d->apparmor_confinement;
}

/**
 * Checks whether the connection is registered with the message bus.
 *
 * @param connection the connection
 * @returns #TRUE if we're an active message bus participant
 */
dbus_bool_t
bus_connection_is_active (DBusConnection *connection)
{
  BusConnectionData *d;

  d = BUS_CONNECTION_DATA (connection);
  _dbus_assert(d != NULL);
  
  return d->name != NULL;
}

dbus_bool_t
bus_connection_preallocate_oom_error (DBusConnection *connection)
{
  DBusMessage *message;
  DBusPreallocatedSend *preallocated;
  BusConnectionData *d;

  d = BUS_CONNECTION_DATA (connection);  

  _dbus_assert (d != NULL);

  if (d->oom_preallocated != NULL)
    return TRUE;
  
  preallocated = dbus_connection_preallocate_send (connection);
  if (preallocated == NULL)
    return FALSE;

  message = dbus_message_new (DBUS_MESSAGE_TYPE_ERROR);

  if (message == NULL)
    {
      dbus_connection_free_preallocated_send (connection, preallocated);
      return FALSE;
    }

  /* d->name may be NULL, but that is OK */
  if (!dbus_message_set_error_name (message, DBUS_ERROR_NO_MEMORY) ||
      !dbus_message_set_destination (message, d->name) ||
      !dbus_message_set_sender (message,
                                DBUS_SERVICE_DBUS))
    {
      dbus_connection_free_preallocated_send (connection, preallocated);
      dbus_message_unref (message);
      return FALSE;
    }
  
  /* set reply serial to placeholder value just so space is already allocated
   * for it.
   */
  if (!dbus_message_set_reply_serial (message, 14))
    {
      dbus_connection_free_preallocated_send (connection, preallocated);
      dbus_message_unref (message);
      return FALSE;
    }

  d->oom_message = message;
  d->oom_preallocated = preallocated;
  
  return TRUE;
}

void
bus_connection_send_oom_error (DBusConnection *connection,
                               DBusMessage    *in_reply_to)
{
  BusConnectionData *d;

  d = BUS_CONNECTION_DATA (connection);  

  _dbus_assert (d != NULL);  
  _dbus_assert (d->oom_message != NULL);

  bus_context_log (d->connections->context, DBUS_SYSTEM_LOG_WARNING,
                   "dbus-daemon transaction failed (OOM), sending error to "
                   "sender %s", bus_connection_get_loginfo (connection));

  /* should always succeed since we set it to a placeholder earlier */
  if (!dbus_message_set_reply_serial (d->oom_message,
                                      dbus_message_get_serial (in_reply_to)))
    _dbus_assert_not_reached ("Failed to set reply serial for preallocated oom message");

  _dbus_assert (dbus_message_get_sender (d->oom_message) != NULL);
  
  dbus_connection_send_preallocated (connection, d->oom_preallocated,
                                     d->oom_message, NULL);

  dbus_message_unref (d->oom_message);
  d->oom_message = NULL;
  d->oom_preallocated = NULL;
}

#ifdef DBUS_ENABLE_STATS
static void
update_peak (int *peak,
             int n)
{
  if (*peak < n)
    *peak = n;
}
#endif

void
bus_connection_add_match_rule_link (DBusConnection *connection,
                                    DBusList       *link)
{
  BusConnectionData *d;

  d = BUS_CONNECTION_DATA (connection);
  _dbus_assert (d != NULL);

  _dbus_list_append_link (&d->match_rules, link);

  d->n_match_rules += 1;

#ifdef DBUS_ENABLE_STATS
  update_peak (&d->peak_match_rules, d->n_match_rules);
  update_peak (&d->connections->peak_match_rules_per_conn, d->n_match_rules);

  d->connections->total_match_rules += 1;
  update_peak (&d->connections->peak_match_rules,
               d->connections->total_match_rules);
#endif
}

dbus_bool_t
bus_connection_add_match_rule (DBusConnection *connection,
                               BusMatchRule   *rule)
{
    DBusList *link;

  link = _dbus_list_alloc_link (rule);

  if (link == NULL)
    return FALSE;

  bus_connection_add_match_rule_link (connection, link);

  return TRUE;
}

void
bus_connection_remove_match_rule (DBusConnection *connection,
                                  BusMatchRule   *rule)
{
  BusConnectionData *d;

  d = BUS_CONNECTION_DATA (connection);
  _dbus_assert (d != NULL);

  _dbus_list_remove_last (&d->match_rules, rule);

  d->n_match_rules -= 1;
  _dbus_assert (d->n_match_rules >= 0);

#ifdef DBUS_ENABLE_STATS
  d->connections->total_match_rules -= 1;
#endif
}

int
bus_connection_get_n_match_rules (DBusConnection *connection)
{
  BusConnectionData *d;

  d = BUS_CONNECTION_DATA (connection);
  _dbus_assert (d != NULL);
  
  return d->n_match_rules;
}

/**
 * Checks whether the connection owns any name with a given prefix,
 * regardless of whether the type of ownership is primary or queued.
 *
 * @note A name matches to a prefix if it is equal to the prefix,
 * or if it starts with the prefix followed by a dot. This is the same
 * rule as the 'own_prefix' checking rule.
 *
 * @param connection the connection
 * @param name_prefix the prefix
 * @returns #TRUE if the connection owns at least one name with the prefix,
 * regardless of the type of ownership
 */
dbus_bool_t
bus_connection_is_queued_owner_by_prefix (DBusConnection *connection,
                                          const char *name_prefix)
{
  BusConnectionData *d;
  DBusList *link;

  d = BUS_CONNECTION_DATA (connection);
  _dbus_assert (d != NULL);

  link = _dbus_list_get_first_link (&d->services_owned);
  while (link != NULL)
    {
      BusService *service = link->data;
      DBusString str;

      _dbus_string_init_const (&str, bus_service_get_name (service));

      if (_dbus_string_starts_with_words_c_str (&str, name_prefix, '.'))
        return TRUE;

      link = _dbus_list_get_next_link (&d->services_owned, link);
    }

  return FALSE;
}

void
bus_connection_add_owned_service_link (DBusConnection *connection,
                                       DBusList       *link)
{
  BusConnectionData *d;

  d = BUS_CONNECTION_DATA (connection);
  _dbus_assert (d != NULL);

  _dbus_list_append_link (&d->services_owned, link);

  d->n_services_owned += 1;

#ifdef DBUS_ENABLE_STATS
  update_peak (&d->peak_bus_names, d->n_services_owned);
  update_peak (&d->connections->peak_bus_names_per_conn,
               d->n_services_owned);

  d->connections->total_bus_names += 1;
  update_peak (&d->connections->peak_bus_names,
               d->connections->total_bus_names);
#endif
}

dbus_bool_t
bus_connection_add_owned_service (DBusConnection *connection,
                                  BusService     *service)
{
  DBusList *link;

  link = _dbus_list_alloc_link (service);

  if (link == NULL)
    return FALSE;

  bus_connection_add_owned_service_link (connection, link);

  return TRUE;
}

void
bus_connection_remove_owned_service (DBusConnection *connection,
                                     BusService     *service)
{
  BusConnectionData *d;

  d = BUS_CONNECTION_DATA (connection);
  _dbus_assert (d != NULL);

  _dbus_list_remove_last (&d->services_owned, service);

  d->n_services_owned -= 1;
  _dbus_assert (d->n_services_owned >= 0);

#ifdef DBUS_ENABLE_STATS
  d->connections->total_bus_names -= 1;
#endif
}

int
bus_connection_get_n_services_owned (DBusConnection *connection)
{
  BusConnectionData *d;

  d = BUS_CONNECTION_DATA (connection);
  _dbus_assert (d != NULL);
  
  return d->n_services_owned;
}

dbus_bool_t
bus_connection_complete (DBusConnection   *connection,
			 const DBusString *name,
                         DBusError        *error)
{
  BusConnectionData *d;
  unsigned long uid;
  
  d = BUS_CONNECTION_DATA (connection);
  _dbus_assert (d != NULL);
  _dbus_assert (d->name == NULL);
  _dbus_assert (d->policy == NULL);

  _dbus_assert (!bus_connection_is_active (connection));
  
  if (!_dbus_string_copy_data (name, &d->name))
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  _dbus_assert (d->name != NULL);
  
  _dbus_verbose ("Name %s assigned to %p\n", d->name, connection);

  d->policy = bus_context_create_client_policy (d->connections->context,
                                                connection,
                                                error);

  /* we may have a NULL policy on OOM or error getting list of
   * groups for a user. In the latter case we don't handle it so
   * well currently, as it will just keep failing over and over.
   */

  if (d->policy == NULL)
    {
      _dbus_verbose ("Failed to create security policy for connection %p\n",
                     connection);
      _DBUS_ASSERT_ERROR_IS_SET (error);
      dbus_free (d->name);
      d->name = NULL;
      return FALSE;
    }
  
  if (dbus_connection_get_unix_user (connection, &uid))
    {
      if (!adjust_connections_for_uid (d->connections,
                                       uid, 1))
        goto fail;
    }

  /* Create and cache a string which holds information about the 
   * peer process; used for logging purposes.
   */
  if (!cache_peer_loginfo_string (d, connection))
    goto fail;

  /* Now the connection is active, move it between lists */
  _dbus_list_unlink (&d->connections->incomplete,
                     d->link_in_connection_list);
  d->connections->n_incomplete -= 1;
  _dbus_list_append_link (&d->connections->completed,
                          d->link_in_connection_list);
  d->connections->n_completed += 1;

  _dbus_assert (d->connections->n_incomplete >= 0);
  _dbus_assert (d->connections->n_completed > 0);

  /* If we have dropped below the max. number of incomplete
   * connections, start accept()ing again */
  bus_context_check_all_watches (d->connections->context);

  /* See if we can remove the timeout */
  bus_connections_expire_incomplete (d->connections);

  _dbus_assert (bus_connection_is_active (connection));
  
  return TRUE;
fail:
  BUS_SET_OOM (error);
  dbus_free (d->name);
  d->name = NULL;
  if (d->policy)
    bus_client_policy_unref (d->policy);
  d->policy = NULL;
  return FALSE;
}

dbus_bool_t
bus_connections_reload_policy (BusConnections *connections,
                               DBusError      *error)
{
  BusConnectionData *d;
  DBusConnection *connection;
  DBusList *link;

  _dbus_assert (connections != NULL);
  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  for (link = _dbus_list_get_first_link (&(connections->completed));
       link;
       link = _dbus_list_get_next_link (&(connections->completed), link))
    {
      connection = link->data;
      d = BUS_CONNECTION_DATA (connection);
      _dbus_assert (d != NULL);
      _dbus_assert (d->policy != NULL);

      bus_client_policy_unref (d->policy);
      d->policy = bus_context_create_client_policy (connections->context,
                                                    connection,
                                                    error);
      if (d->policy == NULL)
        {
          _dbus_verbose ("Failed to create security policy for connection %p\n",
                      connection);
          _DBUS_ASSERT_ERROR_IS_SET (error);
          return FALSE;
        }
    }

  return TRUE;
}

const char *
bus_connection_get_name (DBusConnection *connection)
{
  BusConnectionData *d;
  
  d = BUS_CONNECTION_DATA (connection);
  _dbus_assert (d != NULL);
  
  return d->name;
}

/**
 * Check whether completing the passed-in connection would
 * exceed limits, and if so set error and return #FALSE
 */
dbus_bool_t
bus_connections_check_limits (BusConnections  *connections,
                              DBusConnection  *requesting_completion,
                              const char     **limit_name_out,
                              int             *limit_out,
                              DBusError       *error)
{
  unsigned long uid;
  int limit;

  limit = bus_context_get_max_completed_connections (connections->context);

  if (connections->n_completed >= limit)
    {
      if (limit_name_out != NULL)
        *limit_name_out = "max_completed_connections";

      if (limit_out != NULL)
        *limit_out = limit;

      dbus_set_error (error, DBUS_ERROR_LIMITS_EXCEEDED,
                      "The maximum number of active connections has been reached");
      return FALSE;
    }
  
  if (dbus_connection_get_unix_user (requesting_completion, &uid))
    {
      limit = bus_context_get_max_connections_per_user (connections->context);

      if (get_connections_for_uid (connections, uid) >= limit)
        {
          if (limit_name_out != NULL)
            *limit_name_out = "max_connections_per_user";

          if (limit_out != NULL)
            *limit_out = limit;

          dbus_set_error (error, DBUS_ERROR_LIMITS_EXCEEDED,
                          "The maximum number of active connections for UID %lu has been reached",
                          uid);
          return FALSE;
        }
    }
  
  return TRUE;
}

static void
bus_pending_reply_free (BusPendingReply *pending)
{
  _dbus_verbose ("Freeing pending reply %p, replier %p receiver %p serial %u\n",
                 pending,
                 pending->will_send_reply,
                 pending->will_get_reply,
                 pending->reply_serial);

  dbus_free (pending);
}

static dbus_bool_t
bus_pending_reply_send_no_reply (BusConnections  *connections,
                                 BusTransaction  *transaction,
                                 BusPendingReply *pending)
{
  DBusMessage *message;
  DBusMessageIter iter;
  dbus_bool_t retval;
  const char *errmsg;

  retval = FALSE;
  
  message = dbus_message_new (DBUS_MESSAGE_TYPE_ERROR);
  if (message == NULL)
    return FALSE;
  
  dbus_message_set_no_reply (message, TRUE);
  
  if (!dbus_message_set_reply_serial (message,
                                      pending->reply_serial))
    goto out;

  if (!dbus_message_set_error_name (message,
                                    DBUS_ERROR_NO_REPLY))
    goto out;

  /* If you change these messages, adjust test/dbus-daemon.c to match */
  if (pending->will_send_reply == NULL)
    errmsg = "Message recipient disconnected from message bus without replying";
  else
    errmsg = "Message did not receive a reply (timeout by message bus)";

  dbus_message_iter_init_append (message, &iter);
  if (!dbus_message_iter_append_basic (&iter, DBUS_TYPE_STRING, &errmsg))
    goto out;
    
  if (!bus_transaction_send_from_driver (transaction, pending->will_get_reply,
                                         message))
    goto out;

  retval = TRUE;

 out:
  dbus_message_unref (message);
  return retval;
}

static dbus_bool_t
bus_pending_reply_expired (BusExpireList *list,
                           DBusList      *link,
                           void          *data)
{
  BusPendingReply *pending = link->data;
  BusConnections *connections = data;
  BusTransaction *transaction;
  
  /* No reply is forthcoming. So nuke it if we can. If not,
   * leave it in the list to try expiring again later when we
   * get more memory.
   */

  _dbus_verbose ("Expiring pending reply %p, replier %p receiver %p serial %u\n",
                 pending,
                 pending->will_send_reply,
                 pending->will_get_reply,
                 pending->reply_serial);
  
  transaction = bus_transaction_new (connections->context);
  if (transaction == NULL)
    return FALSE;
  
  if (!bus_pending_reply_send_no_reply (connections,
                                        transaction,
                                        pending))
    {
      bus_transaction_cancel_and_free (transaction);
      return FALSE;
    }

  bus_expire_list_remove_link (connections->pending_replies, link);

  bus_pending_reply_free (pending);
  bus_transaction_execute_and_free (transaction);

  return TRUE;
}

static void
bus_connection_drop_pending_replies (BusConnections  *connections,
                                     DBusConnection  *connection)
{
  /* The DBusConnection is almost 100% finalized here, so you can't
   * do anything with it except check for pointer equality
   */
  DBusList *link;

  _dbus_verbose ("Dropping pending replies that involve connection %p\n",
                 connection);
  
  link = bus_expire_list_get_first_link (connections->pending_replies);
  while (link != NULL)
    {
      DBusList *next;
      BusPendingReply *pending;

      next = bus_expire_list_get_next_link (connections->pending_replies,
                                            link);
      pending = link->data;

      if (pending->will_get_reply == connection)
        {
          /* We don't need to track this pending reply anymore */

          _dbus_verbose ("Dropping pending reply %p, replier %p receiver %p serial %u\n",
                         pending,
                         pending->will_send_reply,
                         pending->will_get_reply,
                         pending->reply_serial);
          
          bus_expire_list_remove_link (connections->pending_replies,
                                       link);
          bus_pending_reply_free (pending);
        }
      else if (pending->will_send_reply == connection)
        {
          /* The reply isn't going to be sent, so set things
           * up so it will be expired right away
           */
          _dbus_verbose ("Will expire pending reply %p, replier %p receiver %p serial %u\n",
                         pending,
                         pending->will_send_reply,
                         pending->will_get_reply,
                         pending->reply_serial);
          
          pending->will_send_reply = NULL;
          pending->expire_item.added_tv_sec = 0;
          pending->expire_item.added_tv_usec = 0;

          bus_expire_list_recheck_immediately (connections->pending_replies);
        }
      
      link = next;
    }
}


typedef struct
{
  BusPendingReply *pending;
  BusConnections  *connections;
} CancelPendingReplyData;

static void
cancel_pending_reply (void *data)
{
  CancelPendingReplyData *d = data;

  _dbus_verbose ("d = %p\n", d);
  
  if (!bus_expire_list_remove (d->connections->pending_replies,
                               &d->pending->expire_item))
    _dbus_assert_not_reached ("pending reply did not exist to be cancelled");

  bus_pending_reply_free (d->pending); /* since it's been cancelled */
}

static void
cancel_pending_reply_data_free (void *data)
{
  CancelPendingReplyData *d = data;

  _dbus_verbose ("d = %p\n", d);
  
  /* d->pending should be either freed or still
   * in the list of pending replies (owned by someone
   * else)
   */
  
  dbus_free (d);
}

/*
 * Record that a reply is allowed; return TRUE on success.
 */
dbus_bool_t
bus_connections_expect_reply (BusConnections  *connections,
                              BusTransaction  *transaction,
                              DBusConnection  *will_get_reply,
                              DBusConnection  *will_send_reply,
                              DBusMessage     *reply_to_this,
                              DBusError       *error)
{
  BusPendingReply *pending;
  dbus_uint32_t reply_serial;
  DBusList *link;
  CancelPendingReplyData *cprd;
  int count;
  int limit;

  _dbus_assert (will_get_reply != NULL);
  _dbus_assert (will_send_reply != NULL);
  _dbus_assert (reply_to_this != NULL);
  
  if (dbus_message_get_no_reply (reply_to_this))
    return TRUE; /* we won't allow a reply, since client doesn't care for one. */
  
  reply_serial = dbus_message_get_serial (reply_to_this);

  link = bus_expire_list_get_first_link (connections->pending_replies);
  count = 0;
  while (link != NULL)
    {
      pending = link->data;

      if (pending->reply_serial == reply_serial &&
          pending->will_get_reply == will_get_reply &&
          pending->will_send_reply == will_send_reply)
        {
          dbus_set_error (error, DBUS_ERROR_ACCESS_DENIED,
                          "Message has the same reply serial as a currently-outstanding existing method call");
          return FALSE;
        }
      
      link = bus_expire_list_get_next_link (connections->pending_replies,
                                            link);
      if (pending->will_get_reply == will_get_reply)
        ++count;
    }

  limit = bus_context_get_max_replies_per_connection (connections->context);

  if (count >= limit)
    {
      bus_context_log (connections->context, DBUS_SYSTEM_LOG_WARNING,
                       "The maximum number of pending replies for "
                       "\"%s\" (%s) has been reached "
                       "(max_replies_per_connection=%d)",
                       bus_connection_get_name (will_get_reply),
                       bus_connection_get_loginfo (will_get_reply),
                       limit);

      dbus_set_error (error, DBUS_ERROR_LIMITS_EXCEEDED,
		      "The maximum number of pending replies per connection has been reached");
      return FALSE;
    }

  pending = dbus_new0 (BusPendingReply, 1);
  if (pending == NULL)
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

#ifdef DBUS_ENABLE_VERBOSE_MODE
  /* so we can see a not-yet-added pending reply */
  pending->expire_item.added_tv_sec = 1;
  pending->expire_item.added_tv_usec = 1;
#endif

  pending->will_get_reply = will_get_reply;
  pending->will_send_reply = will_send_reply;
  pending->reply_serial = reply_serial;
  
  cprd = dbus_new0 (CancelPendingReplyData, 1);
  if (cprd == NULL)
    {
      BUS_SET_OOM (error);
      bus_pending_reply_free (pending);
      return FALSE;
    }
  
  if (!bus_expire_list_add (connections->pending_replies,
                            &pending->expire_item))
    {
      BUS_SET_OOM (error);
      dbus_free (cprd);
      bus_pending_reply_free (pending);
      return FALSE;
    }

  if (!bus_transaction_add_cancel_hook (transaction,
                                        cancel_pending_reply,
                                        cprd,
                                        cancel_pending_reply_data_free))
    {
      BUS_SET_OOM (error);
      bus_expire_list_remove (connections->pending_replies, &pending->expire_item);
      dbus_free (cprd);
      bus_pending_reply_free (pending);
      return FALSE;
    }
                                        
  cprd->pending = pending;
  cprd->connections = connections;
  
  _dbus_get_monotonic_time (&pending->expire_item.added_tv_sec,
                            &pending->expire_item.added_tv_usec);

  _dbus_verbose ("Added pending reply %p, replier %p receiver %p serial %u\n",
                 pending,
                 pending->will_send_reply,
                 pending->will_get_reply,
                 pending->reply_serial);
  
  return TRUE;
}

typedef struct
{
  DBusList        *link;
  BusConnections  *connections;
} CheckPendingReplyData;

static void
cancel_check_pending_reply (void *data)
{
  CheckPendingReplyData *d = data;

  _dbus_verbose ("d = %p\n",d);

  bus_expire_list_add_link (d->connections->pending_replies,
                            d->link);
  d->link = NULL;
}

static void
check_pending_reply_data_free (void *data)
{
  CheckPendingReplyData *d = data;

  _dbus_verbose ("d = %p\n",d);
  
  if (d->link != NULL)
    {
      BusPendingReply *pending = d->link->data;
      
      _dbus_assert (!bus_expire_list_contains_item (d->connections->pending_replies,
                                                    &pending->expire_item));
      
      bus_pending_reply_free (pending);
      _dbus_list_free_link (d->link);
    }
  
  dbus_free (d);
}

/*
 * Check whether a reply is allowed, remove BusPendingReply
 * if so, return TRUE if so.
 */
dbus_bool_t
bus_connections_check_reply (BusConnections *connections,
                             BusTransaction *transaction,
                             DBusConnection *sending_reply,
                             DBusConnection *receiving_reply,
                             DBusMessage    *reply,
                             DBusError      *error)
{
  CheckPendingReplyData *cprd;
  DBusList *link;
  dbus_uint32_t reply_serial;
  
  _dbus_assert (sending_reply != NULL);
  _dbus_assert (receiving_reply != NULL);

  reply_serial = dbus_message_get_reply_serial (reply);

  link = bus_expire_list_get_first_link (connections->pending_replies);
  while (link != NULL)
    {
      BusPendingReply *pending = link->data;

      if (pending->reply_serial == reply_serial &&
          pending->will_get_reply == receiving_reply &&
          pending->will_send_reply == sending_reply)
        {
          _dbus_verbose ("Found pending reply with serial %u\n", reply_serial);
          break;
        }
      
      link = bus_expire_list_get_next_link (connections->pending_replies,
                                            link);
    }

  if (link == NULL)
    {
      _dbus_verbose ("No pending reply expected\n");

      return FALSE;
    }

  cprd = dbus_new0 (CheckPendingReplyData, 1);
  if (cprd == NULL)
    {
      BUS_SET_OOM (error);
      return FALSE;
    }
  
  if (!bus_transaction_add_cancel_hook (transaction,
                                        cancel_check_pending_reply,
                                        cprd,
                                        check_pending_reply_data_free))
    {
      BUS_SET_OOM (error);
      dbus_free (cprd);
      return FALSE;
    }

  cprd->link = link;
  cprd->connections = connections;
  
  bus_expire_list_unlink (connections->pending_replies,
                          link);
  
  _dbus_assert (!bus_expire_list_contains_item (connections->pending_replies, link->data));

  return TRUE;
}

/*
 * Transactions
 *
 * Note that this is fairly fragile; in particular, don't try to use
 * one transaction across any main loop iterations.
 */

typedef struct
{
  BusTransaction *transaction;
  DBusMessage    *message;
  DBusPreallocatedSend *preallocated;
} MessageToSend;

typedef struct
{
  BusTransactionCancelFunction cancel_function;
  DBusFreeFunction free_data_function;
  void *data;
} CancelHook;

struct BusTransaction
{
  DBusList *connections;
  BusContext *context;
  DBusList *cancel_hooks;
};

static void
message_to_send_free (DBusConnection *connection,
                      MessageToSend  *to_send)
{
  if (to_send->message)
    dbus_message_unref (to_send->message);

  if (to_send->preallocated)
    dbus_connection_free_preallocated_send (connection, to_send->preallocated);

  dbus_free (to_send);
}

static void
cancel_hook_cancel (void *element,
                    void *data)
{
  CancelHook *ch = element;

  _dbus_verbose ("Running transaction cancel hook\n");
  
  if (ch->cancel_function)
    (* ch->cancel_function) (ch->data);  
}

static void
cancel_hook_free (void *element,
                  void *data)
{
  CancelHook *ch = element;

  if (ch->free_data_function)
    (* ch->free_data_function) (ch->data);

  dbus_free (ch);
}

static void
free_cancel_hooks (BusTransaction *transaction)
{
  _dbus_list_foreach (&transaction->cancel_hooks,
                      cancel_hook_free, NULL);
  
  _dbus_list_clear (&transaction->cancel_hooks);
}

BusTransaction*
bus_transaction_new (BusContext *context)
{
  BusTransaction *transaction;

  transaction = dbus_new0 (BusTransaction, 1);
  if (transaction == NULL)
    return NULL;

  transaction->context = context;
  
  return transaction;
}

BusContext*
bus_transaction_get_context (BusTransaction  *transaction)
{
  return transaction->context;
}

/**
 * Reserve enough memory to capture the given message if the
 * transaction goes through.
 */
dbus_bool_t
bus_transaction_capture (BusTransaction *transaction,
                         DBusConnection *sender,
                         DBusConnection *addressed_recipient,
                         DBusMessage    *message)
{
  BusConnections *connections;
  BusMatchmaker *mm;
  DBusList *link;
  DBusList *recipients = NULL;
  dbus_bool_t ret = FALSE;

  connections = bus_context_get_connections (transaction->context);

  /* shortcut: don't compose the message unless someone wants it */
  if (connections->monitors == NULL)
    return TRUE;

  mm = connections->monitor_matchmaker;
  /* This is non-null if there has ever been a monitor - we don't GC it.
   * There's little point, since there is up to 1 per process. */
  _dbus_assert (mm != NULL);

  if (!bus_matchmaker_get_recipients (mm, connections, sender,
        addressed_recipient, message, &recipients))
    goto out;

  for (link = _dbus_list_get_first_link (&recipients);
      link != NULL;
      link = _dbus_list_get_next_link (&recipients, link))
    {
      DBusConnection *recipient = link->data;

      if (!bus_transaction_send (transaction, sender, recipient, message))
        goto out;
    }

  ret = TRUE;

out:
  _dbus_list_clear (&recipients);
  return ret;
}

dbus_bool_t
bus_transaction_capture_error_reply (BusTransaction  *transaction,
                                     DBusConnection  *addressed_recipient,
                                     const DBusError *error,
                                     DBusMessage     *in_reply_to)
{
  BusConnections *connections;
  DBusMessage *reply;
  dbus_bool_t ret = FALSE;

  _dbus_assert (error != NULL);
  _DBUS_ASSERT_ERROR_IS_SET (error);

  connections = bus_context_get_connections (transaction->context);

  /* shortcut: don't compose the message unless someone wants it */
  if (connections->monitors == NULL)
    return TRUE;

  reply = dbus_message_new_error (in_reply_to,
                                  error->name,
                                  error->message);

  if (reply == NULL)
    return FALSE;

  if (!dbus_message_set_sender (reply, DBUS_SERVICE_DBUS))
    goto out;

  ret = bus_transaction_capture (transaction, NULL, addressed_recipient, reply);

out:
  dbus_message_unref (reply);
  return ret;
}

dbus_bool_t
bus_transaction_send_from_driver (BusTransaction *transaction,
                                  DBusConnection *connection,
                                  DBusMessage    *message)
{
  DBusError error = DBUS_ERROR_INIT;

  /* We have to set the sender to the driver, and have
   * to check security policy since it was not done in
   * dispatch.c
   */
  _dbus_verbose ("Sending %s %s %s from driver\n",
                 dbus_message_get_interface (message) ?
                 dbus_message_get_interface (message) : "(no interface)",
                 dbus_message_get_member (message) ?
                 dbus_message_get_member (message) : "(no member)",
                 dbus_message_get_error_name (message) ?
                 dbus_message_get_error_name (message) : "(no error name)");
                 
  if (!dbus_message_set_sender (message, DBUS_SERVICE_DBUS))
    return FALSE;

  if (bus_connection_is_active (connection))
    {
      if (!dbus_message_set_destination (message,
                                         bus_connection_get_name (connection)))
        return FALSE;
    }
  
  /* bus driver never wants a reply */
  dbus_message_set_no_reply (message, TRUE);

  /* Capture it for monitors, even if the real recipient's receive policy
   * does not allow it to receive this message from us (which would be odd).
   */
  if (!bus_transaction_capture (transaction, NULL, connection, message))
    return FALSE;

  /* If security policy doesn't allow the message, we would silently
   * eat it; the driver doesn't care about getting a reply. However,
   * if we're actively capturing messages, it's nice to log that we
   * tried to send it and did not allow ourselves to do so.
   */
  if (!bus_context_check_security_policy (bus_transaction_get_context (transaction),
                                          transaction,
                                          NULL, connection, connection,
                                          message, NULL, &error))
    {
      if (!bus_transaction_capture_error_reply (transaction, connection,
                                                &error, message))
        {
          bus_context_log (transaction->context, DBUS_SYSTEM_LOG_WARNING,
                           "message from dbus-daemon rejected but not enough "
                           "memory to capture it");
        }

      /* This is not fatal to the transaction so silently eat the disallowed
       * message (see reasoning above) */
      dbus_error_free (&error);
      return TRUE;
    }

  return bus_transaction_send (transaction, NULL, connection, message);
}

dbus_bool_t
bus_transaction_send (BusTransaction *transaction,
                      DBusConnection *sender,
                      DBusConnection *destination,
                      DBusMessage    *message)
{
  MessageToSend *to_send;
  BusConnectionData *d;
  DBusList *link;

  _dbus_verbose ("  trying to add %s interface=%s member=%s error=%s to transaction%s\n",
                 dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_ERROR ? "error" :
                 dbus_message_get_reply_serial (message) != 0 ? "reply" :
                 "message",
                 dbus_message_get_interface (message) ?
                 dbus_message_get_interface (message) : "(unset)",
                 dbus_message_get_member (message) ?
                 dbus_message_get_member (message) : "(unset)",
                 dbus_message_get_error_name (message) ?
                 dbus_message_get_error_name (message) : "(unset)",
                 dbus_connection_get_is_connected (destination) ?
                 "" : " (disconnected)");

  _dbus_assert (dbus_message_get_sender (message) != NULL);
  
  if (!dbus_connection_get_is_connected (destination))
    return TRUE; /* silently ignore disconnected destinations */
  
  d = BUS_CONNECTION_DATA (destination);
  _dbus_assert (d != NULL);

  /* You might think that this is too late to be setting header fields,
   * because the message is locked before sending - but remember that
   * the message isn't actually queued to be sent (and hence locked)
   * until we know we have enough memory for the entire transaction,
   * and that doesn't happen until we know all the recipients.
   * So this is about the last possible time we could edit the header. */
  if ((d->want_headers & BUS_EXTRA_HEADERS_CONTAINER_INSTANCE) &&
      dbus_message_get_container_instance (message) == NULL)
    {
      const char *path;

      if (sender == NULL ||
          !bus_containers_connection_is_contained (sender, &path,
                                                   NULL, NULL))
        path = "/";

      if (!dbus_message_set_container_instance (message, path))
        return FALSE;
    }

  to_send = dbus_new (MessageToSend, 1);
  if (to_send == NULL)
    {
      return FALSE;
    }

  to_send->preallocated = dbus_connection_preallocate_send (destination);
  if (to_send->preallocated == NULL)
    {
      dbus_free (to_send);
      return FALSE;
    }  
  
  dbus_message_ref (message);
  to_send->message = message;
  to_send->transaction = transaction;

  _dbus_verbose ("about to prepend message\n");
  
  if (!_dbus_list_prepend (&d->transaction_messages, to_send))
    {
      message_to_send_free (destination, to_send);
      return FALSE;
    }

  _dbus_verbose ("prepended message\n");
  
  /* See if we already had this destination in the list
   * for this transaction. If we have a pending message,
   * then we should already be in transaction->connections
   */
  link = _dbus_list_get_first_link (&d->transaction_messages);
  _dbus_assert (link->data == to_send);
  link = _dbus_list_get_next_link (&d->transaction_messages, link);
  while (link != NULL)
    {
      MessageToSend *m = link->data;
      DBusList *next = _dbus_list_get_next_link (&d->transaction_messages, link);
      
      if (m->transaction == transaction)
        break;
        
      link = next;
    }

  if (link == NULL)
    {
      if (!_dbus_list_prepend (&transaction->connections, destination))
        {
          _dbus_list_remove (&d->transaction_messages, to_send);
          message_to_send_free (destination, to_send);
          return FALSE;
        }
    }

  return TRUE;
}

static void
transaction_free (BusTransaction *transaction)
{
  _dbus_assert (transaction->connections == NULL);

  free_cancel_hooks (transaction);

  dbus_free (transaction);
}

static void
connection_cancel_transaction (DBusConnection *connection,
                               BusTransaction *transaction)
{
  DBusList *link;
  BusConnectionData *d;
  
  d = BUS_CONNECTION_DATA (connection);
  _dbus_assert (d != NULL);
  
  link = _dbus_list_get_first_link (&d->transaction_messages);
  while (link != NULL)
    {
      MessageToSend *m = link->data;
      DBusList *next = _dbus_list_get_next_link (&d->transaction_messages, link);
      
      if (m->transaction == transaction)
        {
          _dbus_list_remove_link (&d->transaction_messages,
                                  link);
          
          message_to_send_free (connection, m);
        }
        
      link = next;
    }
}

void
bus_transaction_cancel_and_free (BusTransaction *transaction)
{
  DBusConnection *connection;

  _dbus_verbose ("TRANSACTION: cancelled\n");
  
  while ((connection = _dbus_list_pop_first (&transaction->connections)))
    connection_cancel_transaction (connection, transaction);

  _dbus_list_foreach (&transaction->cancel_hooks,
                      cancel_hook_cancel, NULL);

  transaction_free (transaction);
}

static void
connection_execute_transaction (DBusConnection *connection,
                                BusTransaction *transaction)
{
  DBusList *link;
  BusConnectionData *d;
  
  d = BUS_CONNECTION_DATA (connection);
  _dbus_assert (d != NULL);

  /* Send the queue in order (FIFO) */
  link = _dbus_list_get_last_link (&d->transaction_messages);
  while (link != NULL)
    {
      MessageToSend *m = link->data;
      DBusList *prev = _dbus_list_get_prev_link (&d->transaction_messages, link);
      
      if (m->transaction == transaction)
        {
          _dbus_list_remove_link (&d->transaction_messages,
                                  link);

          _dbus_assert (dbus_message_get_sender (m->message) != NULL);
          
          dbus_connection_send_preallocated (connection,
                                             m->preallocated,
                                             m->message,
                                             NULL);

          m->preallocated = NULL; /* so we don't double-free it */
          
          message_to_send_free (connection, m);
        }
        
      link = prev;
    }
}

void
bus_transaction_execute_and_free (BusTransaction *transaction)
{
  /* For each connection in transaction->connections
   * send the messages
   */
  DBusConnection *connection;

  _dbus_verbose ("TRANSACTION: executing\n");
  
  while ((connection = _dbus_list_pop_first (&transaction->connections)))
    connection_execute_transaction (connection, transaction);

  transaction_free (transaction);
}

static void
bus_connection_remove_transactions (DBusConnection *connection)
{
  MessageToSend *to_send;
  BusConnectionData *d;
  
  d = BUS_CONNECTION_DATA (connection);
  _dbus_assert (d != NULL);
  
  while ((to_send = _dbus_list_get_first (&d->transaction_messages)))
    {
      /* only has an effect for the first MessageToSend listing this transaction */
      _dbus_list_remove (&to_send->transaction->connections,
                         connection);

      _dbus_list_remove (&d->transaction_messages, to_send);
      message_to_send_free (connection, to_send);
    }
}

/**
 * Converts the DBusError to a message reply
 */
dbus_bool_t
bus_transaction_send_error_reply (BusTransaction  *transaction,
                                  DBusConnection  *connection,
                                  const DBusError *error,
                                  DBusMessage     *in_reply_to)
{
  DBusMessage *reply;
  
  _dbus_assert (error != NULL);
  _DBUS_ASSERT_ERROR_IS_SET (error);
  
  _dbus_verbose ("Sending error reply %s \"%s\"\n",
                 error->name, error->message);

  reply = dbus_message_new_error (in_reply_to,
                                  error->name,
                                  error->message);
  if (reply == NULL)
    return FALSE;

  if (!bus_transaction_send_from_driver (transaction, connection, reply))
    {
      dbus_message_unref (reply);
      return FALSE;
    }

  dbus_message_unref (reply);
  
  return TRUE;
}

dbus_bool_t
bus_transaction_add_cancel_hook (BusTransaction               *transaction,
                                 BusTransactionCancelFunction  cancel_function,
                                 void                         *data,
                                 DBusFreeFunction              free_data_function)
{
  CancelHook *ch;

  ch = dbus_new (CancelHook, 1);
  if (ch == NULL)
    return FALSE;

  _dbus_verbose ("     adding cancel hook function = %p data = %p\n",
                 cancel_function, data);
  
  ch->cancel_function = cancel_function;
  ch->data = data;
  ch->free_data_function = free_data_function;

  /* It's important that the hooks get run in reverse order that they
   * were added
   */
  if (!_dbus_list_prepend (&transaction->cancel_hooks, ch))
    {
      dbus_free (ch);
      return FALSE;
    }

  return TRUE;
}

int
bus_connections_get_n_active (BusConnections *connections)
{
  return connections->n_completed;
}

int
bus_connections_get_n_incomplete (BusConnections *connections)
{
  return connections->n_incomplete;
}

#ifdef DBUS_ENABLE_STATS
int
bus_connections_get_total_match_rules (BusConnections *connections)
{
  return connections->total_match_rules;
}

int
bus_connections_get_peak_match_rules (BusConnections *connections)
{
  return connections->peak_match_rules;
}

int
bus_connections_get_peak_match_rules_per_conn (BusConnections *connections)
{
  return connections->peak_match_rules_per_conn;
}

int
bus_connections_get_total_bus_names (BusConnections *connections)
{
  return connections->total_bus_names;
}

int
bus_connections_get_peak_bus_names (BusConnections *connections)
{
  return connections->peak_bus_names;
}

int
bus_connections_get_peak_bus_names_per_conn (BusConnections *connections)
{
  return connections->peak_bus_names_per_conn;
}

int
bus_connection_get_peak_match_rules (DBusConnection *connection)
{
  BusConnectionData *d;

  d = BUS_CONNECTION_DATA (connection);
  _dbus_assert(d != NULL);

  return d->peak_match_rules;
}

int
bus_connection_get_peak_bus_names (DBusConnection *connection)
{
  BusConnectionData *d;

  d = BUS_CONNECTION_DATA (connection);
  _dbus_assert(d != NULL);

  return d->peak_bus_names;
}
#endif /* DBUS_ENABLE_STATS */

dbus_bool_t
bus_connection_is_monitor (DBusConnection *connection)
{
  BusConnectionData *d;

  d = BUS_CONNECTION_DATA (connection);
  _dbus_assert(d != NULL);

  return d->link_in_monitors != NULL;
}

static dbus_bool_t
bcd_add_monitor_rules (BusConnectionData  *d,
                       DBusConnection     *connection,
                       DBusList          **rules)
{
  BusMatchmaker *mm = d->connections->monitor_matchmaker;
  DBusList *iter;

  if (mm == NULL)
    {
      mm = bus_matchmaker_new ();

      if (mm == NULL)
        return FALSE;

      d->connections->monitor_matchmaker = mm;
    }

  for (iter = _dbus_list_get_first_link (rules);
      iter != NULL;
      iter = _dbus_list_get_next_link (rules, iter))
    {
      if (!bus_matchmaker_add_rule (mm, iter->data))
        {
          bus_matchmaker_disconnected (mm, connection);
          return FALSE;
        }
    }

  return TRUE;
}

static void
bcd_drop_monitor_rules (BusConnectionData *d,
                        DBusConnection *connection)
{
  BusMatchmaker *mm = d->connections->monitor_matchmaker;

  if (mm != NULL)
    bus_matchmaker_disconnected (mm, connection);
}

dbus_bool_t
bus_connection_be_monitor (DBusConnection  *connection,
                           BusTransaction  *transaction,
                           DBusList       **rules,
                           DBusError       *error)
{
  BusConnectionData *d;
  DBusList *link;
  DBusList *tmp;
  DBusList *iter;

  d = BUS_CONNECTION_DATA (connection);
  _dbus_assert (d != NULL);

  link = _dbus_list_alloc_link (connection);

  if (link == NULL)
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  if (!bcd_add_monitor_rules (d, connection, rules))
    {
      _dbus_list_free_link (link);
      BUS_SET_OOM (error);
      return FALSE;
    }

  /* release all its names */
  if (!_dbus_list_copy (&d->services_owned, &tmp))
    {
      bcd_drop_monitor_rules (d, connection);
      _dbus_list_free_link (link);
      BUS_SET_OOM (error);
      return FALSE;
    }

  for (iter = _dbus_list_get_first_link (&tmp);
      iter != NULL;
      iter = _dbus_list_get_next_link (&tmp, iter))
    {
      BusService *service = iter->data;

      /* This call is transactional: if there isn't enough memory to
       * do everything, then the service gets all its names back when
       * the transaction is cancelled due to OOM. */
      if (!bus_service_remove_owner (service, connection, transaction, error))
        {
          bcd_drop_monitor_rules (d, connection);
          _dbus_list_free_link (link);
          _dbus_list_clear (&tmp);
          return FALSE;
        }
    }

  /* We have now done everything that can fail, so there is no problem
   * with doing the irrevocable stuff. */

  _dbus_list_clear (&tmp);

  bus_context_log (transaction->context, DBUS_SYSTEM_LOG_INFO,
                   "Connection %s (%s) became a monitor.", d->name,
                   d->cached_loginfo_string);

  if (d->n_match_rules > 0)
    {
      BusMatchmaker *mm;

      mm = bus_context_get_matchmaker (d->connections->context);
      bus_matchmaker_disconnected (mm, connection);
    }

  /* flag it as a monitor */
  d->link_in_monitors = link;
  _dbus_list_append_link (&d->connections->monitors, link);

  /* it isn't allowed to reply, and it is no longer relevant whether it
   * receives replies */
  bus_connection_drop_pending_replies (d->connections, connection);

  return TRUE;
}

void
bus_connection_request_headers (DBusConnection  *connection,
                                BusExtraHeaders  headers)
{
  BusConnectionData *d;

  d = BUS_CONNECTION_DATA (connection);
  _dbus_assert (d != NULL);

  d->want_headers |= headers;
}
