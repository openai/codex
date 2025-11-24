QUIC Thread Assisted Mode Synchronisation Requirements
======================================================

In thread assisted mode, we create a background thread to ensure that periodic
QUIC processing is handled in a timely fashion regardless of whether an
application is frequently calling (or blocked in) SSL API I/O functions.

Part of the QUIC state comprises the TLS handshake layer. However, synchronising
access to this is extremely difficult.

At first glance, one could synchronise handshake layer public APIs by locking a
per-connection mutex for the duration of any public API call which we forward to
the handshake layer. Since we forward a very large number of APIs to the
handshake layer, this would require a very large number of code changes to add
the locking to every single public HL-related API call.

However, on second glance, this does not even solve the problem, as
applications existing usage of the HL APIs assumes exclusive access, and thus
consistency over multiple API calls. For example:

    x = SSL_get_foo(s);
    /* application mutates x */
    SSL_set_foo(s, x);

For locking of API calls the lock would only be held for the separate get and
set calls, but the combination of the two would not be safe if the assist thread
can process some event which causes mutation of `foo`.

As such, there are really only three possible solutions:

- **1. Application-controlled explicit locking.**

  We would offer something like `SSL_lock()` and `SSL_unlock()`.
  An application performing a single HL API call, or a sequence of related HL
  calls, would be required to take the lock. As a special exemption, an
  application is not required to take the lock prior to connection
  (specifically, prior to the instantiation of a QUIC channel and consequent
  assist thread creation).

  The key disadvantage here is that it requires more API changes on the
  application side, although since most HL API calls made by an application
  probably happen prior to initiating a connection, things may not be that bad.
  It would also only be required for applications which want to use thread
  assisted mode.

  Pro: Most “robust” solution in terms of HL evolution.

  Con: API changes.

- **2. Handshake layer always belongs to the application thread.**

  In this model, the handshake layer “belongs” to the application thread
  and the assist thread is never allowed to touch it:

  - `SSL_tick()` (or another I/O function) called by the application fully
    services the connection.

  - The assist thread performs a reduced tick operation which does everything
    except servicing the crypto stream, or any other events we may define in
    future which would be processed by the handshake layer.

  - This is rather hacky but should work adequately. When using TLS 1.3
    as the handshake layer, the only thing we actually need to worry about
    servicing after handshake completion is the New Session Ticket message,
    which doesn't need to be acknowledged and isn't “urgent”. The other
    post-handshake messages used by TLS 1.3 aren't relevant to QUIC TLS:

    - Post-handshake authentication is not allowed;

    - Key update uses a separate, QUIC-specific method;

    - TLS alerts are signalled via `CONNECTION_CLOSE` frames rather than the TLS
      1.3 Alert message; thus if a peer's HL does raise an alert after
      handshake completion (which would in itself be highly unusual), we simply
      receive a `CONNECTION_CLOSE` frame and process it normally.

  Thus so long as we don't expect our own TLS implementation to spontaneously
  generate alerts or New Session Ticket messages after handshake completion,
  this should work.

  Pro: No API changes.

  Con: Somewhat hacky solution.

- **3. Handshake layer belongs to the assist thread after connection begins.**

  In this model, the application may make handshake layer calls freely prior to
  connecting, but after that, ownership of the HL is transferred to the assist
  thread and may not be touched further. We would need to block all API calls
  which would forward to the HL after connection commences (specifically, after
  the QUIC channel is instantiated).

  Con: Many applications probably expect to be able to query the HL after
  connection. We could selectively enable some important post-handshake HL calls
  by specially implementing synchronised forwarders, but doing this in the
  general case runs into the same issues as option 1 above. We could only enable
  APIs we think have safe semantics here; e.g. implement only getters and not
  setters, focus on APIs which return data which doesn't change after
  connection. The work required is proportional to the number of APIs to be
  enabled. Some APIs may not have ways to indicate failure; for such APIs which
  we don't implement for thread assisted post-handshake QUIC, we would
  essentially return incorrect data here.

Option 2 has been chosen as the basis for implementation.
