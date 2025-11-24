Error handling in QUIC code
===========================

Current situation with TLS
--------------------------

The errors are put on the error stack (rather a queue but error stack is
used throughout the code base) during the libssl API calls. In most
(if not all) cases they should appear there only if the API call returns an
error return value. The `SSL_get_error()` call depends on the stack being
clean before the API call to be properly able to determine if the API
call caused a library or system (I/O) error.

The error stacks are thread-local. Libssl API calls from separate threads
push errors to these separate error stacks. It is unusual to invoke libssl
APIs with the same SSL object from different threads, but even if it happens,
it is not a problem as applications are supposed to check for errors
immediately after the API call on the same thread. There is no such thing as
Thread-assisted mode of operation.

Constraints
-----------

We need to keep using the existing ERR API as doing otherwise would
complicate the existing applications and break our API compatibility promise.
Even the ERR_STATE structure is public, although deprecated, and thus its
structure and semantics cannot be changed.

The error stack access is not under a lock (because it is thread-local).
This complicates _moving errors between threads_.

Error stack entries contain allocated data, copying entries between threads
implies duplicating it or losing it.

Assumptions
-----------

This document assumes the actual error state of the QUIC connection (or stream
for stream level errors) is handled separately from the auxiliary error reason
entries on the error stack.

We can assume the internal assistance thread is well-behaving in regards
to the error stack.

We assume there are two types of errors that can be raised in the QUIC
library calls and in the subordinate libcrypto (and provider) calls. First
type is an intermittent error that does not really affect the state of the
QUIC connection - for example EAGAIN returned on a syscall, or unavailability
of some algorithm where there are other algorithms to try. Second type
is a permanent error that affects the error state of the QUIC connection.
Operations on QUIC streams (SSL_write(), SSL_read()) can also trigger errors,
depending on their effect they are either permanent if they cause the
QUIC connection to enter an error state, or if they just affect the stream
they are left on the error stack of the thread that called SSL_write()
or SSL_read() on the stream.

Design
------

Return value of SSL_get_error() on QUIC connections or streams does not
depend on the error stack contents.

Intermittent errors are handled within the library and cleared from the
error stack before returning to the user.

Permanent errors happening within the assist thread, within SSL_tick()
processing, or when calling SSL_read()/SSL_write() on a stream need to be
replicated for SSL_read()/SSL_write() calls on other streams.

Implementation
--------------

There is an error stack in QUIC_CHANNEL which serves as temporary storage
for errors happening in the internal assistance thread. When a permanent error
is detected the error stack entries are moved to this error stack in
QUIC_CHANNEL.

When returning to an application from an SSL_read()/SSL_write() call with
a permanent connection error, entries from the QUIC_CHANNEL error stack
are copied to the thread local error stack. They are always kept on
the QUIC_CHANNEL error stack as well for possible further calls from
an application. An additional error reason
SSL_R_QUIC_CONNECTION_TERMINATED is added to the stack.

SSL_tick() return value
-----------------------

The return value of SSL_tick() does not depend on whether there is
a permanent error on the connection. The only case when SSL_tick() may
return an error is when there was some fatal error processing it
such as a memory allocation error where no further SSL_tick() calls
make any sense.

Multi-stream-multi-thread mode
------------------------------

There is nothing particular that needs to be handled specially for
multi-stream-multi-thread mode as the error stack entries are always
copied from the QUIC_CHANNEL after the failure. So if multiple threads
are calling SSL_read()/SSL_write() simultaneously they all get
the same error stack entries to report to the user.
