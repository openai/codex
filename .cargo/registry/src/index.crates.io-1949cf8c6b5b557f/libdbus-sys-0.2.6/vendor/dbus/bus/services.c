/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* services.c  Service management
 *
 * Copyright (C) 2003  Red Hat, Inc.
 * Copyright (C) 2003  CodeFactory AB
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
#include <dbus/dbus-hash.h>
#include <dbus/dbus-list.h>
#include <dbus/dbus-mempool.h>
#include <dbus/dbus-marshal-validate.h>

#include "driver.h"
#include "services.h"
#include "connection.h"
#include "utils.h"
#include "activation.h"
#include "policy.h"
#include "bus.h"
#include "selinux.h"
#include "apparmor.h"

struct BusService
{
  int refcount;

  BusRegistry *registry;
  char *name;
  DBusList *owners;
};

struct BusOwner
{
  int refcount;

  BusService *service;
  DBusConnection *conn;

  unsigned int allow_replacement : 1;
  unsigned int do_not_queue : 1;
};

struct BusRegistry
{
  int refcount;

  BusContext *context;
  
  DBusHashTable *service_hash;
  DBusMemPool   *service_pool;
  DBusMemPool   *owner_pool;

  DBusHashTable *service_sid_table;
};

BusRegistry*
bus_registry_new (BusContext *context)
{
  BusRegistry *registry;

  _dbus_assert (context);
  registry = dbus_new0 (BusRegistry, 1);
  if (registry == NULL)
    return NULL;

  registry->refcount = 1;
  registry->context = context;
  
  registry->service_hash = _dbus_hash_table_new (DBUS_HASH_STRING,
                                                 NULL, NULL);
  if (registry->service_hash == NULL)
    goto failed;
  
  registry->service_pool = _dbus_mem_pool_new (sizeof (BusService),
                                               TRUE);

  if (registry->service_pool == NULL)
    goto failed;

  registry->owner_pool = _dbus_mem_pool_new (sizeof (BusOwner),
                                             TRUE);

  if (registry->owner_pool == NULL)
    goto failed;

  registry->service_sid_table = NULL;
  
  return registry;

 failed:
  bus_registry_unref (registry);
  return NULL;
}

BusRegistry *
bus_registry_ref (BusRegistry *registry)
{
  _dbus_assert (registry->refcount > 0);
  registry->refcount += 1;

  return registry;
}

void
bus_registry_unref  (BusRegistry *registry)
{
  _dbus_assert (registry->refcount > 0);
  registry->refcount -= 1;

  if (registry->refcount == 0)
    {
      if (registry->service_hash)
        _dbus_hash_table_unref (registry->service_hash);
      if (registry->service_pool)
        _dbus_mem_pool_free (registry->service_pool);
      if (registry->owner_pool)
        _dbus_mem_pool_free (registry->owner_pool);
      if (registry->service_sid_table)
        _dbus_hash_table_unref (registry->service_sid_table);
      
      dbus_free (registry);
    }
}

BusService*
bus_registry_lookup (BusRegistry      *registry,
                     const DBusString *service_name)
{
  BusService *service;

  service = _dbus_hash_table_lookup_string (registry->service_hash,
                                            _dbus_string_get_const_data (service_name));

  return service;
}

static DBusList *
_bus_service_find_owner_link (BusService *service,
                              DBusConnection *connection)
{
  DBusList *link;
  
  link = _dbus_list_get_first_link (&service->owners);

  while (link != NULL)
    {
      BusOwner *bus_owner;

      bus_owner = (BusOwner *) link->data;
      if (bus_owner->conn == connection) 
        break;

      link = _dbus_list_get_next_link (&service->owners, link);
    }

  return link;
}

static void
bus_owner_set_flags (BusOwner *owner,
                     dbus_uint32_t flags)
{
   owner->allow_replacement = 
        (flags & DBUS_NAME_FLAG_ALLOW_REPLACEMENT) != FALSE;

   owner->do_not_queue =
        (flags & DBUS_NAME_FLAG_DO_NOT_QUEUE) != FALSE;
}

