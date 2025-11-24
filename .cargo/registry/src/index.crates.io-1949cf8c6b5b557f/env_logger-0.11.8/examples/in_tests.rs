/*!
Using `env_logger` in tests.

Log events will be captured by `cargo` and only printed if the test fails.
You can run this example by calling:

```text
cargo test --example in_tests
```

You should see the `it_does_not_work` test fail and include its log output.
*/

fn main() {}

#[cfg(test)]
mod tests {
    use log::debug;

    fn init_logger() {
        let _ = env_logger::builder()
            // Include all events in tests
            .filter_level(log::LevelFilter::max())
            // Ensure events are captured by `cargo test`
            .is_test(true)
            // Ignore errors initializing the logger if tests race to configure it
            .try_init();
    }

    #[test]
    fn it_works() {
        init_logger();

        let a = 1;
        let b = 2;

        debug!("checking whether {} + {} = 3", a, b);

        assert_eq!(3, a + b);
    }

    #[test]
    fn it_does_not_work() {
        init_logger();

        let a = 1;
        let b = 2;

        debug!("checking whether {} + {} = 6", a, b);

        assert_eq!(6, a + b);
    }
}
