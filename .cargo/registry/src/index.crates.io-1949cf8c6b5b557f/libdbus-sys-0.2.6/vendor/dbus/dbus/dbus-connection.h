/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-connection.h DBusConnection object
 *
 * Copyright (C) 2002, 2003  Red Hat Inc.
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
#if !defined (DBUS_INSIDE_DBUS_H) && !defined (DBUS_COMPILATION)
#error "Only <dbus/dbus.h> can be included directly, this file may disappear or change contents."
#endif

#ifndef DBUS_CONNECTION_H
#define DBUS_CONNECTION_H

#include <dbus/dbus-errors.h>
#include <dbus/dbus-macros.h>
#include <dbus/dbus-memory.h>
#include <dbus/dbus-message.h>
#include <dbus/dbus-shared.h>

DBUS_BEGIN_DECLS

/**
 * @addtogroup DBusConnection
 * @{
 */

/* documented in dbus-watch.c */
typedef struct DBusWatch DBusWatch;
/* documented in dbus-timeout.c */
typedef struct DBusTimeout DBusTimeout;
/** Opaque type representing preallocated resources so a message can be sent without further memory allocation. */
typedef struct DBusPreallocatedSend DBusPreallocatedSend;
/** Opaque type representing a method call that has not yet received a reply. */
typedef struct DBusPendingCall DBusPendingCall;
/** Opaque type representing a connection to a remote application and associated incoming/outgoing message queues. */
typedef struct DBusConnection DBusConnection;
/** Set of functions that must be implemented to handle messages sent to a particular object path. */
typedef struct DBusObjectPathVTable DBusObjectPathVTable;

/**
 * Indicates the status of a #DBusWatch.
 */
typedef enum
{
  DBUS_WATCH_READABLE = 1 << 0, /**< As in POLLIN */
  DBUS_WATCH_WRITABLE = 1 << 1, /**< As in POLLOUT */
  DBUS_WATCH_ERROR    = 1 << 2, /**< As in POLLERR (can't watch for
                                 *   this, but can be present in
                                 *   current state passed to
                                 *   dbus_watch_handle()).
                                 */
  DBUS_WATCH_HANGUP   = 1 << 3  /**< As in POLLHUP (can't watch for
                                 *   it, but can be present in current
                                 *   state passed to
                                 *   dbus_watch_handle()).
                                 */
  /* Internal to libdbus, there is also _DBUS_WATCH_NVAL in dbus-watch.h */
} DBusWatchFlags;

/**
 * Indicates the status of incoming data on a #DBusConnection. This determines whether
 * dbus_connection_dispatch() needs to be called.
 */
typedef enum
{
  DBUS_DISPATCH_DATA_REMAINS,  /**< There is more data to potentially convert to messages. */
  DBUS_DISPATCH_COMPLETE,      /**< All currently available data has been processed. */
  DBUS_DISPATCH_NEED_MEMORY    /**< More memory is needed to continue. */
} DBusDispatchStatus;

/** Called when libdbus needs a new watch to be monitored by the main
 * loop. Returns #FALSE if it lacks enough memory to add the
 * watch. Set by dbus_connection_set_watch_functions() or
 * dbus_server_set_watch_functions().
 */
typedef dbus_bool_t (* DBusAddWatchFunction)       (DBusWatch      *watch,
                                                    void           *data);
/** Called when dbus_watch_get_enabled() may return a different value
 *  than it did before.  Set by dbus_connection_set_watch_functions()
 *  or dbus_server_set_watch_functions().
 */
typedef void        (* DBusWatchToggledFunction)   (DBusWatch      *watch,
                                                    void           *data);
/** Called when libdbus no longer needs a watch to be monitored by the
 * main loop. Set by dbus_connection_set_watch_functions() or
 * dbus_server_set_watch_functions().
 */
typedef void        (* DBusRemoveWatchFunction)    (DBusWatch      *watch,
                                                    void           *data);
/** Called when libdbus needs a new timeout to be monitored by the main
 * loop. Returns #FALSE if it lacks enough memory to add the
 * watch. Set by dbus_connection_set_timeout_functions() or
 * dbus_server_set_timeout_functions().
 */
typedef dbus_bool_t (* DBusAddTimeoutFunction)     (DBusTimeout    *timeout,
                                                    void           *data);
