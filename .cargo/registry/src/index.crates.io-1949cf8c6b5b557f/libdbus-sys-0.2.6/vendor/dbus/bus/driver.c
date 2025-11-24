/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* driver.c  Bus client (driver)
 *
 * Copyright (C) 2003 CodeFactory AB
 * Copyright (C) 2003, 2004, 2005 Red Hat, Inc.
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
#include "apparmor.h"
#include "connection.h"
#include "containers.h"
#include "driver.h"
#include "dispatch.h"
#include "services.h"
#include "selinux.h"
#include "signals.h"
#include "stats.h"
#include "utils.h"

#include <dbus/dbus-asv-util.h>
#include <dbus/dbus-connection-internal.h>
#include <dbus/dbus-string.h>
#include <dbus/dbus-internals.h>
#include <dbus/dbus-message.h>
#include <dbus/dbus-marshal-recursive.h>
#include <dbus/dbus-marshal-validate.h>
#include <string.h>

static inline const char *
nonnull (const char *maybe_null,
         const char *if_null)
{
  return (maybe_null ? maybe_null : if_null);
}

static DBusConnection *
bus_driver_get_owner_of_name (DBusConnection *observer,
                              const char     *name)
{
  BusRegistry *registry;
  BusService *serv;
  DBusString str;

  registry = bus_connection_get_registry (observer);
  _dbus_string_init_const (&str, name);
  serv = bus_registry_lookup (registry, &str);

  if (serv == NULL)
    return NULL;

  return bus_service_get_primary_owners_connection (serv);
}

BusDriverFound
bus_driver_get_conn_helper (DBusConnection  *connection,
                            DBusMessage     *message,
                            const char      *what_we_want,
                            const char     **name_p,
                            DBusConnection **peer_conn_p,
                            DBusError       *error)
{
  DBusConnection *conn;
  const char *name;

  if (!dbus_message_get_args (message, error,
                              DBUS_TYPE_STRING, &name,
                              DBUS_TYPE_INVALID))
    return BUS_DRIVER_FOUND_ERROR;

  _dbus_assert (name != NULL);
  _dbus_verbose ("asked for %s of connection %s\n", what_we_want, name);

  if (name_p != NULL)
    *name_p = name;

  if (strcmp (name, DBUS_SERVICE_DBUS) == 0)
    return BUS_DRIVER_FOUND_SELF;

  conn = bus_driver_get_owner_of_name (connection, name);

  if (conn == NULL)
    {
      dbus_set_error (error, DBUS_ERROR_NAME_HAS_NO_OWNER,
                      "Could not get %s of name '%s': no such name",
                      what_we_want, name);
      return BUS_DRIVER_FOUND_ERROR;
    }

  if (peer_conn_p != NULL)
    *peer_conn_p = conn;

  return BUS_DRIVER_FOUND_PEER;
}

static dbus_bool_t
bus_driver_check_caller_is_not_container (DBusConnection *connection,
                                          BusTransaction *transaction,
                                          DBusMessage    *message,
                                          DBusError      *error)
{
  if (bus_containers_connection_is_contained (connection, NULL, NULL, NULL))
    {
      const char *method = dbus_message_get_member (message);

      bus_context_log_and_set_error (bus_transaction_get_context (transaction),
          DBUS_SYSTEM_LOG_SECURITY, error, DBUS_ERROR_ACCESS_DENIED,
          "rejected attempt to call %s by connection %s (%s) in "
          "container", method,
          nonnull (bus_connection_get_name (connection), "(inactive)"),
          bus_connection_get_loginfo (connection));
      return FALSE;
    }

  return TRUE;
}

/*
 * Log a security warning and set error unless the uid of the connection
 * is either the uid of this process, or on Unix, uid 0 (root).
 *
 * This is intended to be a second line of defence after <deny> rules,
 * to mitigate incorrect system bus security policy configuration files
 * like the ones in CVE-2014-8148 and CVE-2014-8156, and (if present)
 * LSM rules; so it doesn't need to be perfect, but as long as we have
 * potentially dangerous functionality in the system bus, it does need
 * to exist.
 */
static dbus_bool_t
bus_driver_check_caller_is_privileged (DBusConnection *connection,
                                       BusTransaction *transaction,
                                       DBusMessage    *message,
                                       DBusError      *error)
{
#ifdef DBUS_UNIX
  unsigned long uid;
#elif defined(DBUS_WIN)
  char *windows_sid = NULL;
  dbus_bool_t ret = FALSE;
#endif

  if (!bus_driver_check_caller_is_not_container (connection, transaction,
                                                 message, error))
    return FALSE;

#ifdef DBUS_UNIX
  if (!dbus_connection_get_unix_user (connection, &uid))
    {
      const char *method = dbus_message_get_member (message);

      bus_context_log_and_set_error (bus_transaction_get_context (transaction),
          DBUS_SYSTEM_LOG_SECURITY, error, DBUS_ERROR_ACCESS_DENIED,
          "rejected attempt to call %s by connection %s (%s) with "
          "unknown uid", method,
          nonnull (bus_connection_get_name (connection), "(inactive)"),
          bus_connection_get_loginfo (connection));
      return FALSE;
    }

  /* I'm writing it in this slightly strange form so that it's more
   * obvious that this security-sensitive code is correct.
   */
  if (_dbus_unix_user_is_process_owner (uid))
    {
      /* OK */
    }
  else if (uid == 0)
    {
      /* OK */
    }
  else
    {
      const char *method = dbus_message_get_member (message);

      bus_context_log_and_set_error (bus_transaction_get_context (transaction),
          DBUS_SYSTEM_LOG_SECURITY, error, DBUS_ERROR_ACCESS_DENIED,
          "rejected attempt to call %s by connection %s (%s) with "
          "uid %lu", method,
          nonnull (bus_connection_get_name (connection), "(inactive)"),
          bus_connection_get_loginfo (connection), uid);
      return FALSE;
    }

  return TRUE;
#elif defined(DBUS_WIN)
  if (!dbus_connection_get_windows_user (connection, &windows_sid))
    {
      const char *method = dbus_message_get_member (message);

      bus_context_log_and_set_error (bus_transaction_get_context (transaction),
          DBUS_SYSTEM_LOG_SECURITY, error, DBUS_ERROR_ACCESS_DENIED,
          "rejected attempt to call %s by unknown uid", method);
      goto out;
    }

  if (!_dbus_windows_user_is_process_owner (windows_sid))
    {
      const char *method = dbus_message_get_member (message);

      bus_context_log_and_set_error (bus_transaction_get_context (transaction),
          DBUS_SYSTEM_LOG_SECURITY, error, DBUS_ERROR_ACCESS_DENIED,
          "rejected attempt to call %s by uid %s", method, windows_sid);
      goto out;
    }

  ret = TRUE;
out:
  dbus_free (windows_sid);
  return ret;
#else
  /* make sure we fail closed in the hypothetical case that we are neither
   * Unix nor Windows */
  dbus_set_error (error, DBUS_ERROR_ACCESS_DENIED,
      "please teach bus/driver.c how uids work on this platform");
  return FALSE;
#endif
}

static dbus_bool_t bus_driver_send_welcome_message (DBusConnection *connection,
                                                    DBusMessage    *hello_message,
                                                    BusTransaction *transaction,
                                                    DBusError      *error);

dbus_bool_t
bus_driver_send_service_owner_changed (const char     *service_name,
				       const char     *old_owner,
				       const char     *new_owner,
				       BusTransaction *transaction,
				       DBusError      *error)
{
  DBusMessage *message;
  dbus_bool_t retval;
  const char *null_service;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  null_service = "";
  _dbus_verbose ("sending name owner changed: %s [%s -> %s]\n",
                 service_name,
                 old_owner ? old_owner : null_service,
                 new_owner ? new_owner : null_service);

  message = dbus_message_new_signal (DBUS_PATH_DBUS,
                                     DBUS_INTERFACE_DBUS,
                                     "NameOwnerChanged");

  if (message == NULL)
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  if (!dbus_message_set_sender (message, DBUS_SERVICE_DBUS))
    goto oom;

  if (!dbus_message_append_args (message,
                                 DBUS_TYPE_STRING, &service_name,
                                 DBUS_TYPE_STRING, old_owner ? &old_owner : &null_service,
                                 DBUS_TYPE_STRING, new_owner ? &new_owner : &null_service,
                                 DBUS_TYPE_INVALID))
    goto oom;

  _dbus_assert (dbus_message_has_signature (message, "sss"));

  if (!bus_transaction_capture (transaction, NULL, NULL, message))
    goto oom;

  retval = bus_dispatch_matches (transaction, NULL, NULL, message, error);
  dbus_message_unref (message);

  return retval;

 oom:
  dbus_message_unref (message);
  BUS_SET_OOM (error);
  return FALSE;
}

dbus_bool_t
bus_driver_send_service_lost (DBusConnection *connection,
			      const char     *service_name,
                              BusTransaction *transaction,
                              DBusError      *error)
{
  DBusMessage *message;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  message = dbus_message_new_signal (DBUS_PATH_DBUS,
                                     DBUS_INTERFACE_DBUS,
                                     "NameLost");

  if (message == NULL)
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  if (!dbus_message_set_destination (message, bus_connection_get_name (connection)) ||
      !dbus_message_append_args (message,
                                 DBUS_TYPE_STRING, &service_name,
                                 DBUS_TYPE_INVALID))
    {
      dbus_message_unref (message);
      BUS_SET_OOM (error);
      return FALSE;
    }

  if (!bus_transaction_send_from_driver (transaction, connection, message))
    {
      dbus_message_unref (message);
      BUS_SET_OOM (error);
      return FALSE;
    }
  else
    {
      dbus_message_unref (message);
      return TRUE;
    }
}

dbus_bool_t
bus_driver_send_service_acquired (DBusConnection *connection,
                                  const char     *service_name,
                                  BusTransaction *transaction,
                                  DBusError      *error)
{
  DBusMessage *message;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  message = dbus_message_new_signal (DBUS_PATH_DBUS,
                                     DBUS_INTERFACE_DBUS,
                                     "NameAcquired");

  if (message == NULL)
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  if (!dbus_message_set_destination (message, bus_connection_get_name (connection)) ||
      !dbus_message_append_args (message,
                                 DBUS_TYPE_STRING, &service_name,
                                 DBUS_TYPE_INVALID))
    {
      dbus_message_unref (message);
      BUS_SET_OOM (error);
      return FALSE;
    }

  if (!bus_transaction_send_from_driver (transaction, connection, message))
    {
      dbus_message_unref (message);
      BUS_SET_OOM (error);
      return FALSE;
    }
  else
    {
      dbus_message_unref (message);
      return TRUE;
    }
}

static dbus_bool_t
create_unique_client_name (BusRegistry *registry,
                           DBusString  *str)
{
  /* We never want to use the same unique client name twice, because
   * we want to guarantee that if you send a message to a given unique
   * name, you always get the same application. So we use two numbers
   * for INT_MAX * INT_MAX combinations, should be pretty safe against
   * wraparound.
   */
  /* FIXME these should be in BusRegistry rather than static vars */
  static int next_major_number = 0;
  static int next_minor_number = 0;
  int len;

  len = _dbus_string_get_length (str);

  while (TRUE)
    {
      /* start out with 1-0, go to 1-1, 1-2, 1-3,
       * up to 1-MAXINT, then 2-0, 2-1, etc.
       */
      if (next_minor_number <= 0)
        {
          next_major_number += 1;
          next_minor_number = 0;
          if (next_major_number <= 0)
            _dbus_assert_not_reached ("INT_MAX * INT_MAX clients were added");
        }

      _dbus_assert (next_major_number > 0);
      _dbus_assert (next_minor_number >= 0);

      /* appname:MAJOR-MINOR */

      if (!_dbus_string_append (str, ":"))
        return FALSE;

      if (!_dbus_string_append_int (str, next_major_number))
        return FALSE;

      if (!_dbus_string_append (str, "."))
        return FALSE;

      if (!_dbus_string_append_int (str, next_minor_number))
        return FALSE;

      next_minor_number += 1;

      /* Check if a client with the name exists */
      if (bus_registry_lookup (registry, str) == NULL)
	break;

      /* drop the number again, try the next one. */
      _dbus_string_set_length (str, len);
    }

  return TRUE;
}

