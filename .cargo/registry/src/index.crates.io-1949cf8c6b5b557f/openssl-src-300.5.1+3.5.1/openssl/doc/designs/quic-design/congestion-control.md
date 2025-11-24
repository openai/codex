Congestion control API design
=============================

We use an abstract interface for the QUIC congestion controller to facilitate
use of pluggable QUIC congestion controllers in the future. The interface is
based on interfaces suggested by RFC 9002 and MSQUIC's congestion control APIs.

`OSSL_CC_METHOD` provides a vtable of function pointers to congestion controller
methods. `OSSL_CC_DATA` is an opaque type representing a congestion controller
instance.

For details on the API, see the comments in `include/internal/quic_cc.h`.

Congestion controllers are not thread safe; the caller is responsible for
synchronisation.

Congestion controllers may vary their state with respect to time. This is
facilitated via the `get_wakeup_deadline` method and the `now` argument to the
`new` method, which provides access to a clock. While no current congestion
controller makes use of this facility, it can be used by future congestion
controllers to implement packet pacing.

Congestion controllers may expose arbitrary configuration parameters via the
`set_input_params` method. Equally, congestion controllers may expose diagnostic
outputs via the `bind_diagnostics` and `unbind_diagnostics` methods. The
configuration parameters and diagnostics supported may be specific to the
congestion controller method, although there are some well known ones intended
to be common to all congestion controllers.

Currently, the only dependency injected to a congestion controller is access to
a clock. In the future it is likely that access at least to the statistics
manager will be provided. Excessive futureproofing of the congestion controller
interface has been avoided as this is currently an internal API for which no API
stability guarantees are required; for example, no currently implemented
congestion control algorithm requires access to the statistics manager, but such
access can readily be added later as needed.

QUIC congestion control state is per-path, per-connection. Currently we support
only a single path per connection, so there is one congestion control instance
per connection. This may change in future.

While the congestion control API is roughly based around the arrangement of
functions as described by the congestion control pseudocode in RFC 9002, there
are some deliberate changes in order to obtain cleaner separation between the
loss detection and congestion control functions. Where a literal option of RFC
9002 pseudocode would require a congestion controller to access the ACK
manager's internal state directly, the interface between the two has been
changed to avoid this. This involves some small amounts of functionality which
RFC 9002 considers part of the congestion controller being part of the ACK
manager in our implementation. See the comments in `include/internal/quic_cc.h`
and `ssl/quic/quic_ackm.c` for more information.

The congestion control API may be revised to allow pluggable congestion
controllers via a provider-based interface in the future.
