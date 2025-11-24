//! History API

#[cfg(feature = "with-file-history")]
use fd_lock::RwLock;
#[cfg(feature = "with-file-history")]
use log::{debug, warn};
use std::borrow::Cow;
use std::collections::vec_deque;
use std::collections::VecDeque;
#[cfg(feature = "with-file-history")]
use std::fs::{File, OpenOptions};
#[cfg(feature = "with-file-history")]
use std::io::SeekFrom;
use std::ops::Index;
use std::path::Path;
#[cfg(feature = "with-file-history")]
use std::time::SystemTime;

use super::Result;
use crate::config::{Config, HistoryDuplicates};

/// Search direction
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchDirection {
    /// Search history forward
    Forward,
    /// Search history backward
    Reverse,
}

/// History search result
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SearchResult<'a> {
    /// history entry
    pub entry: Cow<'a, str>,
    /// history index
    pub idx: usize,
    /// match position in `entry`
    pub pos: usize,
}

/// Interface for navigating/loading/storing history
// TODO Split navigation part from backend part
pub trait History {
    // TODO jline3: interface Entry {
    //         int index();
    //         Instant time();
    //         String line();
    //     }
    // replxx: HistoryEntry {
    // 		std::string _timestamp;
    // 		std::string _text;

    // termwiz: fn get(&self, idx: HistoryIndex) -> Option<Cow<str>>;

    /// Return the history entry at position `index`, starting from 0.
    ///
    /// `SearchDirection` is useful only for implementations without direct
    /// indexing.
    fn get(&self, index: usize, dir: SearchDirection) -> Result<Option<SearchResult>>;

    // termwiz: fn last(&self) -> Option<HistoryIndex>;

    // jline3: default void add(String line) {
    //         add(Instant.now(), line);
    //     }
    // jline3: void add(Instant time, String line);
    // termwiz: fn add(&mut self, line: &str);
    // reedline: fn append(&mut self, entry: &str);

    /// Add a new entry in the history.
    fn add(&mut self, line: &str) -> Result<bool>;
    /// Add a new entry in the history.
    fn add_owned(&mut self, line: String) -> Result<bool>; // TODO check AsRef<str> + Into<String> vs object safe

    /// Return the number of entries in the history.
    #[must_use]
    fn len(&self) -> usize;

    /// Return true if the history has no entry.
    #[must_use]
    fn is_empty(&self) -> bool;

    // TODO jline3: int index();
    // TODO jline3: String current();
    // reedline: fn string_at_cursor(&self) -> Option<String>;
    // TODO jline3: boolean previous();
    // reedline: fn back(&mut self);
    // TODO jline3: boolean next();
    // reedline: fn forward(&mut self);
    // TODO jline3: boolean moveToFirst();
    // TODO jline3: boolean moveToFirst();
    // TODO jline3: boolean moveToLast();
    // TODO jline3: boolean moveTo(int index);
    // TODO jline3: void moveToEnd();
    // TODO jline3: void resetIndex();

    // TODO jline3: int first();
    // TODO jline3: default boolean isPersistable(Entry entry) {
    //         return true;
    //     }

    /// Set the maximum length for the history. This function can be called even
    /// if there is already some history, the function will make sure to retain
    /// just the latest `len` elements if the new history length value is
    /// smaller than the amount of items already inside the history.
    ///
    /// Like [stifle_history](http://tiswww.case.edu/php/chet/readline/history.html#IDX11).
    fn set_max_len(&mut self, len: usize) -> Result<()>;

    /// Ignore consecutive duplicates
    fn ignore_dups(&mut self, yes: bool) -> Result<()>;

    /// Ignore lines which begin with a space or not
    fn ignore_space(&mut self, yes: bool);

    /// Save the history in the specified file.
    // TODO history_truncate_file
    // https://tiswww.case.edu/php/chet/readline/history.html#IDX31
    fn save(&mut self, path: &Path) -> Result<()>; // FIXME Path vs AsRef<Path>

    /// Append new entries in the specified file.
    // Like [append_history](http://tiswww.case.edu/php/chet/readline/history.html#IDX30).
    fn append(&mut self, path: &Path) -> Result<()>; // FIXME Path vs AsRef<Path>

    /// Load the history from the specified file.
    ///
    /// # Errors
    /// Will return `Err` if path does not already exist or could not be read.
    fn load(&mut self, path: &Path) -> Result<()>; // FIXME Path vs AsRef<Path>

