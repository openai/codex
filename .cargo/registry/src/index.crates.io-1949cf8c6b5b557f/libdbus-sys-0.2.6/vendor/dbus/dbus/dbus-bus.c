/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-bus.c  Convenience functions for communicating with the bus.
 *
 * Copyright (C) 2003  CodeFactory AB
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
#include "dbus-bus.h"
#include "dbus-protocol.h"
#include "dbus-internals.h"
#include "dbus-message.h"
#include "dbus-marshal-validate.h"
#include "dbus-misc.h"
#include "dbus-threads-internal.h"
#include "dbus-connection-internal.h"
#include "dbus-string.h"

/**
 * @defgroup DBusBus Message bus APIs
 * @ingroup DBus
 * @brief Functions for communicating with the message bus
 *
 * dbus_bus_get() allows all modules and libraries in a given
 * process to share the same connection to the bus daemon by storing
 * the connection globally.
 *
 * All other functions in this module are just convenience functions;
 * most of them invoke methods on the bus daemon, by sending method
 * call messages to #DBUS_SERVICE_DBUS. These convenience functions
 * often make blocking method calls. If you don't want to block,
 * you can send the method call messages manually in the same way
 * you would any other method call message.
 *
 * This module is the only one in libdbus that's specific to
 * communicating with the message bus daemon. The rest of the API can
 * also be used for connecting to another application directly.
 * 
 * @todo right now the default address of the system bus is hardcoded,
 * so if you change it in the global config file suddenly you have to
 * set DBUS_SYSTEM_BUS_ADDRESS env variable.  Might be nice if the
 * client lib somehow read the config file, or if the bus on startup
 * somehow wrote out its address to a well-known spot, but might also
 * not be worth it.
 */

/**
 * @defgroup DBusBusInternals Message bus APIs internals
 * @ingroup DBusInternals
 * @brief Internals of functions for communicating with the message bus
 *
 * @{
 */

/**
 * Block of message-bus-related data we attach to each
 * #DBusConnection used with these convenience functions.
 *
 */
typedef struct
{
  DBusConnection *connection; /**< Connection we're associated with */
  char *unique_name; /**< Unique name of this connection */

  unsigned int is_well_known : 1; /**< Is one of the well-known connections in our global array */
} BusData;

/** The slot we have reserved to store BusData.
 * Protected by _DBUS_LOCK_connection_slots.
 */
static dbus_int32_t bus_data_slot = -1;

/** Number of bus types */
#define N_BUS_TYPES 3

/* Protected by _DBUS_LOCK_bus, except during shutdown, which can't safely
 * be done in a threaded application anyway. */
static DBusConnection *bus_connections[N_BUS_TYPES];
static char *bus_connection_addresses[N_BUS_TYPES] = { NULL, NULL, NULL };
static DBusBusType activation_bus_type = DBUS_BUS_STARTER;
static dbus_bool_t initialized = FALSE;

static void
addresses_shutdown_func (void *data)
{
  int i;

  i = 0;
  while (i < N_BUS_TYPES)
    {
      if (bus_connections[i] != NULL)
        _dbus_warn_check_failed ("dbus_shutdown() called but connections were still live. This probably means the application did not drop all its references to bus connections.");
      
      dbus_free (bus_connection_addresses[i]);
      bus_connection_addresses[i] = NULL;
      ++i;
    }

  activation_bus_type = DBUS_BUS_STARTER;

  initialized = FALSE;
}

static dbus_bool_t
get_from_env (char           **connection_p,
              const char      *env_var)
{
  const char *s;
  
  _dbus_assert (*connection_p == NULL);
  
  s = _dbus_getenv (env_var);
  if (s == NULL || *s == '\0')
    return TRUE; /* successfully didn't use the env var */
  else
    {
      *connection_p = _dbus_strdup (s);
      return *connection_p != NULL;
    }
}

static dbus_bool_t
init_session_address (void)
{
  dbus_bool_t retval;
 
  retval = FALSE;

  /* First, look in the environment.  This is the normal case on 
   * freedesktop.org/Unix systems. */
  get_from_env (&bus_connection_addresses[DBUS_BUS_SESSION],
                     "DBUS_SESSION_BUS_ADDRESS");
  if (bus_connection_addresses[DBUS_BUS_SESSION] == NULL)
    {
      dbus_bool_t supported;
      DBusString addr;
      DBusError error = DBUS_ERROR_INIT;

      if (!_dbus_string_init (&addr))
        return FALSE;

      supported = FALSE;
      /* So it's not in the environment - let's try a platform-specific method.
       * On MacOS, this involves asking launchd.  On Windows (not specified yet)
       * we might do a COM lookup.
       * Ignore errors - if we failed, fall back to autolaunch. */
      retval = _dbus_lookup_session_address (&supported, &addr, &error);
      if (supported && retval)
        {
          retval =_dbus_string_steal_data (&addr, &bus_connection_addresses[DBUS_BUS_SESSION]);
        }
      else if (supported && !retval)
        {
          if (dbus_error_is_set(&error))
            _dbus_warn ("Dynamic session lookup supported but failed: %s", error.message);
          else
            _dbus_warn ("Dynamic session lookup supported but failed silently");
        }
      _dbus_string_free (&addr);
    }
  else
    retval = TRUE;

  if (!retval)
    return FALSE;

  /* We have a hard-coded (but compile-time-configurable) fallback address for
   * the session bus. */
  if (bus_connection_addresses[DBUS_BUS_SESSION] == NULL)
    bus_connection_addresses[DBUS_BUS_SESSION] =
      _dbus_strdup (DBUS_SESSION_BUS_CONNECT_ADDRESS);

  if (bus_connection_addresses[DBUS_BUS_SESSION] == NULL)
    return FALSE;

  return TRUE;
}