static dbus_bool_t
bus_driver_handle_hello (DBusConnection *connection,
                         BusTransaction *transaction,
                         DBusMessage    *message,
                         DBusError      *error)
{
  DBusString unique_name;
  BusService *service;
  dbus_bool_t retval;
  BusRegistry *registry;
  BusConnections *connections;
  DBusError tmp_error;
  int limit;
  const char *limit_name;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  if (bus_connection_is_active (connection))
    {
      /* We already handled an Hello message for this connection. */
      dbus_set_error (error, DBUS_ERROR_FAILED,
                      "Already handled an Hello message");
      return FALSE;
    }

  /* Note that when these limits are exceeded we don't disconnect the
   * connection; we just sort of leave it hanging there until it times
   * out or disconnects itself or is dropped due to the max number of
   * incomplete connections. It's even OK if the connection wants to
   * retry the hello message, we support that.
   */
  dbus_error_init (&tmp_error);
  connections = bus_connection_get_connections (connection);
  if (!bus_connections_check_limits (connections, connection,
                                     &limit_name, &limit,
                                     &tmp_error))
    {
      BusContext *context;

      _DBUS_ASSERT_ERROR_IS_SET (&tmp_error);
      context = bus_connection_get_context (connection);
      bus_context_log (context, DBUS_SYSTEM_LOG_WARNING, "%s (%s=%d)",
          tmp_error.message, limit_name, limit);
      dbus_move_error (&tmp_error, error);
      return FALSE;
    }

  if (!_dbus_string_init (&unique_name))
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  retval = FALSE;

  registry = bus_connection_get_registry (connection);

  if (!create_unique_client_name (registry, &unique_name))
    {
      BUS_SET_OOM (error);
      goto out_0;
    }

  if (!bus_connection_complete (connection, &unique_name, error))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      goto out_0;
    }

  if (!dbus_message_set_sender (message,
                                bus_connection_get_name (connection)))
    {
      BUS_SET_OOM (error);
      goto out_0;
    }

  if (!bus_driver_send_welcome_message (connection, message, transaction, error))
    goto out_0;

  /* Create the service */
  service = bus_registry_ensure (registry,
                                 &unique_name, connection, 0, transaction, error);
  if (service == NULL)
    goto out_0;

  _dbus_assert (bus_connection_is_active (connection));
  retval = TRUE;

 out_0:
  _dbus_string_free (&unique_name);
  return retval;
}

static dbus_bool_t
bus_driver_send_welcome_message (DBusConnection *connection,
                                 DBusMessage    *hello_message,
                                 BusTransaction *transaction,
                                 DBusError      *error)
{
  DBusMessage *welcome;
  const char *name;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  name = bus_connection_get_name (connection);
  _dbus_assert (name != NULL);

  welcome = dbus_message_new_method_return (hello_message);
  if (welcome == NULL)
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  if (!dbus_message_append_args (welcome,
                                 DBUS_TYPE_STRING, &name,
                                 DBUS_TYPE_INVALID))
    {
      dbus_message_unref (welcome);
      BUS_SET_OOM (error);
      return FALSE;
    }

  _dbus_assert (dbus_message_has_signature (welcome, DBUS_TYPE_STRING_AS_STRING));

  if (!bus_transaction_send_from_driver (transaction, connection, welcome))
    {
      dbus_message_unref (welcome);
      BUS_SET_OOM (error);
      return FALSE;
    }
  else
    {
      dbus_message_unref (welcome);
      return TRUE;
    }
}

static dbus_bool_t
bus_driver_handle_list_services (DBusConnection *connection,
                                 BusTransaction *transaction,
                                 DBusMessage    *message,
                                 DBusError      *error)
{
  DBusMessage *reply;
  int len;
  char **services;
  BusRegistry *registry;
  int i;
  DBusMessageIter iter;
  DBusMessageIter sub;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  registry = bus_connection_get_registry (connection);

  reply = dbus_message_new_method_return (message);
  if (reply == NULL)
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  if (!bus_registry_list_services (registry, &services, &len))
    {
      dbus_message_unref (reply);
      BUS_SET_OOM (error);
      return FALSE;
    }

  dbus_message_iter_init_append (reply, &iter);

  if (!dbus_message_iter_open_container (&iter, DBUS_TYPE_ARRAY,
                                         DBUS_TYPE_STRING_AS_STRING,
                                         &sub))
    {
      dbus_free_string_array (services);
      dbus_message_unref (reply);
      BUS_SET_OOM (error);
      return FALSE;
    }

  {
    /* Include the bus driver in the list */
    const char *v_STRING = DBUS_SERVICE_DBUS;
    if (!dbus_message_iter_append_basic (&sub, DBUS_TYPE_STRING,
                                         &v_STRING))
      {
        dbus_free_string_array (services);
        dbus_message_unref (reply);
        BUS_SET_OOM (error);
        return FALSE;
      }
  }

  i = 0;
  while (i < len)
    {
      if (!dbus_message_iter_append_basic (&sub, DBUS_TYPE_STRING,
                                           &services[i]))
        {
          dbus_free_string_array (services);
          dbus_message_unref (reply);
          BUS_SET_OOM (error);
          return FALSE;
        }
      ++i;
    }

  dbus_free_string_array (services);

  if (!dbus_message_iter_close_container (&iter, &sub))
    {
      dbus_message_unref (reply);
      BUS_SET_OOM (error);
      return FALSE;
    }

  if (!bus_transaction_send_from_driver (transaction, connection, reply))
    {
      dbus_message_unref (reply);
      BUS_SET_OOM (error);
      return FALSE;
    }
  else
    {
      dbus_message_unref (reply);
      return TRUE;
    }
}

static dbus_bool_t
bus_driver_handle_list_activatable_services (DBusConnection *connection,
					     BusTransaction *transaction,
					     DBusMessage    *message,
					     DBusError      *error)
{
  DBusMessage *reply;
  int len;
  char **services;
  BusActivation *activation;
  int i;
  DBusMessageIter iter;
  DBusMessageIter sub;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  activation = bus_connection_get_activation (connection);

  reply = dbus_message_new_method_return (message);
  if (reply == NULL)
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  if (!bus_activation_list_services (activation, &services, &len))
    {
      dbus_message_unref (reply);
      BUS_SET_OOM (error);
      return FALSE;
    }

  dbus_message_iter_init_append (reply, &iter);

  if (!dbus_message_iter_open_container (&iter, DBUS_TYPE_ARRAY,
					 DBUS_TYPE_STRING_AS_STRING,
					 &sub))
    {
      dbus_free_string_array (services);
      dbus_message_unref (reply);
      BUS_SET_OOM (error);
      return FALSE;
    }

  {
    /* Include the bus driver in the list */
    const char *v_STRING = DBUS_SERVICE_DBUS;
    if (!dbus_message_iter_append_basic (&sub, DBUS_TYPE_STRING,
					 &v_STRING))
      {
	dbus_free_string_array (services);
	dbus_message_unref (reply);
	BUS_SET_OOM (error);
	return FALSE;
      }
  }

  i = 0;
  while (i < len)
    {
      if (!dbus_message_iter_append_basic (&sub, DBUS_TYPE_STRING,
					   &services[i]))
	{
	  dbus_free_string_array (services);
	  dbus_message_unref (reply);
	  BUS_SET_OOM (error);
	  return FALSE;
	}
      ++i;
    }

  dbus_free_string_array (services);

  if (!dbus_message_iter_close_container (&iter, &sub))
    {
      dbus_message_unref (reply);
      BUS_SET_OOM (error);
      return FALSE;
    }

  if (!bus_transaction_send_from_driver (transaction, connection, reply))
    {
      dbus_message_unref (reply);
      BUS_SET_OOM (error);
      return FALSE;
    }
  else
    {
      dbus_message_unref (reply);
      return TRUE;
    }
}

static dbus_bool_t
bus_driver_handle_acquire_service (DBusConnection *connection,
                                   BusTransaction *transaction,
                                   DBusMessage    *message,
                                   DBusError      *error)
{
  DBusMessage *reply;
  DBusString service_name;
  const char *name;
  dbus_uint32_t service_reply;
  dbus_uint32_t flags;
  dbus_bool_t retval;
  BusRegistry *registry;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  registry = bus_connection_get_registry (connection);

  if (!dbus_message_get_args (message, error,
                              DBUS_TYPE_STRING, &name,
                              DBUS_TYPE_UINT32, &flags,
                              DBUS_TYPE_INVALID))
    return FALSE;

  _dbus_verbose ("Trying to own name %s with flags 0x%x\n", name, flags);

  retval = FALSE;
  reply = NULL;

  _dbus_string_init_const (&service_name, name);

  if (!bus_registry_acquire_service (registry, connection,
                                     &service_name, flags,
                                     &service_reply, transaction,
                                     error))
    goto out;

  reply = dbus_message_new_method_return (message);
  if (reply == NULL)
    {
      BUS_SET_OOM (error);
      goto out;
    }

  if (!dbus_message_append_args (reply, DBUS_TYPE_UINT32, &service_reply, DBUS_TYPE_INVALID))
    {
      BUS_SET_OOM (error);
      goto out;
    }

  if (!bus_transaction_send_from_driver (transaction, connection, reply))
    {
      BUS_SET_OOM (error);
      goto out;
    }

  retval = TRUE;

 out:
  if (reply)
    dbus_message_unref (reply);
  return retval;
}

static dbus_bool_t
bus_driver_handle_release_service (DBusConnection *connection,
                                   BusTransaction *transaction,
                                   DBusMessage    *message,
                                   DBusError      *error)
{
  DBusMessage *reply;
  DBusString service_name;
  const char *name;
  dbus_uint32_t service_reply;
  dbus_bool_t retval;
  BusRegistry *registry;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  registry = bus_connection_get_registry (connection);

  if (!dbus_message_get_args (message, error,
                              DBUS_TYPE_STRING, &name,
                              DBUS_TYPE_INVALID))
    return FALSE;

  _dbus_verbose ("Trying to release name %s\n", name);

  retval = FALSE;
  reply = NULL;

  _dbus_string_init_const (&service_name, name);

  if (!bus_registry_release_service (registry, connection,
                                     &service_name, &service_reply,
                                     transaction, error))
    goto out;

  reply = dbus_message_new_method_return (message);
  if (reply == NULL)
    {
      BUS_SET_OOM (error);
      goto out;
    }

  if (!dbus_message_append_args (reply, DBUS_TYPE_UINT32, &service_reply, DBUS_TYPE_INVALID))
    {
      BUS_SET_OOM (error);
      goto out;
    }

  if (!bus_transaction_send_from_driver (transaction, connection, reply))
    {
      BUS_SET_OOM (error);
      goto out;
    }

  retval = TRUE;

 out:
  if (reply)
    dbus_message_unref (reply);
  return retval;
}

static dbus_bool_t
bus_driver_handle_service_exists (DBusConnection *connection,
                                  BusTransaction *transaction,
                                  DBusMessage    *message,
                                  DBusError      *error)
{
  DBusMessage *reply;
  DBusString service_name;
  BusService *service;
  dbus_bool_t service_exists;
  const char *name;
  dbus_bool_t retval;
  BusRegistry *registry;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  registry = bus_connection_get_registry (connection);

  if (!dbus_message_get_args (message, error,
                              DBUS_TYPE_STRING, &name,
                              DBUS_TYPE_INVALID))
    return FALSE;

  retval = FALSE;

  if (strcmp (name, DBUS_SERVICE_DBUS) == 0)
    {
      service_exists = TRUE;
    }
  else
    {
      _dbus_string_init_const (&service_name, name);
      service = bus_registry_lookup (registry, &service_name);
      service_exists = service != NULL;
    }

  reply = dbus_message_new_method_return (message);
  if (reply == NULL)
    {
      BUS_SET_OOM (error);
      goto out;
    }

  if (!dbus_message_append_args (reply,
                                 DBUS_TYPE_BOOLEAN, &service_exists,
                                 0))
    {
      BUS_SET_OOM (error);
      goto out;
    }

  if (!bus_transaction_send_from_driver (transaction, connection, reply))
    {
      BUS_SET_OOM (error);
      goto out;
    }

  retval = TRUE;

 out:
  if (reply)
    dbus_message_unref (reply);

  return retval;
}

