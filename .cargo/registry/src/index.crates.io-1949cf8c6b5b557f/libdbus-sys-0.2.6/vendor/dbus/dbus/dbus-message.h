/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-message.h DBusMessage object
 *
 * Copyright (C) 2002, 2003, 2005 Red Hat Inc.
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

#ifndef DBUS_MESSAGE_H
#define DBUS_MESSAGE_H

#include <dbus/dbus-macros.h>
#include <dbus/dbus-types.h>
#include <dbus/dbus-arch-deps.h>
#include <dbus/dbus-memory.h>
#include <dbus/dbus-errors.h>
#include <stdarg.h>

DBUS_BEGIN_DECLS

/**
 * @addtogroup DBusMessage
 * @{
 */

typedef struct DBusMessage DBusMessage;
/**
 * Opaque type representing a message iterator. Can be copied by value and
 * allocated on the stack.
 *
 * A DBusMessageIter usually contains no allocated memory. However, there
 * is one special case: after a successful call to
 * dbus_message_iter_open_container(), the caller is responsible for calling
 * either dbus_message_iter_close_container() or
 * dbus_message_iter_abandon_container() exactly once, with the same pair
 * of iterators.
 */
typedef struct DBusMessageIter DBusMessageIter;

/**
 * DBusMessageIter struct; contains no public fields. 
 */
struct DBusMessageIter
{ 
  void *dummy1;         /**< Don't use this */
  void *dummy2;         /**< Don't use this */
  dbus_uint32_t dummy3; /**< Don't use this */
  int dummy4;           /**< Don't use this */
  int dummy5;           /**< Don't use this */
  int dummy6;           /**< Don't use this */
  int dummy7;           /**< Don't use this */
  int dummy8;           /**< Don't use this */
  int dummy9;           /**< Don't use this */
  int dummy10;          /**< Don't use this */
  int dummy11;          /**< Don't use this */
  int pad1;             /**< Don't use this */
  void *pad2;           /**< Don't use this */
  void *pad3;           /**< Don't use this */
};

/**
 * A message iterator for which dbus_message_iter_abandon_container_if_open()
 * is the only valid operation.
 */
#define DBUS_MESSAGE_ITER_INIT_CLOSED \
{ \
  NULL, /* dummy1 */ \
  NULL, /* dummy2 */ \
  0, /* dummy3 */ \
  0, /* dummy4 */ \
  0, /* dummy5 */ \
  0, /* dummy6 */ \
  0, /* dummy7 */ \
  0, /* dummy8 */ \
  0, /* dummy9 */ \
  0, /* dummy10 */ \
  0, /* dummy11 */ \
  0, /* pad1 */ \
  NULL, /* pad2 */ \
  NULL /* pad3 */ \
}

DBUS_EXPORT
DBusMessage* dbus_message_new               (int          message_type);
DBUS_EXPORT
DBusMessage* dbus_message_new_method_call   (const char  *bus_name,
                                             const char  *path,
                                             const char  *iface,
                                             const char  *method);
DBUS_EXPORT
DBusMessage* dbus_message_new_method_return (DBusMessage *method_call);
DBUS_EXPORT
DBusMessage* dbus_message_new_signal        (const char  *path,
                                             const char  *iface,
                                             const char  *name);
DBUS_EXPORT
DBusMessage* dbus_message_new_error         (DBusMessage *reply_to,
                                             const char  *error_name,
                                             const char  *error_message);
DBUS_EXPORT
DBusMessage* dbus_message_new_error_printf  (DBusMessage *reply_to,
                                             const char  *error_name,
                                             const char  *error_format,
                                             ...) _DBUS_GNUC_PRINTF (3, 4);

DBUS_EXPORT
DBusMessage* dbus_message_copy              (const DBusMessage *message);

DBUS_EXPORT
DBusMessage*  dbus_message_ref              (DBusMessage   *message);
DBUS_EXPORT
void          dbus_message_unref            (DBusMessage   *message);
DBUS_EXPORT
int           dbus_message_get_type         (DBusMessage   *message);
DBUS_EXPORT
dbus_bool_t   dbus_message_set_path         (DBusMessage   *message,
                                             const char    *object_path);
DBUS_EXPORT
const char*   dbus_message_get_path         (DBusMessage   *message);
DBUS_EXPORT
dbus_bool_t   dbus_message_has_path         (DBusMessage   *message, 
                                             const char    *object_path);  
DBUS_EXPORT
dbus_bool_t   dbus_message_set_interface    (DBusMessage   *message,
                                             const char    *iface);
DBUS_EXPORT
const char*   dbus_message_get_interface    (DBusMessage   *message);
DBUS_EXPORT
dbus_bool_t   dbus_message_has_interface    (DBusMessage   *message, 
                                             const char    *iface);
