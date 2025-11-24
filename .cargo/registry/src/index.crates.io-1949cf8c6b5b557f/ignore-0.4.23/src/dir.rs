// This module provides a data structure, `Ignore`, that connects "directory
// traversal" with "ignore matchers." Specifically, it knows about gitignore
// semantics and precedence, and is organized based on directory hierarchy.
// Namely, every matcher logically corresponds to ignore rules from a single
// directory, and points to the matcher for its corresponding parent directory.
// In this sense, `Ignore` is a *persistent* data structure.
//
// This design was specifically chosen to make it possible to use this data
// structure in a parallel directory iterator.
//
// My initial intention was to expose this module as part of this crate's
// public API, but I think the data structure's public API is too complicated
// with non-obvious failure modes. Alas, such things haven't been documented
// well.

use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    fs::{File, FileType},
    io::{self, BufRead},
    path::{Path, PathBuf},
    sync::{Arc, RwLock, Weak},
};

use crate::{
    gitignore::{self, Gitignore, GitignoreBuilder},
    overrides::{self, Override},
    pathutil::{is_hidden, strip_prefix},
    types::{self, Types},
    walk::DirEntry,
    {Error, Match, PartialErrorBuilder},
};

/// IgnoreMatch represents information about where a match came from when using
/// the `Ignore` matcher.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(crate) struct IgnoreMatch<'a>(IgnoreMatchInner<'a>);