    /// Clear in-memory history
    fn clear(&mut self) -> Result<()>;

    // termwiz: fn search(
    //         &self,
    //         idx: HistoryIndex,
    //         style: SearchStyle,
    //         direction: SearchDirection,
    //         pattern: &str,
    //     ) -> Option<SearchResult>;
    // reedline: fn set_navigation(&mut self, navigation: HistoryNavigationQuery);
    // reedline: fn get_navigation(&self) -> HistoryNavigationQuery;

    /// Search history (start position inclusive [0, len-1]).
    ///
    /// Return the absolute index of the nearest history entry that matches
    /// `term`.
    ///
    /// Return None if no entry contains `term` between [start, len -1] for
    /// forward search
    /// or between [0, start] for reverse search.
    fn search(
        &self,
        term: &str,
        start: usize,
        dir: SearchDirection,
    ) -> Result<Option<SearchResult>>;

    /// Anchored search
    fn starts_with(
        &self,
        term: &str,
        start: usize,
        dir: SearchDirection,
    ) -> Result<Option<SearchResult>>;

    /* TODO How ? DoubleEndedIterator may be difficult to implement (for an SQLite backend)
    /// Return a iterator.
    #[must_use]
    fn iter(&self) -> impl DoubleEndedIterator<Item = &String> + '_;
     */
}

/// Transient in-memory history implementation.
pub struct MemHistory {
    entries: VecDeque<String>,
    max_len: usize,
    ignore_space: bool,
    ignore_dups: bool,
}

impl MemHistory {
    /// Default constructor
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(Config::default())
    }

    /// Customized constructor with:
    /// - `Config::max_history_size()`,
    /// - `Config::history_ignore_space()`,
    /// - `Config::history_duplicates()`.
    #[must_use]
    pub fn with_config(config: Config) -> Self {
        Self {
            entries: VecDeque::new(),
            max_len: config.max_history_size(),
            ignore_space: config.history_ignore_space(),
            ignore_dups: config.history_duplicates() == HistoryDuplicates::IgnoreConsecutive,
        }
    }

    fn search_match<F>(
        &self,
        term: &str,
        start: usize,
        dir: SearchDirection,
        test: F,
    ) -> Option<SearchResult>
    where
        F: Fn(&str) -> Option<usize>,
    {
        if term.is_empty() || start >= self.len() {
            return None;
        }
        match dir {
            SearchDirection::Reverse => {
                for (idx, entry) in self
                    .entries
                    .iter()
                    .rev()
                    .skip(self.len() - 1 - start)
                    .enumerate()
                {
                    if let Some(cursor) = test(entry) {
                        return Some(SearchResult {
                            idx: start - idx,
                            entry: Cow::Borrowed(entry),
                            pos: cursor,
                        });
                    }
                }
                None
            }
            SearchDirection::Forward => {
                for (idx, entry) in self.entries.iter().skip(start).enumerate() {
                    if let Some(cursor) = test(entry) {
                        return Some(SearchResult {
                            idx: idx + start,
                            entry: Cow::Borrowed(entry),
                            pos: cursor,
                        });
                    }
                }
                None
            }
        }
    }

    fn ignore(&self, line: &str) -> bool {
        if self.max_len == 0 {
            return true;
        }
        if line.is_empty()
            || (self.ignore_space && line.chars().next().map_or(true, char::is_whitespace))
        {
            return true;
        }
        if self.ignore_dups {
            if let Some(s) = self.entries.back() {
                if s == line {
                    return true;
                }
            }
        }
        false
    }

    fn insert(&mut self, line: String) {
        if self.entries.len() == self.max_len {
            self.entries.pop_front();
        }
        self.entries.push_back(line);
    }
}

impl Default for MemHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl History for MemHistory {
    fn get(&self, index: usize, _: SearchDirection) -> Result<Option<SearchResult>> {
        Ok(self
            .entries
            .get(index)
            .map(String::as_ref)
            .map(Cow::Borrowed)
            .map(|entry| SearchResult {
                entry,
                idx: index,
                pos: 0,
            }))
    }

    fn add(&mut self, line: &str) -> Result<bool> {
        if self.ignore(line) {
            return Ok(false);
        }
        self.insert(line.to_owned());
        Ok(true)
    }

