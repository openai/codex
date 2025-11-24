/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-credentials.c Credentials provable through authentication
 *
 * Copyright (C) 2007 Red Hat Inc.
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
#include <stdlib.h>
#include <string.h>
#include "dbus-credentials.h"
#include "dbus-internals.h"

/**
 * @defgroup DBusCredentials Credentials provable through authentication
 * @ingroup  DBusInternals
 * @brief DBusCredentials object
 *
 * Credentials are what you have to prove you have in order to
 * authenticate.  The main credentials right now are a unix user
 * account, a Windows user account, or a UNIX process ID.
 */

/**
 * @defgroup DBusCredentialsInternals Credentials implementation details
 * @ingroup  DBusInternals
 * @brief DBusCredentials implementation details
 *
 * Private details of credentials code.
 *
 * @{
 */

struct DBusCredentials {
  int refcount;
  dbus_uid_t unix_uid;
  dbus_gid_t *unix_gids;
  size_t n_unix_gids;
  dbus_pid_t pid;
  char *windows_sid;
  char *linux_security_label;
  void *adt_audit_data;
  dbus_int32_t adt_audit_data_size;
};

/** @} */

/**
 * @addtogroup DBusCredentials
 * @{
 */

/**
 * Creates a new credentials object.
 *
 * @returns the new object or #NULL if no memory
 */
DBusCredentials*
_dbus_credentials_new (void)
{
  DBusCredentials *creds;

  creds = dbus_new (DBusCredentials, 1);
  if (creds == NULL)
    return NULL;
  
  creds->refcount = 1;
  creds->unix_uid = DBUS_UID_UNSET;
  creds->unix_gids = NULL;
  creds->n_unix_gids = 0;
  creds->pid = DBUS_PID_UNSET;
  creds->windows_sid = NULL;
  creds->linux_security_label = NULL;
  creds->adt_audit_data = NULL;
  creds->adt_audit_data_size = 0;

  return creds;
}

/**
 * Creates a new object with the most important credentials (user ID and process ID) from the current process.
 * @returns the new object or #NULL if no memory
 */
DBusCredentials*
_dbus_credentials_new_from_current_process (void)
{
  DBusCredentials *creds;

  creds = _dbus_credentials_new ();
  if (creds == NULL)
    return NULL;

  if (!_dbus_credentials_add_from_current_process (creds))
    {
      _dbus_credentials_unref (creds);
      return NULL;
    }
  
  return creds;
}

/**
 * Increment refcount on credentials.
 *
 * @param credentials the object
 */
void
_dbus_credentials_ref (DBusCredentials *credentials)
{
  _dbus_assert (credentials->refcount > 0);
  credentials->refcount += 1;
}

/**
 * Decrement refcount on credentials.
 *
 * @param credentials the object
 */
void
_dbus_credentials_unref (DBusCredentials    *credentials)
{
  _dbus_assert (credentials->refcount > 0);

  credentials->refcount -= 1;
  if (credentials->refcount == 0)
    {
      dbus_free (credentials->unix_gids);
      dbus_free (credentials->windows_sid);
      dbus_free (credentials->linux_security_label);
      dbus_free (credentials->adt_audit_data);
      dbus_free (credentials);
    }
}

/**
 * Add a UNIX process ID to the credentials.
 *
 * @param credentials the object
 * @param pid the process ID
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_credentials_add_pid (DBusCredentials    *credentials,
                           dbus_pid_t          pid)
{
  credentials->pid = pid;
  return TRUE;
}

/**
 * Add a UNIX user ID to the credentials.
 *
 * @param credentials the object
 * @param uid the user ID
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_credentials_add_unix_uid(DBusCredentials    *credentials,
                               dbus_uid_t          uid)
{
  credentials->unix_uid = uid;
  return TRUE;

}

static int
cmp_gidp (const void *a_, const void *b_)
{
  const dbus_gid_t *a = a_;
  const dbus_gid_t *b = b_;

  if (*a < *b)
    return -1;

  if (*a > *b)
    return 1;

  return 0;
}

/**
 * Add UNIX group IDs to the credentials, replacing any group IDs that
 * might already have been present.
 *
 * @param credentials the object
 * @param gids the group IDs, which will be freed by the DBusCredentials object
 * @param n_gids the number of group IDs
 */
