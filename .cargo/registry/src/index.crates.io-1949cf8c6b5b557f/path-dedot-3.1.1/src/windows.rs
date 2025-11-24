use std::{
    borrow::Cow,
    ffi::OsString,
    io::{self, ErrorKind},
    path::{Component, Path, PathBuf, PrefixComponent},
};

use crate::{ParseDot, MAIN_SEPARATOR};

impl ParseDot for Path {
    #[inline]
    fn parse_dot(&self) -> io::Result<Cow<Path>> {
        let cwd = get_cwd!();

        self.parse_dot_from(&cwd)
    }

    fn parse_dot_from(&self, cwd: impl AsRef<Path>) -> io::Result<Cow<Path>> {
        let mut iter = self.components();

        let mut has_dots = false;

        if let Some(first_component) = iter.next() {
            let mut tokens = Vec::new();

            let (has_prefix, first_is_root) = match first_component {
                Component::Prefix(prefix) => {
                    tokens.push(prefix.as_os_str());

                    if let Some(second_component) = iter.next() {
                        match second_component {
                            Component::RootDir => {
                                tokens.push(MAIN_SEPARATOR.as_os_str());

                                (true, true)
                            },
                            Component::CurDir => {
                                // may be unreachable

                                has_dots = true;

                                let cwd = cwd.as_ref();

                                for token in cwd.iter().skip(if cwd.get_path_prefix().is_some() {
                                    1
                                } else {
                                    0
                                }) {
                                    tokens.push(token);
                                }

                                (true, tokens.len() > 1 && tokens[1] == MAIN_SEPARATOR.as_os_str())
                            },
                            Component::ParentDir => {
                                has_dots = true;

                                let cwd = cwd.as_ref();

                                match cwd.parent() {
                                    Some(cwd_parent) => {
                                        for token in cwd_parent.iter().skip(
                                            if cwd.get_path_prefix().is_some() { 1 } else { 0 },
                                        ) {
                                            tokens.push(token);
                                        }

                                        (
                                            true,
                                            tokens.len() > 1
                                                && tokens[1] == MAIN_SEPARATOR.as_os_str(),
                                        )
                                    },
                                    None => {
                                        if cwd.get_path_prefix().is_some() {
                                            if cwd.is_absolute() {
                                                tokens.push(MAIN_SEPARATOR.as_os_str());

                                                (true, true)
                                            } else {
                                                (true, false)
                                            }
                                        } else {
                                            // don't care about `cwd` is "\\" or "\\\"
                                            if cwd == MAIN_SEPARATOR.as_os_str() {
                                                tokens.push(MAIN_SEPARATOR.as_os_str());

                                                (true, true)
                                            } else {
                                                (true, false)
                                            }
                                        }
                                    },
                                }
                            },
                            _ => {
                                let path_str = self.as_os_str().to_str().ok_or_else(|| {
                                    io::Error::new(ErrorKind::Other, "The path is not valid UTF-8.")
                                })?;

                                if path_str[first_component.as_os_str().len()..].starts_with(r".\")
                                {
                                    has_dots = true;

                                    let out =
                                        {
                                            let cwd = cwd.as_ref();

                                            for token in cwd.iter().skip(
                                                if cwd.get_path_prefix().is_some() { 1 } else { 0 },
                                            ) {
                                                tokens.push(token);
                                            }

                                            (
                                                true,
                                                tokens.len() > 1
                                                    && tokens[1] == MAIN_SEPARATOR.as_os_str(),
                                            )
                                        };

                                    tokens.push(second_component.as_os_str());

                                    out
                                } else {
                                    tokens.push(second_component.as_os_str());

                                    (true, false)
                                }
                            },
                        }
                    } else {
                        (true, false)
                    }
                },
                Component::RootDir => {
                    tokens.push(MAIN_SEPARATOR.as_os_str());

                    (false, true)
                },
                Component::CurDir => {
                    has_dots = true;

                    let cwd = cwd.as_ref();

                    for token in cwd.iter() {
                        tokens.push(token);
                    }

                    if cwd.get_path_prefix().is_some() {
                        (true, tokens.len() > 1 && tokens[1] == MAIN_SEPARATOR.as_os_str())
                    } else {
                        (false, !tokens.is_empty() && tokens[0] == MAIN_SEPARATOR.as_os_str())
                    }
                },
                Component::ParentDir => {
                    has_dots = true;

                    let cwd = cwd.as_ref();

                    match cwd.parent() {
                        Some(cwd_parent) => {
                            for token in cwd_parent.iter() {
                                tokens.push(token);
                            }

                            if cwd.get_path_prefix().is_some() {
                                (true, tokens.len() > 1 && tokens[1] == MAIN_SEPARATOR.as_os_str())
                            } else {
                                (
                                    false,
                                    !tokens.is_empty() && tokens[0] == MAIN_SEPARATOR.as_os_str(),
                                )
                            }
                        },
                        None => match cwd.get_path_prefix() {
                            Some(prefix) => {
                                tokens.push(prefix.as_os_str());

                                if cwd.is_absolute() {
                                    tokens.push(MAIN_SEPARATOR.as_os_str());

                                    (true, true)
                                } else {
                                    (true, false)
                                }
                            },
                            None => {
                                // don't care about `cwd` is "\\" or "\\\"
                                if cwd == MAIN_SEPARATOR.as_os_str() {
                                    tokens.push(MAIN_SEPARATOR.as_os_str());

                                    (false, true)
                                } else {
                                    (false, false)
                                }
                            },
                        },
                    }
                },
                Component::Normal(token) => {
                    tokens.push(token);

                    (false, false)
                },
            };

            for component in iter {
                match component {
                    Component::CurDir => {
                        // may be unreachable
                        has_dots = true;
                    },
                    Component::ParentDir => {
                        let tokens_length = tokens.len();

                        if tokens_length > 0
                            && ((tokens_length != 1 || (!first_is_root && !has_prefix))
                                && (tokens_length != 2 || !(first_is_root && has_prefix)))
                        {
                            tokens.remove(tokens_length - 1);
                        }

                        has_dots = true;
                    },
                    _ => {
                        tokens.push(component.as_os_str());
                    },
                }
            }

            let tokens_length = tokens.len();

            debug_assert!(tokens_length > 0);

            let mut size = tokens.iter().fold(tokens_length - 1, |acc, &x| acc + x.len());

            if has_prefix {
                if tokens_length > 1 {
                    size -= 1;

                    if first_is_root {
                        if tokens_length > 2 {
                            size -= 1;
                        } else if tokens[0].len() == self.as_os_str().len() {
                            // tokens_length == 2
                            // e.g.
                            // `\\server\share\` -> `\\server\share\`
                            // `\\server\share` -> `\\server\share\` should still be `\\server\share`
                            return Ok(Cow::from(self));
                        }
                    }
                }
            } else if first_is_root && tokens_length > 1 {
                size -= 1;
            }

            if has_dots || size != self.as_os_str().len() {
                let mut path_string = OsString::with_capacity(size);

                let mut iter = tokens.iter();

                path_string.push(iter.next().unwrap());

                if tokens_length > 1 {
                    if has_prefix {
                        if let Some(token) = iter.next() {
                            path_string.push(token);

                            if tokens_length > 2 {
                                if !first_is_root {
                                    path_string.push(MAIN_SEPARATOR.as_os_str());
                                }

                                for token in iter.take(tokens_length - 3) {
                                    path_string.push(token);

                                    path_string.push(MAIN_SEPARATOR.as_os_str());
                                }

                                path_string.push(tokens[tokens_length - 1]);
                            }
                        }
                    } else {
                        if !first_is_root {
                            path_string.push(MAIN_SEPARATOR.as_os_str());
                        }

                        for token in iter.take(tokens_length - 2) {
                            path_string.push(token);

                            path_string.push(MAIN_SEPARATOR.as_os_str());
                        }

                        path_string.push(tokens[tokens_length - 1]);
                    }
                }

                let path_buf = PathBuf::from(path_string);

                Ok(Cow::from(path_buf))
            } else {
                Ok(Cow::from(self))
            }
        } else {
            Ok(Cow::from(self))
        }
    }
}

pub trait ParsePrefix {
    fn get_path_prefix(&self) -> Option<PrefixComponent>;
}

impl ParsePrefix for Path {
    #[inline]
    fn get_path_prefix(&self) -> Option<PrefixComponent> {
        match self.components().next() {
            Some(Component::Prefix(prefix_component)) => Some(prefix_component),
            _ => None,
        }
    }
}

impl ParsePrefix for PathBuf {
    #[inline]
    fn get_path_prefix(&self) -> Option<PrefixComponent> {
        self.as_path().get_path_prefix()
    }
}
