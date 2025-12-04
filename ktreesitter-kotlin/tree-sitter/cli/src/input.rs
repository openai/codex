use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc,
    },
};

use anyhow::{anyhow, bail, Context, Result};
use glob::glob;

use crate::test::{parse_tests, TestEntry};

pub enum CliInput {
    Paths(Vec<PathBuf>),
    Test {
        name: String,
        contents: Vec<u8>,
        languages: Vec<Box<str>>,
    },
    Stdin(Vec<u8>),
}

pub fn get_input(
    paths_file: Option<&Path>,
    paths: Option<Vec<PathBuf>>,
    test_number: Option<u32>,
    cancellation_flag: &Arc<AtomicUsize>,
) -> Result<CliInput> {
    if let Some(paths_file) = paths_file {
        return Ok(CliInput::Paths(
            fs::read_to_string(paths_file)
                .with_context(|| format!("Failed to read paths file {}", paths_file.display()))?
                .trim()
                .lines()
                .map(PathBuf::from)
                .collect::<Vec<_>>(),
        ));
    }

    if let Some(test_number) = test_number {
        let current_dir = std::env::current_dir().unwrap();
        let test_dir = current_dir.join("test").join("corpus");

        if !test_dir.exists() {
            return Err(anyhow!(
                "Test corpus directory not found in current directory, see https://tree-sitter.github.io/tree-sitter/creating-parsers/5-writing-tests"
            ));
        }

        let test_entry = parse_tests(&test_dir)?;
        let mut test_num = 0;
        let Some((name, contents, languages)) =
            get_test_info(&test_entry, test_number.max(1) - 1, &mut test_num)
        else {
            return Err(anyhow!("Failed to fetch contents of test #{test_number}"));
        };

        return Ok(CliInput::Test {
            name,
            contents,
            languages,
        });
    }

    if let Some(paths) = paths {
        let mut result = Vec::new();

        let mut incorporate_path = |path: PathBuf, positive| {
            if positive {
                result.push(path);
            } else if let Some(index) = result.iter().position(|p| *p == path) {
                result.remove(index);
            }
        };

        for mut path in paths {
            let mut positive = true;
            if path.starts_with("!") {
                positive = false;
                path = path.strip_prefix("!").unwrap().to_path_buf();
            }

            if path.exists() {
                incorporate_path(path, positive);
            } else {
                let Some(path_str) = path.to_str() else {
                    bail!("Invalid path: {}", path.display());
                };
                let paths = glob(path_str)
                    .with_context(|| format!("Invalid glob pattern {}", path.display()))?;
                for path in paths {
                    incorporate_path(path?, positive);
                }
            }
        }

        if result.is_empty() {
            return Err(anyhow!(
                "No files were found at or matched by the provided pathname/glob"
            ));
        }

        return Ok(CliInput::Paths(result));
    }

    let reader_flag = cancellation_flag.clone();
    let (tx, rx) = mpsc::channel();

    // Spawn a thread to read from stdin, until ctrl-c or EOF is received
    std::thread::spawn(move || {
        let mut input = Vec::new();
        let stdin = std::io::stdin();
        let mut handle = stdin.lock();

        // Read in chunks, so we can check the ctrl-c flag
        loop {
            if reader_flag.load(Ordering::Relaxed) == 1 {
                break;
            }
            let mut buffer = [0; 1024];
            match handle.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(n) => input.extend_from_slice(&buffer[..n]),
            }
        }

        // Signal to the main thread that we're done
        tx.send(input).ok();
    });

    loop {
        // If we've received a ctrl-c signal, exit
        if cancellation_flag.load(Ordering::Relaxed) == 1 {
            bail!("\n");
        }

        // If we're done receiving input from stdin, return it
        if let Ok(input) = rx.try_recv() {
            return Ok(CliInput::Stdin(input));
        }

        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

#[allow(clippy::type_complexity)]
pub fn get_test_info(
    test_entry: &TestEntry,
    target_test: u32,
    test_num: &mut u32,
) -> Option<(String, Vec<u8>, Vec<Box<str>>)> {
    match test_entry {
        TestEntry::Example {
            name,
            input,
            attributes,
            ..
        } => {
            if *test_num == target_test {
                return Some((name.clone(), input.clone(), attributes.languages.clone()));
            }
            *test_num += 1;
        }
        TestEntry::Group { children, .. } => {
            for child in children {
                if let Some((name, input, languages)) = get_test_info(child, target_test, test_num)
                {
                    return Some((name, input, languages));
                }
            }
        }
    }

    None
}

/// Writes `contents` to a temporary file and returns the path to that file.
pub fn get_tmp_source_file(contents: &[u8]) -> Result<PathBuf> {
    let parse_path = std::env::temp_dir().join(".tree-sitter-temp");
    let mut parse_file = std::fs::File::create(&parse_path)?;
    parse_file.write_all(contents)?;

    Ok(parse_path)
}