static dbus_bool_t
bus_driver_handle_activate_service (DBusConnection *connection,
                                    BusTransaction *transaction,
                                    DBusMessage    *message,
                                    DBusError      *error)
{
  dbus_uint32_t flags;
  const char *name;
  dbus_bool_t retval;
  BusActivation *activation;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  activation = bus_connection_get_activation (connection);

  if (!dbus_message_get_args (message, error,
                              DBUS_TYPE_STRING, &name,
                              DBUS_TYPE_UINT32, &flags,
                              DBUS_TYPE_INVALID))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      _dbus_verbose ("No memory to get arguments to StartServiceByName\n");
      return FALSE;
    }

  retval = FALSE;

  if (!bus_activation_activate_service (activation, connection, transaction, FALSE,
                                        message, name, error))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      _dbus_verbose ("bus_activation_activate_service() failed\n");
      goto out;
    }

  retval = TRUE;

 out:
  return retval;
}

dbus_bool_t
bus_driver_send_ack_reply (DBusConnection *connection,
                           BusTransaction *transaction,
                           DBusMessage    *message,
                           DBusError      *error)
{
  DBusMessage *reply;

  if (dbus_message_get_no_reply (message))
    return TRUE;

  reply = dbus_message_new_method_return (message);
  if (reply == NULL)
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  if (!bus_transaction_send_from_driver (transaction, connection, reply))
    {
      BUS_SET_OOM (error);
      dbus_message_unref (reply);
      return FALSE;
    }

  dbus_message_unref (reply);

  return TRUE;
}

/*
 * Send a message from the driver, activating the destination if necessary.
 * The message must already have a destination set.
 */
static dbus_bool_t
bus_driver_send_or_activate (BusTransaction *transaction,
                             DBusMessage    *message,
                             DBusError      *error)
{
  BusContext *context;
  BusService *service;
  const char *service_name;
  DBusString service_string;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  service_name = dbus_message_get_destination (message);

  _dbus_assert (service_name != NULL);

  _dbus_string_init_const (&service_string, service_name);

  context = bus_transaction_get_context (transaction);

  service = bus_registry_lookup (bus_context_get_registry (context),
                                 &service_string);

  if (service == NULL)
    {
      /* destination isn't connected yet; pass the message to activation */
      BusActivation *activation;

      activation = bus_context_get_activation (context);

      if (!bus_transaction_capture (transaction, NULL, NULL, message))
        {
          BUS_SET_OOM (error);
          _dbus_verbose ("No memory for bus_transaction_capture()");
          return FALSE;
        }

      if (!bus_activation_activate_service (activation, NULL, transaction, TRUE,
                                            message, service_name, error))
        {
          _DBUS_ASSERT_ERROR_IS_SET (error);
          _dbus_verbose ("bus_activation_activate_service() failed");
          return FALSE;
        }
    }
  else
    {
      DBusConnection *service_conn;

      service_conn = bus_service_get_primary_owners_connection (service);

      if (!bus_transaction_send_from_driver (transaction, service_conn, message))
        {
          BUS_SET_OOM (error);
          _dbus_verbose ("No memory for bus_transaction_send_from_driver()");
          return FALSE;
        }
    }

  return TRUE;
}

static dbus_bool_t
bus_driver_handle_update_activation_environment (DBusConnection *connection,
                                                 BusTransaction *transaction,
                                                 DBusMessage    *message,
                                                 DBusError      *error)
{
  dbus_bool_t retval;
  BusActivation *activation;
  BusContext *context;
  DBusMessageIter iter;
  DBusMessageIter dict_iter;
  DBusMessageIter dict_entry_iter;
  int array_type;
  int key_type;
  DBusList *keys, *key_link;
  DBusList *values, *value_link;
  DBusMessage *systemd_message;
  DBusMessageIter systemd_iter;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  context = bus_connection_get_context (connection);

  if (bus_context_get_servicehelper (context) != NULL)
    {
      dbus_set_error (error, DBUS_ERROR_ACCESS_DENIED,
                      "Cannot change activation environment "
                      "on a system bus.");
      return FALSE;
    }

  activation = bus_connection_get_activation (connection);

  dbus_message_iter_init (message, &iter);

  /* The message signature has already been checked for us,
   * so let's just assert it's right.
   */
  _dbus_assert (dbus_message_iter_get_arg_type (&iter) == DBUS_TYPE_ARRAY);

  dbus_message_iter_recurse (&iter, &dict_iter);

  retval = FALSE;
  systemd_message = NULL;

  /* Then loop through the sent dictionary, add the location of
   * the environment keys and values to lists. The result will
   * be in reverse order, so we don't have to constantly search
   * for the end of the list in a loop.
   */
  keys = NULL;
  values = NULL;
  while ((array_type = dbus_message_iter_get_arg_type (&dict_iter)) == DBUS_TYPE_DICT_ENTRY)
    {
      dbus_message_iter_recurse (&dict_iter, &dict_entry_iter);

      while ((key_type = dbus_message_iter_get_arg_type (&dict_entry_iter)) == DBUS_TYPE_STRING)
        {
          char *key;
          char *value;
          int value_type;

          dbus_message_iter_get_basic (&dict_entry_iter, &key);
          dbus_message_iter_next (&dict_entry_iter);

          value_type = dbus_message_iter_get_arg_type (&dict_entry_iter);

          if (value_type != DBUS_TYPE_STRING)
            break;

          dbus_message_iter_get_basic (&dict_entry_iter, &value);

          if (!_dbus_list_append (&keys, key))
            {
              BUS_SET_OOM (error);
              break;
            }

          if (!_dbus_list_append (&values, value))
            {
              BUS_SET_OOM (error);
              break;
            }

          dbus_message_iter_next (&dict_entry_iter);
        }

      if (key_type != DBUS_TYPE_INVALID)
        break;

      dbus_message_iter_next (&dict_iter);
    }

  if (array_type != DBUS_TYPE_INVALID)
    goto out;

  _dbus_assert (_dbus_list_get_length (&keys) == _dbus_list_get_length (&values));

  if (bus_context_get_systemd_activation (bus_connection_get_context (connection)))
    {
      /* Prepare a call to forward environment updates to systemd */
      systemd_message = dbus_message_new_method_call ("org.freedesktop.systemd1",
                                                      "/org/freedesktop/systemd1",
                                                      "org.freedesktop.systemd1.Manager",
                                                      "SetEnvironment");
      if (systemd_message == NULL ||
          !dbus_message_set_sender (systemd_message, DBUS_SERVICE_DBUS))
        {
          BUS_SET_OOM (error);
          _dbus_verbose ("No memory to create systemd message\n");
          goto out;
        }

      dbus_message_set_no_reply (systemd_message, TRUE);
      dbus_message_iter_init_append (systemd_message, &iter);

      if (!dbus_message_iter_open_container (&iter, DBUS_TYPE_ARRAY, "s",
                                             &systemd_iter))
        {
          BUS_SET_OOM (error);
          _dbus_verbose ("No memory to open systemd message container\n");
          goto out;
        }
    }

  key_link = keys;
  value_link = values;
  while (key_link != NULL)
  {
      const char *key;
      const char *value;

      key = key_link->data;
      value = value_link->data;

      if (!bus_activation_set_environment_variable (activation,
                                                    key, value, error))
        {
          _DBUS_ASSERT_ERROR_IS_SET (error);
          _dbus_verbose ("bus_activation_set_environment_variable() failed\n");
          break;
        }

      if (systemd_message != NULL)
        {
          DBusString envline;
          const char *s;

          /* SetEnvironment wants an array of KEY=VALUE strings */
          if (!_dbus_string_init (&envline) ||
              !_dbus_string_append_printf (&envline, "%s=%s", key, value))
            {
              BUS_SET_OOM (error);
              _dbus_verbose ("No memory to format systemd environment line\n");
              _dbus_string_free (&envline);
              break;
            }

          s = _dbus_string_get_data (&envline);

          if (!dbus_message_iter_append_basic (&systemd_iter,
                                               DBUS_TYPE_STRING, &s))
            {
              BUS_SET_OOM (error);
              _dbus_verbose ("No memory to append systemd environment line\n");
              _dbus_string_free (&envline);
              break;
            }

          _dbus_string_free (&envline);
        }

      key_link = _dbus_list_get_next_link (&keys, key_link);
      value_link = _dbus_list_get_next_link (&values, value_link);
  }

  /* FIXME: We can fail early having set only some of the environment variables,
   * (because of OOM failure).  It's sort of hard to fix and it doesn't really
   * matter, so we're punting for now.
   */
  if (key_link != NULL)
    {
      if (systemd_message != NULL)
        dbus_message_iter_abandon_container (&iter, &systemd_iter);
      goto out;
    }

  if (systemd_message != NULL)
    {
      if (!dbus_message_iter_close_container (&iter, &systemd_iter))
        {
          BUS_SET_OOM (error);
          _dbus_verbose ("No memory to close systemd message container\n");
          goto out;
        }

      if (!bus_driver_send_or_activate (transaction, systemd_message, error))
        {
          _DBUS_ASSERT_ERROR_IS_SET (error);
          _dbus_verbose ("bus_driver_send_or_activate() failed\n");
          goto out;
        }
    }

  if (!bus_driver_send_ack_reply (connection, transaction, message, error))
    goto out;

  retval = TRUE;

 out:
  if (systemd_message != NULL)
    dbus_message_unref (systemd_message);
  _dbus_list_clear (&keys);
  _dbus_list_clear (&values);
  return retval;
}

static dbus_bool_t
bus_driver_handle_add_match (DBusConnection *connection,
                             BusTransaction *transaction,
                             DBusMessage    *message,
                             DBusError      *error)
{
  BusMatchRule *rule;
  const char *text, *bustype;
  DBusString str;
  BusMatchmaker *matchmaker;
  int limit;
  BusContext *context;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  text = NULL;
  rule = NULL;

  context = bus_transaction_get_context (transaction);
  limit = bus_context_get_max_match_rules_per_connection (context);

  if (bus_connection_get_n_match_rules (connection) >= limit)
    {
      DBusError tmp_error;

      dbus_error_init (&tmp_error);
      dbus_set_error (&tmp_error, DBUS_ERROR_LIMITS_EXCEEDED,
                      "Connection \"%s\" is not allowed to add more match rules "
                      "(increase limits in configuration file if required; "
                      "max_match_rules_per_connection=%d)",
                      bus_connection_is_active (connection) ?
                      bus_connection_get_name (connection) :
                      "(inactive)",
                      limit);
      bus_context_log (context, DBUS_SYSTEM_LOG_WARNING, "%s",
                       tmp_error.message);
      dbus_move_error (&tmp_error, error);
      goto failed;
    }

  if (!dbus_message_get_args (message, error,
                              DBUS_TYPE_STRING, &text,
                              DBUS_TYPE_INVALID))
    {
      _dbus_verbose ("No memory to get arguments to AddMatch\n");
      goto failed;
    }

  _dbus_string_init_const (&str, text);

  rule = bus_match_rule_parse (connection, &str, error);
  if (rule == NULL)
    goto failed;

  bustype = bus_context_get_type (context);

  if (bus_match_rule_get_client_is_eavesdropping (rule))
    {
      if (!bus_driver_check_caller_is_privileged (connection,
                                                  transaction,
                                                  message,
                                                  error) ||
          !bus_apparmor_allows_eavesdropping (connection, bustype, error))
        goto failed;
    }

  matchmaker = bus_connection_get_matchmaker (connection);

  if (!bus_matchmaker_add_rule (matchmaker, rule))
    {
      BUS_SET_OOM (error);
      goto failed;
    }

  if (!bus_driver_send_ack_reply (connection, transaction, message, error))
    {
      bus_matchmaker_remove_rule (matchmaker, rule);
      goto failed;
    }

  bus_match_rule_unref (rule);

  return TRUE;

 failed:
  _DBUS_ASSERT_ERROR_IS_SET (error);
  if (rule)
    bus_match_rule_unref (rule);
  return FALSE;
}