/** Called when dbus_timeout_get_enabled() may return a different
 * value than it did before.
 * Set by dbus_connection_set_timeout_functions() or
 * dbus_server_set_timeout_functions().
 */
typedef void        (* DBusTimeoutToggledFunction) (DBusTimeout    *timeout,
                                                    void           *data);
/** Called when libdbus no longer needs a timeout to be monitored by the
 * main loop. Set by dbus_connection_set_timeout_functions() or
 * dbus_server_set_timeout_functions().
 */
typedef void        (* DBusRemoveTimeoutFunction)  (DBusTimeout    *timeout,
                                                    void           *data);
/** Called when the return value of dbus_connection_get_dispatch_status()
 * may have changed. Set with dbus_connection_set_dispatch_status_function().
 */
typedef void        (* DBusDispatchStatusFunction) (DBusConnection *connection,
                                                    DBusDispatchStatus new_status,
                                                    void           *data);
/**
 * Called when the main loop's thread should be notified that there's now work
 * to do. Set with dbus_connection_set_wakeup_main_function().
 */
typedef void        (* DBusWakeupMainFunction)     (void           *data);

/**
 * Called during authentication to check whether the given UNIX user
 * ID is allowed to connect, if the client tried to auth as a UNIX
 * user ID. Normally on Windows this would never happen. Set with
 * dbus_connection_set_unix_user_function().
 */ 
typedef dbus_bool_t (* DBusAllowUnixUserFunction)  (DBusConnection *connection,
                                                    unsigned long   uid,
                                                    void           *data);

/**
 * Called during authentication to check whether the given Windows user
 * ID is allowed to connect, if the client tried to auth as a Windows
 * user ID. Normally on UNIX this would never happen. Set with
 * dbus_connection_set_windows_user_function().
 */ 
typedef dbus_bool_t (* DBusAllowWindowsUserFunction)  (DBusConnection *connection,
                                                       const char     *user_sid,
                                                       void           *data);


/**
 * Called when a pending call now has a reply available. Set with
 * dbus_pending_call_set_notify().
 */
typedef void (* DBusPendingCallNotifyFunction) (DBusPendingCall *pending,
                                                void            *user_data);

/**
 * Called when a message needs to be handled. The result indicates whether or
 * not more handlers should be run. Set with dbus_connection_add_filter().
 */
typedef DBusHandlerResult (* DBusHandleMessageFunction) (DBusConnection     *connection,
                                                         DBusMessage        *message,
                                                         void               *user_data);
DBUS_EXPORT
DBusConnection*    dbus_connection_open                         (const char                 *address,
                                                                 DBusError                  *error);
DBUS_EXPORT
DBusConnection*    dbus_connection_open_private                 (const char                 *address,
                                                                 DBusError                  *error);
DBUS_EXPORT
DBusConnection*    dbus_connection_ref                          (DBusConnection             *connection);
DBUS_EXPORT
void               dbus_connection_unref                        (DBusConnection             *connection);
DBUS_EXPORT
void               dbus_connection_close                        (DBusConnection             *connection);
DBUS_EXPORT
dbus_bool_t        dbus_connection_get_is_connected             (DBusConnection             *connection);
DBUS_EXPORT
dbus_bool_t        dbus_connection_get_is_authenticated         (DBusConnection             *connection);
DBUS_EXPORT
dbus_bool_t        dbus_connection_get_is_anonymous             (DBusConnection             *connection);
DBUS_EXPORT
char*              dbus_connection_get_server_id                (DBusConnection             *connection);
DBUS_EXPORT
dbus_bool_t        dbus_connection_can_send_type                (DBusConnection             *connection,
                                                                 int                         type);

DBUS_EXPORT
void               dbus_connection_set_exit_on_disconnect       (DBusConnection             *connection,
                                                                 dbus_bool_t                 exit_on_disconnect);
DBUS_EXPORT
void               dbus_connection_flush                        (DBusConnection             *connection);
DBUS_EXPORT
dbus_bool_t        dbus_connection_read_write_dispatch          (DBusConnection             *connection,
                                                                 int                         timeout_milliseconds);
DBUS_EXPORT
dbus_bool_t        dbus_connection_read_write                   (DBusConnection             *connection,
                                                                 int                         timeout_milliseconds);
DBUS_EXPORT
DBusMessage*       dbus_connection_borrow_message               (DBusConnection             *connection);
DBUS_EXPORT
void               dbus_connection_return_message               (DBusConnection             *connection,
                                                                 DBusMessage                *message);