static BusOwner *
bus_owner_new (BusService *service, 
               DBusConnection *conn, 
	       dbus_uint32_t flags)
{
  BusOwner *result;

  result = _dbus_mem_pool_alloc (service->registry->owner_pool);
  if (result != NULL)
    {
      result->refcount = 1;
      /* don't ref the connection because we don't want
         to block the connection from going away.
         transactions take care of reffing the connection
         but we need to use refcounting on the owner
         so that the owner does not get freed before
         we can deref the connection in the transaction
       */
      result->conn = conn;
      result->service = service;

      if (!bus_connection_add_owned_service (conn, service))
        {
          _dbus_mem_pool_dealloc (service->registry->owner_pool, result);
          return NULL;
        }
        
      bus_owner_set_flags (result, flags);
    }
  return result;
}

static BusOwner *
bus_owner_ref (BusOwner *owner)
{
  _dbus_assert (owner->refcount > 0);
  owner->refcount += 1;

  return owner;
}

static void
bus_owner_unref  (BusOwner *owner)
{
  _dbus_assert (owner->refcount > 0);
  owner->refcount -= 1;

  if (owner->refcount == 0)
    {
      bus_connection_remove_owned_service (owner->conn, owner->service);
      _dbus_mem_pool_dealloc (owner->service->registry->owner_pool, owner);
    }
}

BusService*
bus_registry_ensure (BusRegistry               *registry,
                     const DBusString          *service_name,
                     DBusConnection            *owner_connection_if_created,
                     dbus_uint32_t              flags,
                     BusTransaction            *transaction,
                     DBusError                 *error)
{
  BusService *service;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  
  _dbus_assert (owner_connection_if_created != NULL);
  _dbus_assert (transaction != NULL);

  service = _dbus_hash_table_lookup_string (registry->service_hash,
                                            _dbus_string_get_const_data (service_name));
  if (service != NULL)
    return service;
  
  service = _dbus_mem_pool_alloc (registry->service_pool);
  if (service == NULL)
    {
      BUS_SET_OOM (error);
      return NULL;
    }

  service->registry = registry;  
  service->refcount = 1;

  _dbus_verbose ("copying string %p '%s' to service->name\n",
                 service_name, _dbus_string_get_const_data (service_name));
  if (!_dbus_string_copy_data (service_name, &service->name))
    {
      _dbus_mem_pool_dealloc (registry->service_pool, service);
      BUS_SET_OOM (error);
      return NULL;
    }
  _dbus_verbose ("copied string %p '%s' to '%s'\n",
                 service_name, _dbus_string_get_const_data (service_name),
                 service->name);

  if (!bus_driver_send_service_owner_changed (service->name, 
					      NULL,
					      bus_connection_get_name (owner_connection_if_created),
					      transaction, error))
    {
      bus_service_unref (service);
      return NULL;
    }

  if (!bus_activation_service_created (bus_context_get_activation (registry->context),
				       service->name, transaction, error))
    {
      bus_service_unref (service);
      return NULL;
    }
  
  if (!bus_service_add_owner (service, owner_connection_if_created, flags,
                                              transaction, error))
    {
      bus_service_unref (service);
      return NULL;
    }
  
  if (!_dbus_hash_table_insert_string (registry->service_hash,
                                       service->name,
                                       service))
    {
      /* The add_owner gets reverted on transaction cancel */
      BUS_SET_OOM (error);
      return NULL;
    }
  
  return service;
}

void
bus_registry_foreach (BusRegistry               *registry,
                      BusServiceForeachFunction  function,
                      void                      *data)
{
  DBusHashIter iter;
  
  _dbus_hash_iter_init (registry->service_hash, &iter);
  while (_dbus_hash_iter_next (&iter))
    {
      BusService *service = _dbus_hash_iter_get_value (&iter);

      (* function) (service, data);
    }
}

