use super::indentation::read_block;
use super::slice::read;
use super::*;
use pretty_assertions::assert_eq;
use tempfile::NamedTempFile;

#[tokio::test]
async fn reads_requested_range() -> anyhow::Result<()> {
    let mut temp = NamedTempFile::new()?;
    use std::io::Write as _;
    write!(
        temp,
        "alpha
beta
gamma
"
    )?;

    let lines = read(temp.path(), 2, 2).await?;
    assert_eq!(lines, vec!["L2: beta".to_string(), "L3: gamma".to_string()]);
    Ok(())
}

#[tokio::test]
async fn errors_when_offset_exceeds_length() -> anyhow::Result<()> {
    let mut temp = NamedTempFile::new()?;
    use std::io::Write as _;
    writeln!(temp, "only")?;

    let err = read(temp.path(), 3, 1)
        .await
        .expect_err("offset exceeds length");
    assert_eq!(
        err,
        FunctionCallError::RespondToModel("offset exceeds file length".to_string())
    );
    Ok(())
}

#[tokio::test]
async fn reads_non_utf8_lines() -> anyhow::Result<()> {
    let mut temp = NamedTempFile::new()?;
    use std::io::Write as _;
    temp.as_file_mut().write_all(b"\xff\xfe\nplain\n")?;

    let lines = read(temp.path(), 1, 2).await?;
    let expected_first = format!("L1: {}{}", '\u{FFFD}', '\u{FFFD}');
    assert_eq!(lines, vec![expected_first, "L2: plain".to_string()]);
    Ok(())
}

#[tokio::test]
async fn trims_crlf_endings() -> anyhow::Result<()> {
    let mut temp = NamedTempFile::new()?;
    use std::io::Write as _;
    write!(temp, "one\r\ntwo\r\n")?;

    let lines = read(temp.path(), 1, 2).await?;
    assert_eq!(lines, vec!["L1: one".to_string(), "L2: two".to_string()]);
    Ok(())
}

#[tokio::test]
async fn respects_limit_even_with_more_lines() -> anyhow::Result<()> {
    let mut temp = NamedTempFile::new()?;
    use std::io::Write as _;
    write!(
        temp,
        "first
second
third
"
    )?;

    let lines = read(temp.path(), 1, 2).await?;
    assert_eq!(
        lines,
        vec!["L1: first".to_string(), "L2: second".to_string()]
    );
    Ok(())
}

#[tokio::test]
async fn truncates_lines_longer_than_max_length() -> anyhow::Result<()> {
    let mut temp = NamedTempFile::new()?;
    use std::io::Write as _;
    let long_line = "x".repeat(MAX_LINE_LENGTH + 50);
    writeln!(temp, "{long_line}")?;

    let lines = read(temp.path(), 1, 1).await?;
    let expected = "x".repeat(MAX_LINE_LENGTH);
    assert_eq!(lines, vec![format!("L1: {expected}")]);
    Ok(())
}

#[tokio::test]
async fn indentation_mode_captures_block() -> anyhow::Result<()> {
    let mut temp = NamedTempFile::new()?;
    use std::io::Write as _;
    write!(
        temp,
        "fn outer() {{
    if cond {{
        inner();
    }}
    tail();
}}
"
    )?;

    let options = IndentationArgs {
        anchor_line: Some(3),
        include_siblings: false,
        max_levels: 1,
        ..Default::default()
    };

    let lines = read_block(temp.path(), 3, 10, options).await?;

    assert_eq!(
        lines,
        vec![
            "L2:     if cond {".to_string(),
            "L3:         inner();".to_string(),
            "L4:     }".to_string()
        ]
    );
    Ok(())
}

#[tokio::test]
async fn indentation_mode_expands_parents() -> anyhow::Result<()> {
    let mut temp = NamedTempFile::new()?;
    use std::io::Write as _;
    write!(
        temp,
        "mod root {{
    fn outer() {{
        if cond {{
            inner();
        }}
    }}
}}
"
    )?;

    let mut options = IndentationArgs {
        anchor_line: Some(4),
        max_levels: 2,
        ..Default::default()
    };

    let lines = read_block(temp.path(), 4, 50, options.clone()).await?;
    assert_eq!(
        lines,
        vec![
            "L2:     fn outer() {".to_string(),
            "L3:         if cond {".to_string(),
            "L4:             inner();".to_string(),
            "L5:         }".to_string(),
            "L6:     }".to_string(),
        ]
    );

    options.max_levels = 3;
    let expanded = read_block(temp.path(), 4, 50, options).await?;
    assert_eq!(
        expanded,
        vec![
            "L1: mod root {".to_string(),
            "L2:     fn outer() {".to_string(),
            "L3:         if cond {".to_string(),
            "L4:             inner();".to_string(),
            "L5:         }".to_string(),
            "L6:     }".to_string(),
            "L7: }".to_string(),
        ]
    );
    Ok(())
}