static dbus_bool_t
init_connections_unlocked (void)
{
  if (!initialized)
    {
      const char *s;
      int i;

      i = 0;
      while (i < N_BUS_TYPES)
        {
          bus_connections[i] = NULL;
          ++i;
        }

      /* Don't init these twice, we may run this code twice if
       * init_connections_unlocked() fails midway through.
       * In practice, each block below should contain only one
       * "return FALSE" or running through twice may not
       * work right.
       */
      
       if (bus_connection_addresses[DBUS_BUS_SYSTEM] == NULL)
         {
           _dbus_verbose ("Filling in system bus address...\n");
           
           if (!get_from_env (&bus_connection_addresses[DBUS_BUS_SYSTEM],
                              "DBUS_SYSTEM_BUS_ADDRESS"))
             return FALSE;
         }

                  
       if (bus_connection_addresses[DBUS_BUS_SYSTEM] == NULL)
         {
           /* Use default system bus address if none set in environment */
           bus_connection_addresses[DBUS_BUS_SYSTEM] =
             _dbus_strdup (DBUS_SYSTEM_BUS_DEFAULT_ADDRESS);

           if (bus_connection_addresses[DBUS_BUS_SYSTEM] == NULL)
             return FALSE;
           
           _dbus_verbose ("  used default system bus \"%s\"\n",
                          bus_connection_addresses[DBUS_BUS_SYSTEM]);
         }
       else
         _dbus_verbose ("  used env var system bus \"%s\"\n",
                        bus_connection_addresses[DBUS_BUS_SYSTEM]);
          
      if (bus_connection_addresses[DBUS_BUS_SESSION] == NULL)
        {
          _dbus_verbose ("Filling in session bus address...\n");
          
          if (!init_session_address ())
            return FALSE;

          _dbus_verbose ("  \"%s\"\n", bus_connection_addresses[DBUS_BUS_SESSION] ?
                         bus_connection_addresses[DBUS_BUS_SESSION] : "none set");
        }

      if (bus_connection_addresses[DBUS_BUS_STARTER] == NULL)
        {
          _dbus_verbose ("Filling in activation bus address...\n");
          
          if (!get_from_env (&bus_connection_addresses[DBUS_BUS_STARTER],
                             "DBUS_STARTER_ADDRESS"))
            return FALSE;
          
          _dbus_verbose ("  \"%s\"\n", bus_connection_addresses[DBUS_BUS_STARTER] ?
                         bus_connection_addresses[DBUS_BUS_STARTER] : "none set");
        }


      if (bus_connection_addresses[DBUS_BUS_STARTER] != NULL)
        {
          s = _dbus_getenv ("DBUS_STARTER_BUS_TYPE");
              
          if (s != NULL)
            {
              _dbus_verbose ("Bus activation type was set to \"%s\"\n", s);
                  
              if (strcmp (s, "system") == 0)
                activation_bus_type = DBUS_BUS_SYSTEM;
              else if (strcmp (s, "session") == 0)
                activation_bus_type = DBUS_BUS_SESSION;
            }
        }
      else
        {
          /* Default to the session bus instead if available */
          if (bus_connection_addresses[DBUS_BUS_SESSION] != NULL)
            {
              bus_connection_addresses[DBUS_BUS_STARTER] =
                _dbus_strdup (bus_connection_addresses[DBUS_BUS_SESSION]);
              if (bus_connection_addresses[DBUS_BUS_STARTER] == NULL)
                return FALSE;
            }
        }
      
      /* If we return FALSE we have to be sure that restarting
       * the above code will work right
       */
      
      if (!_dbus_register_shutdown_func (addresses_shutdown_func,
                                         NULL))
        return FALSE;
      
      initialized = TRUE;
    }

  return initialized;
}

static void
bus_data_free (void *data)
{
  BusData *bd = data;
  
  if (bd->is_well_known)
    {
      int i;

      if (!_DBUS_LOCK (bus))
        _dbus_assert_not_reached ("global locks should have been initialized "
            "when we attached bus data");

      /* We may be stored in more than one slot */
      /* This should now be impossible - these slots are supposed to
       * be cleared on disconnect, so should not need to be cleared on
       * finalize
       */
      i = 0;
      while (i < N_BUS_TYPES)
        {
          if (bus_connections[i] == bd->connection)
            bus_connections[i] = NULL;
          
          ++i;
        }
      _DBUS_UNLOCK (bus);
    }
  
  dbus_free (bd->unique_name);
  dbus_free (bd);

  dbus_connection_free_data_slot (&bus_data_slot);
}

static BusData*
ensure_bus_data (DBusConnection *connection)
{
  BusData *bd;

  if (!dbus_connection_allocate_data_slot (&bus_data_slot))
    return NULL;

  bd = dbus_connection_get_data (connection, bus_data_slot);
  if (bd == NULL)
    {      
      bd = dbus_new0 (BusData, 1);
      if (bd == NULL)
        {
          dbus_connection_free_data_slot (&bus_data_slot);
          return NULL;
        }

      bd->connection = connection;
      
      if (!dbus_connection_set_data (connection, bus_data_slot, bd,
                                     bus_data_free))
        {
          dbus_free (bd);
          dbus_connection_free_data_slot (&bus_data_slot);
          return NULL;
        }

      /* Data slot refcount now held by the BusData */
    }
  else
    {
      dbus_connection_free_data_slot (&bus_data_slot);
    }

  return bd;
}

/**
 * Internal function that checks to see if this
 * is a shared connection owned by the bus and if it is unref it.
 *
 * @param connection a connection that has been disconnected.
 */
void
_dbus_bus_notify_shared_connection_disconnected_unlocked (DBusConnection *connection)
{
  int i;

  if (!_DBUS_LOCK (bus))
    {
      /* If it was in bus_connections, we would have initialized global locks
       * when we added it. So, it can't be. */
      return;
    }

  /* We are expecting to have the connection saved in only one of these
   * slots, but someone could in a pathological case set system and session
   * bus to the same bus or something. Or set one of them to the starter
   * bus without setting the starter bus type in the env variable.
   * So we don't break the loop as soon as we find a match.
   */
  for (i = 0; i < N_BUS_TYPES; ++i)
    {
      if (bus_connections[i] == connection)
        {
          bus_connections[i] = NULL;
        }
    }

  _DBUS_UNLOCK (bus);
}

