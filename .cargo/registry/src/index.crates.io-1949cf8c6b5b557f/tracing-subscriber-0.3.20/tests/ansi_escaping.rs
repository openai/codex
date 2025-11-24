use std::sync::{Arc, Mutex};
use tracing_subscriber::fmt::MakeWriter;

/// Shared test writer that collects output for verification
#[derive(Debug, Clone)]
struct TestWriter {
    buf: Arc<Mutex<Vec<u8>>>,
}

impl TestWriter {
    fn new() -> Self {
        Self {
            buf: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn get_output(&self) -> String {
        let buf = self.buf.lock().unwrap();
        String::from_utf8_lossy(&buf).to_string()
    }
}

impl std::io::Write for TestWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buf.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for TestWriter {
    type Writer = TestWriter;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// Test that basic security expectations are met - this is a smoke test
/// for the ANSI escaping functionality using public APIs only
#[test]
fn test_error_ansi_escaping() {
    use std::fmt;

    #[derive(Debug)]
    struct MaliciousError(&'static str);

    impl fmt::Display for MaliciousError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl std::error::Error for MaliciousError {}

    let writer = TestWriter::new();
    let subscriber = tracing_subscriber::fmt::Subscriber::builder()
        .with_writer(writer.clone())
        .with_ansi(false)
        .without_time()
        .with_target(false)
        .with_level(false)
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        let malicious_error = MaliciousError("\x1b]0;PWNED\x07\x1b[2J\x08\x0c\x7f");

        // This demonstrates that errors are logged - the actual escaping
        // is tested by our internal unit tests
        tracing::error!(error = %malicious_error, "An error occurred");
    });

    let output = writer.get_output();

    // Just verify that something was logged
    assert!(
        output.contains("An error occurred"),
        "Error message should be logged"
    );
}

/// Test that ANSI escape sequences in log messages are properly escaped
#[test]
fn test_message_ansi_escaping() {
    let writer = TestWriter::new();
    let subscriber = tracing_subscriber::fmt::Subscriber::builder()
        .with_writer(writer.clone())
        .with_ansi(false)
        .without_time()
        .with_target(false)
        .with_level(false)
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        let malicious_input = "\x1b]0;PWNED\x07\x1b[2J\x08\x0c\x7f";

        // This should not cause ANSI injection
        tracing::info!("User input: {}", malicious_input);
    });

    let output = writer.get_output();

    // Verify ANSI sequences are escaped
    assert!(
        !output.contains('\x1b'),
        "Message output should not contain raw ESC characters"
    );
    assert!(
        !output.contains('\x07'),
        "Message output should not contain raw BEL characters"
    );
}

/// Test that JSON formatter properly escapes ANSI sequences
#[cfg(feature = "json")]
#[test]
fn test_json_ansi_escaping() {
    let writer = TestWriter::new();
    let subscriber = tracing_subscriber::fmt::Subscriber::builder()
        .json()
        .with_writer(writer.clone())
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        let malicious_input = "\x1b]0;PWNED\x07\x1b[2J";

        // JSON formatter should escape ANSI sequences
        tracing::info!("Testing: {}", malicious_input);
        tracing::info!(user_input = %malicious_input, "Field test");
    });

    let output = writer.get_output();

    // JSON should escape ANSI sequences as Unicode escapes
    assert!(
        !output.contains('\x1b'),
        "JSON output should not contain raw ESC characters"
    );
    assert!(
        !output.contains('\x07'),
        "JSON output should not contain raw BEL characters"
    );
}

/// Test that pretty formatter properly escapes ANSI sequences  
#[cfg(feature = "ansi")]
#[test]
fn test_pretty_ansi_escaping() {
    let writer = TestWriter::new();
    let subscriber = tracing_subscriber::fmt::Subscriber::builder()
        .pretty()
        .with_writer(writer.clone())
        .with_ansi(false)
        .without_time()
        .with_target(false)
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        let malicious_input = "\x1b]0;PWNED\x07\x1b[2J";

        // Pretty formatter should escape ANSI sequences
        tracing::info!("Testing: {}", malicious_input);
    });

    let output = writer.get_output();

    // Verify ANSI sequences are escaped
    assert!(
        !output.contains('\x1b'),
        "Pretty output should not contain raw ESC characters"
    );
    assert!(
        !output.contains('\x07'),
        "Pretty output should not contain raw BEL characters"
    );
}

/// Comprehensive test for ANSI sanitization that prevents injection attacks
#[test]
fn ansi_sanitization_prevents_injection() {
    let writer = TestWriter::new();
    let subscriber = tracing_subscriber::fmt::Subscriber::builder()
        .with_writer(writer.clone())
        .with_ansi(false)
        .without_time()
        .with_target(false)
        .with_level(false)
        .finish();

    #[derive(Debug)]
    struct MaliciousError {
        content: String,
    }

    impl std::fmt::Display for MaliciousError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            // This Display implementation contains ANSI escape sequences
            write!(f, "Error: {}", self.content)
        }
    }

    tracing::subscriber::with_default(subscriber, || {
        // Test 1: Field values should remain properly escaped by Debug (baseline)
        let malicious_field_value = "\x1b]0;PWNED\x07\x1b[2J";
        tracing::error!(malicious_field = malicious_field_value, "Field test");

        // Test 2: Message content vulnerability should be mitigated
        let malicious_error = MaliciousError {
            content: "\x1b]0;PWNED\x07\x1b[2J".to_string(),
        };
        tracing::error!("{}", malicious_error);
    });

    let output = writer.get_output();

    // Field values should contain escaped sequences like \u{1b}
    assert!(
        output.contains("\\u{1b}"),
        "Field values should be escaped by Debug formatting"
    );

    // Message content should be sanitized
    assert!(
        output.contains("\\x1b"),
        "Message content should be sanitized"
    );
    assert!(
        !output.contains("\x1b]0;PWNED"),
        "Message content should not contain raw ANSI sequences"
    );
    assert!(
        !output.contains("\x07"),
        "Message content should not contain raw control characters"
    );
}

/// Test that C1 control characters (\x80-\x9f) are also properly escaped
#[test]
fn test_c1_control_characters_escaping() {
    let writer = TestWriter::new();
    let subscriber = tracing_subscriber::fmt::Subscriber::builder()
        .with_writer(writer.clone())
        .with_ansi(false)
        .without_time()
        .with_target(false)
        .with_level(false)
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        // Test C1 control characters that can be used in 8-bit terminal escape sequences
        let c1_controls = "\u{80}\u{85}\u{90}\u{9b}\u{9c}\u{9d}\u{9e}\u{9f}"; // Various C1 controls including CSI

        // This should escape C1 control characters to prevent 8-bit escape sequences
        tracing::info!("C1 controls: {}", c1_controls);
    });

    let output = writer.get_output();

    // Verify C1 control characters are escaped
    assert!(
        !output.contains('\u{80}'),
        "Output should not contain raw C1 control characters"
    );
    assert!(
        !output.contains('\u{9b}'),
        "Output should not contain raw CSI character"
    );
    assert!(
        !output.contains('\u{9c}'),
        "Output should not contain raw ST character"
    );

    // Should contain Unicode escapes for C1 characters
    assert!(
        output.contains("\\u{80}") || output.contains("\\u{8"),
        "Should contain escaped C1 characters"
    );
}