static dbus_bool_t
bus_driver_handle_remove_match (DBusConnection *connection,
                                BusTransaction *transaction,
                                DBusMessage    *message,
                                DBusError      *error)
{
  BusMatchRule *rule;
  const char *text;
  DBusString str;
  BusMatchmaker *matchmaker;
  DBusList *link;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  text = NULL;
  rule = NULL;

  if (!dbus_message_get_args (message, error,
                              DBUS_TYPE_STRING, &text,
                              DBUS_TYPE_INVALID))
    {
      _dbus_verbose ("No memory to get arguments to RemoveMatch\n");
      goto failed;
    }

  _dbus_string_init_const (&str, text);

  rule = bus_match_rule_parse (connection, &str, error);
  if (rule == NULL)
    goto failed;

  matchmaker = bus_connection_get_matchmaker (connection);

  /* Check whether the rule exists and prepare to remove it, but don't
   * actually do it yet. */
  link = bus_matchmaker_prepare_remove_rule_by_value (matchmaker, rule, error);
  if (link == NULL)
    goto failed;

  /* We do this before actually removing the rule, because removing the
   * rule cannot be undone if we run out of memory here. */
  if (!bus_driver_send_ack_reply (connection, transaction, message, error))
    goto failed;

  /* The rule exists, so now we can do things we can't undo. */
  bus_matchmaker_commit_remove_rule_by_value (matchmaker, rule, link);
  bus_match_rule_unref (rule);

  return TRUE;

 failed:
  _DBUS_ASSERT_ERROR_IS_SET (error);
  if (rule)
    bus_match_rule_unref (rule);
  return FALSE;
}

static dbus_bool_t
bus_driver_handle_get_service_owner (DBusConnection *connection,
				     BusTransaction *transaction,
				     DBusMessage    *message,
				     DBusError      *error)
{
  const char *text;
  const char *base_name;
  DBusString str;
  BusRegistry *registry;
  BusService *service;
  DBusMessage *reply;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  registry = bus_connection_get_registry (connection);

  text = NULL;
  reply = NULL;

  if (! dbus_message_get_args (message, error,
			       DBUS_TYPE_STRING, &text,
			       DBUS_TYPE_INVALID))
      goto failed;

  _dbus_string_init_const (&str, text);
  service = bus_registry_lookup (registry, &str);
  if (service == NULL &&
      _dbus_string_equal_c_str (&str, DBUS_SERVICE_DBUS))
    {
      /* DBUS_SERVICE_DBUS owns itself */
      base_name = DBUS_SERVICE_DBUS;
    }
  else if (service == NULL)
    {
      dbus_set_error (error,
                      DBUS_ERROR_NAME_HAS_NO_OWNER,
                      "Could not get owner of name '%s': no such name", text);
      goto failed;
    }
  else
    {
      base_name = bus_connection_get_name (bus_service_get_primary_owners_connection (service));
      if (base_name == NULL)
        {
          /* FIXME - how is this error possible? */
          dbus_set_error (error,
                          DBUS_ERROR_FAILED,
                          "Could not determine unique name for '%s'", text);
          goto failed;
        }
      _dbus_assert (*base_name == ':');
    }

  _dbus_assert (base_name != NULL);

  reply = dbus_message_new_method_return (message);
  if (reply == NULL)
    goto oom;

  if (! dbus_message_append_args (reply,
				  DBUS_TYPE_STRING, &base_name,
				  DBUS_TYPE_INVALID))
    goto oom;

  if (! bus_transaction_send_from_driver (transaction, connection, reply))
    goto oom;

  dbus_message_unref (reply);

  return TRUE;

 oom:
  BUS_SET_OOM (error);

 failed:
  _DBUS_ASSERT_ERROR_IS_SET (error);
  if (reply)
    dbus_message_unref (reply);
  return FALSE;
}

static dbus_bool_t
bus_driver_handle_list_queued_owners (DBusConnection *connection,
				      BusTransaction *transaction,
				      DBusMessage    *message,
				      DBusError      *error)
{
  static const char dbus_service_name[] = DBUS_SERVICE_DBUS;

  const char *text;
  DBusList *base_names;
  DBusList *link;
  DBusString str;
  BusRegistry *registry;
  BusService *service;
  DBusMessage *reply;
  DBusMessageIter iter, array_iter;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  registry = bus_connection_get_registry (connection);

  base_names = NULL;
  text = NULL;
  reply = NULL;

  if (! dbus_message_get_args (message, error,
			       DBUS_TYPE_STRING, &text,
			       DBUS_TYPE_INVALID))
      goto failed;

  _dbus_string_init_const (&str, text);
  service = bus_registry_lookup (registry, &str);
  if (service == NULL &&
      _dbus_string_equal_c_str (&str, DBUS_SERVICE_DBUS))
    {
      /* DBUS_SERVICE_DBUS owns itself */
      if (! _dbus_list_append (&base_names, (char *) dbus_service_name))
        goto oom;
    }
  else if (service == NULL)
    {
      dbus_set_error (error,
                      DBUS_ERROR_NAME_HAS_NO_OWNER,
                      "Could not get owners of name '%s': no such name", text);
      goto failed;
    }
  else
    {
      if (!bus_service_list_queued_owners (service, &base_names))
        {
          BUS_SET_OOM (error);
          goto failed;
        }
    }

  _dbus_assert (base_names != NULL);

  reply = dbus_message_new_method_return (message);
  if (reply == NULL)
    goto oom;

  dbus_message_iter_init_append (reply, &iter);
  if (!dbus_message_iter_open_container (&iter,
                                         DBUS_TYPE_ARRAY,
                                         DBUS_TYPE_STRING_AS_STRING,
                                         &array_iter))
    goto oom;

  link = _dbus_list_get_first_link (&base_names);
  while (link != NULL)
    {
      char *uname;

      _dbus_assert (link->data != NULL);
      uname = (char *)link->data;

      if (!dbus_message_iter_append_basic (&array_iter,
                                           DBUS_TYPE_STRING,
                                           &uname))
        goto oom;

      link = _dbus_list_get_next_link (&base_names, link);
    }

  if (! dbus_message_iter_close_container (&iter, &array_iter))
    goto oom;


  if (! bus_transaction_send_from_driver (transaction, connection, reply))
    goto oom;

  dbus_message_unref (reply);

  return TRUE;

 oom:
  BUS_SET_OOM (error);

 failed:
  _DBUS_ASSERT_ERROR_IS_SET (error);
  if (reply)
    dbus_message_unref (reply);

  if (base_names)
    _dbus_list_clear (&base_names);

  return FALSE;
}

static dbus_bool_t
bus_driver_handle_get_connection_unix_user (DBusConnection *connection,
                                            BusTransaction *transaction,
                                            DBusMessage    *message,
                                            DBusError      *error)
{
  DBusConnection *conn;
  DBusMessage *reply;
  dbus_uid_t uid;
  dbus_uint32_t uid32;
  const char *service;
  BusDriverFound found;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  reply = NULL;

  found = bus_driver_get_conn_helper (connection, message, "UID", &service,
                                      &conn, error);
  switch (found)
    {
      case BUS_DRIVER_FOUND_SELF:
        uid = _dbus_getuid ();
        break;
      case BUS_DRIVER_FOUND_PEER:
        if (!dbus_connection_get_unix_user (conn, &uid))
          uid = DBUS_UID_UNSET;
        break;
      case BUS_DRIVER_FOUND_ERROR:
        /* fall through */
      default:
        goto failed;
    }

  if (uid == DBUS_UID_UNSET)
    {
      dbus_set_error (error,
                      DBUS_ERROR_FAILED,
                      "Could not determine UID for '%s'", service);
      goto failed;
    }

  reply = dbus_message_new_method_return (message);
  if (reply == NULL)
    goto oom;

  uid32 = uid;
  if (! dbus_message_append_args (reply,
                                  DBUS_TYPE_UINT32, &uid32,
                                  DBUS_TYPE_INVALID))
    goto oom;

  if (! bus_transaction_send_from_driver (transaction, connection, reply))
    goto oom;

  dbus_message_unref (reply);

  return TRUE;

 oom:
  BUS_SET_OOM (error);

 failed:
  _DBUS_ASSERT_ERROR_IS_SET (error);
  if (reply)
    dbus_message_unref (reply);
  return FALSE;
}

static dbus_bool_t
bus_driver_handle_get_connection_unix_process_id (DBusConnection *connection,
						  BusTransaction *transaction,
						  DBusMessage    *message,
						  DBusError      *error)
{
  DBusConnection *conn;
  DBusMessage *reply;
  dbus_pid_t pid;
  dbus_uint32_t pid32;
  const char *service;
  BusDriverFound found;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  reply = NULL;

  found = bus_driver_get_conn_helper (connection, message, "PID", &service,
                                      &conn, error);
  switch (found)
    {
      case BUS_DRIVER_FOUND_SELF:
        pid = _dbus_getpid ();
        break;
      case BUS_DRIVER_FOUND_PEER:
        if (!dbus_connection_get_unix_process_id (conn, &pid))
          pid = DBUS_PID_UNSET;
        break;
      case BUS_DRIVER_FOUND_ERROR:
        /* fall through */
      default:
        goto failed;
    }

  if (pid == DBUS_PID_UNSET)
    {
      dbus_set_error (error,
                      DBUS_ERROR_UNIX_PROCESS_ID_UNKNOWN,
                      "Could not determine PID for '%s'", service);
      goto failed;
    }

  reply = dbus_message_new_method_return (message);
  if (reply == NULL)
    goto oom;

  pid32 = pid;
  if (! dbus_message_append_args (reply,
                                  DBUS_TYPE_UINT32, &pid32,
                                  DBUS_TYPE_INVALID))
    goto oom;

  if (! bus_transaction_send_from_driver (transaction, connection, reply))
    goto oom;

  dbus_message_unref (reply);

  return TRUE;

 oom:
  BUS_SET_OOM (error);

 failed:
  _DBUS_ASSERT_ERROR_IS_SET (error);
  if (reply)
    dbus_message_unref (reply);
  return FALSE;
}

static dbus_bool_t
bus_driver_handle_get_adt_audit_session_data (DBusConnection *connection,
					      BusTransaction *transaction,
					      DBusMessage    *message,
					      DBusError      *error)
{
  DBusConnection *conn;
  DBusMessage *reply;
  void *data = NULL;
  dbus_int32_t data_size;
  const char *service;
  BusDriverFound found;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  reply = NULL;

  found = bus_driver_get_conn_helper (connection, message, "audit session data",
                                      &service, &conn, error);

  if (found == BUS_DRIVER_FOUND_ERROR)
    goto failed;

  reply = dbus_message_new_method_return (message);
  if (reply == NULL)
    goto oom;

  /* We don't know how to find "ADT audit session data" for the bus daemon
   * itself. Is that even meaningful?
   * FIXME: Implement this or briefly note it makes no sense.
   */
  if (found != BUS_DRIVER_FOUND_PEER ||
      !dbus_connection_get_adt_audit_session_data (conn, &data, &data_size) ||
      data == NULL)
    {
      dbus_set_error (error,
                      DBUS_ERROR_ADT_AUDIT_DATA_UNKNOWN,
                      "Could not determine audit session data for '%s'", service);
      goto failed;
    }

  if (! dbus_message_append_args (reply,
                                  DBUS_TYPE_ARRAY, DBUS_TYPE_BYTE, &data, data_size,
                                  DBUS_TYPE_INVALID))
    goto oom;

  if (! bus_transaction_send_from_driver (transaction, connection, reply))
    goto oom;

  dbus_message_unref (reply);

  return TRUE;

 oom:
  BUS_SET_OOM (error);

 failed:
  _DBUS_ASSERT_ERROR_IS_SET (error);
  if (reply)
    dbus_message_unref (reply);
  return FALSE;
}

static dbus_bool_t
bus_driver_handle_get_connection_selinux_security_context (DBusConnection *connection,
							   BusTransaction *transaction,
							   DBusMessage    *message,
							   DBusError      *error)
{
  DBusConnection *conn;
  DBusMessage *reply;
  BusSELinuxID *context;
  const char *service;
  BusDriverFound found;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  reply = NULL;

  found = bus_driver_get_conn_helper (connection, message, "security context",
                                      &service, &conn, error);

  if (found == BUS_DRIVER_FOUND_ERROR)
    goto failed;

  reply = dbus_message_new_method_return (message);
  if (reply == NULL)
    goto oom;

  if (found == BUS_DRIVER_FOUND_SELF)
    context = bus_selinux_get_self ();
  else if (found == BUS_DRIVER_FOUND_PEER)
    context = bus_connection_get_selinux_id (conn);
  else
    context = NULL;

  if (!context)
    {
      dbus_set_error (error,
                      DBUS_ERROR_SELINUX_SECURITY_CONTEXT_UNKNOWN,
                      "Could not determine security context for '%s'", service);
      goto failed;
    }

  if (! bus_selinux_append_context (reply, context, error))
    goto failed;

  if (! bus_transaction_send_from_driver (transaction, connection, reply))
    goto oom;

  dbus_message_unref (reply);

  return TRUE;

 oom:
  BUS_SET_OOM (error);

 failed:
  _DBUS_ASSERT_ERROR_IS_SET (error);
  if (reply)
    dbus_message_unref (reply);
  return FALSE;
}