DBUS_EXPORT
void               dbus_connection_steal_borrowed_message       (DBusConnection             *connection,
                                                                 DBusMessage                *message);
DBUS_EXPORT
DBusMessage*       dbus_connection_pop_message                  (DBusConnection             *connection);
DBUS_EXPORT
DBusDispatchStatus dbus_connection_get_dispatch_status          (DBusConnection             *connection);
DBUS_EXPORT
DBusDispatchStatus dbus_connection_dispatch                     (DBusConnection             *connection);
DBUS_EXPORT
dbus_bool_t        dbus_connection_has_messages_to_send         (DBusConnection *connection);
DBUS_EXPORT
dbus_bool_t        dbus_connection_send                         (DBusConnection             *connection,
                                                                 DBusMessage                *message,
                                                                 dbus_uint32_t              *client_serial);
DBUS_EXPORT
dbus_bool_t        dbus_connection_send_with_reply              (DBusConnection             *connection,
                                                                 DBusMessage                *message,
                                                                 DBusPendingCall           **pending_return,
                                                                 int                         timeout_milliseconds);
DBUS_EXPORT
DBusMessage *      dbus_connection_send_with_reply_and_block    (DBusConnection             *connection,
                                                                 DBusMessage                *message,
                                                                 int                         timeout_milliseconds,
                                                                 DBusError                  *error);
DBUS_EXPORT
dbus_bool_t        dbus_connection_set_watch_functions          (DBusConnection             *connection,
                                                                 DBusAddWatchFunction        add_function,
                                                                 DBusRemoveWatchFunction     remove_function,
                                                                 DBusWatchToggledFunction    toggled_function,
                                                                 void                       *data,
                                                                 DBusFreeFunction            free_data_function);
DBUS_EXPORT
dbus_bool_t        dbus_connection_set_timeout_functions        (DBusConnection             *connection,
                                                                 DBusAddTimeoutFunction      add_function,
                                                                 DBusRemoveTimeoutFunction   remove_function,
                                                                 DBusTimeoutToggledFunction  toggled_function,
                                                                 void                       *data,
                                                                 DBusFreeFunction            free_data_function);
DBUS_EXPORT
void               dbus_connection_set_wakeup_main_function     (DBusConnection             *connection,
                                                                 DBusWakeupMainFunction      wakeup_main_function,
                                                                 void                       *data,
                                                                 DBusFreeFunction            free_data_function);
DBUS_EXPORT
void               dbus_connection_set_dispatch_status_function (DBusConnection             *connection,
                                                                 DBusDispatchStatusFunction  function,
                                                                 void                       *data,
                                                                 DBusFreeFunction            free_data_function);
DBUS_EXPORT
dbus_bool_t        dbus_connection_get_unix_user                (DBusConnection             *connection,
                                                                 unsigned long              *uid);
DBUS_EXPORT
dbus_bool_t        dbus_connection_get_unix_process_id          (DBusConnection             *connection,
                                                                 unsigned long              *pid);
DBUS_EXPORT
dbus_bool_t        dbus_connection_get_adt_audit_session_data   (DBusConnection             *connection,
                                                                 void                      **data,
                                                                 dbus_int32_t               *data_size);
DBUS_EXPORT
void               dbus_connection_set_unix_user_function       (DBusConnection             *connection,
                                                                 DBusAllowUnixUserFunction   function,
                                                                 void                       *data,
                                                                 DBusFreeFunction            free_data_function);
DBUS_EXPORT
dbus_bool_t        dbus_connection_get_windows_user             (DBusConnection             *connection,
                                                                 char                      **windows_sid_p); 
DBUS_EXPORT
void               dbus_connection_set_windows_user_function    (DBusConnection             *connection,
                                                                 DBusAllowWindowsUserFunction function,
                                                                 void                       *data,
                                                                 DBusFreeFunction            free_data_function);
DBUS_EXPORT
void               dbus_connection_set_allow_anonymous          (DBusConnection             *connection,
                                                                 dbus_bool_t                 value);
DBUS_EXPORT
void               dbus_connection_set_route_peer_messages      (DBusConnection             *connection,
                                                                 dbus_bool_t                 value);


/* Filters */

DBUS_EXPORT
dbus_bool_t dbus_connection_add_filter    (DBusConnection            *connection,
                                           DBusHandleMessageFunction  function,
                                           void                      *user_data,
                                           DBusFreeFunction           free_data_function);