/// IgnoreMatchInner describes precisely where the match information came from.
/// This is private to allow expansion to more matchers in the future.
#[derive(Clone, Debug)]
#[allow(dead_code)]
enum IgnoreMatchInner<'a> {
    Override(overrides::Glob<'a>),
    Gitignore(&'a gitignore::Glob),
    Types(types::Glob<'a>),
    Hidden,
}

impl<'a> IgnoreMatch<'a> {
    fn overrides(x: overrides::Glob<'a>) -> IgnoreMatch<'a> {
        IgnoreMatch(IgnoreMatchInner::Override(x))
    }

    fn gitignore(x: &'a gitignore::Glob) -> IgnoreMatch<'a> {
        IgnoreMatch(IgnoreMatchInner::Gitignore(x))
    }

    fn types(x: types::Glob<'a>) -> IgnoreMatch<'a> {
        IgnoreMatch(IgnoreMatchInner::Types(x))
    }

    fn hidden() -> IgnoreMatch<'static> {
        IgnoreMatch(IgnoreMatchInner::Hidden)
    }
}

/// Options for the ignore matcher, shared between the matcher itself and the
/// builder.
#[derive(Clone, Copy, Debug)]
struct IgnoreOptions {
    /// Whether to ignore hidden file paths or not.
    hidden: bool,
    /// Whether to read .ignore files.
    ignore: bool,
    /// Whether to respect any ignore files in parent directories.
    parents: bool,
    /// Whether to read git's global gitignore file.
    git_global: bool,
    /// Whether to read .gitignore files.
    git_ignore: bool,
    /// Whether to read .git/info/exclude files.
    git_exclude: bool,
    /// Whether to ignore files case insensitively
    ignore_case_insensitive: bool,
    /// Whether a git repository must be present in order to apply any
    /// git-related ignore rules.
    require_git: bool,
}

/// Ignore is a matcher useful for recursively walking one or more directories.
#[derive(Clone, Debug)]
pub(crate) struct Ignore(Arc<IgnoreInner>);

#[derive(Clone, Debug)]
struct IgnoreInner {
    /// A map of all existing directories that have already been
    /// compiled into matchers.
    ///
    /// Note that this is never used during matching, only when adding new
    /// parent directory matchers. This avoids needing to rebuild glob sets for
    /// parent directories if many paths are being searched.
    compiled: Arc<RwLock<HashMap<OsString, Weak<IgnoreInner>>>>,
    /// The path to the directory that this matcher was built from.
    dir: PathBuf,
    /// An override matcher (default is empty).
    overrides: Arc<Override>,
    /// A file type matcher.
    types: Arc<Types>,
    /// The parent directory to match next.
    ///
    /// If this is the root directory or there are otherwise no more
    /// directories to match, then `parent` is `None`.
    parent: Option<Ignore>,
    /// Whether this is an absolute parent matcher, as added by add_parent.
    is_absolute_parent: bool,
    /// The absolute base path of this matcher. Populated only if parent
    /// directories are added.
    absolute_base: Option<Arc<PathBuf>>,
    /// Explicit global ignore matchers specified by the caller.
    explicit_ignores: Arc<Vec<Gitignore>>,
    /// Ignore files used in addition to `.ignore`
    custom_ignore_filenames: Arc<Vec<OsString>>,
    /// The matcher for custom ignore files
    custom_ignore_matcher: Gitignore,
    /// The matcher for .ignore files.
    ignore_matcher: Gitignore,
    /// A global gitignore matcher, usually from $XDG_CONFIG_HOME/git/ignore.
    git_global_matcher: Arc<Gitignore>,
    /// The matcher for .gitignore files.
    git_ignore_matcher: Gitignore,
    /// Special matcher for `.git/info/exclude` files.
    git_exclude_matcher: Gitignore,
    /// Whether this directory contains a .git sub-directory.
    has_git: bool,
    /// Ignore config.
    opts: IgnoreOptions,
}

impl Ignore {
    /// Return the directory path of this matcher.
    pub(crate) fn path(&self) -> &Path {
        &self.0.dir
    }

    /// Return true if this matcher has no parent.
    pub(crate) fn is_root(&self) -> bool {
        self.0.parent.is_none()
    }

    /// Returns true if this matcher was added via the `add_parents` method.
    pub(crate) fn is_absolute_parent(&self) -> bool {
        self.0.is_absolute_parent
    }

    /// Return this matcher's parent, if one exists.
    pub(crate) fn parent(&self) -> Option<Ignore> {
        self.0.parent.clone()
    }

    /// Create a new `Ignore` matcher with the parent directories of `dir`.
    ///
    /// Note that this can only be called on an `Ignore` matcher with no
    /// parents (i.e., `is_root` returns `true`). This will panic otherwise.
    pub(crate) fn add_parents<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> (Ignore, Option<Error>) {
        if !self.0.opts.parents
            && !self.0.opts.git_ignore
            && !self.0.opts.git_exclude
            && !self.0.opts.git_global
        {
            // If we never need info from parent directories, then don't do
            // anything.
            return (self.clone(), None);
        }
        if !self.is_root() {
            panic!("Ignore::add_parents called on non-root matcher");
        }
        let absolute_base = match path.as_ref().canonicalize() {
            Ok(path) => Arc::new(path),
            Err(_) => {
                // There's not much we can do here, so just return our
                // existing matcher. We drop the error to be consistent
                // with our general pattern of ignoring I/O errors when
                // processing ignore files.
                return (self.clone(), None);
            }
        };
        // List of parents, from child to root.
        let mut parents = vec![];
        let mut path = &**absolute_base;
        while let Some(parent) = path.parent() {
            parents.push(parent);
            path = parent;
        }
        let mut errs = PartialErrorBuilder::default();
        let mut ig = self.clone();
        for parent in parents.into_iter().rev() {
            let mut compiled = self.0.compiled.write().unwrap();
            if let Some(weak) = compiled.get(parent.as_os_str()) {
                if let Some(prebuilt) = weak.upgrade() {
                    ig = Ignore(prebuilt);
                    continue;
                }
            }
            let (mut igtmp, err) = ig.add_child_path(parent);
            errs.maybe_push(err);
            igtmp.is_absolute_parent = true;
            igtmp.absolute_base = Some(absolute_base.clone());
            igtmp.has_git =
                if self.0.opts.require_git && self.0.opts.git_ignore {
                    parent.join(".git").exists()
                } else {
                    false
                };
            let ig_arc = Arc::new(igtmp);
            ig = Ignore(ig_arc.clone());
            compiled.insert(
                parent.as_os_str().to_os_string(),
                Arc::downgrade(&ig_arc),
            );
        }
        (ig, errs.into_error_option())
    }

    /// Create a new `Ignore` matcher for the given child directory.
    ///
    /// Since building the matcher may require reading from multiple
    /// files, it's possible that this method partially succeeds. Therefore,
    /// a matcher is always returned (which may match nothing) and an error is
    /// returned if it exists.
    ///
    /// Note that all I/O errors are completely ignored.
    pub(crate) fn add_child<P: AsRef<Path>>(
        &self,
        dir: P,
    ) -> (Ignore, Option<Error>) {
        let (ig, err) = self.add_child_path(dir.as_ref());
        (Ignore(Arc::new(ig)), err)
    }

    /// Like add_child, but takes a full path and returns an IgnoreInner.
    fn add_child_path(&self, dir: &Path) -> (IgnoreInner, Option<Error>) {
        let git_type = if self.0.opts.require_git
            && (self.0.opts.git_ignore || self.0.opts.git_exclude)
        {
            dir.join(".git").metadata().ok().map(|md| md.file_type())
        } else {
            None
        };
        let has_git = git_type.map(|_| true).unwrap_or(false);

        let mut errs = PartialErrorBuilder::default();
        let custom_ig_matcher = if self.0.custom_ignore_filenames.is_empty() {
            Gitignore::empty()
        } else {
            let (m, err) = create_gitignore(
                &dir,
                &dir,
                &self.0.custom_ignore_filenames,
                self.0.opts.ignore_case_insensitive,
            );
            errs.maybe_push(err);
            m
        };
        let ig_matcher = if !self.0.opts.ignore {
            Gitignore::empty()
        } else {
            let (m, err) = create_gitignore(
                &dir,
                &dir,
                &[".ignore"],
                self.0.opts.ignore_case_insensitive,
            );
            errs.maybe_push(err);
            m
        };
        let gi_matcher = if !self.0.opts.git_ignore {
            Gitignore::empty()
        } else {
            let (m, err) = create_gitignore(
                &dir,
                &dir,
                &[".gitignore"],
                self.0.opts.ignore_case_insensitive,
            );
            errs.maybe_push(err);
            m
        };
        let gi_exclude_matcher = if !self.0.opts.git_exclude {
            Gitignore::empty()
        } else {
            match resolve_git_commondir(dir, git_type) {
                Ok(git_dir) => {
                    let (m, err) = create_gitignore(
                        &dir,
                        &git_dir,
                        &["info/exclude"],
                        self.0.opts.ignore_case_insensitive,
                    );
                    errs.maybe_push(err);
                    m
                }
                Err(err) => {
                    errs.maybe_push(err);
                    Gitignore::empty()
                }
            }
        };
        let ig = IgnoreInner {
            compiled: self.0.compiled.clone(),
            dir: dir.to_path_buf(),
            overrides: self.0.overrides.clone(),
            types: self.0.types.clone(),
            parent: Some(self.clone()),
            is_absolute_parent: false,
            absolute_base: self.0.absolute_base.clone(),
            explicit_ignores: self.0.explicit_ignores.clone(),
            custom_ignore_filenames: self.0.custom_ignore_filenames.clone(),
            custom_ignore_matcher: custom_ig_matcher,
            ignore_matcher: ig_matcher,
            git_global_matcher: self.0.git_global_matcher.clone(),
            git_ignore_matcher: gi_matcher,
            git_exclude_matcher: gi_exclude_matcher,
            has_git,
            opts: self.0.opts,
        };
        (ig, errs.into_error_option())
    }

    /// Returns true if at least one type of ignore rule should be matched.
    fn has_any_ignore_rules(&self) -> bool {
        let opts = self.0.opts;
        let has_custom_ignore_files =
            !self.0.custom_ignore_filenames.is_empty();
        let has_explicit_ignores = !self.0.explicit_ignores.is_empty();

        opts.ignore
            || opts.git_global
            || opts.git_ignore
            || opts.git_exclude
            || has_custom_ignore_files
            || has_explicit_ignores
    }

    /// Like `matched`, but works with a directory entry instead.
    pub(crate) fn matched_dir_entry<'a>(
        &'a self,
        dent: &DirEntry,
    ) -> Match<IgnoreMatch<'a>> {
        let m = self.matched(dent.path(), dent.is_dir());
        if m.is_none() && self.0.opts.hidden && is_hidden(dent) {
            return Match::Ignore(IgnoreMatch::hidden());
        }
        m
    }

    /// Returns a match indicating whether the given file path should be
    /// ignored or not.
    ///
    /// The match contains information about its origin.
    fn matched<'a, P: AsRef<Path>>(
        &'a self,
        path: P,
        is_dir: bool,
    ) -> Match<IgnoreMatch<'a>> {
        // We need to be careful with our path. If it has a leading ./, then
        // strip it because it causes nothing but trouble.
        let mut path = path.as_ref();
        if let Some(p) = strip_prefix("./", path) {
            path = p;
        }
        // Match against the override patterns. If an override matches
        // regardless of whether it's whitelist/ignore, then we quit and
        // return that result immediately. Overrides have the highest
        // precedence.
        if !self.0.overrides.is_empty() {
            let mat = self
                .0
                .overrides
                .matched(path, is_dir)
                .map(IgnoreMatch::overrides);
            if !mat.is_none() {
                return mat;
            }
        }
        let mut whitelisted = Match::None;
        if self.has_any_ignore_rules() {
            let mat = self.matched_ignore(path, is_dir);
            if mat.is_ignore() {
                return mat;
            } else if mat.is_whitelist() {
                whitelisted = mat;
            }
        }
        if !self.0.types.is_empty() {
            let mat =
                self.0.types.matched(path, is_dir).map(IgnoreMatch::types);
            if mat.is_ignore() {
                return mat;
            } else if mat.is_whitelist() {
                whitelisted = mat;
            }
        }
        whitelisted
    }

    /// Performs matching only on the ignore files for this directory and
    /// all parent directories.
    fn matched_ignore<'a>(
        &'a self,
        path: &Path,
        is_dir: bool,
    ) -> Match<IgnoreMatch<'a>> {
        let (
            mut m_custom_ignore,
            mut m_ignore,
            mut m_gi,
            mut m_gi_exclude,
            mut m_explicit,
        ) = (Match::None, Match::None, Match::None, Match::None, Match::None);
        let any_git =
            !self.0.opts.require_git || self.parents().any(|ig| ig.0.has_git);
        let mut saw_git = false;
        for ig in self.parents().take_while(|ig| !ig.0.is_absolute_parent) {
            if m_custom_ignore.is_none() {
                m_custom_ignore =
                    ig.0.custom_ignore_matcher
                        .matched(path, is_dir)
                        .map(IgnoreMatch::gitignore);
            }
            if m_ignore.is_none() {
                m_ignore =
                    ig.0.ignore_matcher
                        .matched(path, is_dir)
                        .map(IgnoreMatch::gitignore);
            }
            if any_git && !saw_git && m_gi.is_none() {
                m_gi =
                    ig.0.git_ignore_matcher
                        .matched(path, is_dir)
                        .map(IgnoreMatch::gitignore);
            }
            if any_git && !saw_git && m_gi_exclude.is_none() {
                m_gi_exclude =
                    ig.0.git_exclude_matcher
                        .matched(path, is_dir)
                        .map(IgnoreMatch::gitignore);
            }
            saw_git = saw_git || ig.0.has_git;
        }
        if self.0.opts.parents {
            if let Some(abs_parent_path) = self.absolute_base() {
                // What we want to do here is take the absolute base path of
                // this directory and join it with the path we're searching.
                // The main issue we want to avoid is accidentally duplicating
                // directory components, so we try to strip any common prefix
                // off of `path`. Overall, this seems a little ham-fisted, but
                // it does fix a nasty bug. It should do fine until we overhaul
                // this crate.
                let dirpath = self.0.dir.as_path();
                let path_prefix = match strip_prefix("./", dirpath) {
                    None => dirpath,
                    Some(stripped_dot_slash) => stripped_dot_slash,
                };
                let path = match strip_prefix(path_prefix, path) {
                    None => abs_parent_path.join(path),
                    Some(p) => {
                        let p = match strip_prefix("/", p) {
                            None => p,
                            Some(p) => p,
                        };
                        abs_parent_path.join(p)
                    }
                };

                for ig in
                    self.parents().skip_while(|ig| !ig.0.is_absolute_parent)
                {
                    if m_custom_ignore.is_none() {
                        m_custom_ignore =
                            ig.0.custom_ignore_matcher
                                .matched(&path, is_dir)
                                .map(IgnoreMatch::gitignore);
                    }
                    if m_ignore.is_none() {
                        m_ignore =
                            ig.0.ignore_matcher
                                .matched(&path, is_dir)
                                .map(IgnoreMatch::gitignore);
                    }
                    if any_git && !saw_git && m_gi.is_none() {
                        m_gi =
                            ig.0.git_ignore_matcher
                                .matched(&path, is_dir)
                                .map(IgnoreMatch::gitignore);
                    }
                    if any_git && !saw_git && m_gi_exclude.is_none() {
                        m_gi_exclude =
                            ig.0.git_exclude_matcher
                                .matched(&path, is_dir)
                                .map(IgnoreMatch::gitignore);
                    }
                    saw_git = saw_git || ig.0.has_git;
                }
            }
        }
        for gi in self.0.explicit_ignores.iter().rev() {
            if !m_explicit.is_none() {
                break;
            }
            m_explicit = gi.matched(&path, is_dir).map(IgnoreMatch::gitignore);
        }
        let m_global = if any_git {
            self.0
                .git_global_matcher
                .matched(&path, is_dir)
                .map(IgnoreMatch::gitignore)
        } else {
            Match::None
        };

        m_custom_ignore
            .or(m_ignore)
            .or(m_gi)
            .or(m_gi_exclude)
            .or(m_global)
            .or(m_explicit)
    }

    /// Returns an iterator over parent ignore matchers, including this one.
    pub(crate) fn parents(&self) -> Parents<'_> {
        Parents(Some(self))
    }

    /// Returns the first absolute path of the first absolute parent, if
    /// one exists.
    fn absolute_base(&self) -> Option<&Path> {
        self.0.absolute_base.as_ref().map(|p| &***p)
    }
}

