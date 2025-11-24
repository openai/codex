qlog Support
============

qlog support is formed of two components:

- A qlog API and implementation.
- A JSON encoder API and implementation, which is used by the qlog
  implementation.

The API for the JSON encoder is detailed in [a separate document](json-encoder.md).

qlog support will involve instrumenting various functions with qlog logging
code. An example call site will look something like this:

```c
{
    QLOG_EVENT_BEGIN(qlog_instance, quic, parameters_set)
        QLOG_STR("owner", "local")
        QLOG_BOOL("resumption_allowed", 1)
        QLOG_STR("tls_cipher", "AES_128_GCM")
        QLOG_BEGIN("subgroup")
            QLOG_U64("u64_value", 123)
            QLOG_BIN("binary_value", buf, buf_len)
        QLOG_END()
    QLOG_EVENT_END()
}
```

Output Format
-------------

The output format is always the JSON-SEQ qlog variant. This has the advantage
that each event simply involves concatenating another record to an output log
file and does not require nesting of syntactic constructs between events.

Output is written to a directory containing multiple qlog files.

Basic Usage
-----------

Basic usage is in the form of

- a `QLOG_EVENT_BEGIN` macro which takes a QLOG instance, category name and
  event name.

  This (category name, event name) tuple is known as the event type.

- zero or more macros which log fields inside a qlog event.

- a `QLOG_EVENT_END` macro.

Usage is synchronised across threads on a per-event basis automatically.

API Definition
--------------

API details can be found in `internal/qlog.h`.

Configuration
-------------

qlog must currently be enabled at build time using `enable-unstable-qlog`. If
not enabled, `OPENSSL_NO_QLOG` is defined.

When built with qlog support, qlog can be turned on using the recommended
environment variable `QLOGDIR`. A filter can be defined using `OSSL_QFILTER`. When
enabled, each connection causes a file `{ODCID}_{ROLE}.sqlog` to be created in
the specified directory, where `{ODCID}` is the original initial DCID used for
the connection and `{ROLE}` is `client` or `server`.

Filters
-------

Each event type can be turned on and off individually.

The filtering is configured using a string with the following syntax, expressed
in ABNF:

```abnf
filter = *filter-term

filter-term = add-sub-term

add-sub-term = ["-" / "+"] specifier

specifier           = global-specifier / qualified-specifier

global-specifier    = wildcard

qualified-specifier = component-specifier ":" component-specifier

component-specifier = name / wildcard

wildcard            = "*"

name                = 1*(ALPHA / DIGIT / "_" / "-")
```

Here is a (somewhat nonsensical) example filter:

```text
+* -quic:version_information -* quic:packet_sent
```

The syntax works as follows:

- A filter term is preceded by `-` (disable an event type) or `+` (enable an
  event type). If this symbol is omitted, `+` is assumed.

- `+*` (or `*`) enables all event types.

- `-*` disables all event types.

- `+quic:*` (or `quic:*`) enables all event types in the `quic` category.

- `-quic:version_information` disables a specific event type.

- Partial wildcard matches are not supported at this time.

Each term is applied in sequence, therefore later items in the filter override
earlier items. In the example above, for example, all event types are enabled,
then the `quic:version_information` event is disabled, then all event types are
disabled, then the `quic:packet_sent` event is re-enabled.

Some examples of more normal filters include:

- `*` (or `+*`): enable all event types

- `quic:version_information quic:packet_sent`: enable some event types explicitly

- `* -quic:version_information`: enable all event types except certain ones

See also
--------

See the manpage openssl-qlog(7) for additional usage guidance.