void
_dbus_credentials_take_unix_gids (DBusCredentials *credentials,
                                  dbus_gid_t      *gids,
                                  size_t           n_gids)
{
  /* So we can compare arrays via a simple memcmp */
  qsort (gids, n_gids, sizeof (dbus_gid_t), cmp_gidp);

  dbus_free (credentials->unix_gids);
  credentials->unix_gids = gids;
  credentials->n_unix_gids = n_gids;
}

/**
 * Get the Unix group IDs.
 *
 * @param credentials the object
 * @param gids the group IDs, which will be freed by the DBusCredentials object
 * @param n_gids the number of group IDs
 */
dbus_bool_t
_dbus_credentials_get_unix_gids (DBusCredentials   *credentials,
                                 const dbus_gid_t **gids,
                                 size_t            *n_gids)
{
  if (gids != NULL)
    *gids = credentials->unix_gids;

  if (n_gids != NULL)
    *n_gids = credentials->n_unix_gids;

  return (credentials->unix_gids != NULL);
}

/**
 * Add a Windows user SID to the credentials.
 *
 * @param credentials the object
 * @param windows_sid the user SID
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_credentials_add_windows_sid (DBusCredentials    *credentials,
                                   const char         *windows_sid)
{
  char *copy;

  copy = _dbus_strdup (windows_sid);
  if (copy == NULL)
    return FALSE;

  dbus_free (credentials->windows_sid);
  credentials->windows_sid = copy;

  return TRUE;
}

/**
 * Add a Linux security label, as used by LSMs such as SELinux, Smack and
 * AppArmor, to the credentials.
 *
 * @param credentials the object
 * @param label the label
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_credentials_add_linux_security_label (DBusCredentials    *credentials,
                                            const char         *label)
{
  char *copy;

  copy = _dbus_strdup (label);
  if (copy == NULL)
    return FALSE;

  dbus_free (credentials->linux_security_label);
  credentials->linux_security_label = copy;

  return TRUE;
}

/**
 * Add ADT audit data to the credentials.
 *
 * @param credentials the object
 * @param audit_data the audit data
 * @param size the length of audit data
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_credentials_add_adt_audit_data (DBusCredentials    *credentials,
                                      void               *audit_data,
                                      dbus_int32_t        size)
{
  void *copy;
  copy = _dbus_memdup (audit_data, size);
  if (copy == NULL)
    return FALSE;

  dbus_free (credentials->adt_audit_data);
  credentials->adt_audit_data = copy;
  credentials->adt_audit_data_size = size;

  return TRUE;
}

/**
 * Checks whether the given credential is present.
 *
 * @param credentials the object
 * @param type the credential to check for
 * @returns #TRUE if the credential is present
 */
dbus_bool_t
_dbus_credentials_include (DBusCredentials    *credentials,
                           DBusCredentialType  type)
{
  switch (type)
    {
    case DBUS_CREDENTIAL_UNIX_PROCESS_ID:
      return credentials->pid != DBUS_PID_UNSET;
    case DBUS_CREDENTIAL_UNIX_USER_ID:
      return credentials->unix_uid != DBUS_UID_UNSET;
    case DBUS_CREDENTIAL_UNIX_GROUP_IDS:
      return credentials->unix_gids != NULL;
    case DBUS_CREDENTIAL_WINDOWS_SID:
      return credentials->windows_sid != NULL;
    case DBUS_CREDENTIAL_LINUX_SECURITY_LABEL:
      return credentials->linux_security_label != NULL;
    case DBUS_CREDENTIAL_ADT_AUDIT_DATA_ID:
      return credentials->adt_audit_data != NULL;
    default:
      _dbus_assert_not_reached ("Unknown credential enum value");
      return FALSE;
    }
}

/**
 * Gets the UNIX process ID in the credentials, or #DBUS_PID_UNSET if
 * the credentials object doesn't contain a process ID.
 *
 * @param credentials the object
 * @returns UNIX process ID
 */