/// An iterator over all parents of an ignore matcher, including itself.
///
/// The lifetime `'a` refers to the lifetime of the initial `Ignore` matcher.
pub(crate) struct Parents<'a>(Option<&'a Ignore>);

impl<'a> Iterator for Parents<'a> {
    type Item = &'a Ignore;

    fn next(&mut self) -> Option<&'a Ignore> {
        match self.0.take() {
            None => None,
            Some(ig) => {
                self.0 = ig.0.parent.as_ref();
                Some(ig)
            }
        }
    }
}

/// A builder for creating an Ignore matcher.
#[derive(Clone, Debug)]
pub(crate) struct IgnoreBuilder {
    /// The root directory path for this ignore matcher.
    dir: PathBuf,
    /// An override matcher (default is empty).
    overrides: Arc<Override>,
    /// A type matcher (default is empty).
    types: Arc<Types>,
    /// Explicit global ignore matchers.
    explicit_ignores: Vec<Gitignore>,
    /// Ignore files in addition to .ignore.
    custom_ignore_filenames: Vec<OsString>,
    /// Ignore config.
    opts: IgnoreOptions,
}

impl IgnoreBuilder {
    /// Create a new builder for an `Ignore` matcher.
    ///
    /// All relative file paths are resolved with respect to the current
    /// working directory.
    pub(crate) fn new() -> IgnoreBuilder {
        IgnoreBuilder {
            dir: Path::new("").to_path_buf(),
            overrides: Arc::new(Override::empty()),
            types: Arc::new(Types::empty()),
            explicit_ignores: vec![],
            custom_ignore_filenames: vec![],
            opts: IgnoreOptions {
                hidden: true,
                ignore: true,
                parents: true,
                git_global: true,
                git_ignore: true,
                git_exclude: true,
                ignore_case_insensitive: false,
                require_git: true,
            },
        }
    }

