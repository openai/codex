/* stats.c - statistics from the bus driver
 *
 * Copyright © 2011-2012 Nokia Corporation
 * Copyright © 2012-2013 Collabora Ltd.
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
 * Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA
 * 02110-1301 USA
 */

#include <config.h>
#include "stats.h"

#include <dbus/dbus-asv-util.h>
#include <dbus/dbus-internals.h>
#include <dbus/dbus-connection-internal.h>

#include "connection.h"
#include "driver.h"
#include "services.h"
#include "signals.h"
#include "utils.h"

#ifdef DBUS_ENABLE_STATS

dbus_bool_t
bus_stats_handle_get_stats (DBusConnection *connection,
                            BusTransaction *transaction,
                            DBusMessage    *message,
                            DBusError      *error)
{
  BusContext *context;
  BusConnections *connections;
  DBusMessage *reply = NULL;
  DBusMessageIter iter, arr_iter;
  static dbus_uint32_t stats_serial = 0;
  dbus_uint32_t in_use, in_free_list, allocated;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  context = bus_transaction_get_context (transaction);
  connections = bus_context_get_connections (context);

  reply = _dbus_asv_new_method_return (message, &iter, &arr_iter);

  if (reply == NULL)
    goto oom;

  /* Globals */

  _dbus_list_get_stats (&in_use, &in_free_list, &allocated);

  if (!_dbus_asv_add_uint32 (&arr_iter, "Serial", stats_serial++) ||
      !_dbus_asv_add_uint32 (&arr_iter, "ListMemPoolUsedBytes", in_use) ||
      !_dbus_asv_add_uint32 (&arr_iter, "ListMemPoolCachedBytes", in_free_list) ||
      !_dbus_asv_add_uint32 (&arr_iter, "ListMemPoolAllocatedBytes", allocated))
    {
      _dbus_asv_abandon (&iter, &arr_iter);
      goto oom;
    }

  /* Connections */

  if (!_dbus_asv_add_uint32 (&arr_iter, "ActiveConnections",
        bus_connections_get_n_active (connections)) ||
      !_dbus_asv_add_uint32 (&arr_iter, "IncompleteConnections",
        bus_connections_get_n_incomplete (connections)) ||
      !_dbus_asv_add_uint32 (&arr_iter, "MatchRules",
        bus_connections_get_total_match_rules (connections)) ||
      !_dbus_asv_add_uint32 (&arr_iter, "PeakMatchRules",
        bus_connections_get_peak_match_rules (connections)) ||
      !_dbus_asv_add_uint32 (&arr_iter, "PeakMatchRulesPerConnection",
        bus_connections_get_peak_match_rules_per_conn (connections)) ||
      !_dbus_asv_add_uint32 (&arr_iter, "BusNames",
        bus_connections_get_total_bus_names (connections)) ||
      !_dbus_asv_add_uint32 (&arr_iter, "PeakBusNames",
        bus_connections_get_peak_bus_names (connections)) ||
      !_dbus_asv_add_uint32 (&arr_iter, "PeakBusNamesPerConnection",
        bus_connections_get_peak_bus_names_per_conn (connections)))
    {
      _dbus_asv_abandon (&iter, &arr_iter);
      goto oom;
    }

  /* end */

  if (!_dbus_asv_close (&iter, &arr_iter))
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

dbus_bool_t
bus_stats_handle_get_connection_stats (DBusConnection *caller_connection,
                                       BusTransaction *transaction,
                                       DBusMessage    *message,
                                       DBusError      *error)
{
  BusDriverFound found;
  DBusMessage *reply = NULL;
  DBusMessageIter iter, arr_iter;
  static dbus_uint32_t stats_serial = 0;
  dbus_uint32_t in_messages, in_bytes, in_fds, in_peak_bytes, in_peak_fds;
  dbus_uint32_t out_messages, out_bytes, out_fds, out_peak_bytes, out_peak_fds;
  DBusConnection *stats_connection;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  found = bus_driver_get_conn_helper (caller_connection, message,
                                      "statistics", NULL, &stats_connection,
                                      error);

  switch (found)
    {
      case BUS_DRIVER_FOUND_SELF:
        dbus_set_error (error, DBUS_ERROR_INVALID_ARGS,
                        "GetConnectionStats is not meaningful for the "
                        "message bus \"%s\" itself", DBUS_SERVICE_DBUS);
        goto failed;

      case BUS_DRIVER_FOUND_PEER:
        break;

      case BUS_DRIVER_FOUND_ERROR:
        /* fall through */
      default:
        goto failed;
    }

  _dbus_assert (stats_connection != NULL);

  reply = _dbus_asv_new_method_return (message, &iter, &arr_iter);

  if (reply == NULL)
    goto oom;

  /* Bus daemon per-connection stats */

  if (!_dbus_asv_add_uint32 (&arr_iter, "Serial", stats_serial++) ||
      !_dbus_asv_add_uint32 (&arr_iter, "MatchRules",
        bus_connection_get_n_match_rules (stats_connection)) ||
      !_dbus_asv_add_uint32 (&arr_iter, "PeakMatchRules",
        bus_connection_get_peak_match_rules (stats_connection)) ||
      !_dbus_asv_add_uint32 (&arr_iter, "BusNames",
        bus_connection_get_n_services_owned (stats_connection)) ||
      !_dbus_asv_add_uint32 (&arr_iter, "PeakBusNames",
        bus_connection_get_peak_bus_names (stats_connection)) ||
      !_dbus_asv_add_string (&arr_iter, "UniqueName",
        bus_connection_get_name (stats_connection)))
    {
      _dbus_asv_abandon (&iter, &arr_iter);
      goto oom;
    }

  /* DBusConnection per-connection stats */

  _dbus_connection_get_stats (stats_connection,
                              &in_messages, &in_bytes, &in_fds,
                              &in_peak_bytes, &in_peak_fds,
                              &out_messages, &out_bytes, &out_fds,
                              &out_peak_bytes, &out_peak_fds);

  if (!_dbus_asv_add_uint32 (&arr_iter, "IncomingMessages", in_messages) ||
      !_dbus_asv_add_uint32 (&arr_iter, "IncomingBytes", in_bytes) ||
      !_dbus_asv_add_uint32 (&arr_iter, "IncomingFDs", in_fds) ||
      !_dbus_asv_add_uint32 (&arr_iter, "PeakIncomingBytes", in_peak_bytes) ||
      !_dbus_asv_add_uint32 (&arr_iter, "PeakIncomingFDs", in_peak_fds) ||
      !_dbus_asv_add_uint32 (&arr_iter, "OutgoingMessages", out_messages) ||
      !_dbus_asv_add_uint32 (&arr_iter, "OutgoingBytes", out_bytes) ||
      !_dbus_asv_add_uint32 (&arr_iter, "OutgoingFDs", out_fds) ||
      !_dbus_asv_add_uint32 (&arr_iter, "PeakOutgoingBytes", out_peak_bytes) ||
      !_dbus_asv_add_uint32 (&arr_iter, "PeakOutgoingFDs", out_peak_fds))
    {
      _dbus_asv_abandon (&iter, &arr_iter);
      goto oom;
    }

  /* end */

  if (!_dbus_asv_close (&iter, &arr_iter))
    goto oom;

  if (!bus_transaction_send_from_driver (transaction, caller_connection,
                                         reply))
    goto oom;

  dbus_message_unref (reply);
  return TRUE;

oom:
  BUS_SET_OOM (error);
  /* fall through */
failed:
  if (reply != NULL)
    dbus_message_unref (reply);

  return FALSE;
}


dbus_bool_t
bus_stats_handle_get_all_match_rules (DBusConnection *caller_connection,
                                      BusTransaction *transaction,
                                      DBusMessage    *message,
                                      DBusError      *error)
{
  BusContext *context;
  DBusString bus_name_str;
  DBusMessage *reply = NULL;
  DBusMessageIter iter, hash_iter, entry_iter, arr_iter;
  BusRegistry *registry;
  char **services = NULL;
  int services_len;
  DBusConnection *conn_filter = NULL;
  BusMatchmaker *matchmaker;
  int i;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  registry = bus_connection_get_registry (caller_connection);
  context = bus_transaction_get_context (transaction);
  matchmaker = bus_context_get_matchmaker (context);

  if (!bus_registry_list_services (registry, &services, &services_len))
    return FALSE;

  reply = dbus_message_new_method_return (message);
  if (reply == NULL)
    goto oom;

  dbus_message_iter_init_append (reply, &iter);

  if (!dbus_message_iter_open_container (&iter, DBUS_TYPE_ARRAY, "{sas}",
                                         &hash_iter))
    goto oom;

  for (i = 0 ; i < services_len ; i++)
    {
      BusService *service;

      /* To avoid duplicate entries, only look for unique names */
      if (services[i][0] != ':')
        continue;

      _dbus_string_init_const (&bus_name_str, services[i]);
      service = bus_registry_lookup (registry, &bus_name_str);
      _dbus_assert (service != NULL);

      conn_filter = bus_service_get_primary_owners_connection (service);
      _dbus_assert (conn_filter != NULL);

      if (!dbus_message_iter_open_container (&hash_iter, DBUS_TYPE_DICT_ENTRY, NULL,
                                             &entry_iter))
        {
          dbus_message_iter_abandon_container (&iter, &hash_iter);
          goto oom;
        }

      if (!dbus_message_iter_append_basic (&entry_iter, DBUS_TYPE_STRING, &services[i]))
        {
          dbus_message_iter_abandon_container (&hash_iter, &entry_iter);
          dbus_message_iter_abandon_container (&iter, &hash_iter);
          goto oom;
        }

      if (!dbus_message_iter_open_container (&entry_iter, DBUS_TYPE_ARRAY, "s",
                                             &arr_iter))
        {
          dbus_message_iter_abandon_container (&hash_iter, &entry_iter);
          dbus_message_iter_abandon_container (&iter, &hash_iter);
          goto oom;
        }

      if (!bus_match_rule_dump (matchmaker, conn_filter, &arr_iter))
        {
          dbus_message_iter_abandon_container (&entry_iter, &arr_iter);
          dbus_message_iter_abandon_container (&hash_iter, &entry_iter);
          dbus_message_iter_abandon_container (&iter, &hash_iter);
          goto oom;
        }

      if (!dbus_message_iter_close_container (&entry_iter, &arr_iter))
        {
          dbus_message_iter_abandon_container (&hash_iter, &entry_iter);
          dbus_message_iter_abandon_container (&iter, &hash_iter);
          goto oom;
        }
      if (!dbus_message_iter_close_container (&hash_iter, &entry_iter))
        {
          dbus_message_iter_abandon_container (&iter, &hash_iter);
          goto oom;
        }
    }

  if (!dbus_message_iter_close_container (&iter, &hash_iter))
    goto oom;

  if (!bus_transaction_send_from_driver (transaction, caller_connection,
                                         reply))
    goto oom;

  dbus_message_unref (reply);
  dbus_free_string_array (services);
  return TRUE;

oom:
  if (reply != NULL)
    dbus_message_unref (reply);

  dbus_free_string_array (services);

  BUS_SET_OOM (error);
  return FALSE;
}

#endif