    fn add_owned(&mut self, line: String) -> Result<bool> {
        if self.ignore(&line) {
            return Ok(false);
        }
        self.insert(line);
        Ok(true)
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn set_max_len(&mut self, len: usize) -> Result<()> {
        self.max_len = len;
        if self.len() > len {
            self.entries.drain(..self.len() - len);
        }
        Ok(())
    }

    fn ignore_dups(&mut self, yes: bool) -> Result<()> {
        self.ignore_dups = yes;
        Ok(())
    }

    fn ignore_space(&mut self, yes: bool) {
        self.ignore_space = yes;
    }

    fn save(&mut self, _: &Path) -> Result<()> {
        unimplemented!();
    }

    fn append(&mut self, _: &Path) -> Result<()> {
        unimplemented!();
    }

    fn load(&mut self, _: &Path) -> Result<()> {
        unimplemented!();
    }

    fn clear(&mut self) -> Result<()> {
        self.entries.clear();
        Ok(())
    }

    fn search(
        &self,
        term: &str,
        start: usize,
        dir: SearchDirection,
    ) -> Result<Option<SearchResult>> {
        #[cfg(not(feature = "case_insensitive_history_search"))]
        {
            let test = |entry: &str| entry.find(term);
            Ok(self.search_match(term, start, dir, test))
        }
        #[cfg(feature = "case_insensitive_history_search")]
        {
            use regex::{escape, RegexBuilder};
            Ok(
                if let Ok(re) = RegexBuilder::new(&escape(term))
                    .case_insensitive(true)
                    .build()
                {
                    let test = |entry: &str| re.find(entry).map(|m| m.start());
                    self.search_match(term, start, dir, test)
                } else {
                    None
                },
            )
        }
    }

    fn starts_with(
        &self,
        term: &str,
        start: usize,
        dir: SearchDirection,
    ) -> Result<Option<SearchResult>> {
        #[cfg(not(feature = "case_insensitive_history_search"))]
        {
            let test = |entry: &str| {
                if entry.starts_with(term) {
                    Some(term.len())
                } else {
                    None
                }
            };
            Ok(self.search_match(term, start, dir, test))
        }
        #[cfg(feature = "case_insensitive_history_search")]
        {
            use regex::{escape, RegexBuilder};
            Ok(
                if let Ok(re) = RegexBuilder::new(&escape(term))
                    .case_insensitive(true)
                    .build()
                {
                    let test = |entry: &str| {
                        re.find(entry)
                            .and_then(|m| if m.start() == 0 { Some(m) } else { None })
                            .map(|m| m.end())
                    };
                    self.search_match(term, start, dir, test)
                } else {
                    None
                },
            )
        }
    }
}

impl Index<usize> for MemHistory {
    type Output = String;

    fn index(&self, index: usize) -> &String {
        &self.entries[index]
    }
}

impl<'a> IntoIterator for &'a MemHistory {
    type IntoIter = vec_deque::Iter<'a, String>;
    type Item = &'a String;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

/// Current state of the history stored in a file.
#[derive(Default)]
#[cfg(feature = "with-file-history")]
pub struct FileHistory {
    mem: MemHistory,
    /// Number of entries inputted by user and not saved yet
    new_entries: usize,
    /// last path used by either `load` or `save`
    path_info: Option<PathInfo>,
}

// TODO impl Deref<MemHistory> for FileHistory ?

/// Last histo path, modified timestamp and size
#[cfg(feature = "with-file-history")]
struct PathInfo(std::path::PathBuf, SystemTime, usize);

#[cfg(feature = "with-file-history")]
impl FileHistory {
    // New multiline-aware history files start with `#V2\n` and have newlines
    // and backslashes escaped in them.
    const FILE_VERSION_V2: &'static str = "#V2";

    /// Default constructor
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(Config::default())
    }

    /// Customized constructor with:
    /// - `Config::max_history_size()`,
    /// - `Config::history_ignore_space()`,
    /// - `Config::history_duplicates()`.
    #[must_use]
    pub fn with_config(config: Config) -> Self {
        Self {
            mem: MemHistory::with_config(config),
            new_entries: 0,
            path_info: None,
        }
    }