dbus_pid_t
_dbus_credentials_get_pid (DBusCredentials    *credentials)
{
  return credentials->pid;
}

/**
 * Gets the UNIX user ID in the credentials, or #DBUS_UID_UNSET if
 * the credentials object doesn't contain a user ID.
 *
 * @param credentials the object
 * @returns UNIX user ID
 */
dbus_uid_t
_dbus_credentials_get_unix_uid (DBusCredentials    *credentials)
{
  return credentials->unix_uid;
}

/**
 * Gets the Windows user SID in the credentials, or #NULL if
 * the credentials object doesn't contain a Windows user SID.
 *
 * @param credentials the object
 * @returns Windows user SID
 */
const char*
_dbus_credentials_get_windows_sid (DBusCredentials    *credentials)
{
  return credentials->windows_sid;
}

/**
 * Gets the Linux security label (as used by LSMs) from the credentials,
 * or #NULL if the credentials object doesn't contain a security label.
 *
 * @param credentials the object
 * @returns the security label
 */
const char *
_dbus_credentials_get_linux_security_label (DBusCredentials *credentials)
{
  return credentials->linux_security_label;
}

/**
 * Gets the ADT audit data in the credentials, or #NULL if
 * the credentials object doesn't contain ADT audit data.
 *
 * @param credentials the object
 * @returns Solaris ADT audit data 
 */
void *
_dbus_credentials_get_adt_audit_data (DBusCredentials    *credentials)
{
  return credentials->adt_audit_data;
}

/**
 * Gets the ADT audit data size in the credentials, or 0 if
 * the credentials object doesn't contain ADT audit data.
 *
 * @param credentials the object
 * @returns Solaris ADT audit data size
 */
dbus_int32_t 
_dbus_credentials_get_adt_audit_data_size (DBusCredentials    *credentials)
{
  return credentials->adt_audit_data_size;
}

/**
 * Checks whether the first credentials object contains
 * all the credentials found in the second credentials object.
 *
 * @param credentials the object
 * @param possible_subset see if credentials in here are also in the first arg
 * @returns #TRUE if second arg is contained in first
 */
dbus_bool_t
_dbus_credentials_are_superset (DBusCredentials    *credentials,
                                DBusCredentials    *possible_subset)
{
  return
    (possible_subset->pid == DBUS_PID_UNSET ||
     possible_subset->pid == credentials->pid) &&
    (possible_subset->unix_uid == DBUS_UID_UNSET ||
     possible_subset->unix_uid == credentials->unix_uid) &&
    (possible_subset->unix_gids == NULL ||
     (possible_subset->n_unix_gids == credentials->n_unix_gids &&
      memcmp (possible_subset->unix_gids, credentials->unix_gids,
              sizeof (dbus_gid_t) * credentials->n_unix_gids) == 0)) &&
    (possible_subset->windows_sid == NULL ||
     (credentials->windows_sid && strcmp (possible_subset->windows_sid,
                                          credentials->windows_sid) == 0)) &&
    (possible_subset->linux_security_label == NULL ||
     (credentials->linux_security_label != NULL &&
      strcmp (possible_subset->linux_security_label,
              credentials->linux_security_label) == 0)) &&
    (possible_subset->adt_audit_data == NULL ||
     (credentials->adt_audit_data && memcmp (possible_subset->adt_audit_data,
                                             credentials->adt_audit_data,
                                             credentials->adt_audit_data_size) == 0));
}

/**
 * Checks whether a credentials object contains anything.
 * 
 * @param credentials the object
 * @returns #TRUE if there are no credentials in the object
 */
dbus_bool_t
_dbus_credentials_are_empty (DBusCredentials    *credentials)
{
  return
    credentials->pid == DBUS_PID_UNSET &&
    credentials->unix_uid == DBUS_UID_UNSET &&
    credentials->unix_gids == NULL &&
    credentials->n_unix_gids == 0 &&
    credentials->windows_sid == NULL &&
    credentials->linux_security_label == NULL &&
    credentials->adt_audit_data == NULL;
}

