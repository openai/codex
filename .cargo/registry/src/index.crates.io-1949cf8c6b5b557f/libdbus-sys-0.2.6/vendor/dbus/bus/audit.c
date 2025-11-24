/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*-
 * audit.c - libaudit integration for SELinux and AppArmor
 *
 * Based on apparmor.c, selinux.c
 *
 * Copyright © 2014-2015 Canonical, Ltd.
 * Copyright © 2015 Collabora Ltd.
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
#include "audit.h"

#ifdef HAVE_ERRNO_H
#include <errno.h>
#endif

#ifdef HAVE_LIBAUDIT
#include <cap-ng.h>
#include <libaudit.h>
#endif

#include <dbus/dbus-internals.h>
#ifdef DBUS_UNIX
#include <dbus/dbus-userdb.h>
#endif

#ifdef HAVE_LIBAUDIT
static int audit_fd = -1;
#endif

/**
 * Open the libaudit fd if appropriate.
 */
void
bus_audit_init (BusContext *context)
{
#ifdef HAVE_LIBAUDIT
  int i;

  if (audit_fd >= 0)
    return;

  capng_get_caps_process ();

  /* Work around a bug in libcap-ng < 0.7.7: it leaks a fd, which isn't
   * close-on-exec. Assume it will be one of the first few fds. */
  for (i = 3; i < 42; i++)
    _dbus_fd_set_close_on_exec (i);

  if (!capng_have_capability (CAPNG_EFFECTIVE, CAP_AUDIT_WRITE))
    return;

  audit_fd = audit_open ();

  if (audit_fd < 0)
    {
      int e = errno;

      /* If kernel doesn't support audit, bail out */
      if (e == EINVAL || e == EPROTONOSUPPORT || e == EAFNOSUPPORT)
        return;

      bus_context_log (context, DBUS_SYSTEM_LOG_WARNING,
                       "Failed to open connection to the audit subsystem: %s",
                       _dbus_strerror (e));
    }
#endif /* HAVE_LIBAUDIT */
}

/**
 * If libaudit is in use and it would be appropriate to write audit records,
 * return the libaudit fd. Otherwise return -1.
 */
int
bus_audit_get_fd (void)
{
#ifdef HAVE_LIBAUDIT
  if (audit_fd >= 0)
  {
    return audit_fd;
  }
#endif

  return -1;
}

/**
 * Close the libaudit fd.
 */
void
bus_audit_shutdown (void)
{
#ifdef HAVE_LIBAUDIT
  if (audit_fd >= 0)
    {
      audit_close (audit_fd);
      audit_fd = -1;
    }
#endif /* HAVE_LIBAUDIT */
}

/* The !HAVE_LIBAUDIT case lives in dbus-sysdeps-util-unix.c */
#ifdef HAVE_LIBAUDIT
/**
 * Changes the user and group the bus is running as.
 *
 * @param user the user to become
 * @param error return location for errors
 * @returns #FALSE on failure
 */
dbus_bool_t
_dbus_change_to_daemon_user  (const char    *user,
                              DBusError     *error)
{
  dbus_uid_t uid;
  dbus_gid_t gid;
  DBusString u;

  _dbus_string_init_const (&u, user);

  if (!_dbus_get_user_id_and_primary_group (&u, &uid, &gid))
    {
      dbus_set_error (error, DBUS_ERROR_FAILED,
                      "User '%s' does not appear to exist?",
                      user);
      return FALSE;
    }

  /* If we were root */
  if (_dbus_geteuid () == 0)
    {
      int rc;
      int have_audit_write;

      have_audit_write = capng_have_capability (CAPNG_PERMITTED, CAP_AUDIT_WRITE);
      capng_clear (CAPNG_SELECT_BOTH);
      /* Only attempt to retain CAP_AUDIT_WRITE if we had it when
       * starting.  See:
       * https://bugs.freedesktop.org/show_bug.cgi?id=49062#c9
       */
      if (have_audit_write)
        capng_update (CAPNG_ADD, CAPNG_EFFECTIVE | CAPNG_PERMITTED,
                      CAP_AUDIT_WRITE);
      rc = capng_change_id (uid, gid, CAPNG_DROP_SUPP_GRP);
      if (rc)
        {
          switch (rc) {
            default:
              dbus_set_error (error, DBUS_ERROR_FAILED,
                              "Failed to drop capabilities: %s\n",
                              _dbus_strerror (errno));
              break;
            case -4:
              dbus_set_error (error, _dbus_error_from_errno (errno),
                              "Failed to set GID to %lu: %s", gid,
                              _dbus_strerror (errno));
              break;
            case -5:
              dbus_set_error (error, _dbus_error_from_errno (errno),
                              "Failed to drop supplementary groups: %s",
                              _dbus_strerror (errno));
              break;
            case -6:
              dbus_set_error (error, _dbus_error_from_errno (errno),
                              "Failed to set UID to %lu: %s", uid,
                              _dbus_strerror (errno));
              break;
            case -7:
              dbus_set_error (error, _dbus_error_from_errno (errno),
                              "Failed to unset keep-capabilities: %s\n",
                              _dbus_strerror (errno));
              break;
          }
          return FALSE;
        }
    }

 return TRUE;
}
#endif
