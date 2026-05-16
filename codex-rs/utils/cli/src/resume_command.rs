//! Shared formatting for user-facing `codex resume` command hints.

use codex_protocol::ThreadId;
use codex_shell_command::parse_command::shlex_join;

pub fn resume_command(thread_name: Option<&str>, thread_id: Option<ThreadId>) -> Option<String> {
    let resume_target = thread_name
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .or_else(|| thread_id.map(|thread_id| thread_id.to_string()));
    resume_target.map(|target| {
        let needs_double_dash = target.starts_with('-');
        let escaped = shlex_join(&[target]);
        if needs_double_dash {
            format!("codex resume -- {escaped}")
        } else {
            format!("codex resume {escaped}")
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn prefers_name_over_id() {
        let thread_id = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174000").unwrap();
        let command = resume_command(Some("my-thread"), Some(thread_id));
        assert_eq!(command, Some("codex resume my-thread".to_string()));
    }

    #[test]
    fn formats_thread_id_when_name_is_missing() {
        let thread_id = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174000").unwrap();
        let command = resume_command(/*thread_name*/ None, Some(thread_id));
        assert_eq!(
            command,
            Some("codex resume 123e4567-e89b-12d3-a456-426614174000".to_string())
        );
    }

    #[test]
    fn returns_none_without_a_resume_target() {
        let command = resume_command(/*thread_name*/ None, /*thread_id*/ None);
        assert_eq!(command, None);
    }

    #[test]
    fn quotes_thread_names_when_needed() {
        let command = resume_command(Some("-starts-with-dash"), /*thread_id*/ None);
        assert_eq!(
            command,
            Some("codex resume -- -starts-with-dash".to_string())
        );

        let command = resume_command(Some("two words"), /*thread_id*/ None);
        assert_eq!(command, Some("codex resume 'two words'".to_string()));

        let command = resume_command(Some("quote'case"), /*thread_id*/ None);
        assert_eq!(command, Some("codex resume \"quote'case\"".to_string()));
    }
}