static DBusConnection *
internal_bus_get (DBusBusType  type,
                  dbus_bool_t  private,
                  DBusError   *error)
{
  const char *address;
  DBusConnection *connection;
  BusData *bd;
  DBusBusType address_type;

  _dbus_return_val_if_fail (type >= 0 && type < N_BUS_TYPES, NULL);
  _dbus_return_val_if_error_is_set (error, NULL);

  connection = NULL;

  if (!_DBUS_LOCK (bus))
    {
      _DBUS_SET_OOM (error);
      /* do not "goto out", that would try to unlock */
      return NULL;
    }

  if (!init_connections_unlocked ())
    {
      _DBUS_SET_OOM (error);
      goto out;
    }

  /* We want to use the activation address even if the
   * activating bus is the session or system bus,
   * per the spec.
   */
  address_type = type;
  
  /* Use the real type of the activation bus for getting its
   * connection, but only if the real type's address is available. (If
   * the activating bus isn't a well-known bus then
   * activation_bus_type == DBUS_BUS_STARTER)
   */
  if (type == DBUS_BUS_STARTER &&
      bus_connection_addresses[activation_bus_type] != NULL)
    type = activation_bus_type;
  
  if (!private && bus_connections[type] != NULL)
    {
      connection = bus_connections[type];
      dbus_connection_ref (connection);
      goto out;
    }

  address = bus_connection_addresses[address_type];
  if (address == NULL)
    {
      dbus_set_error (error, DBUS_ERROR_FAILED,
                      "Unable to determine the address of the message bus (try 'man dbus-launch' and 'man dbus-daemon' for help)");
      goto out;
    }

  if (private)
    connection = dbus_connection_open_private (address, error);
  else
    connection = dbus_connection_open (address, error);
  
  if (!connection)
    {
      goto out;
    }

  if (!dbus_bus_register (connection, error))
    {
      _dbus_connection_close_possibly_shared (connection);
      dbus_connection_unref (connection);
      connection = NULL;
      goto out;
    }

  if (!private)
    {
      /* store a weak ref to the connection (dbus-connection.c is
       * supposed to have a strong ref that it drops on disconnect,
       * since this is a shared connection)
       */
      bus_connections[type] = connection;
    }

  /* By default we're bound to the lifecycle of
   * the message bus.
   */
  dbus_connection_set_exit_on_disconnect (connection,
                                          TRUE);

  if (!_DBUS_LOCK (bus_datas))
    _dbus_assert_not_reached ("global locks were initialized already");

  bd = ensure_bus_data (connection);
  _dbus_assert (bd != NULL); /* it should have been created on
                                register, so OOM not possible */
  bd->is_well_known = TRUE;
  _DBUS_UNLOCK (bus_datas);

out:
  /* Return a reference to the caller, or NULL with error set. */
  if (connection == NULL)
    _DBUS_ASSERT_ERROR_IS_SET (error);

  _DBUS_UNLOCK (bus);
  return connection;
}


/** @} */ /* end of implementation details docs */

/**
 * @addtogroup DBusBus
 * @{
 */

/**
 * Connects to a bus daemon and registers the client with it.  If a
 * connection to the bus already exists, then that connection is
 * returned.  The caller of this function owns a reference to the bus.
 *
 * The caller may NOT call dbus_connection_close() on this connection;
 * see dbus_connection_open() and dbus_connection_close() for details
 * on that.
 *
 * If this function obtains a new connection object never before
 * returned from dbus_bus_get(), it will call
 * dbus_connection_set_exit_on_disconnect(), so the application
 * will exit if the connection closes. You can undo this
 * by calling dbus_connection_set_exit_on_disconnect() yourself
 * after you get the connection.
 *
 * dbus_bus_get() calls dbus_bus_register() for you.
 * 
 * If returning a newly-created connection, this function will block
 * until authentication and bus registration are complete.
 * 
 * @param type bus type
 * @param error address where an error can be returned.
 * @returns a #DBusConnection with new ref or #NULL on error
 */
DBusConnection *
dbus_bus_get (DBusBusType  type,
	      DBusError   *error)
{
  return internal_bus_get (type, FALSE, error);
}

/**
 * Connects to a bus daemon and registers the client with it as with
 * dbus_bus_register().  Unlike dbus_bus_get(), always creates a new
 * connection. This connection will not be saved or recycled by
 * libdbus. Caller owns a reference to the bus and must either close
 * it or know it to be closed prior to releasing this reference.
 *
 * See dbus_connection_open_private() for more details on when to
 * close and unref this connection.
 *
 * This function calls
 * dbus_connection_set_exit_on_disconnect() on the new connection, so the application
 * will exit if the connection closes. You can undo this
 * by calling dbus_connection_set_exit_on_disconnect() yourself
 * after you get the connection.
 *
 * dbus_bus_get_private() calls dbus_bus_register() for you.
 *
 * This function will block until authentication and bus registration
 * are complete.
 *
 * @param type bus type
 * @param error address where an error can be returned.
 * @returns a DBusConnection with new ref
 */
DBusConnection *
dbus_bus_get_private (DBusBusType  type,
                      DBusError   *error)
{
  return internal_bus_get (type, TRUE, error);
}

/**
 * Registers a connection with the bus. This must be the first
 * thing an application does when connecting to the message bus.
 * If registration succeeds, the unique name will be set,
 * and can be obtained using dbus_bus_get_unique_name().
 *
 * This function will block until registration is complete.
 *
 * If the connection has already registered with the bus
 * (determined by checking whether dbus_bus_get_unique_name()
 * returns a non-#NULL value), then this function does nothing.
 *
 * If you use dbus_bus_get() or dbus_bus_get_private() this
 * function will be called for you.
 * 
 * @note Just use dbus_bus_get() or dbus_bus_get_private() instead of
 * dbus_bus_register() and save yourself some pain. Using
 * dbus_bus_register() manually is only useful if you have your
 * own custom message bus not found in #DBusBusType.
 *
 * If you open a bus connection with dbus_connection_open() or
 * dbus_connection_open_private() you will have to dbus_bus_register()
 * yourself, or make the appropriate registration method calls
 * yourself. If you send the method calls yourself, call
 * dbus_bus_set_unique_name() with the unique bus name you get from
 * the bus.
 *
 * For shared connections (created with dbus_connection_open()) in a
 * multithreaded application, you can't really make the registration
 * calls yourself, because you don't know whether some other thread is
 * also registering, and the bus will kick you off if you send two
 * registration messages.
 *
 * If you use dbus_bus_register() however, there is a lock that
 * keeps both apps from registering at the same time.
 *
 * The rule in a multithreaded app, then, is that dbus_bus_register()
 * must be used to register, or you need to have your own locks that
 * all threads in the app will respect.
 *
 * In a single-threaded application you can register by hand instead
 * of using dbus_bus_register(), as long as you check
 * dbus_bus_get_unique_name() to see if a unique name has already been
 * stored by another thread before you send the registration messages.
 * 
 * @param connection the connection
 * @param error place to store errors
 * @returns #TRUE on success
 */