DBUS_EXPORT
dbus_bool_t   dbus_message_set_member       (DBusMessage   *message,
                                             const char    *member);
DBUS_EXPORT
const char*   dbus_message_get_member       (DBusMessage   *message);
DBUS_EXPORT
dbus_bool_t   dbus_message_has_member       (DBusMessage   *message, 
                                             const char    *member);
DBUS_EXPORT
dbus_bool_t   dbus_message_set_error_name   (DBusMessage   *message,
                                             const char    *name);
DBUS_EXPORT
const char*   dbus_message_get_error_name   (DBusMessage   *message);
DBUS_EXPORT
dbus_bool_t   dbus_message_set_destination  (DBusMessage   *message,
                                             const char    *destination);
DBUS_EXPORT
const char*   dbus_message_get_destination  (DBusMessage   *message);
DBUS_EXPORT
dbus_bool_t   dbus_message_set_sender       (DBusMessage   *message,
                                             const char    *sender);
DBUS_EXPORT
const char*   dbus_message_get_sender       (DBusMessage   *message);
DBUS_EXPORT
const char*   dbus_message_get_signature    (DBusMessage   *message);
DBUS_EXPORT
void          dbus_message_set_no_reply     (DBusMessage   *message,
                                             dbus_bool_t    no_reply);
DBUS_EXPORT
dbus_bool_t   dbus_message_get_no_reply     (DBusMessage   *message);
DBUS_EXPORT
dbus_bool_t   dbus_message_is_method_call   (DBusMessage   *message,
                                             const char    *iface,
                                             const char    *method);
DBUS_EXPORT
dbus_bool_t   dbus_message_is_signal        (DBusMessage   *message,
                                             const char    *iface,
                                             const char    *signal_name);
DBUS_EXPORT
dbus_bool_t   dbus_message_is_error         (DBusMessage   *message,
                                             const char    *error_name);
DBUS_EXPORT
dbus_bool_t   dbus_message_has_destination  (DBusMessage   *message,
                                             const char    *bus_name);
DBUS_EXPORT
dbus_bool_t   dbus_message_has_sender       (DBusMessage   *message,
                                             const char    *unique_bus_name);
DBUS_EXPORT
dbus_bool_t   dbus_message_has_signature    (DBusMessage   *message,
                                             const char    *signature);
DBUS_EXPORT
dbus_uint32_t dbus_message_get_serial       (DBusMessage   *message);
DBUS_EXPORT
void          dbus_message_set_serial       (DBusMessage   *message, 
                                             dbus_uint32_t  serial);
DBUS_EXPORT
dbus_bool_t   dbus_message_set_reply_serial (DBusMessage   *message,
                                             dbus_uint32_t  reply_serial);
DBUS_EXPORT
dbus_uint32_t dbus_message_get_reply_serial (DBusMessage   *message);

DBUS_EXPORT
void          dbus_message_set_auto_start   (DBusMessage   *message,
                                             dbus_bool_t    auto_start);
DBUS_EXPORT
dbus_bool_t   dbus_message_get_auto_start   (DBusMessage   *message);

DBUS_EXPORT
dbus_bool_t   dbus_message_get_path_decomposed (DBusMessage   *message,
                                                char        ***path);

DBUS_EXPORT
const char   *dbus_message_get_container_instance (DBusMessage   *message);
DBUS_EXPORT
dbus_bool_t   dbus_message_set_container_instance (DBusMessage   *message,
                                                   const char    *object_path);

DBUS_EXPORT
dbus_bool_t dbus_message_append_args          (DBusMessage     *message,
					       int              first_arg_type,
					       ...);
DBUS_EXPORT
dbus_bool_t dbus_message_append_args_valist   (DBusMessage     *message,
					       int              first_arg_type,
					       va_list          var_args);
DBUS_EXPORT
dbus_bool_t dbus_message_get_args             (DBusMessage     *message,
					       DBusError       *error,
					       int              first_arg_type,
					       ...);
DBUS_EXPORT
dbus_bool_t dbus_message_get_args_valist      (DBusMessage     *message,
					       DBusError       *error,
					       int              first_arg_type,
					       va_list          var_args);

DBUS_EXPORT
dbus_bool_t dbus_message_contains_unix_fds    (DBusMessage *message);

DBUS_EXPORT
void        dbus_message_iter_init_closed        (DBusMessageIter *iter);
DBUS_EXPORT
dbus_bool_t dbus_message_iter_init             (DBusMessage     *message,
                                                DBusMessageIter *iter);