DBUS_EXPORT
void        dbus_connection_remove_filter (DBusConnection            *connection,
                                           DBusHandleMessageFunction  function,
                                           void                      *user_data);


/* Other */
DBUS_EXPORT
dbus_bool_t dbus_connection_allocate_data_slot (dbus_int32_t     *slot_p);
DBUS_EXPORT
void        dbus_connection_free_data_slot     (dbus_int32_t     *slot_p);
DBUS_EXPORT
dbus_bool_t dbus_connection_set_data           (DBusConnection   *connection,
                                                dbus_int32_t      slot,
                                                void             *data,
                                                DBusFreeFunction  free_data_func);
DBUS_EXPORT
void*       dbus_connection_get_data           (DBusConnection   *connection,
                                                dbus_int32_t      slot);

DBUS_EXPORT
void        dbus_connection_set_change_sigpipe (dbus_bool_t       will_modify_sigpipe); 

DBUS_EXPORT
void dbus_connection_set_max_message_size  (DBusConnection *connection,
                                            long            size);
DBUS_EXPORT
long dbus_connection_get_max_message_size  (DBusConnection *connection);
DBUS_EXPORT
void dbus_connection_set_max_received_size (DBusConnection *connection,
                                            long            size);
DBUS_EXPORT
long dbus_connection_get_max_received_size (DBusConnection *connection);

DBUS_EXPORT
void dbus_connection_set_max_message_unix_fds (DBusConnection *connection,
                                               long            n);
DBUS_EXPORT
long dbus_connection_get_max_message_unix_fds (DBusConnection *connection);
DBUS_EXPORT
void dbus_connection_set_max_received_unix_fds(DBusConnection *connection,
                                               long            n);
DBUS_EXPORT
long dbus_connection_get_max_received_unix_fds(DBusConnection *connection);

DBUS_EXPORT
long dbus_connection_get_outgoing_size     (DBusConnection *connection);
DBUS_EXPORT
long dbus_connection_get_outgoing_unix_fds (DBusConnection *connection);

DBUS_EXPORT
DBusPreallocatedSend* dbus_connection_preallocate_send       (DBusConnection       *connection);
DBUS_EXPORT
void                  dbus_connection_free_preallocated_send (DBusConnection       *connection,
                                                              DBusPreallocatedSend *preallocated);
DBUS_EXPORT
void                  dbus_connection_send_preallocated      (DBusConnection       *connection,
                                                              DBusPreallocatedSend *preallocated,
                                                              DBusMessage          *message,
                                                              dbus_uint32_t        *client_serial);


/* Object tree functionality */

/**
 * Called when a #DBusObjectPathVTable is unregistered (or its connection is freed).
 * Found in #DBusObjectPathVTable.
 */
typedef void              (* DBusObjectPathUnregisterFunction) (DBusConnection  *connection,
                                                                void            *user_data);
/**
 * Called when a message is sent to a registered object path. Found in
 * #DBusObjectPathVTable which is registered with dbus_connection_register_object_path()
 * or dbus_connection_register_fallback().
 */
typedef DBusHandlerResult (* DBusObjectPathMessageFunction)    (DBusConnection  *connection,
                                                                DBusMessage     *message,
                                                                void            *user_data);

/**
 * Virtual table that must be implemented to handle a portion of the
 * object path hierarchy. Attach the vtable to a particular path using
 * dbus_connection_register_object_path() or
 * dbus_connection_register_fallback().
 */
struct DBusObjectPathVTable
{
  DBusObjectPathUnregisterFunction   unregister_function; /**< Function to unregister this handler */
  DBusObjectPathMessageFunction      message_function; /**< Function to handle messages */
  
  void (* dbus_internal_pad1) (void *); /**< Reserved for future expansion */
  void (* dbus_internal_pad2) (void *); /**< Reserved for future expansion */
  void (* dbus_internal_pad3) (void *); /**< Reserved for future expansion */
  void (* dbus_internal_pad4) (void *); /**< Reserved for future expansion */
};

DBUS_EXPORT
dbus_bool_t dbus_connection_try_register_object_path (DBusConnection              *connection,
                                                      const char                  *path,
                                                      const DBusObjectPathVTable  *vtable,
                                                      void                        *user_data,
                                                      DBusError                   *error);