dbus_bool_t
dbus_bus_register (DBusConnection *connection,
                   DBusError      *error)
{
  DBusMessage *message, *reply;
  char *name;
  BusData *bd;
  dbus_bool_t retval;

  _dbus_return_val_if_fail (connection != NULL, FALSE);
  _dbus_return_val_if_error_is_set (error, FALSE);

  retval = FALSE;
  message = NULL;
  reply = NULL;

  if (!_DBUS_LOCK (bus_datas))
    {
      _DBUS_SET_OOM (error);
      /* do not "goto out", that would try to unlock */
      return FALSE;
    }

  bd = ensure_bus_data (connection);
  if (bd == NULL)
    {
      _DBUS_SET_OOM (error);
      goto out;
    }

  if (bd->unique_name != NULL)
    {
      _dbus_verbose ("Ignoring attempt to register the same DBusConnection %s with the message bus a second time.\n",
                     bd->unique_name);
      /* Success! */
      retval = TRUE;
      goto out;
    }
  
  message = dbus_message_new_method_call (DBUS_SERVICE_DBUS,
                                          DBUS_PATH_DBUS,
                                          DBUS_INTERFACE_DBUS,
                                          "Hello"); 

  if (!message)
    {
      _DBUS_SET_OOM (error);
      goto out;
    }
  
  reply = dbus_connection_send_with_reply_and_block (connection, message, -1, error);

  if (reply == NULL)
    goto out;
  else if (dbus_set_error_from_message (error, reply))
    goto out;
  else if (!dbus_message_get_args (reply, error,
                                   DBUS_TYPE_STRING, &name,
                                   DBUS_TYPE_INVALID))
    goto out;
  
  bd->unique_name = _dbus_strdup (name);
  if (bd->unique_name == NULL)
    {
      _DBUS_SET_OOM (error);
      goto out;
    }
  
  retval = TRUE;
  
 out:
  _DBUS_UNLOCK (bus_datas);

  if (message)
    dbus_message_unref (message);

  if (reply)
    dbus_message_unref (reply);

  if (!retval)
    _DBUS_ASSERT_ERROR_IS_SET (error);

  return retval;
}


/**
 * Sets the unique name of the connection, as assigned by the message
 * bus.  Can only be used if you registered with the bus manually
 * (i.e. if you did not call dbus_bus_register()). Can only be called
 * once per connection.  After the unique name is set, you can get it
 * with dbus_bus_get_unique_name().
 *
 * The only reason to use this function is to re-implement the
 * equivalent of dbus_bus_register() yourself. One (probably unusual)
 * reason to do that might be to do the bus registration call
 * asynchronously instead of synchronously.
 *
 * @note Just use dbus_bus_get() or dbus_bus_get_private(), or worst
 * case dbus_bus_register(), instead of messing with this
 * function. There's really no point creating pain for yourself by
 * doing things manually.
 *
 * It's hard to use this function safely on shared connections
 * (created by dbus_connection_open()) in a multithreaded application,
 * because only one registration attempt can be sent to the bus. If
 * two threads are both sending the registration message, there is no
 * mechanism in libdbus itself to avoid sending it twice.
 *
 * Thus, you need a way to coordinate which thread sends the
 * registration attempt; which also means you know which thread
 * will call dbus_bus_set_unique_name(). If you don't know
 * about all threads in the app (for example, if some libraries
 * you're using might start libdbus-using threads), then you
 * need to avoid using this function on shared connections.
 *
 * @param connection the connection
 * @param unique_name the unique name
 * @returns #FALSE if not enough memory
 */
dbus_bool_t
dbus_bus_set_unique_name (DBusConnection *connection,
                          const char     *unique_name)
{
  BusData *bd;
  dbus_bool_t success = FALSE;

  _dbus_return_val_if_fail (connection != NULL, FALSE);
  _dbus_return_val_if_fail (unique_name != NULL, FALSE);

  if (!_DBUS_LOCK (bus_datas))
    {
      /* do not "goto out", that would try to unlock */
      return FALSE;
    }

  bd = ensure_bus_data (connection);
  if (bd == NULL)
    goto out;

  _dbus_assert (bd->unique_name == NULL);
  
  bd->unique_name = _dbus_strdup (unique_name);
  success = bd->unique_name != NULL;

out:
  _DBUS_UNLOCK (bus_datas);
  
  return success;
}

/**
 * Gets the unique name of the connection as assigned by the message
 * bus. Only possible after the connection has been registered with
 * the message bus. All connections returned by dbus_bus_get() or
 * dbus_bus_get_private() have been successfully registered.
 *
 * The name remains valid until the connection is freed, and
 * should not be freed by the caller.
 *
 * Other than dbus_bus_get(), there are two ways to set the unique
 * name; one is dbus_bus_register(), the other is
 * dbus_bus_set_unique_name().  You are responsible for calling
 * dbus_bus_set_unique_name() if you register by hand instead of using
 * dbus_bus_register().
 * 
 * @param connection the connection
 * @returns the unique name or #NULL on error
 */
const char*
dbus_bus_get_unique_name (DBusConnection *connection)
{
  BusData *bd;
  const char *unique_name = NULL;

  _dbus_return_val_if_fail (connection != NULL, NULL);

  if (!_DBUS_LOCK (bus_datas))
    {
      /* We'd have initialized locks when we gave it its unique name, if it
       * had one. Don't "goto out", that would try to unlock. */
      return NULL;
    }

  bd = ensure_bus_data (connection);
  if (bd == NULL)
    goto out;

  unique_name = bd->unique_name;

out:
  _DBUS_UNLOCK (bus_datas);

  return unique_name;
}