    fn save_to(&mut self, file: &File, append: bool) -> Result<()> {
        use std::io::{BufWriter, Write};

        fix_perm(file);
        let mut wtr = BufWriter::new(file);
        let first_new_entry = if append {
            self.mem.len().saturating_sub(self.new_entries)
        } else {
            wtr.write_all(Self::FILE_VERSION_V2.as_bytes())?;
            wtr.write_all(b"\n")?;
            0
        };
        for entry in self.mem.entries.iter().skip(first_new_entry) {
            let mut bytes = entry.as_bytes();
            while let Some(i) = memchr::memchr2(b'\\', b'\n', bytes) {
                let (head, tail) = bytes.split_at(i);
                wtr.write_all(head)?;

                let (&escapable_byte, tail) = tail
                    .split_first()
                    .expect("memchr guarantees i is a valid index");
                if escapable_byte == b'\n' {
                    wtr.write_all(br"\n")?; // escaped line feed
                } else {
                    debug_assert_eq!(escapable_byte, b'\\');
                    wtr.write_all(br"\\")?; // escaped backslash
                }
                bytes = tail;
            }
            wtr.write_all(bytes)?; // remaining bytes with no \n or \
            wtr.write_all(b"\n")?;
        }
        // https://github.com/rust-lang/rust/issues/32677#issuecomment-204833485
        wtr.flush()?;
        Ok(())
    }

    fn load_from(&mut self, file: &File) -> Result<bool> {
        use std::io::{BufRead, BufReader};

        let rdr = BufReader::new(file);
        let mut lines = rdr.lines();
        let mut v2 = false;
        if let Some(first) = lines.next() {
            let line = first?;
            if line == Self::FILE_VERSION_V2 {
                v2 = true;
            } else {
                self.add_owned(line)?;
            }
        }
        let mut appendable = v2;
        for line in lines {
            let mut line = line?;
            if line.is_empty() {
                continue;
            }
            if v2 {
                let mut copy = None; // lazily copy line if unescaping is needed
                let mut str = line.as_str();
                while let Some(i) = str.find('\\') {
                    if copy.is_none() {
                        copy = Some(String::with_capacity(line.len()));
                    }
                    let s = copy.as_mut().unwrap();
                    s.push_str(&str[..i]);
                    let j = i + 1; // escaped char idx
                    let b = if j < str.len() {
                        str.as_bytes()[j]
                    } else {
                        0 // unexpected if History::save works properly
                    };
                    match b {
                        b'n' => {
                            s.push('\n'); // unescaped line feed
                        }
                        b'\\' => {
                            s.push('\\'); // unescaped back slash
                        }
                        _ => {
                            // only line feed and back slash should have been escaped
                            warn!(target: "rustyline", "bad escaped line: {}", line);
                            copy = None;
                            break;
                        }
                    }
                    str = &str[j + 1..];
                }
                if let Some(mut s) = copy {
                    s.push_str(str); // remaining bytes with no escaped char
                    line = s;
                }
            }
            appendable &= self.add_owned(line)?; // TODO truncate to MAX_LINE
        }
        self.new_entries = 0; // TODO we may lost new entries if loaded lines < max_len
        Ok(appendable)
    }

    fn update_path(&mut self, path: &Path, file: &File, size: usize) -> Result<()> {
        let modified = file.metadata()?.modified()?;
        if let Some(PathInfo(
            ref mut previous_path,
            ref mut previous_modified,
            ref mut previous_size,
        )) = self.path_info
        {
            if previous_path.as_path() != path {
                *previous_path = path.to_owned();
            }
            *previous_modified = modified;
            *previous_size = size;
        } else {
            self.path_info = Some(PathInfo(path.to_owned(), modified, size));
        }
        debug!(target: "rustyline", "PathInfo({:?}, {:?}, {})", path, modified, size);
        Ok(())
    }

    fn can_just_append(&self, path: &Path, file: &File) -> Result<bool> {
        if let Some(PathInfo(ref previous_path, ref previous_modified, ref previous_size)) =
            self.path_info
        {
            if previous_path.as_path() != path {
                debug!(target: "rustyline", "cannot append: {:?} <> {:?}", previous_path, path);
                return Ok(false);
            }
            let modified = file.metadata()?.modified()?;
            if *previous_modified != modified
                || self.mem.max_len <= *previous_size
                || self.mem.max_len < (*previous_size).saturating_add(self.new_entries)
            {
                debug!(target: "rustyline", "cannot append: {:?} < {:?} or {} < {} + {}",
                       previous_modified, modified, self.mem.max_len, previous_size, self.new_entries);
                Ok(false)
            } else {
                Ok(true)
            }
        } else {
            Ok(false)
        }
    }

    /// Return a forward iterator.
    #[must_use]
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &String> + '_ {
        self.mem.entries.iter()
    }
}