#[tokio::test]
async fn indentation_mode_respects_sibling_flag() -> anyhow::Result<()> {
    let mut temp = NamedTempFile::new()?;
    use std::io::Write as _;
    write!(
        temp,
        "fn wrapper() {{
    if first {{
        do_first();
    }}
    if second {{
        do_second();
    }}
}}
"
    )?;

    let mut options = IndentationArgs {
        anchor_line: Some(3),
        include_siblings: false,
        max_levels: 1,
        ..Default::default()
    };

    let lines = read_block(temp.path(), 3, 50, options.clone()).await?;
    assert_eq!(
        lines,
        vec![
            "L2:     if first {".to_string(),
            "L3:         do_first();".to_string(),
            "L4:     }".to_string(),
        ]
    );

    options.include_siblings = true;
    let with_siblings = read_block(temp.path(), 3, 50, options).await?;
    assert_eq!(
        with_siblings,
        vec![
            "L2:     if first {".to_string(),
            "L3:         do_first();".to_string(),
            "L4:     }".to_string(),
            "L5:     if second {".to_string(),
            "L6:         do_second();".to_string(),
            "L7:     }".to_string(),
        ]
    );
    Ok(())
}

#[tokio::test]
async fn indentation_mode_handles_python_sample() -> anyhow::Result<()> {
    let mut temp = NamedTempFile::new()?;
    use std::io::Write as _;
    write!(
        temp,
        "class Foo:
    def __init__(self, size):
        self.size = size
    def double(self, value):
        if value is None:
            return 0
        result = value * self.size
        return result
class Bar:
    def compute(self):
        helper = Foo(2)
        return helper.double(5)
"
    )?;

    let options = IndentationArgs {
        anchor_line: Some(7),
        include_siblings: true,
        max_levels: 1,
        ..Default::default()
    };

    let lines = read_block(temp.path(), 1, 200, options).await?;
    assert_eq!(
        lines,
        vec![
            "L2:     def __init__(self, size):".to_string(),
            "L3:         self.size = size".to_string(),
            "L4:     def double(self, value):".to_string(),
            "L5:         if value is None:".to_string(),
            "L6:             return 0".to_string(),
            "L7:         result = value * self.size".to_string(),
            "L8:         return result".to_string(),
        ]
    );
    Ok(())
}

#[tokio::test]
#[ignore]
async fn indentation_mode_handles_javascript_sample() -> anyhow::Result<()> {
    let mut temp = NamedTempFile::new()?;
    use std::io::Write as _;
    write!(
        temp,
        "export function makeThing() {{
    const cache = new Map();
    function ensure(key) {{
        if (!cache.has(key)) {{
            cache.set(key, []);
        }}
        return cache.get(key);
    }}
    const handlers = {{
        init() {{
            console.log(\"init\");
        }},
        run() {{
            if (Math.random() > 0.5) {{
                return \"heads\";
            }}
            return \"tails\";
        }},
    }};
    return {{ cache, handlers }};
}}
export function other() {{
    return makeThing();
}}
"
    )?;

    let options = IndentationArgs {
        anchor_line: Some(15),
        max_levels: 1,
        ..Default::default()
    };

    let lines = read_block(temp.path(), 15, 200, options).await?;
    assert_eq!(
        lines,
        vec![
            "L10:         init() {".to_string(),
            "L11:             console.log(\"init\");".to_string(),
            "L12:         },".to_string(),
            "L13:         run() {".to_string(),
            "L14:             if (Math.random() > 0.5) {".to_string(),
            "L15:                 return \"heads\";".to_string(),
            "L16:             }".to_string(),
            "L17:             return \"tails\";".to_string(),
            "L18:         },".to_string(),
        ]
    );
    Ok(())
}