    /// Builds a new `Ignore` matcher.
    ///
    /// The matcher returned won't match anything until ignore rules from
    /// directories are added to it.
    pub(crate) fn build(&self) -> Ignore {
        let git_global_matcher = if !self.opts.git_global {
            Gitignore::empty()
        } else {
            let mut builder = GitignoreBuilder::new("");
            builder
                .case_insensitive(self.opts.ignore_case_insensitive)
                .unwrap();
            let (gi, err) = builder.build_global();
            if let Some(err) = err {
                log::debug!("{}", err);
            }
            gi
        };

        Ignore(Arc::new(IgnoreInner {
            compiled: Arc::new(RwLock::new(HashMap::new())),
            dir: self.dir.clone(),
            overrides: self.overrides.clone(),
            types: self.types.clone(),
            parent: None,
            is_absolute_parent: true,
            absolute_base: None,
            explicit_ignores: Arc::new(self.explicit_ignores.clone()),
            custom_ignore_filenames: Arc::new(
                self.custom_ignore_filenames.clone(),
            ),
            custom_ignore_matcher: Gitignore::empty(),
            ignore_matcher: Gitignore::empty(),
            git_global_matcher: Arc::new(git_global_matcher),
            git_ignore_matcher: Gitignore::empty(),
            git_exclude_matcher: Gitignore::empty(),
            has_git: false,
            opts: self.opts,
        }))
    }