/*
 * Write the unix group ids of credentials @credentials, if available, into
 * the a{sv} @asv_iter. Return #FALSE on OOM.
 */
static dbus_bool_t
bus_driver_credentials_fill_unix_gids (DBusCredentials *credentials,
                                       DBusMessageIter *asv_iter)
{
  const dbus_gid_t *gids = NULL;
  size_t n_gids = 0;

  if (!_dbus_credentials_get_unix_gids (credentials, &gids, &n_gids))
    return TRUE;

  if (sizeof (dbus_gid_t) == sizeof (dbus_uint32_t))
    {
      return _dbus_asv_add_fixed_array (asv_iter, "UnixGroupIDs",
                                        DBUS_TYPE_UINT32, gids, n_gids);
    }
  else
    {
      /* we can't represent > 32-bit uids; if your system needs them, please
       * add UnixGroupIDs64 to the spec or something */
      dbus_uint32_t *gids_u32;
      size_t i;
      dbus_bool_t result;

      gids_u32 = dbus_new (dbus_uint32_t, n_gids);
      if (gids_u32 == NULL)
        return FALSE;

      for (i = 0; i < n_gids; i++)
        {
          if (gids[i] > _DBUS_UINT32_MAX)
            {
              /* At least one gid is unrepresentable, so behave as though
               * we didn't know the group IDs at all (not an error, just
               * success with less information) */
              dbus_free (gids_u32);
              return TRUE;
            }
          gids_u32[i] = gids[i];
        }

      result = _dbus_asv_add_fixed_array (asv_iter, "UnixGroupIDs",
                                          DBUS_TYPE_UINT32, gids_u32, n_gids);

      dbus_free (gids_u32);

      return result;
    }
}

/*
 * Write the credentials of connection @conn (or the bus daemon itself,
 * if @conn is #NULL) into the a{sv} @asv_iter. Return #FALSE on OOM.
 */
dbus_bool_t
bus_driver_fill_connection_credentials (DBusCredentials *credentials,
                                        DBusConnection  *conn,
                                        DBusMessageIter *asv_iter)
{
  dbus_uid_t uid = DBUS_UID_UNSET;
  dbus_pid_t pid = DBUS_PID_UNSET;
  const char *windows_sid = NULL;
  const char *linux_security_label = NULL;
#ifdef DBUS_ENABLE_CONTAINERS
  const char *path;
#endif

  if (credentials == NULL && conn != NULL)
    credentials = _dbus_connection_get_credentials (conn);

  if (credentials != NULL)
    {
      pid = _dbus_credentials_get_pid (credentials);
      uid = _dbus_credentials_get_unix_uid (credentials);
      windows_sid = _dbus_credentials_get_windows_sid (credentials);
      linux_security_label =
        _dbus_credentials_get_linux_security_label (credentials);
    }

  /* we can't represent > 32-bit pids; if your system needs them, please
   * add ProcessID64 to the spec or something */
  if (pid <= _DBUS_UINT32_MAX && pid != DBUS_PID_UNSET &&
      !_dbus_asv_add_uint32 (asv_iter, "ProcessID", pid))
    return FALSE;

  /* we can't represent > 32-bit uids; if your system needs them, please
   * add UnixUserID64 to the spec or something */
  if (uid <= _DBUS_UINT32_MAX && uid != DBUS_UID_UNSET &&
      !_dbus_asv_add_uint32 (asv_iter, "UnixUserID", uid))
    return FALSE;

  if (credentials != NULL &&
      !bus_driver_credentials_fill_unix_gids (credentials, asv_iter))
    return FALSE;

  if (windows_sid != NULL)
    {
      DBusString str;
      dbus_bool_t result;

      _dbus_string_init_const (&str, windows_sid);
      result = _dbus_validate_utf8 (&str, 0, _dbus_string_get_length (&str));
      _dbus_string_free (&str);
      if (result)
        {
          if (!_dbus_asv_add_string (asv_iter, "WindowsSID", windows_sid))
            return FALSE;
        }
    }

  if (linux_security_label != NULL)
    {
      /* use the GVariant bytestring convention for strings of unknown
       * encoding: include the \0 in the payload, for zero-copy reading */
      if (!_dbus_asv_add_byte_array (asv_iter, "LinuxSecurityLabel",
                                     linux_security_label,
                                     strlen (linux_security_label) + 1))
        return FALSE;
    }

#ifdef DBUS_ENABLE_CONTAINERS
  /* This has to come from the connection, not the credentials */
  if (conn != NULL &&
      bus_containers_connection_is_contained (conn, &path, NULL, NULL))
    {
      if (!_dbus_asv_add_object_path (asv_iter,
                                      DBUS_INTERFACE_CONTAINERS1 ".Instance",
                                      path))
        return FALSE;
    }
#endif

  return TRUE;
}

static dbus_bool_t
bus_driver_handle_get_connection_credentials (DBusConnection *connection,
                                              BusTransaction *transaction,
                                              DBusMessage    *message,
                                              DBusError      *error)
{
  DBusConnection *conn;
  DBusCredentials *credentials = NULL;
  DBusMessage *reply;
  DBusMessageIter reply_iter;
  DBusMessageIter array_iter;
  const char *service;
  BusDriverFound found;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  reply = NULL;

  found = bus_driver_get_conn_helper (connection, message, "credentials",
                                      &service, &conn, error);

  switch (found)
    {
      case BUS_DRIVER_FOUND_SELF:
        conn = NULL;
        /* FIXME: Obtain the security label for the bus daemon itself,
         * if we can (this doesn't include it, both for performance
         * reasons and because LSMs don't guarantee that there is a way
         * to get the same string that would have come from SO_PEERSEC) */
        credentials = _dbus_credentials_new_from_current_process ();
        break;

      case BUS_DRIVER_FOUND_PEER:
        _dbus_assert (conn != NULL);
        break;

      case BUS_DRIVER_FOUND_ERROR:
        /* fall through */
      default:
        goto failed;
    }

  reply = _dbus_asv_new_method_return (message, &reply_iter, &array_iter);

  if (reply == NULL ||
      !bus_driver_fill_connection_credentials (credentials, conn, &array_iter) ||
      !_dbus_asv_close (&reply_iter, &array_iter))
    goto oom;

  if (! bus_transaction_send_from_driver (transaction, connection, reply))
    {
      /* this time we don't want to close the iterator again, so just
       * get rid of the message */
      dbus_message_unref (reply);
      reply = NULL;
      goto oom;
    }

  dbus_message_unref (reply);
  _dbus_clear_credentials (&credentials);
  return TRUE;

 oom:
  BUS_SET_OOM (error);

 failed:
  _DBUS_ASSERT_ERROR_IS_SET (error);

  if (reply)
    {
      _dbus_asv_abandon (&reply_iter, &array_iter);
      dbus_message_unref (reply);
    }

  _dbus_clear_credentials (&credentials);
  return FALSE;
}

static dbus_bool_t
bus_driver_handle_reload_config (DBusConnection *connection,
				 BusTransaction *transaction,
				 DBusMessage    *message,
				 DBusError      *error)
{
  BusContext *context;
  DBusMessage *reply;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  reply = NULL;

  context = bus_connection_get_context (connection);
  if (!bus_context_reload_config (context, error))
    goto failed;

  reply = dbus_message_new_method_return (message);
  if (reply == NULL)
    goto oom;

  if (! bus_transaction_send_from_driver (transaction, connection, reply))
    goto oom;

  dbus_message_unref (reply);
  return TRUE;

 oom:
  BUS_SET_OOM (error);

 failed:
  _DBUS_ASSERT_ERROR_IS_SET (error);
  if (reply)
    dbus_message_unref (reply);
  return FALSE;
}

#ifdef DBUS_ENABLE_VERBOSE_MODE
static dbus_bool_t
bus_driver_handle_enable_verbose (DBusConnection *connection,
                                  BusTransaction *transaction,
                                  DBusMessage    *message,
                                  DBusError      *error)
{
    DBusMessage *reply = NULL;

    _DBUS_ASSERT_ERROR_IS_CLEAR (error);

    reply = dbus_message_new_method_return (message);
    if (reply == NULL)
      goto oom;

    if (! bus_transaction_send_from_driver (transaction, connection, reply))
      goto oom;

    _dbus_set_verbose(TRUE);

    dbus_message_unref (reply);
    return TRUE;

   oom:
    _DBUS_ASSERT_ERROR_IS_CLEAR (error);

    BUS_SET_OOM (error);

    if (reply)
      dbus_message_unref (reply);
    return FALSE;
}

static dbus_bool_t
bus_driver_handle_disable_verbose (DBusConnection *connection,
                                   BusTransaction *transaction,
                                   DBusMessage    *message,
                                   DBusError      *error)
{
    DBusMessage *reply = NULL;

    _DBUS_ASSERT_ERROR_IS_CLEAR (error);

    reply = dbus_message_new_method_return (message);
    if (reply == NULL)
      goto oom;

    if (! bus_transaction_send_from_driver (transaction, connection, reply))
      goto oom;

    _dbus_set_verbose(FALSE);

    dbus_message_unref (reply);
    return TRUE;

   oom:
    _DBUS_ASSERT_ERROR_IS_CLEAR (error);

    BUS_SET_OOM (error);

    if (reply)
      dbus_message_unref (reply);
    return FALSE;
}
#endif

static dbus_bool_t
bus_driver_handle_get_id (DBusConnection *connection,
                          BusTransaction *transaction,
                          DBusMessage    *message,
                          DBusError      *error)
{
  BusContext *context;
  DBusMessage *reply;
  DBusString uuid;
  const char *v_STRING;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  if (!_dbus_string_init (&uuid))
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  reply = NULL;

  context = bus_connection_get_context (connection);
  if (!bus_context_get_id (context, &uuid))
    goto oom;

  reply = dbus_message_new_method_return (message);
  if (reply == NULL)
    goto oom;

  v_STRING = _dbus_string_get_const_data (&uuid);
  if (!dbus_message_append_args (reply,
                                 DBUS_TYPE_STRING, &v_STRING,
                                 DBUS_TYPE_INVALID))
    goto oom;

  _dbus_assert (dbus_message_has_signature (reply, "s"));

  if (! bus_transaction_send_from_driver (transaction, connection, reply))
    goto oom;

  _dbus_string_free (&uuid);
  dbus_message_unref (reply);
  return TRUE;

 oom:
  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  BUS_SET_OOM (error);

  if (reply)
    dbus_message_unref (reply);
  _dbus_string_free (&uuid);
  return FALSE;
}

static dbus_bool_t
bus_driver_handle_become_monitor (DBusConnection *connection,
                                  BusTransaction *transaction,
                                  DBusMessage    *message,
                                  DBusError      *error)
{
  char **match_rules = NULL;
  const char *bustype;
  BusContext *context;
  BusMatchRule *rule;
  DBusList *rules = NULL;
  DBusList *iter;
  DBusString str;
  int i;
  int n_match_rules;
  dbus_uint32_t flags;
  dbus_bool_t ret = FALSE;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  context = bus_transaction_get_context (transaction);
  bustype = context ? bus_context_get_type (context) : NULL;
  if (!bus_apparmor_allows_eavesdropping (connection, bustype, error))
    goto out;

  if (!dbus_message_get_args (message, error,
        DBUS_TYPE_ARRAY, DBUS_TYPE_STRING, &match_rules, &n_match_rules,
        DBUS_TYPE_UINT32, &flags,
        DBUS_TYPE_INVALID))
    goto out;

  if (flags != 0)
    {
      dbus_set_error (error, DBUS_ERROR_INVALID_ARGS,
          "BecomeMonitor does not support any flags yet");
      goto out;
    }

  /* Special case: a zero-length array becomes [""] */
  if (n_match_rules == 0)
    {
      dbus_free (match_rules);
      match_rules = dbus_malloc (2 * sizeof (char *));

      if (match_rules == NULL)
        {
          BUS_SET_OOM (error);
          goto out;
        }

      match_rules[0] = _dbus_strdup ("");

      if (match_rules[0] == NULL)
        {
          BUS_SET_OOM (error);
          goto out;
        }

      match_rules[1] = NULL;
      n_match_rules = 1;
    }

  for (i = 0; i < n_match_rules; i++)
    {
      _dbus_string_init_const (&str, match_rules[i]);
      rule = bus_match_rule_parse (connection, &str, error);

      if (rule == NULL)
        goto out;

      /* monitors always eavesdrop */
      bus_match_rule_set_client_is_eavesdropping (rule, TRUE);

      if (!_dbus_list_append (&rules, rule))
        {
          BUS_SET_OOM (error);
          bus_match_rule_unref (rule);
          goto out;
        }
    }

  /* Send the ack before we remove the rule, since the ack is undone
   * on transaction cancel, but becoming a monitor isn't.
   */
  if (!bus_driver_send_ack_reply (connection, transaction, message, error))
    goto out;

  if (!bus_connection_be_monitor (connection, transaction, &rules, error))
    goto out;

  ret = TRUE;

out:
  _DBUS_ASSERT_ERROR_XOR_BOOL (error, ret);

  for (iter = _dbus_list_get_first_link (&rules);
      iter != NULL;
      iter = _dbus_list_get_next_link (&rules, iter))
    bus_match_rule_unref (iter->data);

  _dbus_list_clear (&rules);

  dbus_free_string_array (match_rules);
  return ret;
}

