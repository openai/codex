/*
 * Copyright 2019 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

// This is not public API, but it is used by Starlark command line utility.
#![doc(hidden)]

#[cfg(not(target_arch = "wasm32"))]
mod with_or_without_rustyline {
    use std::env;
    use std::io;

    use rustyline::error::ReadlineError;
    use rustyline::history::DefaultHistory;
    use rustyline::Editor;

    /// Wrapper for the readline library, whichever we are using at the moment.
    pub struct ReadLine {
        editor: Editor<(), DefaultHistory>,
        histfile: Option<String>,
    }

    impl ReadLine {
        pub fn new(histfile_env: &str) -> anyhow::Result<ReadLine> {
            let mut editor = Editor::new()?;
            let histfile = if let Ok(histfile) = env::var(histfile_env) {
                if let Err(e) = editor.load_history(&histfile) {
                    match e {
                        ReadlineError::Io(e) if e.kind() == io::ErrorKind::NotFound => {}
                        e => eprintln!("Failed to load history from `{}`: {}", histfile, e),
                    }
                }
                Some(histfile)
            } else {
                None
            };
            Ok(ReadLine { editor, histfile })
        }

        /// Read line. Return `None` on EOF or interrupt.
        pub fn read_line(&mut self, prompt: &str) -> anyhow::Result<Option<String>> {
            match self.editor.readline(prompt) {
                Ok(line) => {
                    self.editor.add_history_entry(line.as_str())?;
                    if let Some(histfile) = &self.histfile {
                        if let Err(e) = self.editor.save_history(&histfile) {
                            eprintln!("Failed to save history to `{}`: {}", histfile, e);
                        }
                    }
                    Ok(Some(line))
                }
                // User pressed EOF - disconnected terminal, or similar
                Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => Ok(None),
                Err(e) => Err(e.into()),
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod with_or_without_rustyline {
    #[derive(thiserror::Error, Debug)]
    #[error("Rustyline is not supported on wasm32")]
    struct NoRustyline;

    pub struct ReadLine(());

    impl ReadLine {
        pub fn new(_histfile_env: &str) -> anyhow::Result<ReadLine> {
            Err(NoRustyline.into())
        }

        pub fn read_line(&mut self, _prompt: &str) -> anyhow::Result<Option<String>> {
            Err(NoRustyline.into())
        }
    }
}

pub use with_or_without_rustyline::ReadLine;