/// Default transient in-memory history implementation
#[cfg(not(feature = "with-file-history"))]
pub type DefaultHistory = MemHistory;
/// Default file-based history implementation
#[cfg(feature = "with-file-history")]
pub type DefaultHistory = FileHistory;

#[cfg(feature = "with-file-history")]
impl History for FileHistory {
    fn get(&self, index: usize, dir: SearchDirection) -> Result<Option<SearchResult>> {
        self.mem.get(index, dir)
    }

    fn add(&mut self, line: &str) -> Result<bool> {
        if self.mem.add(line)? {
            self.new_entries = self.new_entries.saturating_add(1).min(self.len());
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn add_owned(&mut self, line: String) -> Result<bool> {
        if self.mem.add_owned(line)? {
            self.new_entries = self.new_entries.saturating_add(1).min(self.len());
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn len(&self) -> usize {
        self.mem.len()
    }

    fn is_empty(&self) -> bool {
        self.mem.is_empty()
    }

    fn set_max_len(&mut self, len: usize) -> Result<()> {
        self.mem.set_max_len(len)?;
        self.new_entries = self.new_entries.min(len);
        Ok(())
    }

    fn ignore_dups(&mut self, yes: bool) -> Result<()> {
        self.mem.ignore_dups(yes)
    }

    fn ignore_space(&mut self, yes: bool) {
        self.mem.ignore_space(yes);
    }

    fn save(&mut self, path: &Path) -> Result<()> {
        if self.is_empty() || self.new_entries == 0 {
            return Ok(());
        }
        let old_umask = umask();
        let f = File::create(path);
        restore_umask(old_umask);
        let file = f?;
        let mut lock = RwLock::new(file);
        let lock_guard = lock.write()?;
        self.save_to(&lock_guard, false)?;
        self.new_entries = 0;
        self.update_path(path, &lock_guard, self.len())
    }

    fn append(&mut self, path: &Path) -> Result<()> {
        use std::io::Seek;

        if self.is_empty() || self.new_entries == 0 {
            return Ok(());
        }
        if !path.exists() || self.new_entries == self.mem.max_len {
            return self.save(path);
        }
        let file = OpenOptions::new().write(true).read(true).open(path)?;
        let mut lock = RwLock::new(file);
        let mut lock_guard = lock.write()?;
        if self.can_just_append(path, &lock_guard)? {
            lock_guard.seek(SeekFrom::End(0))?;
            self.save_to(&lock_guard, true)?;
            let size = self
                .path_info
                .as_ref()
                .unwrap()
                .2
                .saturating_add(self.new_entries);
            self.new_entries = 0;
            return self.update_path(path, &lock_guard, size);
        }
        // we may need to truncate file before appending new entries
        let mut other = Self {
            mem: MemHistory {
                entries: VecDeque::new(),
                max_len: self.mem.max_len,
                ignore_space: self.mem.ignore_space,
                ignore_dups: self.mem.ignore_dups,
            },
            new_entries: 0,
            path_info: None,
        };
        other.load_from(&lock_guard)?;
        let first_new_entry = self.mem.len().saturating_sub(self.new_entries);
        for entry in self.mem.entries.iter().skip(first_new_entry) {
            other.add(entry)?;
        }
        lock_guard.seek(SeekFrom::Start(0))?;
        lock_guard.set_len(0)?; // if new size < old size
        other.save_to(&lock_guard, false)?;
        self.update_path(path, &lock_guard, other.len())?;
        self.new_entries = 0;
        Ok(())
    }

    fn load(&mut self, path: &Path) -> Result<()> {
        let file = File::open(path)?;
        let lock = RwLock::new(file);
        let lock_guard = lock.read()?;
        let len = self.len();
        if self.load_from(&lock_guard)? {
            self.update_path(path, &lock_guard, self.len() - len)
        } else {
            // discard old version on next save
            self.path_info = None;
            Ok(())
        }
    }

    fn clear(&mut self) -> Result<()> {
        self.mem.clear()?;
        self.new_entries = 0;
        Ok(())
    }

    fn search(
        &self,
        term: &str,
        start: usize,
        dir: SearchDirection,
    ) -> Result<Option<SearchResult>> {
        self.mem.search(term, start, dir)
    }

    fn starts_with(
        &self,
        term: &str,
        start: usize,
        dir: SearchDirection,
    ) -> Result<Option<SearchResult>> {
        self.mem.starts_with(term, start, dir)
    }
}

#[cfg(feature = "with-file-history")]
impl Index<usize> for FileHistory {
    type Output = String;

    fn index(&self, index: usize) -> &String {
        &self.mem.entries[index]
    }
}

#[cfg(feature = "with-file-history")]
impl<'a> IntoIterator for &'a FileHistory {
    type IntoIter = vec_deque::Iter<'a, String>;
    type Item = &'a String;

    fn into_iter(self) -> Self::IntoIter {
        self.mem.entries.iter()
    }
}

#[cfg(feature = "with-file-history")]
cfg_if::cfg_if! {
    if #[cfg(any(windows, target_arch = "wasm32"))] {
        fn umask() -> u16 {
            0
        }

        fn restore_umask(_: u16) {}

        fn fix_perm(_: &File) {}
    } else if #[cfg(unix)] {
        use nix::sys::stat::{self, Mode, fchmod};
        fn umask() -> Mode {
            stat::umask(Mode::S_IXUSR | Mode::S_IRWXG | Mode::S_IRWXO)
        }

        fn restore_umask(old_umask: Mode) {
            stat::umask(old_umask);
        }

        fn fix_perm(file: &File) {
            use std::os::unix::io::AsRawFd;
            let _ = fchmod(file.as_raw_fd(), Mode::S_IRUSR | Mode::S_IWUSR);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DefaultHistory, History, SearchDirection, SearchResult};
    use crate::config::Config;
    use crate::Result;

    fn init() -> DefaultHistory {
        let mut history = DefaultHistory::new();
        assert!(history.add("line1").unwrap());
        assert!(history.add("line2").unwrap());
        assert!(history.add("line3").unwrap());
        history
    }

    #[test]
    fn new() {
        let history = DefaultHistory::new();
        assert_eq!(0, history.len());
    }

    #[test]
    fn add() {
        let config = Config::builder().history_ignore_space(true).build();
        let mut history = DefaultHistory::with_config(config);
        #[cfg(feature = "with-file-history")]
        assert_eq!(config.max_history_size(), history.mem.max_len);
        assert!(history.add("line1").unwrap());
        assert!(history.add("line2").unwrap());
        assert!(!history.add("line2").unwrap());
        assert!(!history.add("").unwrap());
        assert!(!history.add(" line3").unwrap());
    }

    #[test]
    fn set_max_len() {
        let mut history = init();
        history.set_max_len(1).unwrap();
        assert_eq!(1, history.len());
        assert_eq!(Some(&"line3".to_owned()), history.into_iter().last());
    }

    #[test]
    #[cfg(feature = "with-file-history")]
    #[cfg_attr(miri, ignore)] // unsupported operation: `getcwd` not available when isolation is enabled
    fn save() -> Result<()> {
        check_save("line\nfour \\ abc")
    }

    #[test]
    #[cfg(feature = "with-file-history")]
    #[cfg_attr(miri, ignore)] // unsupported operation: `open` not available when isolation is enabled
    fn save_windows_path() -> Result<()> {
        let path = "cd source\\repos\\forks\\nushell\\";
        check_save(path)
    }

    #[cfg(feature = "with-file-history")]
    fn check_save(line: &str) -> Result<()> {
        let mut history = init();
        assert!(history.add(line)?);
        let tf = tempfile::NamedTempFile::new()?;

        history.save(tf.path())?;
        let mut history2 = DefaultHistory::new();
        history2.load(tf.path())?;
        for (a, b) in history.iter().zip(history2.iter()) {
            assert_eq!(a, b);
        }
        tf.close()?;
        Ok(())
    }

    #[test]
    #[cfg(feature = "with-file-history")]
    #[cfg_attr(miri, ignore)] // unsupported operation: `getcwd` not available when isolation is enabled
    fn load_legacy() -> Result<()> {
        use std::io::Write;
        let tf = tempfile::NamedTempFile::new()?;
        {
            let mut legacy = std::fs::File::create(tf.path())?;
            // Some data we'd accidentally corrupt if we got the version wrong
            let data = b"\
                test\\n \\abc \\123\n\
                123\\n\\\\n\n\
                abcde
            ";
            legacy.write_all(data)?;
            legacy.flush()?;
        }
        let mut history = DefaultHistory::new();
        history.load(tf.path())?;
        assert_eq!(history[0], "test\\n \\abc \\123");
        assert_eq!(history[1], "123\\n\\\\n");
        assert_eq!(history[2], "abcde");

        tf.close()?;
        Ok(())
    }

    #[test]
    #[cfg(feature = "with-file-history")]
    #[cfg_attr(miri, ignore)] // unsupported operation: `getcwd` not available when isolation is enabled
    fn append() -> Result<()> {
        let mut history = init();
        let tf = tempfile::NamedTempFile::new()?;

        history.append(tf.path())?;

        let mut history2 = DefaultHistory::new();
        history2.load(tf.path())?;
        history2.add("line4")?;
        history2.append(tf.path())?;

        history.add("line5")?;
        history.append(tf.path())?;

        let mut history3 = DefaultHistory::new();
        history3.load(tf.path())?;
        assert_eq!(history3.len(), 5);

        tf.close()?;
        Ok(())
    }

    #[test]
    #[cfg(feature = "with-file-history")]
    #[cfg_attr(miri, ignore)] // unsupported operation: `getcwd` not available when isolation is enabled
    fn truncate() -> Result<()> {
        let tf = tempfile::NamedTempFile::new()?;

        let config = Config::builder().history_ignore_dups(false)?.build();
        let mut history = DefaultHistory::with_config(config);
        history.add("line1")?;
        history.add("line1")?;
        history.append(tf.path())?;

        let mut history = DefaultHistory::new();
        history.load(tf.path())?;
        history.add("l")?;
        history.append(tf.path())?;

        let mut history = DefaultHistory::new();
        history.load(tf.path())?;
        assert_eq!(history.len(), 2);
        assert_eq!(history[1], "l");

        tf.close()?;
        Ok(())
    }

    #[test]
    fn search() -> Result<()> {
        let history = init();
        assert_eq!(None, history.search("", 0, SearchDirection::Forward)?);
        assert_eq!(None, history.search("none", 0, SearchDirection::Forward)?);
        assert_eq!(None, history.search("line", 3, SearchDirection::Forward)?);

        assert_eq!(
            Some(SearchResult {
                idx: 0,
                entry: history.get(0, SearchDirection::Forward)?.unwrap().entry,
                pos: 0
            }),
            history.search("line", 0, SearchDirection::Forward)?
        );
        assert_eq!(
            Some(SearchResult {
                idx: 1,
                entry: history.get(1, SearchDirection::Forward)?.unwrap().entry,
                pos: 0
            }),
            history.search("line", 1, SearchDirection::Forward)?
        );
        assert_eq!(
            Some(SearchResult {
                idx: 2,
                entry: history.get(2, SearchDirection::Forward)?.unwrap().entry,
                pos: 0
            }),
            history.search("line3", 1, SearchDirection::Forward)?
        );
        Ok(())
    }

    #[test]
    fn reverse_search() -> Result<()> {
        let history = init();
        assert_eq!(None, history.search("", 2, SearchDirection::Reverse)?);
        assert_eq!(None, history.search("none", 2, SearchDirection::Reverse)?);
        assert_eq!(None, history.search("line", 3, SearchDirection::Reverse)?);

        assert_eq!(
            Some(SearchResult {
                idx: 2,
                entry: history.get(2, SearchDirection::Reverse)?.unwrap().entry,
                pos: 0
            }),
            history.search("line", 2, SearchDirection::Reverse)?
        );
        assert_eq!(
            Some(SearchResult {
                idx: 1,
                entry: history.get(1, SearchDirection::Reverse)?.unwrap().entry,
                pos: 0
            }),
            history.search("line", 1, SearchDirection::Reverse)?
        );
        assert_eq!(
            Some(SearchResult {
                idx: 0,
                entry: history.get(0, SearchDirection::Reverse)?.unwrap().entry,
                pos: 0
            }),
            history.search("line1", 1, SearchDirection::Reverse)?
        );
        Ok(())
    }

    #[test]
    #[cfg(feature = "case_insensitive_history_search")]
    fn anchored_search() -> Result<()> {
        let history = init();
        assert_eq!(
            Some(SearchResult {
                idx: 2,
                entry: history.get(2, SearchDirection::Reverse)?.unwrap().entry,
                pos: 4
            }),
            history.starts_with("LiNe", 2, SearchDirection::Reverse)?
        );
        assert_eq!(
            None,
            history.starts_with("iNe", 2, SearchDirection::Reverse)?
        );
        Ok(())
    }
}