/**
 * Asks the bus to return the UID the named connection authenticated
 * as, if any.  Only works on UNIX; only works for connections on the
 * same machine as the bus. If you are not on the same machine as the
 * bus, then calling this is probably a bad idea, since the UID will
 * mean little to your application.
 *
 * For the system message bus you're guaranteed to be on the same
 * machine since it only listens on a UNIX domain socket (at least,
 * as shipped by default).
 *
 * This function only works for connections that authenticated as
 * a UNIX user, right now that includes all bus connections, but
 * it's very possible to have connections with no associated UID.
 * So check for errors and do something sensible if they happen.
 * 
 * This function will always return an error on Windows.
 * 
 * @param connection the connection
 * @param name a name owned by the connection
 * @param error location to store the error
 * @returns the unix user id, or ((unsigned)-1) if error is set
 */ 
unsigned long
dbus_bus_get_unix_user (DBusConnection *connection,
                        const char     *name,
                        DBusError      *error)
{
  DBusMessage *message, *reply;
  dbus_uint32_t uid;

  _dbus_return_val_if_fail (connection != NULL, DBUS_UID_UNSET);
  _dbus_return_val_if_fail (name != NULL, DBUS_UID_UNSET);
  _dbus_return_val_if_fail (_dbus_check_is_valid_bus_name (name), DBUS_UID_UNSET);
  _dbus_return_val_if_error_is_set (error, DBUS_UID_UNSET);
  
  message = dbus_message_new_method_call (DBUS_SERVICE_DBUS,
                                          DBUS_PATH_DBUS,
                                          DBUS_INTERFACE_DBUS,
                                          "GetConnectionUnixUser");

  if (message == NULL)
    {
      _DBUS_SET_OOM (error);
      return DBUS_UID_UNSET;
    }
 
  if (!dbus_message_append_args (message,
				 DBUS_TYPE_STRING, &name,
				 DBUS_TYPE_INVALID))
    {
      dbus_message_unref (message);
      _DBUS_SET_OOM (error);
      return DBUS_UID_UNSET;
    }
  
  reply = dbus_connection_send_with_reply_and_block (connection, message, -1,
                                                     error);
  
  dbus_message_unref (message);
  
  if (reply == NULL)
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      return DBUS_UID_UNSET;
    }  

  if (dbus_set_error_from_message (error, reply))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      dbus_message_unref (reply);
      return DBUS_UID_UNSET;
    }
  
  if (!dbus_message_get_args (reply, error,
                              DBUS_TYPE_UINT32, &uid,
                              DBUS_TYPE_INVALID))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      dbus_message_unref (reply);
      return DBUS_UID_UNSET;
    }

  dbus_message_unref (reply);
  
  return (unsigned long) uid;
}

/**
 * Asks the bus to return its globally unique ID, as described in the
 * D-Bus specification. For the session bus, this is useful as a way
 * to uniquely identify each user session. For the system bus,
 * probably the bus ID is not useful; instead, use the machine ID
 * since it's accessible without necessarily connecting to the bus and
 * may be persistent beyond a single bus instance (across reboots for
 * example). See dbus_try_get_local_machine_id().
 *
 * In addition to an ID for each bus and an ID for each machine, there is
 * an ID for each address that the bus is listening on; that can
 * be retrieved with dbus_connection_get_server_id(), though it is
 * probably not very useful.
 * 
 * @param connection the connection
 * @param error location to store the error
 * @returns the bus ID or #NULL if error is set
 */ 
char*
dbus_bus_get_id (DBusConnection *connection,
                 DBusError      *error)
{
  DBusMessage *message, *reply;
  char *id;
  const char *v_STRING;

  _dbus_return_val_if_fail (connection != NULL, NULL);
  _dbus_return_val_if_error_is_set (error, NULL);
  
  message = dbus_message_new_method_call (DBUS_SERVICE_DBUS,
                                          DBUS_PATH_DBUS,
                                          DBUS_INTERFACE_DBUS,
                                          "GetId");
  
  if (message == NULL)
    {
      _DBUS_SET_OOM (error);
      return NULL;
    }
  
  reply = dbus_connection_send_with_reply_and_block (connection, message, -1,
                                                     error);
  
  dbus_message_unref (message);
  
  if (reply == NULL)
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      return NULL;
    }  

  if (dbus_set_error_from_message (error, reply))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      dbus_message_unref (reply);
      return NULL;
    }

  v_STRING = NULL;
  if (!dbus_message_get_args (reply, error,
                              DBUS_TYPE_STRING, &v_STRING,
                              DBUS_TYPE_INVALID))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      dbus_message_unref (reply);
      return NULL;
    }

  id = _dbus_strdup (v_STRING); /* may be NULL */
  
  dbus_message_unref (reply);

  if (id == NULL)
    _DBUS_SET_OOM (error);

  /* FIXME it might be nice to cache the ID locally */
  
  return id;
}