/**
 * Checks whether a credentials object contains a user identity.
 * 
 * @param credentials the object
 * @returns #TRUE if there are no user identities in the object
 */
dbus_bool_t
_dbus_credentials_are_anonymous (DBusCredentials    *credentials)
{
  return
    credentials->unix_uid == DBUS_UID_UNSET &&
    credentials->windows_sid == NULL;
}

/**
 * Merge all credentials found in the second object into the first object,
 * overwriting the first object if there are any overlaps.
 * 
 * @param credentials the object
 * @param other_credentials credentials to merge
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_credentials_add_credentials (DBusCredentials    *credentials,
                                   DBusCredentials    *other_credentials)
{
  return
    _dbus_credentials_add_credential (credentials,
                                      DBUS_CREDENTIAL_UNIX_PROCESS_ID,
                                      other_credentials) &&
    _dbus_credentials_add_credential (credentials,
                                      DBUS_CREDENTIAL_UNIX_USER_ID,
                                      other_credentials) &&
    _dbus_credentials_add_credential (credentials,
                                      DBUS_CREDENTIAL_UNIX_GROUP_IDS,
                                      other_credentials) &&
    _dbus_credentials_add_credential (credentials,
                                      DBUS_CREDENTIAL_ADT_AUDIT_DATA_ID,
                                      other_credentials) &&
    _dbus_credentials_add_credential (credentials,
                                      DBUS_CREDENTIAL_LINUX_SECURITY_LABEL,
                                      other_credentials) &&
    _dbus_credentials_add_credential (credentials,
                                      DBUS_CREDENTIAL_WINDOWS_SID,
                                      other_credentials);
}

/**
 * Merge the given credential found in the second object into the first object,
 * overwriting the first object's value for that credential.
 *
 * Does nothing if the second object does not contain the specified credential.
 * i.e., will never delete a credential from the first object.
 * 
 * @param credentials the object
 * @param which the credential to overwrite
 * @param other_credentials credentials to merge
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_credentials_add_credential (DBusCredentials    *credentials,
                                  DBusCredentialType  which,
                                  DBusCredentials    *other_credentials)
{
  if (which == DBUS_CREDENTIAL_UNIX_PROCESS_ID &&
      other_credentials->pid != DBUS_PID_UNSET)
    {
      if (!_dbus_credentials_add_pid (credentials, other_credentials->pid))
        return FALSE;
    }
  else if (which == DBUS_CREDENTIAL_UNIX_USER_ID &&
           other_credentials->unix_uid != DBUS_UID_UNSET)
    {
      if (!_dbus_credentials_add_unix_uid (credentials, other_credentials->unix_uid))
        return FALSE;
    }
  else if (which == DBUS_CREDENTIAL_UNIX_GROUP_IDS &&
           other_credentials->unix_gids != NULL)
    {
      dbus_gid_t *gids;

      gids = dbus_new (dbus_gid_t, other_credentials->n_unix_gids);

      if (gids == NULL)
        return FALSE;

      memcpy (gids, other_credentials->unix_gids,
              sizeof (dbus_gid_t) * other_credentials->n_unix_gids);

      _dbus_credentials_take_unix_gids (credentials, gids,
                                        other_credentials->n_unix_gids);
    }
  else if (which == DBUS_CREDENTIAL_WINDOWS_SID &&
           other_credentials->windows_sid != NULL)
    {
      if (!_dbus_credentials_add_windows_sid (credentials, other_credentials->windows_sid))
        return FALSE;
    } 
  else if (which == DBUS_CREDENTIAL_LINUX_SECURITY_LABEL &&
           other_credentials->linux_security_label != NULL)
    {
      if (!_dbus_credentials_add_linux_security_label (credentials,
            other_credentials->linux_security_label))
        return FALSE;
    }
  else if (which == DBUS_CREDENTIAL_ADT_AUDIT_DATA_ID &&
           other_credentials->adt_audit_data != NULL) 
    {
      if (!_dbus_credentials_add_adt_audit_data (credentials, other_credentials->adt_audit_data, other_credentials->adt_audit_data_size))
        return FALSE;
    }

  return TRUE;
}

/**
 * Clear all credentials in the object.
 * 
 * @param credentials the object
 */