DBUS_EXPORT
dbus_bool_t dbus_connection_register_object_path   (DBusConnection              *connection,
                                                    const char                  *path,
                                                    const DBusObjectPathVTable  *vtable,
                                                    void                        *user_data);

DBUS_EXPORT
dbus_bool_t dbus_connection_try_register_fallback (DBusConnection              *connection,
                                                   const char                  *path,
                                                   const DBusObjectPathVTable  *vtable,
                                                   void                        *user_data,
                                                   DBusError                   *error);

DBUS_EXPORT
dbus_bool_t dbus_connection_register_fallback      (DBusConnection              *connection,
                                                    const char                  *path,
                                                    const DBusObjectPathVTable  *vtable,
                                                    void                        *user_data);
DBUS_EXPORT
dbus_bool_t dbus_connection_unregister_object_path (DBusConnection              *connection,
                                                    const char                  *path);

DBUS_EXPORT
dbus_bool_t dbus_connection_get_object_path_data   (DBusConnection              *connection,
                                                    const char                  *path,
                                                    void                       **data_p);

DBUS_EXPORT
dbus_bool_t dbus_connection_list_registered        (DBusConnection              *connection,
                                                    const char                  *parent_path,
                                                    char                      ***child_entries);

DBUS_EXPORT
dbus_bool_t dbus_connection_get_unix_fd            (DBusConnection              *connection,
                                                    int                         *fd);
DBUS_EXPORT
dbus_bool_t dbus_connection_get_socket             (DBusConnection              *connection,
                                                    int                         *fd);

/**
 * Clear a variable or struct member that contains a #DBusConnection.
 * If it does not contain #NULL, the connection that was previously
 * there is unreferenced with dbus_connection_unref().
 *
 * For example, this function and the similar functions for
 * other reference-counted types can be used in code like this:
 *
 * @code
 * DBusConnection *conn = NULL;
 * struct { ...; DBusMessage *m; ... } *larger_structure = ...;
 *
 * ... code that might set conn or m to be non-NULL ...
 *
 * dbus_clear_connection (&conn);
 * dbus_clear_message (&larger_structure->m);
 * @endcode
 *
 * @param pointer_to_connection A pointer to a variable or struct member.
 * pointer_to_connection must not be #NULL, but *pointer_to_connection
 * may be #NULL.
 */
static inline void
dbus_clear_connection (DBusConnection **pointer_to_connection)
{
  _dbus_clear_pointer_impl (DBusConnection, pointer_to_connection,
                            dbus_connection_unref);
}

/** @} */


/**
 * @addtogroup DBusWatch
 * @{
 */

#ifndef DBUS_DISABLE_DEPRECATED
DBUS_EXPORT
DBUS_DEPRECATED int dbus_watch_get_fd      (DBusWatch        *watch);
#endif

DBUS_EXPORT
int          dbus_watch_get_unix_fd (DBusWatch        *watch);
DBUS_EXPORT
int          dbus_watch_get_socket  (DBusWatch        *watch);
DBUS_EXPORT
unsigned int dbus_watch_get_flags   (DBusWatch        *watch);
DBUS_EXPORT
void*        dbus_watch_get_data    (DBusWatch        *watch);
DBUS_EXPORT
void         dbus_watch_set_data    (DBusWatch        *watch,
                                     void             *data,
                                     DBusFreeFunction  free_data_function);
DBUS_EXPORT
dbus_bool_t  dbus_watch_handle      (DBusWatch        *watch,
                                     unsigned int      flags);
DBUS_EXPORT
dbus_bool_t  dbus_watch_get_enabled (DBusWatch        *watch);

/** @} */

/**
 * @addtogroup DBusTimeout
 * @{
 */

DBUS_EXPORT
int         dbus_timeout_get_interval (DBusTimeout      *timeout);
DBUS_EXPORT
void*       dbus_timeout_get_data     (DBusTimeout      *timeout);
DBUS_EXPORT
void        dbus_timeout_set_data     (DBusTimeout      *timeout,
                                       void             *data,
                                       DBusFreeFunction  free_data_function);
DBUS_EXPORT
dbus_bool_t dbus_timeout_handle       (DBusTimeout      *timeout);
DBUS_EXPORT
dbus_bool_t dbus_timeout_get_enabled  (DBusTimeout      *timeout);

/** @} */

DBUS_END_DECLS

#endif /* DBUS_CONNECTION_H */
