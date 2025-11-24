/*!
Changing the default logging format.

Before running this example, try setting the `MY_LOG_LEVEL` environment variable to `info`:

```no_run,shell
$ export MY_LOG_LEVEL='info'
```

Also try setting the `MY_LOG_STYLE` environment variable to `never` to disable colors
or `auto` to enable them:

```no_run,shell
$ export MY_LOG_STYLE=never
```

If you want to control the logging output completely, see the `custom_logger` example.
*/

#[cfg(all(feature = "color", feature = "humantime"))]
fn main() {
    use env_logger::{Builder, Env};

    use std::io::Write;

    fn init_logger() {
        let env = Env::default()
            .filter("MY_LOG_LEVEL")
            .write_style("MY_LOG_STYLE");

        Builder::from_env(env)
            .format(|buf, record| {
                // We are reusing `anstyle` but there are `anstyle-*` crates to adapt it to your
                // preferred styling crate.
                let warn_style = buf.default_level_style(log::Level::Warn);
                let timestamp = buf.timestamp();

                writeln!(
                    buf,
                    "My formatted log ({timestamp}): {warn_style}{}{warn_style:#}",
                    record.args()
                )
            })
            .init();
    }

    init_logger();

    log::info!("a log from `MyLogger`");
}

#[cfg(not(all(feature = "color", feature = "humantime")))]
fn main() {}