static dbus_bool_t
bus_driver_handle_get_machine_id (DBusConnection *connection,
                                  BusTransaction *transaction,
                                  DBusMessage *message,
                                  DBusError *error)
{
  DBusMessage *reply = NULL;
  DBusString uuid;
  const char *str;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  if (!_dbus_string_init (&uuid))
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  if (!_dbus_get_local_machine_uuid_encoded (&uuid, error))
    goto fail;

  reply = dbus_message_new_method_return (message);

  if (reply == NULL)
    goto oom;

  str = _dbus_string_get_const_data (&uuid);

  if (!dbus_message_append_args (reply,
                                 DBUS_TYPE_STRING, &str,
                                 DBUS_TYPE_INVALID))
    goto oom;

  _dbus_assert (dbus_message_has_signature (reply, "s"));

  if (!bus_transaction_send_from_driver (transaction, connection, reply))
    goto oom;

  _dbus_string_free (&uuid);
  dbus_message_unref (reply);
  return TRUE;

oom:
  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  BUS_SET_OOM (error);

fail:
  _DBUS_ASSERT_ERROR_IS_SET (error);

  if (reply != NULL)
    dbus_message_unref (reply);

  _dbus_string_free (&uuid);
  return FALSE;
}

static dbus_bool_t
bus_driver_handle_ping (DBusConnection *connection,
                        BusTransaction *transaction,
                        DBusMessage *message,
                        DBusError *error)
{
  return bus_driver_send_ack_reply (connection, transaction, message, error);
}

static dbus_bool_t bus_driver_handle_get (DBusConnection *connection,
                                          BusTransaction *transaction,
                                          DBusMessage *message,
                                          DBusError *error);

static dbus_bool_t bus_driver_handle_get_all (DBusConnection *connection,
                                              BusTransaction *transaction,
                                              DBusMessage *message,
                                              DBusError *error);

static dbus_bool_t bus_driver_handle_set (DBusConnection *connection,
                                          BusTransaction *transaction,
                                          DBusMessage *message,
                                          DBusError *error);

static dbus_bool_t features_getter (BusContext      *context,
                                    DBusMessageIter *variant_iter);
static dbus_bool_t interfaces_getter (BusContext      *context,
                                      DBusMessageIter *variant_iter);

typedef enum
{
  /* Various older methods were available at every object path. We have to
   * preserve that behaviour for backwards compatibility, but we can at least
   * stop doing that for newly added methods.
   * The special Peer interface should also work at any object path.
   * <https://bugs.freedesktop.org/show_bug.cgi?id=101256> */
  METHOD_FLAG_ANY_PATH = (1 << 0),

  /* If set, callers must be privileged. On Unix, the uid of the connection
   * must either be the uid of this process, or 0 (root). On Windows,
   * the SID of the connection must be the SID of this process.
   *
   * This flag effectively implies METHOD_FLAG_NO_CONTAINERS, because
   * containers are never privileged. */
  METHOD_FLAG_PRIVILEGED = (1 << 1),

  /* If set, callers must not be associated with a container instance. */
  METHOD_FLAG_NO_CONTAINERS = (1 << 2),

  METHOD_FLAG_NONE = 0
} MethodFlags;

typedef struct
{
  const char *name;
  const char *in_args;
  const char *out_args;
  dbus_bool_t (* handler) (DBusConnection *connection,
                           BusTransaction *transaction,
                           DBusMessage    *message,
                           DBusError      *error);
  MethodFlags flags;
} MessageHandler;

typedef struct
{
  const char *name;
  const char *type;
  dbus_bool_t (* getter) (BusContext      *context,
                          DBusMessageIter *variant_iter);
} PropertyHandler;

/* For speed it might be useful to sort this in order of
 * frequency of use (but doesn't matter with only a few items
 * anyhow)
 */
static const MessageHandler dbus_message_handlers[] = {
  { "Hello",
    "",
    DBUS_TYPE_STRING_AS_STRING,
    bus_driver_handle_hello,
    METHOD_FLAG_ANY_PATH },
  { "RequestName",
    DBUS_TYPE_STRING_AS_STRING DBUS_TYPE_UINT32_AS_STRING,
    DBUS_TYPE_UINT32_AS_STRING,
    bus_driver_handle_acquire_service,
    METHOD_FLAG_ANY_PATH },
  { "ReleaseName",
    DBUS_TYPE_STRING_AS_STRING,
    DBUS_TYPE_UINT32_AS_STRING,
    bus_driver_handle_release_service,
    METHOD_FLAG_ANY_PATH },
  { "StartServiceByName",
    DBUS_TYPE_STRING_AS_STRING DBUS_TYPE_UINT32_AS_STRING,
    DBUS_TYPE_UINT32_AS_STRING,
    bus_driver_handle_activate_service,
    METHOD_FLAG_ANY_PATH },
  { "UpdateActivationEnvironment",
    DBUS_TYPE_ARRAY_AS_STRING DBUS_DICT_ENTRY_BEGIN_CHAR_AS_STRING DBUS_TYPE_STRING_AS_STRING DBUS_TYPE_STRING_AS_STRING DBUS_DICT_ENTRY_END_CHAR_AS_STRING,
    "",
    bus_driver_handle_update_activation_environment,
    METHOD_FLAG_PRIVILEGED },
  { "NameHasOwner",
    DBUS_TYPE_STRING_AS_STRING,
    DBUS_TYPE_BOOLEAN_AS_STRING,
    bus_driver_handle_service_exists,
    METHOD_FLAG_ANY_PATH },
  { "ListNames",
    "",
    DBUS_TYPE_ARRAY_AS_STRING DBUS_TYPE_STRING_AS_STRING,
    bus_driver_handle_list_services,
    METHOD_FLAG_ANY_PATH },
  { "ListActivatableNames",
    "",
    DBUS_TYPE_ARRAY_AS_STRING DBUS_TYPE_STRING_AS_STRING,
    bus_driver_handle_list_activatable_services,
    METHOD_FLAG_ANY_PATH },
  { "AddMatch",
    DBUS_TYPE_STRING_AS_STRING,
    "",
    bus_driver_handle_add_match,
    METHOD_FLAG_ANY_PATH },
  { "RemoveMatch",
    DBUS_TYPE_STRING_AS_STRING,
    "",
    bus_driver_handle_remove_match,
    METHOD_FLAG_ANY_PATH },
  { "GetNameOwner",
    DBUS_TYPE_STRING_AS_STRING,
    DBUS_TYPE_STRING_AS_STRING,
    bus_driver_handle_get_service_owner,
    METHOD_FLAG_ANY_PATH },
  { "ListQueuedOwners",
    DBUS_TYPE_STRING_AS_STRING,
    DBUS_TYPE_ARRAY_AS_STRING DBUS_TYPE_STRING_AS_STRING,
    bus_driver_handle_list_queued_owners,
    METHOD_FLAG_ANY_PATH },
  { "GetConnectionUnixUser",
    DBUS_TYPE_STRING_AS_STRING,
    DBUS_TYPE_UINT32_AS_STRING,
    bus_driver_handle_get_connection_unix_user,
    METHOD_FLAG_ANY_PATH },
  { "GetConnectionUnixProcessID",
    DBUS_TYPE_STRING_AS_STRING,
    DBUS_TYPE_UINT32_AS_STRING,
    bus_driver_handle_get_connection_unix_process_id,
    METHOD_FLAG_ANY_PATH },
  { "GetAdtAuditSessionData",
    DBUS_TYPE_STRING_AS_STRING,
    DBUS_TYPE_ARRAY_AS_STRING DBUS_TYPE_BYTE_AS_STRING,
    bus_driver_handle_get_adt_audit_session_data,
    METHOD_FLAG_ANY_PATH },
  { "GetConnectionSELinuxSecurityContext",
    DBUS_TYPE_STRING_AS_STRING,
    DBUS_TYPE_ARRAY_AS_STRING DBUS_TYPE_BYTE_AS_STRING,
    bus_driver_handle_get_connection_selinux_security_context,
    METHOD_FLAG_ANY_PATH },
  { "ReloadConfig",
    "",
    "",
    bus_driver_handle_reload_config,
    METHOD_FLAG_ANY_PATH },
  { "GetId",
    "",
    DBUS_TYPE_STRING_AS_STRING,
    bus_driver_handle_get_id,
    METHOD_FLAG_ANY_PATH },
  { "GetConnectionCredentials", "s", "a{sv}",
    bus_driver_handle_get_connection_credentials,
    METHOD_FLAG_ANY_PATH },
  { NULL, NULL, NULL, NULL }
};

static const PropertyHandler dbus_property_handlers[] = {
  { "Features", "as", features_getter },
  { "Interfaces", "as", interfaces_getter },
  { NULL, NULL, NULL }
};

static dbus_bool_t bus_driver_handle_introspect (DBusConnection *,
    BusTransaction *, DBusMessage *, DBusError *);

static const MessageHandler properties_message_handlers[] = {
  { "Get", "ss", "v", bus_driver_handle_get, METHOD_FLAG_NONE },
  { "GetAll", "s", "a{sv}", bus_driver_handle_get_all, METHOD_FLAG_NONE },
  { "Set", "ssv", "", bus_driver_handle_set, METHOD_FLAG_NONE },
  { NULL, NULL, NULL, NULL }
};

static const MessageHandler introspectable_message_handlers[] = {
  { "Introspect", "", DBUS_TYPE_STRING_AS_STRING, bus_driver_handle_introspect,
    METHOD_FLAG_ANY_PATH },
  { NULL, NULL, NULL, NULL }
};

#ifdef DBUS_ENABLE_CONTAINERS
static const MessageHandler containers_message_handlers[] = {
  { "AddServer", "ssa{sv}a{sv}", "oays", bus_containers_handle_add_server,
    METHOD_FLAG_NO_CONTAINERS },
  { "StopInstance", "o", "", bus_containers_handle_stop_instance,
    METHOD_FLAG_NO_CONTAINERS },
  { "StopListening", "o", "", bus_containers_handle_stop_listening,
    METHOD_FLAG_NO_CONTAINERS },
  { "GetConnectionInstance", "s", "oa{sv}ssa{sv}",
    bus_containers_handle_get_connection_instance,
    METHOD_FLAG_NONE },
  { "GetInstanceInfo", "o", "a{sv}ssa{sv}", bus_containers_handle_get_instance_info,
    METHOD_FLAG_NONE },
  { "RequestHeader", "", "", bus_containers_handle_request_header,
    METHOD_FLAG_NONE },
  { NULL, NULL, NULL, NULL }
};
static const PropertyHandler containers_property_handlers[] = {
  { "SupportedArguments", "as", bus_containers_supported_arguments_getter },
  { NULL, NULL, NULL }
};
#endif

static const MessageHandler monitoring_message_handlers[] = {
  { "BecomeMonitor", "asu", "", bus_driver_handle_become_monitor,
    METHOD_FLAG_PRIVILEGED },
  { NULL, NULL, NULL, NULL }
};

#ifdef DBUS_ENABLE_VERBOSE_MODE
static const MessageHandler verbose_message_handlers[] = {
  { "EnableVerbose", "", "", bus_driver_handle_enable_verbose,
    METHOD_FLAG_NO_CONTAINERS },
  { "DisableVerbose", "", "", bus_driver_handle_disable_verbose,
    METHOD_FLAG_NO_CONTAINERS },
  { NULL, NULL, NULL, NULL }
};
#endif