fn write_cpp_sample() -> anyhow::Result<NamedTempFile> {
    let mut temp = NamedTempFile::new()?;
    use std::io::Write as _;
    write!(
        temp,
        "#include <vector>
#include <string>

namespace sample {{
class Runner {{
public:
    void setup() {{
        if (enabled_) {{
            init();
        }}
    }}

    // Run the code
    int run() const {{
        switch (mode_) {{
            case Mode::Fast:
                return fast();
            case Mode::Slow:
                return slow();
            default:
                return fallback();
        }}
    }}

private:
    bool enabled_ = false;
    Mode mode_ = Mode::Fast;

    int fast() const {{
        return 1;
    }}
}};
}}  // namespace sample
"
    )?;
    Ok(temp)
}

#[tokio::test]
async fn indentation_mode_handles_cpp_sample_shallow() -> anyhow::Result<()> {
    let temp = write_cpp_sample()?;

    let options = IndentationArgs {
        include_siblings: false,
        anchor_line: Some(18),
        max_levels: 1,
        ..Default::default()
    };

    let lines = read_block(temp.path(), 18, 200, options).await?;
    assert_eq!(
        lines,
        vec![
            "L15:         switch (mode_) {".to_string(),
            "L16:             case Mode::Fast:".to_string(),
            "L17:                 return fast();".to_string(),
            "L18:             case Mode::Slow:".to_string(),
            "L19:                 return slow();".to_string(),
            "L20:             default:".to_string(),
            "L21:                 return fallback();".to_string(),
            "L22:         }".to_string(),
        ]
    );
    Ok(())
}

#[tokio::test]
async fn indentation_mode_handles_cpp_sample() -> anyhow::Result<()> {
    let temp = write_cpp_sample()?;

    let options = IndentationArgs {
        include_siblings: false,
        anchor_line: Some(18),
        max_levels: 2,
        ..Default::default()
    };

    let lines = read_block(temp.path(), 18, 200, options).await?;
    assert_eq!(
        lines,
        vec![
            "L13:     // Run the code".to_string(),
            "L14:     int run() const {".to_string(),
            "L15:         switch (mode_) {".to_string(),
            "L16:             case Mode::Fast:".to_string(),
            "L17:                 return fast();".to_string(),
            "L18:             case Mode::Slow:".to_string(),
            "L19:                 return slow();".to_string(),
            "L20:             default:".to_string(),
            "L21:                 return fallback();".to_string(),
            "L22:         }".to_string(),
            "L23:     }".to_string(),
        ]
    );
    Ok(())
}

#[tokio::test]
async fn indentation_mode_handles_cpp_sample_no_headers() -> anyhow::Result<()> {
    let temp = write_cpp_sample()?;

    let options = IndentationArgs {
        include_siblings: false,
        include_header: false,
        anchor_line: Some(18),
        max_levels: 2,
        ..Default::default()
    };

    let lines = read_block(temp.path(), 18, 200, options).await?;
    assert_eq!(
        lines,
        vec![
            "L14:     int run() const {".to_string(),
            "L15:         switch (mode_) {".to_string(),
            "L16:             case Mode::Fast:".to_string(),
            "L17:                 return fast();".to_string(),
            "L18:             case Mode::Slow:".to_string(),
            "L19:                 return slow();".to_string(),
            "L20:             default:".to_string(),
            "L21:                 return fallback();".to_string(),
            "L22:         }".to_string(),
            "L23:     }".to_string(),
        ]
    );
    Ok(())
}

#[tokio::test]
async fn indentation_mode_handles_cpp_sample_siblings() -> anyhow::Result<()> {
    let temp = write_cpp_sample()?;

    let options = IndentationArgs {
        include_siblings: true,
        include_header: false,
        anchor_line: Some(18),
        max_levels: 2,
        ..Default::default()
    };

    let lines = read_block(temp.path(), 18, 200, options).await?;
    assert_eq!(
        lines,
        vec![
            "L7:     void setup() {".to_string(),
            "L8:         if (enabled_) {".to_string(),
            "L9:             init();".to_string(),
            "L10:         }".to_string(),
            "L11:     }".to_string(),
            "L12: ".to_string(),
            "L13:     // Run the code".to_string(),
            "L14:     int run() const {".to_string(),
            "L15:         switch (mode_) {".to_string(),
            "L16:             case Mode::Fast:".to_string(),
            "L17:                 return fast();".to_string(),
            "L18:             case Mode::Slow:".to_string(),
            "L19:                 return slow();".to_string(),
            "L20:             default:".to_string(),
            "L21:                 return fallback();".to_string(),
            "L22:         }".to_string(),
            "L23:     }".to_string(),
        ]
    );
    Ok(())
}