dbus_bool_t
bus_registry_list_services (BusRegistry *registry,
                            char      ***listp,
                            int         *array_len)
{
  int i, j, len;
  char **retval;
  DBusHashIter iter;
   
  len = _dbus_hash_table_get_n_entries (registry->service_hash);
  retval = dbus_new (char *, len + 1);

  if (retval == NULL)
    return FALSE;

  _dbus_hash_iter_init (registry->service_hash, &iter);
  i = 0;
  while (_dbus_hash_iter_next (&iter))
    {
      BusService *service = _dbus_hash_iter_get_value (&iter);

      retval[i] = _dbus_strdup (service->name);
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
bus_registry_acquire_service (BusRegistry      *registry,
                              DBusConnection   *connection,
                              const DBusString *service_name,
                              dbus_uint32_t     flags,
                              dbus_uint32_t    *result,
                              BusTransaction   *transaction,
                              DBusError        *error)
{
  dbus_bool_t retval;
  DBusConnection *old_owner_conn;
  BusClientPolicy *policy;
  BusService *service;
  BusActivation  *activation;
  BusSELinuxID *sid;
  BusOwner *primary_owner;
  int limit;

  retval = FALSE;

  if (!_dbus_validate_bus_name (service_name, 0,
                                _dbus_string_get_length (service_name)))
    {
      dbus_set_error (error, DBUS_ERROR_INVALID_ARGS,
                      "Requested bus name \"%s\" is not valid",
                      _dbus_string_get_const_data (service_name));
      
      _dbus_verbose ("Attempt to acquire invalid service name\n");
      
      goto out;
    }
  
  if (_dbus_string_get_byte (service_name, 0) == ':')
    {
      /* Not allowed; only base services can start with ':' */
      dbus_set_error (error, DBUS_ERROR_INVALID_ARGS,
                      "Cannot acquire a service starting with ':' such as \"%s\"",
                      _dbus_string_get_const_data (service_name));
      
      _dbus_verbose ("Attempt to acquire invalid base service name \"%s\"",
                     _dbus_string_get_const_data (service_name));
      
      goto out;
    }

  if (_dbus_string_equal_c_str (service_name, DBUS_SERVICE_DBUS))
    {
      dbus_set_error (error, DBUS_ERROR_INVALID_ARGS,
                      "Connection \"%s\" is not allowed to own the service \"%s\"because "
                      "it is reserved for D-Bus' use only",
                      bus_connection_is_active (connection) ?
                      bus_connection_get_name (connection) :
                      "(inactive)",
                      DBUS_SERVICE_DBUS);
      goto out;
    }

  policy = bus_connection_get_policy (connection);
  _dbus_assert (policy != NULL);

  /* Note that if sid is #NULL then the bus's own context gets used
   * in bus_connection_selinux_allows_acquire_service()
   */
  sid = bus_selinux_id_table_lookup (registry->service_sid_table,
                                     service_name);

  if (!bus_selinux_allows_acquire_service (connection, sid,
					   _dbus_string_get_const_data (service_name), error))
    {

      if (dbus_error_is_set (error) &&
	  dbus_error_has_name (error, DBUS_ERROR_NO_MEMORY))
	{
	  goto out;
	}

      dbus_set_error (error, DBUS_ERROR_ACCESS_DENIED,
                      "Connection \"%s\" is not allowed to own the service \"%s\" due "
                      "to SELinux policy",
                      bus_connection_is_active (connection) ?
                      bus_connection_get_name (connection) :
                      "(inactive)",
                      _dbus_string_get_const_data (service_name));
      goto out;
    }

  if (!bus_apparmor_allows_acquire_service (connection,
                                            bus_context_get_type (registry->context),
                                            _dbus_string_get_const_data (service_name), error))
    goto out;
  
  if (!bus_client_policy_check_can_own (policy, service_name))
    {
      dbus_set_error (error, DBUS_ERROR_ACCESS_DENIED,
                      "Connection \"%s\" is not allowed to own the service \"%s\" due "
                      "to security policies in the configuration file",
                      bus_connection_is_active (connection) ?
                      bus_connection_get_name (connection) :
                      "(inactive)",
                      _dbus_string_get_const_data (service_name));
      goto out;
    }

  limit = bus_context_get_max_services_per_connection (registry->context);

  if (bus_connection_get_n_services_owned (connection) >= limit)
    {
      DBusError tmp_error;

      dbus_error_init (&tmp_error);
      dbus_set_error (&tmp_error, DBUS_ERROR_LIMITS_EXCEEDED,
                      "Connection \"%s\" is not allowed to own more services "
                      "(increase limits in configuration file if required; "
                      "max_names_per_connection=%d)",
                      bus_connection_is_active (connection) ?
                      bus_connection_get_name (connection) :
                      "(inactive)",
                      limit);
      bus_context_log (registry->context, DBUS_SYSTEM_LOG_WARNING,
          "%s", tmp_error.message);
      dbus_move_error (&tmp_error, error);
      goto out;
    }
  
  service = bus_registry_lookup (registry, service_name);

  if (service != NULL)
    {
      primary_owner = bus_service_get_primary_owner (service);
      if (primary_owner != NULL)
        old_owner_conn = primary_owner->conn;
      else
        old_owner_conn = NULL;
    }
  else
    old_owner_conn = NULL;
      
  if (service == NULL)
    {
      service = bus_registry_ensure (registry,
                                     service_name, connection, flags,
                                     transaction, error);
      if (service == NULL)
        goto out;
    }

  primary_owner = bus_service_get_primary_owner (service);
  if (primary_owner == NULL)
    goto out;

  if (old_owner_conn == NULL)
    {
      _dbus_assert (primary_owner->conn == connection);

      *result = DBUS_REQUEST_NAME_REPLY_PRIMARY_OWNER;      
    }
  else if (old_owner_conn == connection)
    {
      bus_owner_set_flags (primary_owner, flags);
      *result = DBUS_REQUEST_NAME_REPLY_ALREADY_OWNER;
    }
  else if (((flags & DBUS_NAME_FLAG_DO_NOT_QUEUE) &&
           !(bus_service_get_allow_replacement (service))) ||
	   ((flags & DBUS_NAME_FLAG_DO_NOT_QUEUE) &&
           !(flags & DBUS_NAME_FLAG_REPLACE_EXISTING))) 
    {
      DBusList *link;
      BusOwner *temp_owner;
    /* Since we can't be queued if we are already in the queue
       remove us */

      link = _bus_service_find_owner_link (service, connection);
      if (link != NULL)
        {
          _dbus_list_unlink (&service->owners, link);
          temp_owner = (BusOwner *)link->data;
          bus_owner_unref (temp_owner); 
          _dbus_list_free_link (link);
        }
      
      *result = DBUS_REQUEST_NAME_REPLY_EXISTS;
    }
  else if (!(flags & DBUS_NAME_FLAG_DO_NOT_QUEUE) &&
           (!(flags & DBUS_NAME_FLAG_REPLACE_EXISTING) ||
	    !(bus_service_get_allow_replacement (service))))
    {
      /* Queue the connection */
      if (!bus_service_add_owner (service, connection, 
                                  flags,
                                  transaction, error))
        goto out;
      
      *result = DBUS_REQUEST_NAME_REPLY_IN_QUEUE;
    }
  else
    {
      /* Replace the current owner */

      /* We enqueue the new owner and remove the first one because
       * that will cause NameAcquired and NameLost messages to
       * be sent.
       */
      
      if (!bus_service_add_owner (service, connection,
                                  flags,
                                  transaction, error))
        goto out;

      if (primary_owner->do_not_queue)
        {
          if (!bus_service_remove_owner (service, old_owner_conn,
                                         transaction, error))
            goto out;
        }
      else
        {
          if (!bus_service_swap_owner (service, old_owner_conn,
                                       transaction, error))
            goto out;
        }
        
    
      _dbus_assert (connection == bus_service_get_primary_owner (service)->conn);
      *result = DBUS_REQUEST_NAME_REPLY_PRIMARY_OWNER;
    }

  activation = bus_context_get_activation (registry->context);
  retval = bus_activation_send_pending_auto_activation_messages (activation,
								 service,
								 transaction);
  if (!retval)
    BUS_SET_OOM (error);
  
 out:
  return retval;
}

dbus_bool_t
bus_registry_release_service (BusRegistry      *registry,
                              DBusConnection   *connection,
                              const DBusString *service_name,
                              dbus_uint32_t    *result,
                              BusTransaction   *transaction,
                              DBusError        *error)
{
  dbus_bool_t retval;
  BusService *service;

  retval = FALSE;

  if (!_dbus_validate_bus_name (service_name, 0,
                                _dbus_string_get_length (service_name)))
    {
      dbus_set_error (error, DBUS_ERROR_INVALID_ARGS,
                      "Given bus name \"%s\" is not valid",
                      _dbus_string_get_const_data (service_name));

      _dbus_verbose ("Attempt to release invalid service name\n");

      goto out;
    }

  if (_dbus_string_get_byte (service_name, 0) == ':')
    {
      /* Not allowed; the base service name cannot be created or released */
      dbus_set_error (error, DBUS_ERROR_INVALID_ARGS,
                      "Cannot release a service starting with ':' such as \"%s\"",
                      _dbus_string_get_const_data (service_name));

      _dbus_verbose ("Attempt to release invalid base service name \"%s\"",
                     _dbus_string_get_const_data (service_name));

      goto out;
    }

   if (_dbus_string_equal_c_str (service_name, DBUS_SERVICE_DBUS))
    {
      /* Not allowed; the base service name cannot be created or released */
      dbus_set_error (error, DBUS_ERROR_INVALID_ARGS,
                      "Cannot release the %s service because it is owned by the bus",
                     DBUS_SERVICE_DBUS);

      _dbus_verbose ("Attempt to release service name \"%s\"",
                     DBUS_SERVICE_DBUS);

      goto out;
    }

  service = bus_registry_lookup (registry, service_name);

  if (service == NULL)
    {
      *result = DBUS_RELEASE_NAME_REPLY_NON_EXISTENT;
    }
  else if (!bus_service_owner_in_queue (service, connection))
    {
      *result = DBUS_RELEASE_NAME_REPLY_NOT_OWNER;
    }
  else
    {
      if (!bus_service_remove_owner (service, connection,
                                     transaction, error))
        goto out;

      _dbus_assert (!bus_service_owner_in_queue (service, connection));
      *result = DBUS_RELEASE_NAME_REPLY_RELEASED;
    }

  retval = TRUE;

 out:
  return retval;
}

dbus_bool_t
bus_registry_set_service_context_table (BusRegistry   *registry,
					DBusHashTable *table)
{
  DBusHashTable *new_table;
  DBusHashIter iter;
  
  new_table = bus_selinux_id_table_new ();
  if (!new_table)
    return FALSE;

  _dbus_hash_iter_init (table, &iter);
  while (_dbus_hash_iter_next (&iter))
    {
      const char *service = _dbus_hash_iter_get_string_key (&iter);
      const char *context = _dbus_hash_iter_get_value (&iter);

      if (!bus_selinux_id_table_insert (new_table,
					service,
					context))
	return FALSE;
    }
  
  if (registry->service_sid_table)
    _dbus_hash_table_unref (registry->service_sid_table);
  registry->service_sid_table = new_table;
  return TRUE;
}

static void
bus_service_unlink_owner (BusService      *service,
                          BusOwner        *owner)
{
  _dbus_list_remove_last (&service->owners, owner);
  bus_owner_unref (owner);
}

static void
bus_service_unlink (BusService *service)
{
  _dbus_assert (service->owners == NULL);

  /* the service may not be in the hash, if
   * the failure causing transaction cancel
   * was in the right place, but that's OK
   */
  _dbus_hash_table_remove_string (service->registry->service_hash,
                                  service->name);
  
  bus_service_unref (service);
}

static void
bus_service_relink (BusService           *service,
                    DBusPreallocatedHash *preallocated)
{
  _dbus_assert (service->owners == NULL);
  _dbus_assert (preallocated != NULL);

  _dbus_hash_table_insert_string_preallocated (service->registry->service_hash,
                                               preallocated,
                                               service->name,
                                               service);
  
  bus_service_ref (service);
}

/**
 * Data used to represent an ownership cancellation in
 * a bus transaction.
 */
typedef struct
{
  BusOwner *owner;            /**< the owner */
  BusService *service;        /**< service to cancel ownership of */
} OwnershipCancelData;

static void
cancel_ownership (void *data)
{
  OwnershipCancelData *d = data;

  /* We don't need to send messages notifying of these
   * changes, since we're reverting something that was
   * cancelled (effectively never really happened)
   */
  bus_service_unlink_owner (d->service, d->owner);
  
  if (d->service->owners == NULL)
    bus_service_unlink (d->service);
}

static void
free_ownership_cancel_data (void *data)
{
  OwnershipCancelData *d = data;

  dbus_connection_unref (d->owner->conn);
  bus_owner_unref (d->owner);
  bus_service_unref (d->service);
  
  dbus_free (d);
}

static dbus_bool_t
add_cancel_ownership_to_transaction (BusTransaction *transaction,
                                     BusService     *service,
                                     BusOwner       *owner)
{
  OwnershipCancelData *d;

  d = dbus_new (OwnershipCancelData, 1);
  if (d == NULL)
    return FALSE;
  
  d->service = service;
  d->owner = owner;

  if (!bus_transaction_add_cancel_hook (transaction, cancel_ownership, d,
                                        free_ownership_cancel_data))
    {
      dbus_free (d);
      return FALSE;
    }

  bus_service_ref (d->service);
  bus_owner_ref (owner);
  dbus_connection_ref (d->owner->conn);
 
  return TRUE;
}

/* this function is self-cancelling if you cancel the transaction */
dbus_bool_t
bus_service_add_owner (BusService     *service,
                       DBusConnection *connection,
                       dbus_uint32_t  flags,
                       BusTransaction *transaction,
                       DBusError      *error)
{
  BusOwner *bus_owner;
  DBusList *bus_owner_link;
  
  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  
 /* Send service acquired message first, OOM will result
  * in cancelling the transaction
  */
  if (service->owners == NULL)
    {
      if (!bus_driver_send_service_acquired (connection, service->name, transaction, error))
        return FALSE;
    }
  
  bus_owner_link = _bus_service_find_owner_link (service, connection);
  
  if (bus_owner_link == NULL)
    {
      bus_owner = bus_owner_new (service, connection, flags);
      if (bus_owner == NULL)
        {
          BUS_SET_OOM (error);
          return FALSE;
        }

      bus_owner_set_flags (bus_owner, flags);
      if (!(flags & DBUS_NAME_FLAG_REPLACE_EXISTING) || service->owners == NULL)
        {
          if (!_dbus_list_append (&service->owners,
                                  bus_owner))
            {
              bus_owner_unref (bus_owner);
              BUS_SET_OOM (error);
              return FALSE;
            }
        }
      else
        {
          if (!_dbus_list_insert_after (&service->owners,
                                         _dbus_list_get_first_link (&service->owners),
                                         bus_owner))
            {
              bus_owner_unref (bus_owner);
              BUS_SET_OOM (error);
              return FALSE;
            }
        }      
    } 
  else 
    {
      /* Update the link since we are already in the queue
       * No need for operations that can produce OOM
       */

      bus_owner = (BusOwner *) bus_owner_link->data;
      if (flags & DBUS_NAME_FLAG_REPLACE_EXISTING)
        {
	  DBusList *link;
          _dbus_list_unlink (&service->owners, bus_owner_link);
	  link = _dbus_list_get_first_link (&service->owners);
	  _dbus_assert (link != NULL);
	  
          _dbus_list_insert_after_link (&service->owners, link, bus_owner_link);
        }
      
      bus_owner_set_flags (bus_owner, flags);
      return TRUE;
    }

  if (!add_cancel_ownership_to_transaction (transaction,
                                            service,
                                            bus_owner))
    {
      bus_service_unlink_owner (service, bus_owner);
      BUS_SET_OOM (error);
      return FALSE;
    }

  return TRUE;
}

typedef struct
{
  BusOwner       *owner;
  BusService     *service;
  BusOwner       *before_owner; /* restore to position before this connection in owners list */
  DBusList       *owner_link;
  DBusList       *service_link;
  DBusPreallocatedHash *hash_entry;
} OwnershipRestoreData;

static void
restore_ownership (void *data)
{
  OwnershipRestoreData *d = data;
  DBusList *link;

  _dbus_assert (d->service_link != NULL);
  _dbus_assert (d->owner_link != NULL);
  
  if (d->service->owners == NULL)
    {
      _dbus_assert (d->hash_entry != NULL);
      bus_service_relink (d->service, d->hash_entry);
    }
  else
    {
      _dbus_assert (d->hash_entry == NULL);
    }
  
  /* We don't need to send messages notifying of these
   * changes, since we're reverting something that was
   * cancelled (effectively never really happened)
   */
  link = _dbus_list_get_first_link (&d->service->owners);
  while (link != NULL)
    {
      if (link->data == d->before_owner)
        break;

      link = _dbus_list_get_next_link (&d->service->owners, link);
    }
  
  _dbus_list_insert_before_link (&d->service->owners, link, d->owner_link);

  /* Note that removing then restoring this changes the order in which
   * ServiceDeleted messages are sent on destruction of the
   * connection.  This should be OK as the only guarantee there is
   * that the base service is destroyed last, and we never even
   * tentatively remove the base service.
   */
  bus_connection_add_owned_service_link (d->owner->conn, d->service_link);
  
  d->hash_entry = NULL;
  d->service_link = NULL;
  d->owner_link = NULL;
}

static void
free_ownership_restore_data (void *data)
{
  OwnershipRestoreData *d = data;

  if (d->service_link)
    _dbus_list_free_link (d->service_link);
  if (d->owner_link)
    _dbus_list_free_link (d->owner_link);
  if (d->hash_entry)
    _dbus_hash_table_free_preallocated_entry (d->service->registry->service_hash,
                                              d->hash_entry);

  dbus_connection_unref (d->owner->conn);
  bus_owner_unref (d->owner);
  bus_service_unref (d->service);
  
  dbus_free (d);
}

static dbus_bool_t
add_restore_ownership_to_transaction (BusTransaction *transaction,
                                      BusService     *service,
                                      BusOwner       *owner)
{
  OwnershipRestoreData *d;
  DBusList *link;

  d = dbus_new (OwnershipRestoreData, 1);
  if (d == NULL)
    return FALSE;
  
  d->service = service;
  d->owner = owner;
  d->service_link = _dbus_list_alloc_link (service);
  d->owner_link = _dbus_list_alloc_link (owner);
  d->hash_entry = _dbus_hash_table_preallocate_entry (service->registry->service_hash);
  
  bus_service_ref (d->service);
  bus_owner_ref (d->owner);
  dbus_connection_ref (d->owner->conn);

  d->before_owner = NULL;
  link = _dbus_list_get_first_link (&service->owners);
  while (link != NULL)
    {
      if (link->data == owner)
        {
          link = _dbus_list_get_next_link (&service->owners, link);

          if (link)
            d->before_owner = link->data;

          break;
        }
      
      link = _dbus_list_get_next_link (&service->owners, link);
    }
  
  if (d->service_link == NULL ||
      d->owner_link == NULL ||
      d->hash_entry == NULL ||
      !bus_transaction_add_cancel_hook (transaction, restore_ownership, d,
                                        free_ownership_restore_data))
    {
      free_ownership_restore_data (d);
      return FALSE;
    }
  
  return TRUE;
}

dbus_bool_t
bus_service_swap_owner (BusService     *service,
                        DBusConnection *connection,
                        BusTransaction *transaction,
                        DBusError      *error)
{
  DBusList *swap_link;
  BusOwner *primary_owner;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  /* We send out notifications before we do any work we
   * might have to undo if the notification-sending failed
   */
  
  /* Send service lost message */
  primary_owner = bus_service_get_primary_owner (service);
  if (primary_owner == NULL || primary_owner->conn != connection)
    _dbus_assert_not_reached ("Tried to swap a non primary owner");

    
  if (!bus_driver_send_service_lost (connection, service->name,
                                     transaction, error))
    return FALSE;

  if (service->owners == NULL)
    {
      _dbus_assert_not_reached ("Tried to swap owner of a service that has no owners");
    }
  else if (_dbus_list_length_is_one (&service->owners))
    {
      _dbus_assert_not_reached ("Tried to swap owner of a service that has no other owners in the queue");
    }
  else
    {
      DBusList *link;
      BusOwner *new_owner;
      DBusConnection *new_owner_conn;
      link = _dbus_list_get_first_link (&service->owners);
      _dbus_assert (link != NULL);
      link = _dbus_list_get_next_link (&service->owners, link);
      _dbus_assert (link != NULL);

      new_owner = (BusOwner *)link->data;
      new_owner_conn = new_owner->conn;

      if (!bus_driver_send_service_owner_changed (service->name,
 						  bus_connection_get_name (connection),
 						  bus_connection_get_name (new_owner_conn),
 						  transaction, error))
        return FALSE;

      /* This will be our new owner */
      if (!bus_driver_send_service_acquired (new_owner_conn,
                                             service->name,
                                             transaction,
                                             error))
        return FALSE;
    }

  if (!add_restore_ownership_to_transaction (transaction, service, primary_owner))
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  /* unlink the primary and make it the second link */
  swap_link = _dbus_list_get_first_link (&service->owners);
  _dbus_list_unlink (&service->owners, swap_link);

  _dbus_list_insert_after_link (&service->owners,
                                _dbus_list_get_first_link (&service->owners),
				swap_link);

  return TRUE;
}

/* this function is self-cancelling if you cancel the transaction */
dbus_bool_t
bus_service_remove_owner (BusService     *service,
                          DBusConnection *connection,
                          BusTransaction *transaction,
                          DBusError      *error)
{
  BusOwner *primary_owner;
  
  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  
  /* We send out notifications before we do any work we
   * might have to undo if the notification-sending failed
   */
  
  /* Send service lost message */
  primary_owner = bus_service_get_primary_owner (service);
  if (primary_owner != NULL && primary_owner->conn == connection)
    {
      if (!bus_driver_send_service_lost (connection, service->name,
                                         transaction, error))
        return FALSE;
    }
  else
    {
      /* if we are not the primary owner then just remove us from the queue */
      DBusList *link;
      BusOwner *temp_owner;

      link = _bus_service_find_owner_link (service, connection);
      _dbus_list_unlink (&service->owners, link);
      temp_owner = (BusOwner *)link->data;
      bus_owner_unref (temp_owner); 
      _dbus_list_free_link (link);

      return TRUE; 
    }

  if (service->owners == NULL)
    {
      _dbus_assert_not_reached ("Tried to remove owner of a service that has no owners");
    }
  else if (_dbus_list_length_is_one (&service->owners))
    {
      if (!bus_driver_send_service_owner_changed (service->name,
 						  bus_connection_get_name (connection),
 						  NULL,
 						  transaction, error))
        return FALSE;
    }
  else
    {
      DBusList *link;
      BusOwner *new_owner;
      DBusConnection *new_owner_conn;
      link = _dbus_list_get_first_link (&service->owners);
      _dbus_assert (link != NULL);
      link = _dbus_list_get_next_link (&service->owners, link);
      _dbus_assert (link != NULL);

      new_owner = (BusOwner *)link->data;
      new_owner_conn = new_owner->conn;

      if (!bus_driver_send_service_owner_changed (service->name,
 						  bus_connection_get_name (connection),
 						  bus_connection_get_name (new_owner_conn),
 						  transaction, error))
        return FALSE;

      /* This will be our new owner */
      if (!bus_driver_send_service_acquired (new_owner_conn,
                                             service->name,
                                             transaction,
                                             error))
        return FALSE;
    }

  if (!add_restore_ownership_to_transaction (transaction, service, primary_owner))
    {
      BUS_SET_OOM (error);
      return FALSE;
    }
 
  bus_service_unlink_owner (service, primary_owner);

  if (service->owners == NULL)
    bus_service_unlink (service);

  return TRUE;
}

BusService *
bus_service_ref (BusService *service)
{
  _dbus_assert (service->refcount > 0);
  
  service->refcount += 1;

  return service;
}

void
bus_service_unref (BusService *service)
{
  _dbus_assert (service->refcount > 0);
  
  service->refcount -= 1;

  if (service->refcount == 0)
    {
      _dbus_assert (service->owners == NULL);
      
      dbus_free (service->name);
      _dbus_mem_pool_dealloc (service->registry->service_pool, service);
    }
}

DBusConnection *
bus_service_get_primary_owners_connection (BusService *service)
{
  BusOwner *owner;

  owner = bus_service_get_primary_owner (service);

  if (owner != NULL)
    return owner->conn;
  else
    return NULL;
}

BusOwner*
bus_service_get_primary_owner (BusService *service)
{
  return _dbus_list_get_first (&service->owners);
}

const char*
bus_service_get_name (BusService *service)
{
  return service->name;
}

dbus_bool_t
bus_service_get_allow_replacement (BusService *service)
{
  BusOwner *owner;
  DBusList *link;
 
  _dbus_assert (service->owners != NULL);

  link = _dbus_list_get_first_link (&service->owners);
  owner = (BusOwner *) link->data;

  return owner->allow_replacement;
}

dbus_bool_t
bus_service_owner_in_queue (BusService     *service,
                            DBusConnection *connection)
{
  DBusList *link;

  link = _bus_service_find_owner_link (service, connection);
 
  if (link == NULL)
    return FALSE;
  else
    return TRUE;
}

dbus_bool_t 
bus_service_list_queued_owners (BusService *service,
                                DBusList  **return_list)
{
  DBusList *link;

  _dbus_assert (*return_list == NULL);

  link = _dbus_list_get_first_link (&service->owners);
  _dbus_assert (link != NULL);
  
  while (link != NULL)
    {
      BusOwner *owner;
      const char *uname;

      owner = (BusOwner *) link->data;
      uname = bus_connection_get_name (owner->conn);

      if (!_dbus_list_append (return_list, (char *)uname))
        goto oom;

      link = _dbus_list_get_next_link (&service->owners, link);
    }
  
  return TRUE;
  
 oom:
  _dbus_list_clear (return_list);
  return FALSE;
}
