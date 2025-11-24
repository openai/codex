/* dbus-server-launchd.c Server methods for interacting with launchd.
 * Copyright (C) 2007, Tanner Lovelace <lovelace@wayfarer.org>
 * Copyright (C) 2008, Colin Walters <walters@verbum.org>
 * Copyright (C) 2008-2009, Benjamin Reed <rangerrick@befunk.com>
 * Copyright (C) 2009, Jonas Bähr <jonas.baehr@web.de>
 *
 * Permission is hereby granted, free of charge, to any person
 * obtaining a copy of this software and associated documentation
 * files (the "Software"), to deal in the Software without
 * restriction, including without limitation the rights to use, copy,
 * modify, merge, publish, distribute, sublicense, and/or sell copies
 * of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be
 * included in all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
 * EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
 * MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
 * NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT
 * HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
 * WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
 * DEALINGS IN THE SOFTWARE.
 */

#include <config.h>
#include "dbus-server-launchd.h"

/**
 * @defgroup DBusServerLaunchd DBusServer implementations for Launchd
 * @ingroup  DBusInternals
 * @brief Implementation details of DBusServer with Launchd support
 *
 * @{
 */

#ifdef DBUS_ENABLE_LAUNCHD
#include <launch.h>
#include <errno.h>
#include <unistd.h>

#include "dbus-misc.h"
#include "dbus-server-socket.h"
#include "dbus-sysdeps-unix.h"

/* put other private launchd functions here */

#endif /* DBUS_ENABLE_LAUNCHD */

/**
 * @brief Creates a new server from launchd.
 *
 * launchd has allocaed a socket for us. We now query launchd for the
 * file descriptor of this socket and create a server on it.
 * In addition we inherit launchd's environment which holds a variable
 * containing the path to the socket. This is used to init the server's
 * address which is passed to autolaunched services.
 *
 * @param launchd_env_var the environment variable which holds the unix path to the socket
 * @param error location to store reason for failure.
 * @returns the new server, or #NULL on failure.
 */

DBusServer *
_dbus_server_new_for_launchd (const char *launchd_env_var, DBusError * error)
  {
#ifdef DBUS_ENABLE_LAUNCHD
    DBusServer *server;
    DBusString address;
    int launchd_fd = -1;
    launch_data_t sockets_dict, checkin_response;
    launch_data_t checkin_request;
    launch_data_t listening_fd_array, listening_fd;
    launch_data_t environment_dict, environment_param;
    const char *launchd_socket_path, *display;

    launchd_socket_path = _dbus_getenv (launchd_env_var);
    display = _dbus_getenv ("DISPLAY");

    _DBUS_ASSERT_ERROR_IS_CLEAR (error);

    if (launchd_socket_path == NULL || *launchd_socket_path == '\0')
      {
        dbus_set_error (error, DBUS_ERROR_BAD_ADDRESS,
                        "launchd's environment variable %s is empty, but should contain a socket path.\n", launchd_env_var);
        return NULL;
      }

    if (!_dbus_string_init (&address))
      {
        dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
        return NULL;
      }
    if (!_dbus_string_append (&address, "unix:path="))
      {
        dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
        goto l_failed_0;
      }
    if (!_dbus_string_append (&address, launchd_socket_path))
      {
        dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
        goto l_failed_0;
      }

    if ((checkin_request = launch_data_new_string (LAUNCH_KEY_CHECKIN)) == NULL)
      {
        dbus_set_error (error, DBUS_ERROR_NO_MEMORY,
                        "launch_data_new_string(\"%s\") Unable to create string.\n",
                        LAUNCH_KEY_CHECKIN);
        goto l_failed_0;
      }

    if ((checkin_response = launch_msg (checkin_request)) == NULL)
      {
        dbus_set_error (error, DBUS_ERROR_IO_ERROR,
                        "launch_msg(\"%s\") IPC failure: %s\n",
                        LAUNCH_KEY_CHECKIN, strerror (errno));
        goto l_failed_0;
      }

    if (LAUNCH_DATA_ERRNO == launch_data_get_type (checkin_response))
      {
        dbus_set_error (error, DBUS_ERROR_FAILED, "Check-in failed: %s\n",
                        strerror (launch_data_get_errno (checkin_response)));
        goto l_failed_0;
      }

    sockets_dict =
      launch_data_dict_lookup (checkin_response, LAUNCH_JOBKEY_SOCKETS);
    if (NULL == sockets_dict)
      {
        dbus_set_error (error, DBUS_ERROR_IO_ERROR,
                        "No sockets found to answer requests on!\n");
        goto l_failed_0;
      }

    listening_fd_array =
      launch_data_dict_lookup (sockets_dict, "unix_domain_listener");
    if (NULL == listening_fd_array)
      {
        dbus_set_error (error, DBUS_ERROR_IO_ERROR,
                        "No known sockets found to answer requests on!\n");
        goto l_failed_0;
      }

    if (launch_data_array_get_count (listening_fd_array) != 1)
      {
        dbus_set_error (error, DBUS_ERROR_LIMITS_EXCEEDED,
                        "Expected 1 socket from launchd, got %d.\n",
                        launch_data_array_get_count (listening_fd_array));
        goto l_failed_0;
      }

    listening_fd = launch_data_array_get_index (listening_fd_array, 0);
    launchd_fd = launch_data_get_fd (listening_fd);

    _dbus_fd_set_close_on_exec (launchd_fd);

    if (launchd_fd < 0)
      {
        _DBUS_ASSERT_ERROR_IS_SET (error);
        goto l_failed_0;
  if (display == NULL || *display == '\0')
    {
      environment_dict = launch_data_dict_lookup (checkin_response, LAUNCH_JOBKEY_USERENVIRONMENTVARIABLES);
      if (NULL == environment_dict)
        {
          _dbus_warn ("Unable to retrieve user environment from launchd.");
        }
      else
        {
          environment_param = launch_data_dict_lookup (environment_dict, "DISPLAY");
          if (NULL == environment_param)
            {
              _dbus_warn ("Unable to retrieve DISPLAY from launchd.");
            }
          else
            {
              display = launch_data_get_string(environment_param);
              dbus_setenv ("DISPLAY", display);
            }
        }
    }

      }

    server = _dbus_server_new_for_socket (&launchd_fd, 1, &address, 0, error);
    if (server == NULL)
      {
        goto l_failed_0;
      }

    _dbus_string_free (&address);

    return server;

  l_failed_0:
    if (launchd_fd >= 0)
      close (launchd_fd);

    _dbus_string_free (&address);

    return NULL;
#else /* DBUS_ENABLE_LAUNCHD */
    dbus_set_error (error, DBUS_ERROR_BAD_ADDRESS,
                    "address type 'launchd' requested, but launchd support not compiled in");
    return NULL;
#endif
  }

/** @} */