    /// Add an override matcher.
    ///
    /// By default, no override matcher is used.
    ///
    /// This overrides any previous setting.
    pub(crate) fn overrides(
        &mut self,
        overrides: Override,
    ) -> &mut IgnoreBuilder {
        self.overrides = Arc::new(overrides);
        self
    }

    /// Add a file type matcher.
    ///
    /// By default, no file type matcher is used.
    ///
    /// This overrides any previous setting.
    pub(crate) fn types(&mut self, types: Types) -> &mut IgnoreBuilder {
        self.types = Arc::new(types);
        self
    }

    /// Adds a new global ignore matcher from the ignore file path given.
    pub(crate) fn add_ignore(&mut self, ig: Gitignore) -> &mut IgnoreBuilder {
        self.explicit_ignores.push(ig);
        self
    }

    /// Add a custom ignore file name
    ///
    /// These ignore files have higher precedence than all other ignore files.
    ///
    /// When specifying multiple names, earlier names have lower precedence than
    /// later names.
    pub(crate) fn add_custom_ignore_filename<S: AsRef<OsStr>>(
        &mut self,
        file_name: S,
    ) -> &mut IgnoreBuilder {
        self.custom_ignore_filenames.push(file_name.as_ref().to_os_string());
        self
    }

    /// Enables ignoring hidden files.
    ///
    /// This is enabled by default.
    pub(crate) fn hidden(&mut self, yes: bool) -> &mut IgnoreBuilder {
        self.opts.hidden = yes;
        self
    }

    /// Enables reading `.ignore` files.
    ///
    /// `.ignore` files have the same semantics as `gitignore` files and are
    /// supported by search tools such as ripgrep and The Silver Searcher.
    ///
    /// This is enabled by default.
    pub(crate) fn ignore(&mut self, yes: bool) -> &mut IgnoreBuilder {
        self.opts.ignore = yes;
        self
    }

    /// Enables reading ignore files from parent directories.
    ///
    /// If this is enabled, then .gitignore files in parent directories of each
    /// file path given are respected. Otherwise, they are ignored.
    ///
    /// This is enabled by default.
    pub(crate) fn parents(&mut self, yes: bool) -> &mut IgnoreBuilder {
        self.opts.parents = yes;
        self
    }

    /// Add a global gitignore matcher.
    ///
    /// Its precedence is lower than both normal `.gitignore` files and
    /// `.git/info/exclude` files.
    ///
    /// This overwrites any previous global gitignore setting.
    ///
    /// This is enabled by default.
    pub(crate) fn git_global(&mut self, yes: bool) -> &mut IgnoreBuilder {
        self.opts.git_global = yes;
        self
    }

    /// Enables reading `.gitignore` files.
    ///
    /// `.gitignore` files have match semantics as described in the `gitignore`
    /// man page.
    ///
    /// This is enabled by default.
    pub(crate) fn git_ignore(&mut self, yes: bool) -> &mut IgnoreBuilder {
        self.opts.git_ignore = yes;
        self
    }

    /// Enables reading `.git/info/exclude` files.
    ///
    /// `.git/info/exclude` files have match semantics as described in the
    /// `gitignore` man page.
    ///
    /// This is enabled by default.
    pub(crate) fn git_exclude(&mut self, yes: bool) -> &mut IgnoreBuilder {
        self.opts.git_exclude = yes;
        self
    }

    /// Whether a git repository is required to apply git-related ignore
    /// rules (global rules, .gitignore and local exclude rules).
    ///
    /// When disabled, git-related ignore rules are applied even when searching
    /// outside a git repository.
    pub(crate) fn require_git(&mut self, yes: bool) -> &mut IgnoreBuilder {
        self.opts.require_git = yes;
        self
    }

    /// Process ignore files case insensitively
    ///
    /// This is disabled by default.
    pub(crate) fn ignore_case_insensitive(
        &mut self,
        yes: bool,
    ) -> &mut IgnoreBuilder {
        self.opts.ignore_case_insensitive = yes;
        self
    }
}

