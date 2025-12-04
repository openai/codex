use std::{
    fs,
    io::{self, Write},
    path::Path,
    str,
    sync::{atomic::AtomicUsize, Arc},
    time::Instant,
};

use anyhow::Result;
use tree_sitter_tags::{TagsConfiguration, TagsContext};

pub struct TagsOptions {
    pub scope: Option<String>,
    pub quiet: bool,
    pub print_time: bool,
    pub cancellation_flag: Arc<AtomicUsize>,
}

pub fn generate_tags(
    path: &Path,
    name: &str,
    config: &TagsConfiguration,
    indent: bool,
    opts: &TagsOptions,
) -> Result<()> {
    let mut context = TagsContext::new();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    let indent_str = if indent {
        if !opts.quiet {
            writeln!(&mut stdout, "{name}")?;
        }
        "\t"
    } else {
        ""
    };

    let source = fs::read(path)?;
    let start = Instant::now();
    for tag in context
        .generate_tags(config, &source, Some(&opts.cancellation_flag))?
        .0
    {
        let tag = tag?;
        if !opts.quiet {
            write!(
                &mut stdout,
                "{indent_str}{:<10}\t | {:<8}\t{} {} - {} `{}`",
                str::from_utf8(&source[tag.name_range]).unwrap_or(""),
                &config.syntax_type_name(tag.syntax_type_id),
                if tag.is_definition { "def" } else { "ref" },
                tag.span.start,
                tag.span.end,
                str::from_utf8(&source[tag.line_range]).unwrap_or(""),
            )?;
            if let Some(docs) = tag.docs {
                if docs.len() > 120 {
                    write!(&mut stdout, "\t{:?}...", docs.get(0..120).unwrap_or(""))?;
                } else {
                    write!(&mut stdout, "\t{:?}", &docs)?;
                }
            }
            writeln!(&mut stdout)?;
        }
    }

    if opts.print_time {
        writeln!(
            &mut stdout,
            "{indent_str}time: {}ms",
            start.elapsed().as_millis(),
        )?;
    }

    Ok(())
}
