use std::io::Write as _;

use pretty_assertions::assert_eq;

use super::ReplayClearState;
use super::run_synchronized_draw;

#[test]
fn synchronized_draw_wraps_replay_clear_and_repaint() {
    let mut output = Vec::new();
    let mut state = ReplayClearState::default();
    state.request();
    let mut draw = state.begin_draw();

    let value = run_synchronized_draw(&mut output, |writer| {
        assert!(draw.requested());
        writer.write_all(b"clear")?;
        writer.write_all(b"replay")?;
        draw.commit_replay();
        Ok(42)
    })
    .expect("synchronized draw");
    state.finish_draw(draw);

    assert_eq!(value, 42);
    assert!(!state.is_pending());
    assert_eq!(output, b"\x1b[?2026hclearreplay\x1b[?2026l");
}

#[test]
fn synchronized_draw_retries_replay_clear_after_render_error() {
    let mut output = Vec::new();
    let mut state = ReplayClearState::default();
    state.request();
    let draw = state.begin_draw();

    let error = run_synchronized_draw(&mut output, |writer| {
        assert!(draw.requested());
        writer.write_all(b"clear")?;
        Err::<(), _>(std::io::Error::other("render failed"))
    })
    .expect_err("draw should fail");
    state.finish_draw(draw);

    assert_eq!(error.kind(), std::io::ErrorKind::Other);
    assert!(state.is_pending());
    assert_eq!(output, b"\x1b[?2026hclear\x1b[?2026l");
}

#[test]
fn synchronized_draw_does_not_retry_clear_after_replay_is_committed() {
    struct FailOnFlush {
        output: Vec<u8>,
    }

    impl std::io::Write for FailOnFlush {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.output.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::other("flush failed"))
        }
    }

    let mut writer = FailOnFlush { output: Vec::new() };
    let mut state = ReplayClearState::default();
    state.request();
    let mut draw = state.begin_draw();

    let error = run_synchronized_draw(&mut writer, |writer| {
        assert!(draw.requested());
        writer.write_all(b"clear")?;
        writer.write_all(b"replay")?;
        draw.commit_replay();
        Ok(())
    })
    .expect_err("sync flush should fail");
    state.finish_draw(draw);

    assert_eq!(error.kind(), std::io::ErrorKind::Other);
    assert!(!state.is_pending());
    assert_eq!(writer.output, b"\x1b[?2026hclearreplay\x1b[?2026l");
}