/// Creates a new gitignore matcher for the directory given.
///
/// The matcher is meant to match files below `dir`.
/// Ignore globs are extracted from each of the file names relative to
/// `dir_for_ignorefile` in the order given (earlier names have lower
/// precedence than later names).
///
/// I/O errors are ignored.
pub(crate) fn create_gitignore<T: AsRef<OsStr>>(
    dir: &Path,
    dir_for_ignorefile: &Path,
    names: &[T],
    case_insensitive: bool,
) -> (Gitignore, Option<Error>) {
    let mut builder = GitignoreBuilder::new(dir);
    let mut errs = PartialErrorBuilder::default();
    builder.case_insensitive(case_insensitive).unwrap();
    for name in names {
        let gipath = dir_for_ignorefile.join(name.as_ref());
        // This check is not necessary, but is added for performance. Namely,
        // a simple stat call checking for existence can often be just a bit
        // quicker than actually trying to open a file. Since the number of
        // directories without ignore files likely greatly exceeds the number
        // with ignore files, this check generally makes sense.
        //
        // However, until demonstrated otherwise, we speculatively do not do
        // this on Windows since Windows is notorious for having slow file
        // system operations. Namely, it's not clear whether this analysis
        // makes sense on Windows.
        //
        // For more details: https://github.com/BurntSushi/ripgrep/pull/1381
        if cfg!(windows) || gipath.exists() {
            errs.maybe_push_ignore_io(builder.add(gipath));
        }
    }
    let gi = match builder.build() {
        Ok(gi) => gi,
        Err(err) => {
            errs.push(err);
            GitignoreBuilder::new(dir).build().unwrap()
        }
    };
    (gi, errs.into_error_option())
}

/// Find the GIT_COMMON_DIR for the given git worktree.
///
/// This is the directory that may contain a private ignore file
/// "info/exclude". Unlike git, this function does *not* read environment
/// variables GIT_DIR and GIT_COMMON_DIR, because it is not clear how to use
/// them when multiple repositories are searched.
///
/// Some I/O errors are ignored.
fn resolve_git_commondir(
    dir: &Path,
    git_type: Option<FileType>,
) -> Result<PathBuf, Option<Error>> {
    let git_dir_path = || dir.join(".git");
    let git_dir = git_dir_path();
    if !git_type.map_or(false, |ft| ft.is_file()) {
        return Ok(git_dir);
    }
    let file = match File::open(git_dir) {
        Ok(file) => io::BufReader::new(file),
        Err(err) => {
            return Err(Some(Error::Io(err).with_path(git_dir_path())));
        }
    };
    let dot_git_line = match file.lines().next() {
        Some(Ok(line)) => line,
        Some(Err(err)) => {
            return Err(Some(Error::Io(err).with_path(git_dir_path())));
        }
        None => return Err(None),
    };
    if !dot_git_line.starts_with("gitdir: ") {
        return Err(None);
    }
    let real_git_dir = PathBuf::from(&dot_git_line["gitdir: ".len()..]);
    let git_commondir_file = || real_git_dir.join("commondir");
    let file = match File::open(git_commondir_file()) {
        Ok(file) => io::BufReader::new(file),
        Err(_) => return Err(None),
    };
    let commondir_line = match file.lines().next() {
        Some(Ok(line)) => line,
        Some(Err(err)) => {
            return Err(Some(Error::Io(err).with_path(git_commondir_file())));
        }
        None => return Err(None),
    };
    let commondir_abs = if commondir_line.starts_with(".") {
        real_git_dir.join(commondir_line) // relative commondir
    } else {
        PathBuf::from(commondir_line)
    };
    Ok(commondir_abs)
}

#[cfg(test)]
mod tests {
    use std::{io::Write, path::Path};

    use crate::{
        dir::IgnoreBuilder, gitignore::Gitignore, tests::TempDir, Error,
    };

    fn wfile<P: AsRef<Path>>(path: P, contents: &str) {
        let mut file = std::fs::File::create(path).unwrap();
        file.write_all(contents.as_bytes()).unwrap();
    }

    fn mkdirp<P: AsRef<Path>>(path: P) {
        std::fs::create_dir_all(path).unwrap();
    }

    fn partial(err: Error) -> Vec<Error> {
        match err {
            Error::Partial(errs) => errs,
            _ => panic!("expected partial error but got {:?}", err),
        }
    }

    fn tmpdir() -> TempDir {
        TempDir::new().unwrap()
    }

    #[test]
    fn explicit_ignore() {
        let td = tmpdir();
        wfile(td.path().join("not-an-ignore"), "foo\n!bar");

        let (gi, err) = Gitignore::new(td.path().join("not-an-ignore"));
        assert!(err.is_none());
        let (ig, err) =
            IgnoreBuilder::new().add_ignore(gi).build().add_child(td.path());
        assert!(err.is_none());
        assert!(ig.matched("foo", false).is_ignore());
        assert!(ig.matched("bar", false).is_whitelist());
        assert!(ig.matched("baz", false).is_none());
    }

    #[test]
    fn git_exclude() {
        let td = tmpdir();
        mkdirp(td.path().join(".git/info"));
        wfile(td.path().join(".git/info/exclude"), "foo\n!bar");

        let (ig, err) = IgnoreBuilder::new().build().add_child(td.path());
        assert!(err.is_none());
        assert!(ig.matched("foo", false).is_ignore());
        assert!(ig.matched("bar", false).is_whitelist());
        assert!(ig.matched("baz", false).is_none());
    }

    #[test]
    fn gitignore() {
        let td = tmpdir();
        mkdirp(td.path().join(".git"));
        wfile(td.path().join(".gitignore"), "foo\n!bar");

        let (ig, err) = IgnoreBuilder::new().build().add_child(td.path());
        assert!(err.is_none());
        assert!(ig.matched("foo", false).is_ignore());
        assert!(ig.matched("bar", false).is_whitelist());
        assert!(ig.matched("baz", false).is_none());
    }

