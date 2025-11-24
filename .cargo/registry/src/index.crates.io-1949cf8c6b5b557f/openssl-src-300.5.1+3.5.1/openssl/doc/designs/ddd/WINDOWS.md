Windows-related issues
======================

Supporting Windows introduces some complications due to some "fun" peculiarities
of Windows socket API.

In general, Windows does not provide a poll(2) system call. WSAPoll(2) was introduced
in Vista and was supposed to bring this functionality, but it had a bug in it which
Microsoft refused to fix, making it rather pointless. However, Microsoft has now
finally fixed this bug in a build of Windows 10. So WSAPoll(2) is a viable
method, but only on fairly new versions of Windows.

Traditionally, polling has been done on windows using select(). However, this
call works a little differently than on POSIX platforms. Whereas on POSIX
platforms select() accepts a bitmask of FDs, on Windows select() accepts a
structure which embeds a fixed-length array of socket handles. This is necessary
because sockets are NT kernel handles on Windows and thus are not allocated
contiguously like FDs. As such, Windows select() is actually very similar to
POSIX poll(), making select() a viable option for polling on Windows.

Neither select() nor poll() are, of course, high performance polling options.
Windows does not provide anything like epoll or kqueue. For high performance
network I/O, you are expected to use a Windows API called I/O Completion Ports
(IOCP).

Supporting these can be a pain for applications designed around polling. The reason
is that IOCPs are a higher-level interface; it is easy to build an IOCP-like
interface on top of polling, but it is not really possible to build a
polling-like interface on top of IOCPs.

For this reason it's actually common for asynchronous I/O libraries to basically
contain two separate implementations of their APIs internally, or at least a
substantial chunk of their code (e.g. libuv, nanomsg). It turns out to be easier
just to write a poll-based implementation of an I/O reactor and an IOCP-based
implementation than try to overcome the impedance discontinuities.

The difference between polling and IOCPs is that polling reports *readiness*
whereas IOCPs report *completion of an operation*. For example, in the IOCP
model, you make a read or write on a socket and an event is posted to the IOCP
when the read or write is complete. This is a fundamentally different model and
actually more similar to a high-level asynchronous I/O library such as libuv or
so on.

Evaluation of the existing demos and their applicability to Windows IOCP:

- ddd-01-conn-blocking: Blocking example, use of IOCP is not applicable.

- ddd-02-conn-nonblocking: Socket is managed by OpenSSL, and IOCP is not
  supported.

- ddd-03-fd-blocking: Blocking example, use of IOCP is not applicable.

- ddd-04-fd-nonblocking: libssl is passed an FD with BIO_set_fd.

  BIO_s_sock doesn't appear to support overlapped (that is, IOCP-based) I/O
  as this requires use of special WSASend() and WSARecv() functions, rather
  than standard send()/recv().

  Since libssl already doesn't support IOCP for use of BIO_s_sock,
  we might say here that any existing application using BIO_s_sock
  obviously isn't trying to use IOCP, and therefore we don't need to
  worry about the adapability of this example to IOCP.

- ddd-05-mem-nonblocking: Since the application is in full control of passing
  data from the memory BIO to the network, or vice versa, the application
  can use IOCP if it wishes.

  This is demonstrated in the following demo:

- ddd-06-mem-uv: This demo uses a memory BIO and libuv. Since libuv supports
  IOCP, it proves that a memory BIO can be used to support IOCP-based usage.

Further, a cursory examination of code on GitHub seems to suggest that when
people do use IOCP with libssl, they do it using memory BIOs passed to libssl.
So ddd-05 and ddd-06 essentially demonstrate this use case, especially ddd-06 as
it uses IOCP internally on Windows.

My conclusion here is that since libssl does not support IOCP in the first
place, we don't need to be particularly worried about this. But in the worst
case there are always workable solutions, as in demos 5 and 6.