#ifdef DBUS_ENABLE_STATS
static const MessageHandler stats_message_handlers[] = {
  { "GetStats", "", "a{sv}", bus_stats_handle_get_stats,
    METHOD_FLAG_NO_CONTAINERS },
  { "GetConnectionStats", "s", "a{sv}", bus_stats_handle_get_connection_stats,
    METHOD_FLAG_NO_CONTAINERS },
  { "GetAllMatchRules", "", "a{sas}", bus_stats_handle_get_all_match_rules,
    METHOD_FLAG_NO_CONTAINERS },
  { NULL, NULL, NULL, NULL }
};
#endif

static const MessageHandler peer_message_handlers[] = {
  { "GetMachineId", "", "s", bus_driver_handle_get_machine_id,
    METHOD_FLAG_ANY_PATH },
  { "Ping", "", "", bus_driver_handle_ping, METHOD_FLAG_ANY_PATH },
  { NULL, NULL, NULL, NULL }
};

typedef enum
{
  /* Various older interfaces were available at every object path. We have to
   * preserve that behaviour for backwards compatibility, but we can at least
   * stop doing that for newly added interfaces:
   * <https://bugs.freedesktop.org/show_bug.cgi?id=101256>
   * Introspectable and Peer are also useful at all object paths. */
  INTERFACE_FLAG_ANY_PATH = (1 << 0),

  /* Set this flag for interfaces that should not show up in the
   * Interfaces property. */
  INTERFACE_FLAG_UNINTERESTING = (1 << 1),

  INTERFACE_FLAG_NONE = 0
} InterfaceFlags;

typedef struct {
  const char *name;
  const MessageHandler *message_handlers;
  const char *extra_introspection;
  InterfaceFlags flags;
  const PropertyHandler *property_handlers;
} InterfaceHandler;

/* These should ideally be sorted by frequency of use, although it
 * probably doesn't matter with this few items */
static InterfaceHandler interface_handlers[] = {
  { DBUS_INTERFACE_DBUS, dbus_message_handlers,
    "    <signal name=\"NameOwnerChanged\">\n"
    "      <arg type=\"s\"/>\n"
    "      <arg type=\"s\"/>\n"
    "      <arg type=\"s\"/>\n"
    "    </signal>\n"
    "    <signal name=\"NameLost\">\n"
    "      <arg type=\"s\"/>\n"
    "    </signal>\n"
    "    <signal name=\"NameAcquired\">\n"
    "      <arg type=\"s\"/>\n"
    "    </signal>\n"
    "    <signal name=\"ActivatableServicesChanged\">\n"
    "    </signal>\n",
    /* Not in the Interfaces property because if you can get the properties
     * of the o.fd.DBus interface, then you certainly have the o.fd.DBus
     * interface, so there is little point in listing it explicitly.
     * Partially available at all paths for backwards compatibility. */
    INTERFACE_FLAG_ANY_PATH | INTERFACE_FLAG_UNINTERESTING,
    dbus_property_handlers },
  { DBUS_INTERFACE_PROPERTIES, properties_message_handlers,
    "    <signal name=\"PropertiesChanged\">\n"
    "      <arg type=\"s\" name=\"interface_name\"/>\n"
    "      <arg type=\"a{sv}\" name=\"changed_properties\"/>\n"
    "      <arg type=\"as\" name=\"invalidated_properties\"/>\n"
    "    </signal>\n",
    /* Not in the Interfaces property because if you can get the properties
     * of the o.fd.DBus interface, then you certainly have Properties. */
    INTERFACE_FLAG_UNINTERESTING },
  { DBUS_INTERFACE_INTROSPECTABLE, introspectable_message_handlers, NULL,
    /* Not in the Interfaces property because introspection isn't really a
     * feature in the same way as e.g. Monitoring.
     * Available at all paths so tools like d-feet can start from "/". */
    INTERFACE_FLAG_ANY_PATH | INTERFACE_FLAG_UNINTERESTING },
  { DBUS_INTERFACE_MONITORING, monitoring_message_handlers, NULL,
    INTERFACE_FLAG_NONE },
#ifdef DBUS_ENABLE_VERBOSE_MODE
  { DBUS_INTERFACE_VERBOSE, verbose_message_handlers, NULL,
    INTERFACE_FLAG_NONE },
#endif
#ifdef DBUS_ENABLE_STATS
  { BUS_INTERFACE_STATS, stats_message_handlers, NULL,
    INTERFACE_FLAG_NONE },
#endif
#ifdef DBUS_ENABLE_CONTAINERS
  { DBUS_INTERFACE_CONTAINERS1, containers_message_handlers,
    "    <signal name=\"InstanceRemoved\">\n"
    "      <arg type=\"o\" name=\"path\"/>\n"
    "    </signal>\n",
    INTERFACE_FLAG_NONE, containers_property_handlers },
#endif
  { DBUS_INTERFACE_PEER, peer_message_handlers, NULL,
    /* Not in the Interfaces property because it's a pseudo-interface
     * on all object paths of all connections, rather than a feature of the
     * bus driver object. */
    INTERFACE_FLAG_ANY_PATH | INTERFACE_FLAG_UNINTERESTING },
  { NULL, NULL, NULL }
};

static dbus_bool_t
write_args_for_direction (DBusString *xml,
			  const char *signature,
			  dbus_bool_t in)
{
  DBusTypeReader typereader;
  DBusString sigstr;
  int current_type;

  _dbus_string_init_const (&sigstr, signature);
  _dbus_type_reader_init_types_only (&typereader, &sigstr, 0);

  while ((current_type = _dbus_type_reader_get_current_type (&typereader)) != DBUS_TYPE_INVALID)
    {
      const DBusString *subsig;
      int start, len;

      _dbus_type_reader_get_signature (&typereader, &subsig, &start, &len);
      if (!_dbus_string_append_printf (xml, "      <arg direction=\"%s\" type=\"",
				       in ? "in" : "out"))
	goto oom;
      if (!_dbus_string_append_len (xml,
				    _dbus_string_get_const_data (subsig) + start,
				    len))
	goto oom;
      if (!_dbus_string_append (xml, "\"/>\n"))
	goto oom;

      _dbus_type_reader_next (&typereader);
    }
  return TRUE;
 oom:
  return FALSE;
}

dbus_bool_t
bus_driver_generate_introspect_string (DBusString *xml,
                                       dbus_bool_t is_canonical_path,
                                       DBusMessage *message)
{
  const InterfaceHandler *ih;
  const MessageHandler *mh;
  const PropertyHandler *ph;

  if (!_dbus_string_append (xml, DBUS_INTROSPECT_1_0_XML_DOCTYPE_DECL_NODE))
    return FALSE;
  if (!_dbus_string_append (xml, "<node>\n"))
    return FALSE;

  for (ih = interface_handlers; ih->name != NULL; ih++)
    {
      if (!(is_canonical_path || (ih->flags & INTERFACE_FLAG_ANY_PATH)))
        continue;

      if (!_dbus_string_append_printf (xml, "  <interface name=\"%s\">\n",
                                       ih->name))
        return FALSE;

      for (mh = ih->message_handlers; mh->name != NULL; mh++)
        {
          if (!_dbus_string_append_printf (xml, "    <method name=\"%s\">\n",
                                           mh->name))
            return FALSE;

          if (!write_args_for_direction (xml, mh->in_args, TRUE))
            return FALSE;

          if (!write_args_for_direction (xml, mh->out_args, FALSE))
            return FALSE;

          if (!_dbus_string_append (xml, "    </method>\n"))
            return FALSE;
        }

      for (ph = ih->property_handlers; ph != NULL && ph->name != NULL; ph++)
        {
          /* We only have constant properties so far, so hard-code that bit */
          if (!_dbus_string_append_printf (xml,
                                           "    <property name=\"%s\" type=\"%s\" access=\"read\">\n",
                                           ph->name, ph->type))
            return FALSE;

          if (!_dbus_string_append (xml,
                                    "      <annotation name=\"org.freedesktop.DBus.Property.EmitsChangedSignal\" value=\"const\"/>\n"
                                    "    </property>\n"))
            return FALSE;
        }

      if (ih->extra_introspection != NULL &&
          !_dbus_string_append (xml, ih->extra_introspection))
        return FALSE;

      if (!_dbus_string_append (xml, "  </interface>\n"))
        return FALSE;
    }

  if (message != NULL)
    {
      /* Make the bus driver object path discoverable */
      if (dbus_message_has_path (message, "/"))
        {
          if (!_dbus_string_append (xml,
                "  <node name=\"org/freedesktop/DBus\"/>\n"))
            return FALSE;
        }
      else if (dbus_message_has_path (message, "/org"))
        {
          if (!_dbus_string_append (xml,
                "  <node name=\"freedesktop/DBus\"/>\n"))
            return FALSE;
        }
      else if (dbus_message_has_path (message, "/org/freedesktop"))
        {
          if (!_dbus_string_append (xml, "  <node name=\"DBus\"/>\n"))
            return FALSE;
        }
    }

  if (!_dbus_string_append (xml, "</node>\n"))
    return FALSE;

  return TRUE;
}

static dbus_bool_t
bus_driver_handle_introspect (DBusConnection *connection,
                              BusTransaction *transaction,
                              DBusMessage    *message,
                              DBusError      *error)
{
  DBusString xml;
  DBusMessage *reply;
  const char *v_STRING;
  dbus_bool_t is_canonical_path;

  _dbus_verbose ("Introspect() on bus driver\n");

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  reply = NULL;

  if (! dbus_message_get_args (message, error,
			       DBUS_TYPE_INVALID))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      return FALSE;
    }

  if (!_dbus_string_init (&xml))
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  is_canonical_path = dbus_message_has_path (message, DBUS_PATH_DBUS);

  if (!bus_driver_generate_introspect_string (&xml, is_canonical_path, message))
    goto oom;

  v_STRING = _dbus_string_get_const_data (&xml);

  reply = dbus_message_new_method_return (message);
  if (reply == NULL)
    goto oom;

  if (! dbus_message_append_args (reply,
                                  DBUS_TYPE_STRING, &v_STRING,
                                  DBUS_TYPE_INVALID))
    goto oom;

  if (! bus_transaction_send_from_driver (transaction, connection, reply))
    goto oom;

  dbus_message_unref (reply);
  _dbus_string_free (&xml);

  return TRUE;

 oom:
  BUS_SET_OOM (error);

  if (reply)
    dbus_message_unref (reply);

  _dbus_string_free (&xml);

  return FALSE;
}

