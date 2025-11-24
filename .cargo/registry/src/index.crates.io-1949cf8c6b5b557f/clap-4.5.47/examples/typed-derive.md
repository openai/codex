**This requires enabling the [`derive` feature flag][crate::_features].**

Help:
```console
$ typed-derive --help
Usage: typed-derive[EXE] [OPTIONS]

Options:
  -O <OPTIMIZATION>            Implicitly using `std::str::FromStr`
  -I <DIR>                     Allow invalid UTF-8 paths
      --bind <BIND>            Handle IP addresses
      --sleep <SLEEP>          Allow human-readable durations
  -D <DEFINES>                 Hand-written parser for tuples
      --port <PORT>            Support for discrete numbers [default: 22] [possible values: 22, 80]
      --log-level <LOG_LEVEL>  Support enums from a foreign crate that don't implement `ValueEnum` [default: info] [possible values: trace, debug, info, warn, error]
  -h, --help                   Print help

```

Optimization-level (number)
```console
$ typed-derive -O 1
Args { optimization: Some(1), include: None, bind: None, sleep: None, defines: [], port: 22, log_level: Info }

$ typed-derive -O plaid
? failed
error: invalid value 'plaid' for '-O <OPTIMIZATION>': invalid digit found in string

For more information, try '--help'.

```

Include (path)
```console
$ typed-derive -I../hello
Args { optimization: None, include: Some("../hello"), bind: None, sleep: None, defines: [], port: 22, log_level: Info }

```

IP Address
```console
$ typed-derive --bind 192.0.0.1
Args { optimization: None, include: None, bind: Some(192.0.0.1), sleep: None, defines: [], port: 22, log_level: Info }

$ typed-derive --bind localhost
? failed
error: invalid value 'localhost' for '--bind <BIND>': invalid IP address syntax

For more information, try '--help'.

```

Time
```console
$ typed-derive --sleep 10s
Args { optimization: None, include: None, bind: None, sleep: Some(10s), defines: [], port: 22, log_level: Info }

$ typed-derive --sleep forever
? failed
error: invalid value 'forever' for '--sleep <SLEEP>': failed to parse "forever" in the "friendly" format: parsing a friendly duration requires it to start with a unit value (a decimal integer) after an optional sign, but no integer was found

For more information, try '--help'.

```

Defines (key-value pairs)
```console
$ typed-derive -D Foo=10 -D Alice=30
Args { optimization: None, include: None, bind: None, sleep: None, defines: [("Foo", 10), ("Alice", 30)], port: 22, log_level: Info }

$ typed-derive -D Foo
? failed
error: invalid value 'Foo' for '-D <DEFINES>': invalid KEY=value: no `=` found in `Foo`

For more information, try '--help'.

$ typed-derive -D Foo=Bar
? failed
error: invalid value 'Foo=Bar' for '-D <DEFINES>': invalid digit found in string

For more information, try '--help'.

```

Discrete numbers
```console
$ typed-derive --port 22
Args { optimization: None, include: None, bind: None, sleep: None, defines: [], port: 22, log_level: Info }

$ typed-derive --port 80
Args { optimization: None, include: None, bind: None, sleep: None, defines: [], port: 80, log_level: Info }

$ typed-derive --port
? failed
error: a value is required for '--port <PORT>' but none was supplied
  [possible values: 22, 80]

For more information, try '--help'.

$ typed-derive --port 3000
? failed
error: invalid value '3000' for '--port <PORT>'
  [possible values: 22, 80]

For more information, try '--help'.

```

Enums from crates that can't implement `ValueEnum`
```console
$ typed-derive --log-level debug
Args { optimization: None, include: None, bind: None, sleep: None, defines: [], port: 22, log_level: Debug }

$ typed-derive --log-level error
Args { optimization: None, include: None, bind: None, sleep: None, defines: [], port: 22, log_level: Error }

$ typed-derive --log-level
? failed
error: a value is required for '--log-level <LOG_LEVEL>' but none was supplied
  [possible values: trace, debug, info, warn, error]

For more information, try '--help'.

$ typed-derive --log-level critical
? failed
error: invalid value 'critical' for '--log-level <LOG_LEVEL>'
  [possible values: trace, debug, info, warn, error]

For more information, try '--help'.

```
