// Note: It's recommended, not to change anything above or below (see big comment below)

use color_eyre::{eyre::Report, Section};

#[rustfmt::skip]
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct TestError(&'static str);

#[rustfmt::skip]
#[tracing::instrument]
fn get_error(msg: &'static str) -> Report {

    #[rustfmt::skip]
    #[inline(never)]
    fn create_report(msg: &'static str) -> Report {
        Report::msg(msg)
            .note("note")
            .warning("warning")
            .suggestion("suggestion")
            .error(TestError("error"))
    }

    // Using `Option` to trigger `is_dependency_code`.
    // See https://github.com/eyre-rs/color-eyre/blob/4ddaeb2126ed8b14e4e6aa03d7eef49eb8561cf0/src/config.rs#L56
    None::<Option<()>>.ok_or_else(|| create_report(msg)).unwrap_err()
}

#[cfg(all(not(feature = "track-caller"), not(feature = "capture-spantrace"),))]
static ERROR_FILE_NAME: &str = "theme_error_control_minimal.txt";

#[cfg(all(feature = "track-caller", not(feature = "capture-spantrace"),))]
static ERROR_FILE_NAME: &str = "theme_error_control_location.txt";

#[cfg(all(not(feature = "track-caller"), feature = "capture-spantrace",))]
static ERROR_FILE_NAME: &str = "theme_error_control_spantrace.txt";

#[cfg(all(feature = "capture-spantrace", feature = "track-caller",))]
static ERROR_FILE_NAME: &str = "theme_error_control.txt";

#[test]
#[cfg(not(miri))]
fn test_error_backwards_compatibility() {
    setup();
    let error = get_error("test");

    /*
        Note: If you change anything above this comment, it could make the stored test data invalid (because the structure of the generated error might change). In most cases, small changes shouldn't be a problem, but keep this in mind if you change something and suddenly this test case fails.

        The empty lines at the beginning are needed because `color_eyre` sometimes seems to not be able to find the correct line of source and uses the first line of the module (plus the next four lines).

        If a change of the code above leads to incompatibility, you therefore have to backport this (changed) file to the version of `color_eyre` that you want to test against and execute it to generate new control test data.

        To do this, do the following:

        1) Change this file, and if the test now fails do:

        2) Checkout the `color_eyre` version from Git that you want to test against

        3) Add this test file to '/tests'

        4) If `error_file_path` or `panic_file_path` exist (see below), delete these files

        5) If you now run this test, it will fail and generate test data files in the current working directory

        6) copy these files to `error_file_path` and `panic_file_path` in the current version of `color_eyre` (see the instructions that are printed out in step 5)

        Now this test shouldn't fail anymore in the current version.

        Alternatively, you also could just regenerate the test data of the current repo (as described above, but without backporting), and use this test data from now on (this makes sense, if you only changed the above code, and nothing else that could lead to the test failing).


        # How the tests in this file work:

        1) generate a error (for example, with the code above)

        2) convert this error to a string

        3) load stored error data to compare to (stored in `error_file_path` and `panic_file_path`)

        4) if `error_file_path` and/or `panic_file_path` doesn't exist, generate corresponding files in the current working directory and request the user to fix the issue (see below)

        5) extract ANSI escaping sequences (of controls and current errors)

        6) compare if the current error and the control contains the same ANSI escape sequences

        7) If not, fail and show the full strings of the control and the current error

        Below you'll find instructions about how to debug failures of the tests in this file
    */

    let target = format!("{:?}", error);
    test_backwards_compatibility(target, ERROR_FILE_NAME)
}

#[cfg(not(feature = "capture-spantrace"))]
static PANIC_FILE_NAME: &str = "theme_panic_control_no_spantrace.txt";

#[cfg(feature = "capture-spantrace")]
static PANIC_FILE_NAME: &str = "theme_panic_control.txt";

// The following tests the installed panic handler
#[test]
#[allow(unused_mut)]
#[allow(clippy::vec_init_then_push)]
#[cfg(not(miri))]
fn test_panic_backwards_compatibility() {
    let mut features: Vec<&str> = vec![];
    #[cfg(feature = "capture-spantrace")]
    features.push("capture-spantrace");
    #[cfg(feature = "issue-url")]
    features.push("issue-url");
    #[cfg(feature = "track-caller")]
    features.push("track-caller");

    let features = features.join(",");
    let features = if !features.is_empty() {
        vec!["--features", &features]
    } else {
        vec![]
    };

    let output = std::process::Command::new("cargo")
        .args(["run", "--example", "theme_test_helper"])
        .arg("--no-default-features")
        .args(&features)
        .env("RUSTFLAGS", "-Awarnings")
        .env("CARGO_FUTURE_INCOMPAT_REPORT_FREQUENCY", "never")
        .output()
        .expect("failed to execute process");
    let target = String::from_utf8(output.stderr).expect("failed to convert output to `String`");
    println!("{}", target);
    test_backwards_compatibility(target, PANIC_FILE_NAME)
}

/// Helper for `test_error` and `test_panic`
fn test_backwards_compatibility(target: String, file_name: &str) {
    use ansi_parser::{AnsiParser, AnsiSequence, Output};
    use owo_colors::OwoColorize;
    use std::{fs, path::Path};

    let file_path = ["tests/data/", file_name].concat();

    // If `file_path` is missing, save corresponding file to current working directory, and panic with the request to move the file to `file_path`, and to commit it to Git. Being explicit (instead of saving directly to `file_path`) to make sure `file_path` is committed to Git.

    if !Path::new(&file_path).is_file() {
        std::fs::write(file_name, &target)
            .expect("\n\nError saving missing `control target` to a file");
        panic!("Required test data missing! Fix this, by moving '{}' to '{}', and commit it to Git.\n\nNote: '{0}' was just generated in the current working directory.\n\n", file_name, file_path);
    }

    // `unwrap` should never fail with files generated by this function
    let control = String::from_utf8(fs::read(file_path).unwrap()).unwrap();

    fn split_ansi_output(input: &str) -> (Vec<Output>, Vec<AnsiSequence>) {
        let all: Vec<_> = input.ansi_parse().collect();
        let ansi: Vec<_> = input
            .ansi_parse()
            .filter_map(|x| {
                if let Output::Escape(ansi) = x {
                    Some(ansi)
                } else {
                    None
                }
            })
            .collect();
        (all, ansi)
    }

    fn normalize_backtrace(input: &str) -> String {
        input
            .lines()
            .take_while(|v| {
                !v.contains("core::panic")
                    && !v.contains("theme_test_helper::main")
                    && !v.contains("theme::test_error_backwards_compatibility::closure")
                    && !v.contains("theme::test_error_backwards_compatibility::{{closure}}")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    let control = normalize_backtrace(&control);
    let target = normalize_backtrace(&target);
    let (_control_tokens, control_ansi) = split_ansi_output(&control);
    let (_target_tokens, target_ansi) = split_ansi_output(&target);

    fn section(title: &str, content: impl AsRef<str>) -> String {
        format!(
            "{}\n{}",
            format!("-------- {title} --------").red(),
            content.as_ref()
        )
    }

    // pretty_assertions::assert_eq!(target, control);
    let msg = [
        // comment out / un-comment what you need or don't need for debugging (see below for more instructions):

        format!("{}", "\x1b[0m\n\nANSI escape sequences are not identical to control!".red()),
        // ^ `\x1b[0m` clears previous ANSI escape sequences

        section("CONTROL STRING", &control),
        // section("CONTROL DEBUG STRING", format!("{control:?}")),
        // section("CONTROL ANSI PARSER OUTPUT", format!("{_control_tokens:?}")),
        // section("CONTROL ANSI PARSER ANSI", format!("{control_ansi:?}")),

        section("CURRENT STRING", &target),
        // section("CURRENT DEBUG STRING", format!("{target:?}")),
        // section("CURRENT ANSI PARSER OUTPUT", format!("{_target_tokens:?}")),
        // section("CURRENT ANSI PARSER ANSI", format!("{target_ansi:?}")),

        format!("{}", "See the src of this test for more information about the test and ways to include/exclude debugging information.\n\n".red()),

    ].join("\n\n");

    pretty_assertions::assert_eq!(target_ansi, control_ansi, "{}", &msg);

    /*
        # Tips for debugging test failures

        It's a bit a pain to find the reason for test failures. To make it as easy as possible, I recommend the following workflow:

        ## Compare the actual errors

        1) Run the test in two terminals with "CONTROL STRING" and "CURRENT STRING" active

        2) In on terminal have the output of "CONTROL STRING" visible, in the out that of "CURRENT STRING"

        3) Make sure, both errors are at the same location of their terminal

        4) Now switch between the two terminal rapidly and often. This way it's easy to see changes.

        Note that we only compare ANSI escape sequences – so if the text changes, that is not a problem.

        A problem would it be, if there is a new section of content (which might contain new ANSI escape sequences). This could happen, for example, if the current code produces warnings, etc. (especially, with the panic handler test).

        ## Compare `ansi_parser` tokens

        If you fixed all potential problems above, and the test still failes, compare the actual ANSI escape sequences:

        1) Activate "CURRENT ANSI PARSER OUTPUT" and "CURRENT ANSI PARSER OUTPUT" above

        2) Copy this output to a text editor and replace all "), " with ")," + a newline (this way every token is on its own line)

        3) Compare this new output with a diff tool (https://meldmerge.org/ is a nice option with a GUI)

        With this approach, you should see what has changed. Just remember that we only compare the ANSI escape sequences, text is skipped. With "CURRENT ANSI PARSER OUTPUT" and "CURRENT ANSI PARSER OUTPUT", however, text tokens are shown as well (to make it easier to figure out the source of ANSI escape sequences.)
    */
}

fn setup() {
    std::env::set_var("RUST_LIB_BACKTRACE", "1");

    #[cfg(feature = "capture-spantrace")]
    {
        use tracing_subscriber::prelude::*;
        use tracing_subscriber::{fmt, EnvFilter};

        let fmt_layer = fmt::layer().with_target(false);
        let filter_layer = EnvFilter::try_from_default_env()
            .or_else(|_| EnvFilter::try_new("info"))
            .unwrap();

        tracing_subscriber::registry()
            .with(filter_layer)
            .with(fmt_layer)
            .with(tracing_error::ErrorLayer::default())
            .init();
    }

    color_eyre::install().expect("Failed to install `color_eyre`");

    /*
        # Easy way to test styles

        1) uncomment the last line

        2) activate the following code

        3) change the styles

        4) run this test via `cargo test test_error_backwards_compatibility --test styles`

        5) your new styled error will be below the output "CURRENT STRING ="

        6) if there is not such output, search for "CURRENT STRING =" above, and activate the line

        7) if you are interested in running this test for actual testing this crate, don't forget to uncomment the code below, activate the above line
    */

    /*
    use owo_colors::style;
    let styles = color_eyre::config::Styles::dark()
        // ^ or, instead of `dark`, use `new` for blank styles or `light` if you what to derive from a light theme. Now configure your styles (see the docs for all options):
        .line_number(style().blue())
        .help_info_suggestion(style().red());

    color_eyre::config::HookBuilder::new()
        .styles(styles)
        .install()
        .expect("Failed to install `color_eyre`");
    */
}