dbus_bool_t
bus_driver_handle_message (DBusConnection *connection,
                           BusTransaction *transaction,
			   DBusMessage    *message,
                           DBusError      *error)
{
  const char *name, *interface;
  const InterfaceHandler *ih;
  const MessageHandler *mh;
  dbus_bool_t found_interface = FALSE;
  dbus_bool_t is_canonical_path;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  if (dbus_message_is_signal (message, "org.freedesktop.systemd1.Activator", "ActivationFailure"))
    {
      BusContext *context;
      DBusConnection *systemd;

      /* This is a directed signal, not a method call, so the log message
       * is a little weird (it talks about "calling" ActivationFailure),
       * but it's close enough */
      if (!bus_driver_check_caller_is_privileged (connection,
                                                  transaction,
                                                  message,
                                                  error))
        return FALSE;

      context = bus_connection_get_context (connection);
      systemd = bus_driver_get_owner_of_name (connection,
          "org.freedesktop.systemd1");

      if (systemd != connection)
        {
          const char *attacker;

          attacker = bus_connection_get_name (connection);
          bus_context_log (context, DBUS_SYSTEM_LOG_SECURITY,
                           "Ignoring forged ActivationFailure message from "
                           "connection %s (%s)",
                           attacker ? attacker : "(unauthenticated)",
                           bus_connection_get_loginfo (connection));
          /* ignore it */
          return TRUE;
        }

      if (!bus_context_get_systemd_activation (context))
        {
          bus_context_log (context, DBUS_SYSTEM_LOG_WARNING,
                           "Ignoring unexpected ActivationFailure message "
                           "while not using systemd activation");
          return FALSE;
        }

      return dbus_activation_systemd_failure(bus_context_get_activation(context), message);
    }

  if (dbus_message_get_type (message) != DBUS_MESSAGE_TYPE_METHOD_CALL)
    {
      _dbus_verbose ("Driver got a non-method-call message, ignoring\n");
      return TRUE; /* we just ignore this */
    }

  /* may be NULL, which means "any interface will do" */
  interface = dbus_message_get_interface (message);

  _dbus_assert (dbus_message_get_member (message) != NULL);

  name = dbus_message_get_member (message);

  _dbus_verbose ("Driver got a method call: %s\n", name);

  /* security checks should have kept this from getting here */
  _dbus_assert (dbus_message_get_sender (message) != NULL ||
                strcmp (name, "Hello") == 0);

  is_canonical_path = dbus_message_has_path (message, DBUS_PATH_DBUS);

  for (ih = interface_handlers; ih->name != NULL; ih++)
    {
      if (!(is_canonical_path || (ih->flags & INTERFACE_FLAG_ANY_PATH)))
        continue;

      if (interface != NULL && strcmp (interface, ih->name) != 0)
        continue;

      found_interface = TRUE;

      for (mh = ih->message_handlers; mh->name != NULL; mh++)
        {
          if (strcmp (mh->name, name) != 0)
            continue;

          _dbus_verbose ("Found driver handler for %s\n", name);

          if (mh->flags & METHOD_FLAG_PRIVILEGED)
            {
              if (!bus_driver_check_caller_is_privileged (connection,
                                                          transaction, message,
                                                          error))
                {
                  _DBUS_ASSERT_ERROR_IS_SET (error);
                  return FALSE;
                }
            }
          else if (mh->flags & METHOD_FLAG_NO_CONTAINERS)
            {
              if (!bus_driver_check_caller_is_not_container (connection,
                                                             transaction,
                                                             message, error))
                {
                  _DBUS_ASSERT_ERROR_IS_SET (error);
                  return FALSE;
                }
            }

          if (!(is_canonical_path || (mh->flags & METHOD_FLAG_ANY_PATH)))
            {
              _DBUS_ASSERT_ERROR_IS_CLEAR (error);
              dbus_set_error (error, DBUS_ERROR_ACCESS_DENIED,
                  "Method '%s' is only available at the canonical object path '%s'",
                  dbus_message_get_member (message), DBUS_PATH_DBUS);
              _DBUS_ASSERT_ERROR_IS_SET (error);
              return FALSE;
            }

          if (!dbus_message_has_signature (message, mh->in_args))
            {
              _DBUS_ASSERT_ERROR_IS_CLEAR (error);
              _dbus_verbose ("Call to %s has wrong args (%s, expected %s)\n",
                             name, dbus_message_get_signature (message),
                             mh->in_args);

              dbus_set_error (error, DBUS_ERROR_INVALID_ARGS,
                              "Call to %s has wrong args (%s, expected %s)\n",
                              name, dbus_message_get_signature (message),
                              mh->in_args);
              _DBUS_ASSERT_ERROR_IS_SET (error);
              return FALSE;
            }

          if ((* mh->handler) (connection, transaction, message, error))
            {
              _DBUS_ASSERT_ERROR_IS_CLEAR (error);
              _dbus_verbose ("Driver handler succeeded\n");
              return TRUE;
            }
          else
            {
              _DBUS_ASSERT_ERROR_IS_SET (error);
              _dbus_verbose ("Driver handler returned failure\n");
              return FALSE;
            }
        }
    }

  _dbus_verbose ("No driver handler for message \"%s\"\n",
                 name);

  dbus_set_error (error, found_interface ? DBUS_ERROR_UNKNOWN_METHOD : DBUS_ERROR_UNKNOWN_INTERFACE,
                  "%s does not understand message %s",
                  DBUS_SERVICE_DBUS, name);

  return FALSE;
}

void
bus_driver_remove_connection (DBusConnection *connection)
{
  /* FIXME 1.0 Does nothing for now, should unregister the connection
   * with the bus driver.
   */
}

static dbus_bool_t
features_getter (BusContext      *context,
                 DBusMessageIter *variant_iter)
{
  DBusMessageIter arr_iter;
  const char *s;

  if (!dbus_message_iter_open_container (variant_iter, DBUS_TYPE_ARRAY,
                                         DBUS_TYPE_STRING_AS_STRING,
                                         &arr_iter))
    return FALSE;

  s = "ActivatableServicesChanged";

  if (!dbus_message_iter_append_basic (&arr_iter, DBUS_TYPE_STRING, &s))
    goto abandon;


  if (bus_apparmor_enabled ())
    {
      s = "AppArmor";

      if (!dbus_message_iter_append_basic (&arr_iter, DBUS_TYPE_STRING, &s))
        goto abandon;
    }

  s = "HeaderFiltering";

  if (!dbus_message_iter_append_basic (&arr_iter, DBUS_TYPE_STRING, &s))
    goto abandon;

  if (bus_selinux_enabled ())
    {
      s = "SELinux";

      if (!dbus_message_iter_append_basic (&arr_iter, DBUS_TYPE_STRING, &s))
        goto abandon;
    }

  if (bus_context_get_systemd_activation (context))
    {
      s = "SystemdActivation";

      if (!dbus_message_iter_append_basic (&arr_iter, DBUS_TYPE_STRING, &s))
        goto abandon;
    }

  return dbus_message_iter_close_container (variant_iter, &arr_iter);

abandon:
  dbus_message_iter_abandon_container (variant_iter, &arr_iter);
  return FALSE;
}

static dbus_bool_t
interfaces_getter (BusContext      *context,
                   DBusMessageIter *variant_iter)
{
  DBusMessageIter arr_iter;
  const InterfaceHandler *ih;

  if (!dbus_message_iter_open_container (variant_iter, DBUS_TYPE_ARRAY,
                                         DBUS_TYPE_STRING_AS_STRING,
                                         &arr_iter))
    return FALSE;

  for (ih = interface_handlers; ih->name != NULL; ih++)
    {
      if (ih->flags & INTERFACE_FLAG_UNINTERESTING)
        continue;

      if (!dbus_message_iter_append_basic (&arr_iter, DBUS_TYPE_STRING,
                                           &ih->name))
        goto abandon;
    }

  return dbus_message_iter_close_container (variant_iter, &arr_iter);

abandon:
  dbus_message_iter_abandon_container (variant_iter, &arr_iter);
  return FALSE;
}

static const InterfaceHandler *
bus_driver_find_interface (const char  *name,
                           dbus_bool_t  canonical_path,
                           DBusError   *error)
{
  const InterfaceHandler *ih;

  for (ih = interface_handlers; ih->name != NULL; ih++)
    {
      if (!(canonical_path || (ih->flags & INTERFACE_FLAG_ANY_PATH)))
        continue;

      if (strcmp (name, ih->name) == 0)
        return ih;
    }

  dbus_set_error (error, DBUS_ERROR_UNKNOWN_INTERFACE,
                  "Interface \"%s\" not found", name);
  return NULL;
}

static const PropertyHandler *
interface_handler_find_property (const InterfaceHandler *ih,
                                 const char             *name,
                                 DBusError              *error)
{
  const PropertyHandler *ph;

  for (ph = ih->property_handlers; ph != NULL && ph->name != NULL; ph++)
    {
      if (strcmp (name, ph->name) == 0)
        return ph;
    }

  dbus_set_error (error, DBUS_ERROR_UNKNOWN_PROPERTY,
                  "Property \"%s.%s\" not found", ih->name, name);
  return NULL;
}

static dbus_bool_t
bus_driver_handle_get (DBusConnection *connection,
                       BusTransaction *transaction,
                       DBusMessage    *message,
                       DBusError      *error)
{
  const InterfaceHandler *ih;
  const PropertyHandler *handler;
  const char *iface;
  const char *prop;
  BusContext *context;
  DBusMessage *reply = NULL;
  DBusMessageIter iter;
  DBusMessageIter var_iter;

  /* The message signature has already been checked for us,
   * so this should always succeed. */
  if (!dbus_message_get_args (message, error,
                              DBUS_TYPE_STRING, &iface,
                              DBUS_TYPE_STRING, &prop,
                              DBUS_TYPE_INVALID))
    return FALSE;

  /* We only implement Properties on /org/freedesktop/DBus so far. */
  ih = bus_driver_find_interface (iface, TRUE, error);

  if (ih == NULL)
    return FALSE;

  handler = interface_handler_find_property (ih, prop, error);

  if (handler == NULL)
    return FALSE;

  context = bus_transaction_get_context (transaction);

  reply = dbus_message_new_method_return (message);

  if (reply == NULL)
    goto oom;

  dbus_message_iter_init_append (reply, &iter);

  if (!dbus_message_iter_open_container (&iter, DBUS_TYPE_VARIANT,
                                         handler->type, &var_iter))
    goto oom;

  if (!handler->getter (context, &var_iter))
    {
      dbus_message_iter_abandon_container (&iter, &var_iter);
      goto oom;
    }

  if (!dbus_message_iter_close_container (&iter, &var_iter))
    goto oom;

  if (!bus_transaction_send_from_driver (transaction, connection, reply))
    goto oom;

  dbus_message_unref (reply);
  return TRUE;

oom:
  if (reply != NULL)
    dbus_message_unref (reply);

  BUS_SET_OOM (error);
  return FALSE;
}

static dbus_bool_t
bus_driver_handle_get_all (DBusConnection *connection,
                           BusTransaction *transaction,
                           DBusMessage    *message,
                           DBusError      *error)
{
  const InterfaceHandler *ih;
  const char *iface;
  const PropertyHandler *ph;
  DBusMessageIter reply_iter;
  DBusMessageIter array_iter;
  BusContext *context;
  DBusMessage *reply = NULL;

  /* The message signature has already been checked for us,
   * so this should always succeed. */
  if (!dbus_message_get_args (message, error,
                              DBUS_TYPE_STRING, &iface,
                              DBUS_TYPE_INVALID))
    return FALSE;

  /* We only implement Properties on /org/freedesktop/DBus so far. */
  ih = bus_driver_find_interface (iface, TRUE, error);

  if (ih == NULL)
    return FALSE;

  context = bus_transaction_get_context (transaction);

  reply = _dbus_asv_new_method_return (message, &reply_iter, &array_iter);

  if (reply == NULL)
    goto oom;

  for (ph = ih->property_handlers; ph != NULL && ph->name != NULL; ph++)
    {
      DBusMessageIter entry_iter;
      DBusMessageIter var_iter;

      if (!_dbus_asv_open_entry (&array_iter, &entry_iter, ph->name,
                                 ph->type, &var_iter))
        goto oom_abandon_message;

      if (!ph->getter (context, &var_iter))
        {
          _dbus_asv_abandon_entry (&array_iter, &entry_iter, &var_iter);
          goto oom_abandon_message;
        }

      if (!_dbus_asv_close_entry (&array_iter, &entry_iter, &var_iter))
        goto oom_abandon_message;
    }

  if (!_dbus_asv_close (&reply_iter, &array_iter))
    goto oom;

  if (!bus_transaction_send_from_driver (transaction, connection, reply))
    goto oom;

  dbus_message_unref (reply);
  return TRUE;

oom_abandon_message:
  _dbus_asv_abandon (&reply_iter, &array_iter);
  /* fall through */
oom:
  if (reply != NULL)
    dbus_message_unref (reply);

  BUS_SET_OOM (error);
  return FALSE;
}

static dbus_bool_t
bus_driver_handle_set (DBusConnection *connection,
                       BusTransaction *transaction,
                       DBusMessage    *message,
                       DBusError      *error)
{
  const InterfaceHandler *ih;
  const char *iface;
  const char *prop;
  const PropertyHandler *handler;
  DBusMessageIter iter;

  /* We already checked this in bus_driver_handle_message() */
  _dbus_assert (dbus_message_has_signature (message, "ssv"));

  if (!dbus_message_iter_init (message, &iter))
    _dbus_assert_not_reached ("Message type was already checked to be 'ssv'");

  dbus_message_iter_get_basic (&iter, &iface);

  if (!dbus_message_iter_next (&iter))
    _dbus_assert_not_reached ("Message type was already checked to be 'ssv'");

  dbus_message_iter_get_basic (&iter, &prop);

  /* We only implement Properties on /org/freedesktop/DBus so far. */
  ih = bus_driver_find_interface (iface, TRUE, error);

  if (ih == NULL)
    return FALSE;

  handler = interface_handler_find_property (ih, prop, error);

  if (handler == NULL)
    return FALSE;

  /* We don't implement any properties that can be set yet. */
  dbus_set_error (error, DBUS_ERROR_PROPERTY_READ_ONLY,
                  "Property '%s.%s' cannot be set", iface, prop);
  return FALSE;
}
