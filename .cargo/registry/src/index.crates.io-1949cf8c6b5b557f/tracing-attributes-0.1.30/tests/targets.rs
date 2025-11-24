use tracing::subscriber::with_default;
use tracing_attributes::instrument;
use tracing_mock::*;

#[instrument]
fn default_target() {}

#[instrument(target = "my_target")]
fn custom_target() {}

mod my_mod {
    use tracing_attributes::instrument;

    pub const MODULE_PATH: &str = module_path!();

    #[instrument]
    pub fn default_target() {}

    #[instrument(target = "my_other_target")]
    pub fn custom_target() {}
}

#[test]
fn default_targets() {
    let (subscriber, handle) = subscriber::mock()
        .new_span(
            expect::span()
                .named("default_target")
                .with_target(module_path!()),
        )
        .enter(
            expect::span()
                .named("default_target")
                .with_target(module_path!()),
        )
        .exit(
            expect::span()
                .named("default_target")
                .with_target(module_path!()),
        )
        .new_span(
            expect::span()
                .named("default_target")
                .with_target(my_mod::MODULE_PATH),
        )
        .enter(
            expect::span()
                .named("default_target")
                .with_target(my_mod::MODULE_PATH),
        )
        .exit(
            expect::span()
                .named("default_target")
                .with_target(my_mod::MODULE_PATH),
        )
        .only()
        .run_with_handle();

    with_default(subscriber, || {
        default_target();
        my_mod::default_target();
    });

    handle.assert_finished();
}

#[test]
fn custom_targets() {
    let (subscriber, handle) = subscriber::mock()
        .new_span(
            expect::span()
                .named("custom_target")
                .with_target("my_target"),
        )
        .enter(
            expect::span()
                .named("custom_target")
                .with_target("my_target"),
        )
        .exit(
            expect::span()
                .named("custom_target")
                .with_target("my_target"),
        )
        .new_span(
            expect::span()
                .named("custom_target")
                .with_target("my_other_target"),
        )
        .enter(
            expect::span()
                .named("custom_target")
                .with_target("my_other_target"),
        )
        .exit(
            expect::span()
                .named("custom_target")
                .with_target("my_other_target"),
        )
        .only()
        .run_with_handle();

    with_default(subscriber, || {
        custom_target();
        my_mod::custom_target();
    });

    handle.assert_finished();
}