DBUS_EXPORT
dbus_bool_t dbus_message_iter_has_next         (DBusMessageIter *iter);
DBUS_EXPORT
dbus_bool_t dbus_message_iter_next             (DBusMessageIter *iter);
DBUS_EXPORT
char*       dbus_message_iter_get_signature    (DBusMessageIter *iter);
DBUS_EXPORT
int         dbus_message_iter_get_arg_type     (DBusMessageIter *iter);
DBUS_EXPORT
int         dbus_message_iter_get_element_type (DBusMessageIter *iter);
DBUS_EXPORT
void        dbus_message_iter_recurse          (DBusMessageIter *iter,
                                                DBusMessageIter *sub);
DBUS_EXPORT
void        dbus_message_iter_get_basic        (DBusMessageIter *iter,
                                                void            *value);
DBUS_EXPORT
int         dbus_message_iter_get_element_count(DBusMessageIter *iter);

#ifndef DBUS_DISABLE_DEPRECATED
/* This function returns the wire protocol size of the array in bytes,
 * you do not want to know that probably
 */
DBUS_EXPORT
DBUS_DEPRECATED int         dbus_message_iter_get_array_len    (DBusMessageIter *iter);
#endif
DBUS_EXPORT
void        dbus_message_iter_get_fixed_array  (DBusMessageIter *iter,
                                                void            *value,
                                                int             *n_elements);


DBUS_EXPORT
void        dbus_message_iter_init_append        (DBusMessage     *message,
                                                  DBusMessageIter *iter);
DBUS_EXPORT
dbus_bool_t dbus_message_iter_append_basic       (DBusMessageIter *iter,
                                                  int              type,
                                                  const void      *value);
DBUS_EXPORT
dbus_bool_t dbus_message_iter_append_fixed_array (DBusMessageIter *iter,
                                                  int              element_type,
                                                  const void      *value,
                                                  int              n_elements);
DBUS_EXPORT
dbus_bool_t dbus_message_iter_open_container     (DBusMessageIter *iter,
                                                  int              type,
                                                  const char      *contained_signature,
                                                  DBusMessageIter *sub);
DBUS_EXPORT
dbus_bool_t dbus_message_iter_close_container    (DBusMessageIter *iter,
                                                  DBusMessageIter *sub);
DBUS_EXPORT
void        dbus_message_iter_abandon_container  (DBusMessageIter *iter,
                                                  DBusMessageIter *sub);

DBUS_EXPORT
void        dbus_message_iter_abandon_container_if_open (DBusMessageIter *iter,
                                                         DBusMessageIter *sub);

DBUS_EXPORT
void dbus_message_lock    (DBusMessage  *message);

DBUS_EXPORT
dbus_bool_t  dbus_set_error_from_message  (DBusError    *error,
                                           DBusMessage  *message);


DBUS_EXPORT
dbus_bool_t dbus_message_allocate_data_slot (dbus_int32_t     *slot_p);
DBUS_EXPORT
void        dbus_message_free_data_slot     (dbus_int32_t     *slot_p);
DBUS_EXPORT
dbus_bool_t dbus_message_set_data           (DBusMessage      *message,
                                             dbus_int32_t      slot,
                                             void             *data,
                                             DBusFreeFunction  free_data_func);
DBUS_EXPORT
void*       dbus_message_get_data           (DBusMessage      *message,
                                             dbus_int32_t      slot);

DBUS_EXPORT
int         dbus_message_type_from_string (const char *type_str);
DBUS_EXPORT
const char* dbus_message_type_to_string   (int type);

DBUS_EXPORT
dbus_bool_t  dbus_message_marshal   (DBusMessage  *msg,
                                     char        **marshalled_data_p,
                                     int          *len_p);
DBUS_EXPORT
DBusMessage* dbus_message_demarshal (const char *str,
                                     int         len,
                                     DBusError  *error);

DBUS_EXPORT
int          dbus_message_demarshal_bytes_needed (const char *str, 
                                                  int len);

DBUS_EXPORT
void dbus_message_set_allow_interactive_authorization (DBusMessage *message,
    dbus_bool_t allow);

DBUS_EXPORT
dbus_bool_t dbus_message_get_allow_interactive_authorization (
    DBusMessage *message);

/**
 * Clear a variable or struct member that contains a #DBusMessage.
 * If it does not contain #NULL, the message that was previously
 * there is unreferenced with dbus_message_unref().
 *
 * This is very similar to dbus_clear_connection(): see that function
 * for more details.
 *
 * @param pointer_to_message A pointer to a variable or struct member.
 * pointer_to_message must not be #NULL, but *pointer_to_message
 * may be #NULL.
 */
static inline void
dbus_clear_message (DBusMessage **pointer_to_message)
{
  _dbus_clear_pointer_impl (DBusMessage, pointer_to_message,
                            dbus_message_unref);
}

/** @} */

DBUS_END_DECLS

#endif /* DBUS_MESSAGE_H */
