use std::collections::VecDeque;
use std::future::Future;

use pretty_assertions::assert_eq;

use super::FsmonitorOverride;
use super::FsmonitorProbeRunner;
use super::detect_fsmonitor_override;

struct ProbeResponse {
    args: Vec<&'static str>,
    output: Option<Vec<u8>>,
}

struct FakeRunner {
    responses: VecDeque<ProbeResponse>,
}

impl FsmonitorProbeRunner for FakeRunner {
    fn run_probe(&mut self, args: &[&str]) -> impl Future<Output = Option<Vec<u8>>> + Send {
        let response = self.responses.pop_front().expect("missing probe response");
        assert_eq!(args, response.args);
        std::future::ready(response.output)
    }
}

#[tokio::test]
async fn detects_supported_builtin_fsmonitor_values() {
    let cases = [
        (
            "missing config",
            vec![response(config_args(), /*output*/ None)],
            FsmonitorOverride::Disabled,
        ),
        (
            "helper path",
            vec![
                response(config_args(), Some(b"/tmp/fsmonitor-helper\0")),
                response(
                    typed_config_args("/tmp/fsmonitor-helper"),
                    /*output*/ None,
                ),
            ],
            FsmonitorOverride::Disabled,
        ),
        (
            "false spelling",
            vec![response(config_args(), Some(b"OFF\0"))],
            FsmonitorOverride::Disabled,
        ),
        (
            "unsupported Git",
            vec![
                response(config_args(), Some(b"yes\0")),
                response(capability_args(), Some(b"")),
            ],
            FsmonitorOverride::Disabled,
        ),
        (
            "common true spelling",
            vec![
                response(config_args(), Some(b"On\0")),
                response(capability_args(), Some(fsmonitor_capability())),
            ],
            FsmonitorOverride::BuiltIn,
        ),
        (
            "numeric true",
            vec![
                response(config_args(), Some(b"2k\0")),
                response(typed_config_args("2k"), Some(b"true\0")),
                response(capability_args(), Some(fsmonitor_capability())),
            ],
            FsmonitorOverride::BuiltIn,
        ),
        (
            "valueless true",
            vec![
                response(config_args(), Some(b"\0")),
                response(typed_config_args(""), Some(b"true\0")),
                response(capability_args(), Some(fsmonitor_capability())),
            ],
            FsmonitorOverride::BuiltIn,
        ),
        (
            "explicit empty false",
            vec![
                response(config_args(), Some(b"\0")),
                response(typed_config_args(""), Some(b"false\0")),
            ],
            FsmonitorOverride::Disabled,
        ),
    ];

    for (name, responses, expected) in cases {
        let mut runner = FakeRunner {
            responses: responses.into(),
        };

        let actual = detect_fsmonitor_override(&mut runner).await;

        assert_eq!(
            (actual, runner.responses.len()),
            (expected, 0),
            "case: {name}"
        );
    }
}

fn response(args: Vec<&'static str>, output: Option<&[u8]>) -> ProbeResponse {
    ProbeResponse {
        args,
        output: output.map(<[u8]>::to_vec),
    }
}

fn config_args() -> Vec<&'static str> {
    vec!["config", "--null", "--get", "core.fsmonitor"]
}

fn typed_config_args(value: &'static str) -> Vec<&'static str> {
    vec![
        "config",
        "--null",
        "--type=bool",
        "--fixed-value",
        "--get",
        "core.fsmonitor",
        value,
    ]
}

fn capability_args() -> Vec<&'static str> {
    vec!["version", "--build-options"]
}

fn fsmonitor_capability() -> &'static [u8] {
    b"feature: fsmonitor--daemon\n"
}