    #[test]
    fn gitignore_no_git() {
        let td = tmpdir();
        wfile(td.path().join(".gitignore"), "foo\n!bar");

        let (ig, err) = IgnoreBuilder::new().build().add_child(td.path());
        assert!(err.is_none());
        assert!(ig.matched("foo", false).is_none());
        assert!(ig.matched("bar", false).is_none());
        assert!(ig.matched("baz", false).is_none());
    }

    #[test]
    fn gitignore_allowed_no_git() {
        let td = tmpdir();
        wfile(td.path().join(".gitignore"), "foo\n!bar");

        let (ig, err) = IgnoreBuilder::new()
            .require_git(false)
            .build()
            .add_child(td.path());
        assert!(err.is_none());
        assert!(ig.matched("foo", false).is_ignore());
        assert!(ig.matched("bar", false).is_whitelist());
        assert!(ig.matched("baz", false).is_none());
    }

    #[test]
    fn ignore() {
        let td = tmpdir();
        wfile(td.path().join(".ignore"), "foo\n!bar");

        let (ig, err) = IgnoreBuilder::new().build().add_child(td.path());
        assert!(err.is_none());
        assert!(ig.matched("foo", false).is_ignore());
        assert!(ig.matched("bar", false).is_whitelist());
        assert!(ig.matched("baz", false).is_none());
    }

    #[test]
    fn custom_ignore() {
        let td = tmpdir();
        let custom_ignore = ".customignore";
        wfile(td.path().join(custom_ignore), "foo\n!bar");

        let (ig, err) = IgnoreBuilder::new()
            .add_custom_ignore_filename(custom_ignore)
            .build()
            .add_child(td.path());
        assert!(err.is_none());
        assert!(ig.matched("foo", false).is_ignore());
        assert!(ig.matched("bar", false).is_whitelist());
        assert!(ig.matched("baz", false).is_none());
    }

    // Tests that a custom ignore file will override an .ignore.
    #[test]
    fn custom_ignore_over_ignore() {
        let td = tmpdir();
        let custom_ignore = ".customignore";
        wfile(td.path().join(".ignore"), "foo");
        wfile(td.path().join(custom_ignore), "!foo");

        let (ig, err) = IgnoreBuilder::new()
            .add_custom_ignore_filename(custom_ignore)
            .build()
            .add_child(td.path());
        assert!(err.is_none());
        assert!(ig.matched("foo", false).is_whitelist());
    }

    // Tests that earlier custom ignore files have lower precedence than later.
    #[test]
    fn custom_ignore_precedence() {
        let td = tmpdir();
        let custom_ignore1 = ".customignore1";
        let custom_ignore2 = ".customignore2";
        wfile(td.path().join(custom_ignore1), "foo");
        wfile(td.path().join(custom_ignore2), "!foo");

        let (ig, err) = IgnoreBuilder::new()
            .add_custom_ignore_filename(custom_ignore1)
            .add_custom_ignore_filename(custom_ignore2)
            .build()
            .add_child(td.path());
        assert!(err.is_none());
        assert!(ig.matched("foo", false).is_whitelist());
    }

    // Tests that an .ignore will override a .gitignore.
    #[test]
    fn ignore_over_gitignore() {
        let td = tmpdir();
        wfile(td.path().join(".gitignore"), "foo");
        wfile(td.path().join(".ignore"), "!foo");

        let (ig, err) = IgnoreBuilder::new().build().add_child(td.path());
        assert!(err.is_none());
        assert!(ig.matched("foo", false).is_whitelist());
    }

    // Tests that exclude has lower precedent than both .ignore and .gitignore.
    #[test]
    fn exclude_lowest() {
        let td = tmpdir();
        wfile(td.path().join(".gitignore"), "!foo");
        wfile(td.path().join(".ignore"), "!bar");
        mkdirp(td.path().join(".git/info"));
        wfile(td.path().join(".git/info/exclude"), "foo\nbar\nbaz");

        let (ig, err) = IgnoreBuilder::new().build().add_child(td.path());
        assert!(err.is_none());
        assert!(ig.matched("baz", false).is_ignore());
        assert!(ig.matched("foo", false).is_whitelist());
        assert!(ig.matched("bar", false).is_whitelist());
    }

    #[test]
    fn errored() {
        let td = tmpdir();
        wfile(td.path().join(".gitignore"), "{foo");

        let (_, err) = IgnoreBuilder::new().build().add_child(td.path());
        assert!(err.is_some());
    }

    #[test]
    fn errored_both() {
        let td = tmpdir();
        wfile(td.path().join(".gitignore"), "{foo");
        wfile(td.path().join(".ignore"), "{bar");

        let (_, err) = IgnoreBuilder::new().build().add_child(td.path());
        assert_eq!(2, partial(err.expect("an error")).len());
    }

    #[test]
    fn errored_partial() {
        let td = tmpdir();
        mkdirp(td.path().join(".git"));
        wfile(td.path().join(".gitignore"), "{foo\nbar");

        let (ig, err) = IgnoreBuilder::new().build().add_child(td.path());
        assert!(err.is_some());
        assert!(ig.matched("bar", false).is_ignore());
    }