/**
 * Asks the bus to assign the given name to this connection by invoking
 * the RequestName method on the bus. This method is fully documented
 * in the D-Bus specification. For quick reference, the flags and
 * result codes are discussed here, but the specification is the
 * canonical version of this information.
 *
 * First you should know that for each bus name, the bus stores
 * a queue of connections that would like to own it. Only
 * one owns it at a time - called the primary owner. If the primary
 * owner releases the name or disconnects, then the next owner in the
 * queue atomically takes over.
 *
 * So for example if you have an application org.freedesktop.TextEditor
 * and multiple instances of it can be run, you can have all of them
 * sitting in the queue. The first one to start up will receive messages
 * sent to org.freedesktop.TextEditor, but if that one exits another
 * will become the primary owner and receive messages.
 *
 * The queue means you don't need to manually watch for the current owner to
 * disappear and then request the name again.
 *
 * When requesting a name, you can specify several flags.
 * 
 * #DBUS_NAME_FLAG_ALLOW_REPLACEMENT and #DBUS_NAME_FLAG_DO_NOT_QUEUE
 * are properties stored by the bus for this connection with respect to
 * each requested bus name. These properties are stored even if the
 * connection is queued and does not become the primary owner.
 * You can update these flags by calling RequestName again (even if
 * you already own the name).
 *
 * #DBUS_NAME_FLAG_ALLOW_REPLACEMENT means that another requestor of the
 * name can take it away from you by specifying #DBUS_NAME_FLAG_REPLACE_EXISTING.
 *
 * #DBUS_NAME_FLAG_DO_NOT_QUEUE means that if you aren't the primary owner,
 * you don't want to be queued up - you only care about being the
 * primary owner.
 *
 * Unlike the other two flags, #DBUS_NAME_FLAG_REPLACE_EXISTING is a property
 * of the individual RequestName call, i.e. the bus does not persistently
 * associate it with the connection-name pair. If a RequestName call includes
 * the #DBUS_NAME_FLAG_REPLACE_EXISTING flag, and the current primary
 * owner has #DBUS_NAME_FLAG_ALLOW_REPLACEMENT set, then the current primary
 * owner will be kicked off.
 *
 * If no flags are given, an application will receive the requested
 * name only if the name is currently unowned; and it will NOT give
 * up the name if another application asks to take it over using
 * #DBUS_NAME_FLAG_REPLACE_EXISTING.
 *
 * This function returns a result code. The possible result codes
 * are as follows.
 * 
 * #DBUS_REQUEST_NAME_REPLY_PRIMARY_OWNER means that the name had no
 * existing owner, and the caller is now the primary owner; or that
 * the name had an owner, and the caller specified
 * #DBUS_NAME_FLAG_REPLACE_EXISTING, and the current owner
 * specified #DBUS_NAME_FLAG_ALLOW_REPLACEMENT.
 *
 * #DBUS_REQUEST_NAME_REPLY_IN_QUEUE happens only if the caller does NOT
 * specify #DBUS_NAME_FLAG_DO_NOT_QUEUE and either the current owner
 * did NOT specify #DBUS_NAME_FLAG_ALLOW_REPLACEMENT or the caller did NOT
 * specify #DBUS_NAME_FLAG_REPLACE_EXISTING. In this case the caller ends up 
 * in a queue to own the name after the current owner gives it up.
 *
 * #DBUS_REQUEST_NAME_REPLY_EXISTS happens if the name has an owner
 * already and the caller specifies #DBUS_NAME_FLAG_DO_NOT_QUEUE
 * and either the current owner has NOT specified 
 * #DBUS_NAME_FLAG_ALLOW_REPLACEMENT or the caller did NOT specify 
 * #DBUS_NAME_FLAG_REPLACE_EXISTING.
 *
 * #DBUS_REQUEST_NAME_REPLY_ALREADY_OWNER happens if an application
 * requests a name it already owns. (Re-requesting a name is useful if
 * you want to change the #DBUS_NAME_FLAG_ALLOW_REPLACEMENT or
 * #DBUS_NAME_FLAG_DO_NOT_QUEUE settings.)
 *
 * When a service represents an application, say "text editor," then
 * it should specify #DBUS_NAME_FLAG_ALLOW_REPLACEMENT if it wants
 * the last editor started to be the user's editor vs. the first one
 * started.  Then any editor that can be the user's editor should
 * specify #DBUS_NAME_FLAG_REPLACE_EXISTING to either take over
 * (last-started-wins) or be queued up (first-started-wins) according
 * to whether #DBUS_NAME_FLAG_ALLOW_REPLACEMENT was given.
 *
 * Conventionally, single-instance applications often offer a command
 * line option called --replace which means to replace the current
 * instance.  To implement this, always set
 * #DBUS_NAME_FLAG_ALLOW_REPLACEMENT when you request your
 * application's bus name.  When you lose ownership of your bus name,
 * you need to exit.  Look for the signal "NameLost" from
 * #DBUS_SERVICE_DBUS and #DBUS_INTERFACE_DBUS (the signal's first
 * argument is the bus name that was lost).  If starting up without
 * --replace, do not specify #DBUS_NAME_FLAG_REPLACE_EXISTING, and
 * exit if you fail to become the bus name owner. If --replace is
 * given, ask to replace the old owner.
 *
 * @param connection the connection
 * @param name the name to request
 * @param flags flags
 * @param error location to store the error
 * @returns a result code, -1 if error is set
 */ 
int
dbus_bus_request_name (DBusConnection *connection,
                       const char     *name,
                       unsigned int    flags,
                       DBusError      *error)
{
  DBusMessage *message, *reply;
  dbus_uint32_t result;

  _dbus_return_val_if_fail (connection != NULL, 0);
  _dbus_return_val_if_fail (name != NULL, 0);
  _dbus_return_val_if_fail (_dbus_check_is_valid_bus_name (name), 0);
  _dbus_return_val_if_error_is_set (error, 0);
  
  message = dbus_message_new_method_call (DBUS_SERVICE_DBUS,
                                          DBUS_PATH_DBUS,
                                          DBUS_INTERFACE_DBUS,
                                          "RequestName");

  if (message == NULL)
    {
      _DBUS_SET_OOM (error);
      return -1;
    }
 
  if (!dbus_message_append_args (message,
				 DBUS_TYPE_STRING, &name,
				 DBUS_TYPE_UINT32, &flags,
				 DBUS_TYPE_INVALID))
    {
      dbus_message_unref (message);
      _DBUS_SET_OOM (error);
      return -1;
    }
  
  reply = dbus_connection_send_with_reply_and_block (connection, message, -1,
                                                     error);
  
  dbus_message_unref (message);
  
  if (reply == NULL)
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      return -1;
    }  

  if (dbus_set_error_from_message (error, reply))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      dbus_message_unref (reply);
      return -1;
    }
  
  if (!dbus_message_get_args (reply, error,
                              DBUS_TYPE_UINT32, &result,
                              DBUS_TYPE_INVALID))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      dbus_message_unref (reply);
      return -1;
    }

  dbus_message_unref (reply);
  
  return result;
}


/**
 * Asks the bus to unassign the given name from this connection by
 * invoking the ReleaseName method on the bus. The "ReleaseName"
 * method is canonically documented in the D-Bus specification.
 *
 * Possible results are: #DBUS_RELEASE_NAME_REPLY_RELEASED
 * which means you owned the name or were in the queue to own it,
 * and and now you don't own it and aren't in the queue.
 * #DBUS_RELEASE_NAME_REPLY_NOT_OWNER which means someone else
 * owns the name so you can't release it.
 * #DBUS_RELEASE_NAME_REPLY_NON_EXISTENT
 * which means nobody owned the name.
 * 
 * @param connection the connection
 * @param name the name to remove 
 * @param error location to store the error
 * @returns a result code, -1 if error is set
 */ 