void
_dbus_credentials_clear (DBusCredentials    *credentials)
{
  credentials->pid = DBUS_PID_UNSET;
  credentials->unix_uid = DBUS_UID_UNSET;
  dbus_free (credentials->unix_gids);
  credentials->unix_gids = NULL;
  credentials->n_unix_gids = 0;
  dbus_free (credentials->windows_sid);
  credentials->windows_sid = NULL;
  dbus_free (credentials->linux_security_label);
  credentials->linux_security_label = NULL;
  dbus_free (credentials->adt_audit_data);
  credentials->adt_audit_data = NULL;
  credentials->adt_audit_data_size = 0;
}

/**
 * Copy a credentials object.
 * 
 * @param credentials the object
 * @returns the copy or #NULL
 */
DBusCredentials*
_dbus_credentials_copy (DBusCredentials    *credentials)
{
  DBusCredentials *copy;

  copy = _dbus_credentials_new ();
  if (copy == NULL)
    return NULL;

  if (!_dbus_credentials_add_credentials (copy, credentials))
    {
      _dbus_credentials_unref (copy);
      return NULL;
    }

  return copy;
}

/**
 * Check whether the user-identifying credentials in two credentials
 * objects are identical. Credentials that are not related to the
 * user are ignored, but any kind of user ID credentials must be the
 * same (UNIX user ID, Windows user SID, etc.) and present in both
 * objects for the function to return #TRUE.
 * 
 * @param credentials the object
 * @param other_credentials credentials to compare
 * @returns #TRUE if the two credentials refer to the same user
 */
dbus_bool_t
_dbus_credentials_same_user (DBusCredentials    *credentials,
                             DBusCredentials    *other_credentials)
{
  /* both windows and unix user must be the same (though pretty much
   * in all conceivable cases, one will be unset)
   */
  return credentials->unix_uid == other_credentials->unix_uid &&
    ((!(credentials->windows_sid || other_credentials->windows_sid)) ||
     (credentials->windows_sid && other_credentials->windows_sid &&
      strcmp (credentials->windows_sid, other_credentials->windows_sid) == 0));
}

/**
 * Convert the credentials in this object to a human-readable
 * string format, and append to the given string.
 *
 * @param credentials the object
 * @param string append to this string
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_credentials_to_string_append (DBusCredentials    *credentials,
                                    DBusString         *string)
{
  dbus_bool_t join;

  join = FALSE;
  if (credentials->unix_uid != DBUS_UID_UNSET)
    {
      if (!_dbus_string_append_printf (string, "uid=" DBUS_UID_FORMAT, credentials->unix_uid))
        goto oom;
      join = TRUE;
    }
  if (credentials->pid != DBUS_PID_UNSET)
    {
      if (!_dbus_string_append_printf (string, "%spid=" DBUS_PID_FORMAT, join ? " " : "", credentials->pid))
        goto oom;
      join = TRUE;
    }

  if (credentials->unix_gids != NULL)
    {
      size_t i;

      for (i = 0; i < credentials->n_unix_gids; i++)
        {
          if (!_dbus_string_append_printf (string, "%sgid=" DBUS_GID_FORMAT,
                                           join ? " " : "",
                                           credentials->unix_gids[i]))
            goto oom;

          join = TRUE;
        }
    }

  if (credentials->windows_sid != NULL)
    {
      if (!_dbus_string_append_printf (string, "%ssid=%s", join ? " " : "", credentials->windows_sid))
        goto oom;
      join = TRUE;
    }

  if (credentials->linux_security_label != NULL)
    {
      if (!_dbus_string_append_printf (string, "%slsm='%s'",
                                       join ? " " : "",
                                       credentials->linux_security_label))
        goto oom;
      join = TRUE;
    }

  return TRUE;
oom:
  return FALSE;
}

/** @} */

/* tests in dbus-credentials-util.c */
