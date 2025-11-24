QUIC Statistics Manager
=======================

The statistics manager keeps track of RTT statistics for use by the QUIC
implementation.

It provides the following interface:

Instantiation
-------------

The QUIC statistics manager is instantiated as follows:

```c
typedef struct ossl_statm_st {
    ...
} OSSL_STATM;

int ossl_statm_init(OSSL_STATM *statm);

void ossl_statm_destroy(OSSL_STATM *statm);
```

The structure is defined in headers, so it may be initialised without needing
its own memory allocation. However, other code should not examine the fields of
`OSSL_STATM` directly.

Get RTT Info
------------

The current RTT info is retrieved using the function `ossl_statm_get_rtt_info`,
which fills an `OSSL_RTT_INFO` structure:

```c
typedef struct ossl_rtt_info_st {
    /* As defined in RFC 9002. */
    OSSL_TIME smoothed_rtt, latest_rtt, rtt_variance, min_rtt,
              max_ack_delay;
} OSSL_RTT_INFO;

void ossl_statm_get_rtt_info(OSSL_STATM *statm, OSSL_RTT_INFO *rtt_info);
```

Update RTT
----------

New RTT samples are provided using the  `ossl_statm_update_rtt` function:

  - `ack_delay`. This is the ACK Delay value; see RFC 9000.

  - `override_latest_rtt` provides a new latest RTT sample. If it is
    `OSSL_TIME_ZERO`, the existing Latest RTT value is used when updating the
    RTT.

The maximum ACK delay configured using `ossl_statm_set_max_ack_delay` is not
enforced automatically on the `ack_delay` argument as the circumstances where
this should be enforced are context sensitive. It is the caller's responsibility
to retrieve the value and enforce the maximum ACK delay if appropriate.

```c
void ossl_statm_update_rtt(OSSL_STATM *statm,
                           OSSL_TIME ack_delay,
                           OSSL_TIME override_latest_rtt);
```

Set Max. Ack Delay
------------------

Sets the maximum ACK delay field reported by `OSSL_RTT_INFO`.

```c
void ossl_statm_set_max_ack_delay(OSSL_STATM *statm, OSSL_TIME max_ack_delay);
```
