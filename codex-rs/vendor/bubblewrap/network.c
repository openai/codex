/* bubblewrap
 * Copyright (C) 2016 Alexander Larsson
 * SPDX-License-Identifier: LGPL-2.0-or-later
 *
 * This program is free software; you can redistribute it and/or
 * modify it under the terms of the GNU Lesser General Public
 * License as published by the Free Software Foundation; either
 * version 2 of the License, or (at your option) any later version.
 *
 * This library is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * Lesser General Public License for more details.
 *
 * You should have received a copy of the GNU Lesser General Public
 * License along with this library. If not, see <http://www.gnu.org/licenses/>.
 *
 */

#include "config.h"

#include <arpa/inet.h>
#include <net/if.h>
#include <netinet/in.h>
#include <linux/loop.h>
#include <linux/netlink.h>
#include <linux/rtnetlink.h>

#include "utils.h"
#include "network.h"

static void *
add_rta (struct nlmsghdr *header,
         int              type,
         size_t           size)
{
  struct rtattr *rta;
  size_t rta_size = RTA_LENGTH (size);

  rta = (struct rtattr *) ((char *) header + NLMSG_ALIGN (header->nlmsg_len));
  rta->rta_type = type;
  rta->rta_len = rta_size;

  header->nlmsg_len = NLMSG_ALIGN (header->nlmsg_len) + rta_size;

  return RTA_DATA (rta);
}

static int
rtnl_send_request (int              rtnl_fd,
                   struct nlmsghdr *header)
{
  struct sockaddr_nl dst_addr = { .nl_family = AF_NETLINK, .nl_pid = 0, .nl_groups = 0 };
  ssize_t sent;

  sent = TEMP_FAILURE_RETRY (sendto (rtnl_fd, (void *) header, header->nlmsg_len, 0,
                                     (struct sockaddr *) &dst_addr, sizeof (dst_addr)));
  if (sent < 0)
    return -1;

  return 0;
}

static int
rtnl_read_reply (int          rtnl_fd,
                 unsigned int seq_nr)
{
  char buffer[1024];
  ssize_t received;
  struct nlmsghdr *rheader;

  while (1)
    {
      received = TEMP_FAILURE_RETRY (recv (rtnl_fd, buffer, sizeof (buffer), 0));
      if (received < 0)
        return -1;

      rheader = (struct nlmsghdr *) buffer;
      while (received >= NLMSG_HDRLEN)
        {
          if (rheader->nlmsg_seq != seq_nr)
            return -1;
          if ((pid_t)rheader->nlmsg_pid != getpid ())
            return -1;
          if (rheader->nlmsg_type == NLMSG_ERROR)
            {
              uint32_t *err = NLMSG_DATA (rheader);
              if (*err == 0)
                return 0;

              return -1;
            }
          if (rheader->nlmsg_type == NLMSG_DONE)
            return 0;

          rheader = NLMSG_NEXT (rheader, received);
        }
    }
}

static int
rtnl_do_request (int              rtnl_fd,
                 struct nlmsghdr *header)
{
  if (rtnl_send_request (rtnl_fd, header) != 0)
    return -1;

  if (rtnl_read_reply (rtnl_fd, header->nlmsg_seq) != 0)
    return -1;

  return 0;
}

static struct nlmsghdr *
rtnl_setup_request (char  *buffer,
                    int    type,
                    int    flags,
                    size_t size)
{
  struct nlmsghdr *header;
  size_t len = NLMSG_LENGTH (size);
  static uint32_t counter = 0;

  memset (buffer, 0, len);

  header = (struct nlmsghdr *) buffer;
  header->nlmsg_len = len;
  header->nlmsg_type = type;
  header->nlmsg_flags = flags | NLM_F_REQUEST;
  header->nlmsg_seq = counter++;
  header->nlmsg_pid = getpid ();

  return header;
}

void
loopback_setup (void)
{
  int r, if_loopback;
  cleanup_fd int rtnl_fd = -1;
  char buffer[1024];
  struct sockaddr_nl src_addr = { .nl_family = AF_NETLINK, .nl_pid = 0, .nl_groups = 0 };
  struct nlmsghdr *header;
  struct ifaddrmsg *addmsg;
  struct ifinfomsg *infomsg;
  struct in_addr *ip_addr;

  src_addr.nl_pid = getpid ();

  if_loopback = (int) if_nametoindex ("lo");
  if (if_loopback <= 0)
    die_with_error ("loopback: Failed to look up lo");

  rtnl_fd = socket (PF_NETLINK, SOCK_RAW | SOCK_CLOEXEC, NETLINK_ROUTE);
  if (rtnl_fd < 0)
    die_with_error ("loopback: Failed to create NETLINK_ROUTE socket");

  r = bind (rtnl_fd, (struct sockaddr *) &src_addr, sizeof (src_addr));
  if (r < 0)
    die_with_error ("loopback: Failed to bind NETLINK_ROUTE socket");

  header = rtnl_setup_request (buffer, RTM_NEWADDR,
                               NLM_F_CREATE | NLM_F_EXCL | NLM_F_ACK,
                               sizeof (struct ifaddrmsg));
  addmsg = NLMSG_DATA (header);

  addmsg->ifa_family = AF_INET;
  addmsg->ifa_prefixlen = 8;
  addmsg->ifa_flags = IFA_F_PERMANENT;
  addmsg->ifa_scope = RT_SCOPE_HOST;
  addmsg->ifa_index = if_loopback;

  ip_addr = add_rta (header, IFA_LOCAL, sizeof (*ip_addr));
  ip_addr->s_addr = htonl (INADDR_LOOPBACK);

  ip_addr = add_rta (header, IFA_ADDRESS, sizeof (*ip_addr));
  ip_addr->s_addr = htonl (INADDR_LOOPBACK);

  assert (header->nlmsg_len < sizeof (buffer));

  if (rtnl_do_request (rtnl_fd, header) != 0)
    die_with_error ("loopback: Failed RTM_NEWADDR");

  header = rtnl_setup_request (buffer, RTM_NEWLINK,
                               NLM_F_ACK,
                               sizeof (struct ifinfomsg));
  infomsg = NLMSG_DATA (header);

  infomsg->ifi_family = AF_UNSPEC;
  infomsg->ifi_type = 0;
  infomsg->ifi_index = if_loopback;
  infomsg->ifi_flags = IFF_UP;
  infomsg->ifi_change = IFF_UP;

  assert (header->nlmsg_len < sizeof (buffer));

  if (rtnl_do_request (rtnl_fd, header) != 0)
    die_with_error ("loopback: Failed RTM_NEWLINK");
}