    #[test]
    fn errored_partial_and_ignore() {
        let td = tmpdir();
        wfile(td.path().join(".gitignore"), "{foo\nbar");
        wfile(td.path().join(".ignore"), "!bar");

        let (ig, err) = IgnoreBuilder::new().build().add_child(td.path());
        assert!(err.is_some());
        assert!(ig.matched("bar", false).is_whitelist());
    }

    #[test]
    fn not_present_empty() {
        let td = tmpdir();

        let (_, err) = IgnoreBuilder::new().build().add_child(td.path());
        assert!(err.is_none());
    }

    #[test]
    fn stops_at_git_dir() {
        // This tests that .gitignore files beyond a .git barrier aren't
        // matched, but .ignore files are.
        let td = tmpdir();
        mkdirp(td.path().join(".git"));
        mkdirp(td.path().join("foo/.git"));
        wfile(td.path().join(".gitignore"), "foo");
        wfile(td.path().join(".ignore"), "bar");

        let ig0 = IgnoreBuilder::new().build();
        let (ig1, err) = ig0.add_child(td.path());
        assert!(err.is_none());
        let (ig2, err) = ig1.add_child(ig1.path().join("foo"));
        assert!(err.is_none());

        assert!(ig1.matched("foo", false).is_ignore());
        assert!(ig2.matched("foo", false).is_none());

        assert!(ig1.matched("bar", false).is_ignore());
        assert!(ig2.matched("bar", false).is_ignore());
    }

    #[test]
    fn absolute_parent() {
        let td = tmpdir();
        mkdirp(td.path().join(".git"));
        mkdirp(td.path().join("foo"));
        wfile(td.path().join(".gitignore"), "bar");

        // First, check that the parent gitignore file isn't detected if the
        // parent isn't added. This establishes a baseline.
        let ig0 = IgnoreBuilder::new().build();
        let (ig1, err) = ig0.add_child(td.path().join("foo"));
        assert!(err.is_none());
        assert!(ig1.matched("bar", false).is_none());

        // Second, check that adding a parent directory actually works.
        let ig0 = IgnoreBuilder::new().build();
        let (ig1, err) = ig0.add_parents(td.path().join("foo"));
        assert!(err.is_none());
        let (ig2, err) = ig1.add_child(td.path().join("foo"));
        assert!(err.is_none());
        assert!(ig2.matched("bar", false).is_ignore());
    }

    #[test]
    fn absolute_parent_anchored() {
        let td = tmpdir();
        mkdirp(td.path().join(".git"));
        mkdirp(td.path().join("src/llvm"));
        wfile(td.path().join(".gitignore"), "/llvm/\nfoo");

        let ig0 = IgnoreBuilder::new().build();
        let (ig1, err) = ig0.add_parents(td.path().join("src"));
        assert!(err.is_none());
        let (ig2, err) = ig1.add_child("src");
        assert!(err.is_none());

        assert!(ig1.matched("llvm", true).is_none());
        assert!(ig2.matched("llvm", true).is_none());
        assert!(ig2.matched("src/llvm", true).is_none());
        assert!(ig2.matched("foo", false).is_ignore());
        assert!(ig2.matched("src/foo", false).is_ignore());
    }

    #[test]
    fn git_info_exclude_in_linked_worktree() {
        let td = tmpdir();
        let git_dir = td.path().join(".git");
        mkdirp(git_dir.join("info"));
        wfile(git_dir.join("info/exclude"), "ignore_me");
        mkdirp(git_dir.join("worktrees/linked-worktree"));
        let commondir_path =
            || git_dir.join("worktrees/linked-worktree/commondir");
        mkdirp(td.path().join("linked-worktree"));
        let worktree_git_dir_abs = format!(
            "gitdir: {}",
            git_dir.join("worktrees/linked-worktree").to_str().unwrap(),
        );
        wfile(td.path().join("linked-worktree/.git"), &worktree_git_dir_abs);

        // relative commondir
        wfile(commondir_path(), "../..");
        let ib = IgnoreBuilder::new().build();
        let (ignore, err) = ib.add_child(td.path().join("linked-worktree"));
        assert!(err.is_none());
        assert!(ignore.matched("ignore_me", false).is_ignore());

        // absolute commondir
        wfile(commondir_path(), git_dir.to_str().unwrap());
        let (ignore, err) = ib.add_child(td.path().join("linked-worktree"));
        assert!(err.is_none());
        assert!(ignore.matched("ignore_me", false).is_ignore());

        // missing commondir file
        assert!(std::fs::remove_file(commondir_path()).is_ok());
        let (_, err) = ib.add_child(td.path().join("linked-worktree"));
        // We squash the error in this case, because it occurs in repositories
        // that are not linked worktrees but have submodules.
        assert!(err.is_none());

        wfile(td.path().join("linked-worktree/.git"), "garbage");
        let (_, err) = ib.add_child(td.path().join("linked-worktree"));
        assert!(err.is_none());

        wfile(td.path().join("linked-worktree/.git"), "gitdir: garbage");
        let (_, err) = ib.add_child(td.path().join("linked-worktree"));
        assert!(err.is_none());
    }
}