int
dbus_bus_release_name (DBusConnection *connection,
                       const char     *name,
                       DBusError      *error)
{
  DBusMessage *message, *reply;
  dbus_uint32_t result;

  _dbus_return_val_if_fail (connection != NULL, 0);
  _dbus_return_val_if_fail (name != NULL, 0);
  _dbus_return_val_if_fail (_dbus_check_is_valid_bus_name (name), 0);
  _dbus_return_val_if_error_is_set (error, 0);

  message = dbus_message_new_method_call (DBUS_SERVICE_DBUS,
                                          DBUS_PATH_DBUS,
                                          DBUS_INTERFACE_DBUS,
                                          "ReleaseName");

  if (message == NULL)
    {
      _DBUS_SET_OOM (error);
      return -1;
    }

  if (!dbus_message_append_args (message,
                                 DBUS_TYPE_STRING, &name,
                                 DBUS_TYPE_INVALID))
    {
      dbus_message_unref (message);
      _DBUS_SET_OOM (error);
      return -1;
    }

  reply = dbus_connection_send_with_reply_and_block (connection, message, -1,
                                                     error);

  dbus_message_unref (message);

  if (reply == NULL)
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      return -1;
    }

  if (dbus_set_error_from_message (error, reply))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      dbus_message_unref (reply);
      return -1;
    }

  if (!dbus_message_get_args (reply, error,
                              DBUS_TYPE_UINT32, &result,
                              DBUS_TYPE_INVALID))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      dbus_message_unref (reply);
      return -1;
    }

  dbus_message_unref (reply);

  return result;
}

/**
 * Asks the bus whether a certain name has an owner.
 *
 * Using this can easily result in a race condition,
 * since an owner can appear or disappear after you
 * call this.
 *
 * If you want to request a name, just request it;
 * if you want to avoid replacing a current owner,
 * don't specify #DBUS_NAME_FLAG_REPLACE_EXISTING and
 * you will get an error if there's already an owner.
 * 
 * @param connection the connection
 * @param name the name
 * @param error location to store any errors
 * @returns #TRUE if the name exists, #FALSE if not or on error
 */
dbus_bool_t
dbus_bus_name_has_owner (DBusConnection *connection,
			 const char     *name,
                         DBusError      *error)
{
  DBusMessage *message, *reply;
  dbus_bool_t exists;

  _dbus_return_val_if_fail (connection != NULL, FALSE);
  _dbus_return_val_if_fail (name != NULL, FALSE);
  _dbus_return_val_if_fail (_dbus_check_is_valid_bus_name (name), FALSE);
  _dbus_return_val_if_error_is_set (error, FALSE);
  
  message = dbus_message_new_method_call (DBUS_SERVICE_DBUS,
                                          DBUS_PATH_DBUS,
                                          DBUS_INTERFACE_DBUS,
                                          "NameHasOwner");
  if (message == NULL)
    {
      _DBUS_SET_OOM (error);
      return FALSE;
    }
  
  if (!dbus_message_append_args (message,
				 DBUS_TYPE_STRING, &name,
				 DBUS_TYPE_INVALID))
    {
      dbus_message_unref (message);
      _DBUS_SET_OOM (error);
      return FALSE;
    }
  
  reply = dbus_connection_send_with_reply_and_block (connection, message, -1, error);
  dbus_message_unref (message);

  if (reply == NULL)
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      return FALSE;
    }

  if (!dbus_message_get_args (reply, error,
                              DBUS_TYPE_BOOLEAN, &exists,
                              DBUS_TYPE_INVALID))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      dbus_message_unref (reply);
      return FALSE;
    }
  
  dbus_message_unref (reply);
  return exists;
}

/**
 * Starts a service that will request ownership of the given name.
 * The returned result will be one of be one of
 * #DBUS_START_REPLY_SUCCESS or #DBUS_START_REPLY_ALREADY_RUNNING if
 * successful.  Pass #NULL if you don't care about the result.
 * 
 * The flags parameter is for future expansion, currently you should
 * specify 0.
 *
 * It's often easier to avoid explicitly starting services, and
 * just send a method call to the service's bus name instead.
 * Method calls start a service to handle them by default
 * unless you call dbus_message_set_auto_start() to disable this
 * behavior.
 * 
 * @param connection the connection
 * @param name the name we want the new service to request
 * @param flags the flags (should always be 0 for now)
 * @param result a place to store the result or #NULL
 * @param error location to store any errors
 * @returns #TRUE if the activation succeeded, #FALSE if not
 */
dbus_bool_t
dbus_bus_start_service_by_name (DBusConnection *connection,
                                const char     *name,
                                dbus_uint32_t   flags,
                                dbus_uint32_t  *result,
                                DBusError      *error)
{
  DBusMessage *msg;
  DBusMessage *reply;

  _dbus_return_val_if_fail (connection != NULL, FALSE);
  _dbus_return_val_if_fail (_dbus_check_is_valid_bus_name (name), FALSE);
  
  msg = dbus_message_new_method_call (DBUS_SERVICE_DBUS,
                                      DBUS_PATH_DBUS,
                                      DBUS_INTERFACE_DBUS,
                                      "StartServiceByName");

  if (!dbus_message_append_args (msg, DBUS_TYPE_STRING, &name,
			  	 DBUS_TYPE_UINT32, &flags, DBUS_TYPE_INVALID))
    {
      dbus_message_unref (msg);
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  reply = dbus_connection_send_with_reply_and_block (connection, msg,
                                                     -1, error);
  dbus_message_unref (msg);

  if (reply == NULL)
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      return FALSE;
    }

  if (dbus_set_error_from_message (error, reply))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      dbus_message_unref (reply);
      return FALSE;
    }

  if (result != NULL &&
      !dbus_message_get_args (reply, error, DBUS_TYPE_UINT32,
	      		      result, DBUS_TYPE_INVALID))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      dbus_message_unref (reply);
      return FALSE;
    }
  
  dbus_message_unref (reply);
  return TRUE;
}

static void
send_no_return_values (DBusConnection *connection,
                       DBusMessage    *msg,
                       DBusError      *error)
{
  if (error)
    {
      /* Block to check success codepath */
      DBusMessage *reply;
      
      reply = dbus_connection_send_with_reply_and_block (connection, msg,
                                                         -1, error);
      
      if (reply == NULL)
        _DBUS_ASSERT_ERROR_IS_SET (error);
      else
        dbus_message_unref (reply);
    }
  else
    {
      /* Silently-fail nonblocking codepath */
      dbus_message_set_no_reply (msg, TRUE);
      dbus_connection_send (connection, msg, NULL);
    }
}

/**
 * Adds a match rule to match messages going through the message bus.
 * The "rule" argument is the string form of a match rule.
 *
 * If you pass #NULL for the error, this function will not
 * block; the match thus won't be added until you flush the
 * connection, and if there's an error adding the match
 * you won't find out about it. This is generally acceptable, since the
 * possible errors (including a lack of resources in the bus, the connection
 * having exceeded its quota of active match rules, or the match rule being
 * unparseable) are generally unrecoverable.
 *
 * If you pass non-#NULL for the error this function will
 * block until it gets a reply. This may be useful when using match rule keys
 * introduced in recent versions of D-Bus, like 'arg0namespace', to allow the
 * application to fall back to less efficient match rules supported by older
 * versions of the daemon if the running version is not new enough; or when
 * using user-supplied rules rather than rules hard-coded at compile time.
 *
 * Normal API conventions would have the function return
 * a boolean value indicating whether the error was set,
 * but that would require blocking always to determine
 * the return value.
 *
 * The AddMatch method is fully documented in the D-Bus 
 * specification. For quick reference, the format of the 
 * match rules is discussed here, but the specification 
 * is the canonical version of this information.
 *
 * Rules are specified as a string of comma separated 
 * key/value pairs. An example is 
 * "type='signal',sender='org.freedesktop.DBus',
 * interface='org.freedesktop.DBus',member='Foo',
 * path='/bar/foo',destination=':452345.34'"
 *
 * Possible keys you can match on are type, sender, 
 * interface, member, path, destination and numbered
 * keys to match message args (keys are 'arg0', 'arg1', etc.).
 * Omitting a key from the rule indicates 
 * a wildcard match.  For instance omitting
 * the member from a match rule but adding a sender would
 * let all messages from that sender through regardless of
 * the member.
 *
 * Matches are inclusive not exclusive so as long as one 
 * rule matches the message will get through.  It is important
 * to note this because every time a message is received the 
 * application will be paged into memory to process it.  This
 * can cause performance problems such as draining batteries
 * on embedded platforms.
 *
 * If you match message args ('arg0', 'arg1', and so forth)
 * only string arguments will match. That is, arg0='5' means
 * match the string "5" not the integer 5.
 *
 * Currently there is no way to match against non-string arguments.
 *
 * A specialised form of wildcard matching on arguments is
 * supported for path-like namespaces.  If your argument match has
 * a 'path' suffix (eg: "arg0path='/some/path/'") then it is
 * considered a match if the argument exactly matches the given
 * string or if one of them ends in a '/' and is a prefix of the
 * other.
 *
 * Matching on interface is tricky because method call
 * messages only optionally specify the interface.
 * If a message omits the interface, then it will NOT match
 * if the rule specifies an interface name. This means match
 * rules on method calls should not usually give an interface.
 *
 * However, signal messages are required to include the interface
 * so when matching signals usually you should specify the interface
 * in the match rule.
 * 
 * For security reasons, you can match arguments only up to
 * #DBUS_MAXIMUM_MATCH_RULE_ARG_NUMBER.
 *
 * Match rules have a maximum length of #DBUS_MAXIMUM_MATCH_RULE_LENGTH
 * bytes.
 *
 * Both of these maximums are much higher than you're likely to need,
 * they only exist because the D-Bus bus daemon has fixed limits on
 * all resource usage.
 *
 * @param connection connection to the message bus
 * @param rule textual form of match rule
 * @param error location to store any errors
 */
void
dbus_bus_add_match (DBusConnection *connection,
                    const char     *rule,
                    DBusError      *error)
{
  DBusMessage *msg;

  _dbus_return_if_fail (rule != NULL);

  msg = dbus_message_new_method_call (DBUS_SERVICE_DBUS,
                                      DBUS_PATH_DBUS,
                                      DBUS_INTERFACE_DBUS,
                                      "AddMatch");

  if (msg == NULL)
    {
      _DBUS_SET_OOM (error);
      return;
    }

  if (!dbus_message_append_args (msg, DBUS_TYPE_STRING, &rule,
                                 DBUS_TYPE_INVALID))
    {
      dbus_message_unref (msg);
      _DBUS_SET_OOM (error);
      return;
    }

  send_no_return_values (connection, msg, error);

  dbus_message_unref (msg);
}

/**
 * Removes a previously-added match rule "by value" (the most
 * recently-added identical rule gets removed).  The "rule" argument
 * is the string form of a match rule.
 *
 * The bus compares match rules semantically, not textually, so
 * whitespace and ordering don't have to be identical to
 * the rule you passed to dbus_bus_add_match().
 * 
 * If you pass #NULL for the error, this function will not
 * block; otherwise it will. See detailed explanation in
 * docs for dbus_bus_add_match().
 * 
 * @param connection connection to the message bus
 * @param rule textual form of match rule
 * @param error location to store any errors
 */
void
dbus_bus_remove_match (DBusConnection *connection,
                       const char     *rule,
                       DBusError      *error)
{
  DBusMessage *msg;

  _dbus_return_if_fail (rule != NULL);
  
  msg = dbus_message_new_method_call (DBUS_SERVICE_DBUS,
                                      DBUS_PATH_DBUS,
                                      DBUS_INTERFACE_DBUS,
                                      "RemoveMatch");

  if (!dbus_message_append_args (msg, DBUS_TYPE_STRING, &rule,
                                 DBUS_TYPE_INVALID))
    {
      dbus_message_unref (msg);
      _DBUS_SET_OOM (error);
      return;
    }

  send_no_return_values (connection, msg, error);

  dbus_message_unref (msg);
}

/** @} */
