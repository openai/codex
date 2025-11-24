//! [`std::process::Output`] assertions.

use std::borrow::Cow;
use std::error::Error;
use std::fmt;
use std::process;
use std::str;

#[cfg(feature = "color")]
use anstream::panic;
use predicates::str::PredicateStrExt;
use predicates_tree::CaseTreeExt;

use crate::output::output_fmt;
use crate::output::DebugBytes;

/// Assert the state of an [`Output`].
///
/// # Examples
///
/// ```rust,no_run
/// use assert_cmd::prelude::*;
///
/// use std::process::Command;
///
/// let mut cmd = Command::cargo_bin("bin_fixture")
///     .unwrap();
/// cmd.assert()
///     .success();
/// ```
///
/// [`Output`]: std::process::Output
pub trait OutputAssertExt {
    /// Wrap with an interface for that provides assertions on the [`Output`].
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use assert_cmd::prelude::*;
    ///
    /// use std::process::Command;
    ///
    /// let mut cmd = Command::cargo_bin("bin_fixture")
    ///     .unwrap();
    /// cmd.assert()
    ///     .success();
    /// ```
    ///
    /// [`Output`]: std::process::Output
    fn assert(self) -> Assert;
}

impl OutputAssertExt for process::Output {
    fn assert(self) -> Assert {
        Assert::new(self)
    }
}

impl OutputAssertExt for &mut process::Command {
    fn assert(self) -> Assert {
        let output = match self.output() {
            Ok(output) => output,
            Err(err) => {
                panic!("Failed to spawn {self:?}: {err}");
            }
        };
        Assert::new(output).append_context("command", format!("{self:?}"))
    }
}

/// Assert the state of an [`Output`].
///
/// Create an `Assert` through the [`OutputAssertExt`] trait.
///
/// # Examples
///
/// ```rust,no_run
/// use assert_cmd::prelude::*;
///
/// use std::process::Command;
///
/// let mut cmd = Command::cargo_bin("bin_fixture")
///     .unwrap();
/// cmd.assert()
///     .success();
/// ```
///
/// [`Output`]: std::process::Output
pub struct Assert {
    output: process::Output,
    context: Vec<(&'static str, Box<dyn fmt::Display + Send + Sync>)>,
}

impl Assert {
    /// Create an `Assert` for a given [`Output`].
    ///
    /// [`Output`]: std::process::Output
    pub fn new(output: process::Output) -> Self {
        Self {
            output,
            context: vec![],
        }
    }

    fn into_error(self, reason: AssertReason) -> AssertError {
        AssertError {
            assert: self,
            reason,
        }
    }

    /// Clarify failures with additional context.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use assert_cmd::prelude::*;
    ///
    /// use std::process::Command;
    ///
    /// Command::cargo_bin("bin_fixture")
    ///     .unwrap()
    ///     .assert()
    ///     .append_context("main", "no args")
    ///     .success();
    /// ```
    pub fn append_context<D>(mut self, name: &'static str, context: D) -> Self
    where
        D: fmt::Display + Send + Sync + 'static,
    {
        self.context.push((name, Box::new(context)));
        self
    }

    /// Access the contained [`Output`].
    ///
    /// [`Output`]: std::process::Output
    pub fn get_output(&self) -> &process::Output {
        &self.output
    }

    /// Ensure the command succeeded.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use assert_cmd::prelude::*;
    ///
    /// use std::process::Command;
    ///
    /// Command::cargo_bin("bin_fixture")
    ///     .unwrap()
    ///     .assert()
    ///     .success();
    /// ```
    #[track_caller]
    pub fn success(self) -> Self {
        self.try_success().unwrap_or_else(AssertError::panic)
    }

    /// `try_` variant of [`Assert::success`].
    pub fn try_success(self) -> AssertResult {
        if !self.output.status.success() {
            let actual_code = self.output.status.code();
            return Err(self.into_error(AssertReason::UnexpectedFailure { actual_code }));
        }
        Ok(self)
    }

    /// Ensure the command failed.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use assert_cmd::prelude::*;
    ///
    /// use std::process::Command;
    ///
    /// Command::cargo_bin("bin_fixture")
    ///     .unwrap()
    ///     .env("exit", "1")
    ///     .assert()
    ///     .failure();
    /// ```
    #[track_caller]
    pub fn failure(self) -> Self {
        self.try_failure().unwrap_or_else(AssertError::panic)
    }

    /// Variant of [`Assert::failure`] that returns an [`AssertResult`].
    pub fn try_failure(self) -> AssertResult {
        if self.output.status.success() {
            return Err(self.into_error(AssertReason::UnexpectedSuccess));
        }
        Ok(self)
    }

    /// Ensure the command aborted before returning a code.
    #[track_caller]
    pub fn interrupted(self) -> Self {
        self.try_interrupted().unwrap_or_else(AssertError::panic)
    }

    /// Variant of [`Assert::interrupted`] that returns an [`AssertResult`].
    pub fn try_interrupted(self) -> AssertResult {
        if self.output.status.code().is_some() {
            return Err(self.into_error(AssertReason::UnexpectedCompletion));
        }
        Ok(self)
    }

    /// Ensure the command returned the expected code.
    ///
    /// This uses [`IntoCodePredicate`] to provide short-hands for common cases.
    ///
    /// See [`predicates`] for more predicates.
    ///
    /// # Examples
    ///
    /// Accepting a predicate:
    /// ```rust,no_run
    /// use assert_cmd::prelude::*;
    ///
    /// use std::process::Command;
    /// use predicates::prelude::*;
    ///
    /// Command::cargo_bin("bin_fixture")
    ///     .unwrap()
    ///     .env("exit", "42")
    ///     .assert()
    ///     .code(predicate::eq(42));
    /// ```
    ///
    /// Accepting an exit code:
    /// ```rust,no_run
    /// use assert_cmd::prelude::*;
    ///
    /// use std::process::Command;
    ///
    /// Command::cargo_bin("bin_fixture")
    ///     .unwrap()
    ///     .env("exit", "42")
    ///     .assert()
    ///     .code(42);
    /// ```
    ///
    /// Accepting multiple exit codes:
    /// ```rust,no_run
    /// use assert_cmd::prelude::*;
    ///
    /// use std::process::Command;
    ///
    /// Command::cargo_bin("bin_fixture")
    ///     .unwrap()
    ///     .env("exit", "42")
    ///     .assert()
    ///     .code(&[2, 42] as &[i32]);
    /// ```
    ///
    #[track_caller]
    pub fn code<I, P>(self, pred: I) -> Self
    where
        I: IntoCodePredicate<P>,
        P: predicates_core::Predicate<i32>,
    {
        self.try_code(pred).unwrap_or_else(AssertError::panic)
    }

    /// Variant of [`Assert::code`] that returns an [`AssertResult`].
    pub fn try_code<I, P>(self, pred: I) -> AssertResult
    where
        I: IntoCodePredicate<P>,
        P: predicates_core::Predicate<i32>,
    {
        self.code_impl(&pred.into_code())
    }

    fn code_impl(self, pred: &dyn predicates_core::Predicate<i32>) -> AssertResult {
        let actual_code = if let Some(actual_code) = self.output.status.code() {
            actual_code
        } else {
            return Err(self.into_error(AssertReason::CommandInterrupted));
        };
        if let Some(case) = pred.find_case(false, &actual_code) {
            return Err(self.into_error(AssertReason::UnexpectedReturnCode {
                case_tree: CaseTree(case.tree()),
            }));
        }
        Ok(self)
    }

    /// Ensure the command wrote the expected data to `stdout`.
    ///
    /// This uses [`IntoOutputPredicate`] to provide short-hands for common cases.
    ///
    /// See [`predicates`] for more predicates.
    ///
    /// # Examples
    ///
    /// Accepting a bytes predicate:
    /// ```rust,no_run
    /// use assert_cmd::prelude::*;
    ///
    /// use std::process::Command;
    /// use predicates::prelude::*;
    ///
    /// Command::cargo_bin("bin_fixture")
    ///     .unwrap()
    ///     .env("stdout", "hello")
    ///     .env("stderr", "world")
    ///     .assert()
    ///     .stdout(predicate::eq(b"hello\n" as &[u8]));
    /// ```
    ///
    /// Accepting a `str` predicate:
    /// ```rust,no_run
    /// use assert_cmd::prelude::*;
    ///
    /// use std::process::Command;
    /// use predicates::prelude::*;
    ///
    /// Command::cargo_bin("bin_fixture")
    ///     .unwrap()
    ///     .env("stdout", "hello")
    ///     .env("stderr", "world")
    ///     .assert()
    ///     .stdout(predicate::str::diff("hello\n"));
    /// ```
    ///
    /// Accepting bytes:
    /// ```rust,no_run
    /// use assert_cmd::prelude::*;
    ///
    /// use std::process::Command;
    ///
    /// Command::cargo_bin("bin_fixture")
    ///     .unwrap()
    ///     .env("stdout", "hello")
    ///     .env("stderr", "world")
    ///     .assert()
    ///     .stdout(b"hello\n" as &[u8]);
    /// ```
    ///
    /// Accepting a `str`:
    /// ```rust,no_run
    /// use assert_cmd::prelude::*;
    ///
    /// use std::process::Command;
    ///
    /// Command::cargo_bin("bin_fixture")
    ///     .unwrap()
    ///     .env("stdout", "hello")
    ///     .env("stderr", "world")
    ///     .assert()
    ///     .stdout("hello\n");
    /// ```
    ///
    #[track_caller]
    pub fn stdout<I, P>(self, pred: I) -> Self
    where
        I: IntoOutputPredicate<P>,
        P: predicates_core::Predicate<[u8]>,
    {
        self.try_stdout(pred).unwrap_or_else(AssertError::panic)
    }

    /// Variant of [`Assert::stdout`] that returns an [`AssertResult`].
    pub fn try_stdout<I, P>(self, pred: I) -> AssertResult
    where
        I: IntoOutputPredicate<P>,
        P: predicates_core::Predicate<[u8]>,
    {
        self.stdout_impl(&pred.into_output())
    }

    fn stdout_impl(self, pred: &dyn predicates_core::Predicate<[u8]>) -> AssertResult {
        {
            let actual = &self.output.stdout;
            if let Some(case) = pred.find_case(false, actual) {
                return Err(self.into_error(AssertReason::UnexpectedStdout {
                    case_tree: CaseTree(case.tree()),
                }));
            }
        }
        Ok(self)
    }

    /// Ensure the command wrote the expected data to `stderr`.
    ///
    /// This uses [`IntoOutputPredicate`] to provide short-hands for common cases.
    ///
    /// See [`predicates`] for more predicates.
    ///
    /// # Examples
    ///
    /// Accepting a bytes predicate:
    /// ```rust,no_run
    /// use assert_cmd::prelude::*;
    ///
    /// use std::process::Command;
    /// use predicates::prelude::*;
    ///
    /// Command::cargo_bin("bin_fixture")
    ///     .unwrap()
    ///     .env("stdout", "hello")
    ///     .env("stderr", "world")
    ///     .assert()
    ///     .stderr(predicate::eq(b"world\n" as &[u8]));
    /// ```
    ///
    /// Accepting a `str` predicate:
    /// ```rust,no_run
    /// use assert_cmd::prelude::*;
    ///
    /// use std::process::Command;
    /// use predicates::prelude::*;
    ///
    /// Command::cargo_bin("bin_fixture")
    ///     .unwrap()
    ///     .env("stdout", "hello")
    ///     .env("stderr", "world")
    ///     .assert()
    ///     .stderr(predicate::str::diff("world\n"));
    /// ```
    ///
    /// Accepting bytes:
    /// ```rust,no_run
    /// use assert_cmd::prelude::*;
    ///
    /// use std::process::Command;
    ///
    /// Command::cargo_bin("bin_fixture")
    ///     .unwrap()
    ///     .env("stdout", "hello")
    ///     .env("stderr", "world")
    ///     .assert()
    ///     .stderr(b"world\n" as &[u8]);
    /// ```
    ///
    /// Accepting a `str`:
    /// ```rust,no_run
    /// use assert_cmd::prelude::*;
    ///
    /// use std::process::Command;
    ///
    /// Command::cargo_bin("bin_fixture")
    ///     .unwrap()
    ///     .env("stdout", "hello")
    ///     .env("stderr", "world")
    ///     .assert()
    ///     .stderr("world\n");
    /// ```
    ///
    #[track_caller]
    pub fn stderr<I, P>(self, pred: I) -> Self
    where
        I: IntoOutputPredicate<P>,
        P: predicates_core::Predicate<[u8]>,
    {
        self.try_stderr(pred).unwrap_or_else(AssertError::panic)
    }

    /// Variant of [`Assert::stderr`] that returns an [`AssertResult`].
    pub fn try_stderr<I, P>(self, pred: I) -> AssertResult
    where
        I: IntoOutputPredicate<P>,
        P: predicates_core::Predicate<[u8]>,
    {
        self.stderr_impl(&pred.into_output())
    }

    fn stderr_impl(self, pred: &dyn predicates_core::Predicate<[u8]>) -> AssertResult {
        {
            let actual = &self.output.stderr;
            if let Some(case) = pred.find_case(false, actual) {
                return Err(self.into_error(AssertReason::UnexpectedStderr {
                    case_tree: CaseTree(case.tree()),
                }));
            }
        }
        Ok(self)
    }
}

impl fmt::Display for Assert {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let palette = crate::Palette::color();
        for (name, context) in &self.context {
            writeln!(f, "{:#}=`{:#}`", palette.key(name), palette.value(context))?;
        }
        output_fmt(&self.output, f)
    }
}

impl fmt::Debug for Assert {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Assert")
            .field("output", &self.output)
            .finish()
    }
}

/// Used by [`Assert::code`] to convert `Self` into the needed
/// [`predicates_core::Predicate<i32>`].
///
/// # Examples
///
/// ```rust,no_run
/// use assert_cmd::prelude::*;
///
/// use std::process::Command;
/// use predicates::prelude::*;
///
/// Command::cargo_bin("bin_fixture")
///     .unwrap()
///     .env("exit", "42")
///     .assert()
///     .code(predicate::eq(42));
///
/// // which can be shortened to:
/// Command::cargo_bin("bin_fixture")
///     .unwrap()
///     .env("exit", "42")
///     .assert()
///     .code(42);
/// ```
pub trait IntoCodePredicate<P>
where
    P: predicates_core::Predicate<i32>,
{
    /// The type of the predicate being returned.
    type Predicate;

    /// Convert to a predicate for testing a program's exit code.
    fn into_code(self) -> P;
}

impl<P> IntoCodePredicate<P> for P
where
    P: predicates_core::Predicate<i32>,
{
    type Predicate = P;

    fn into_code(self) -> Self::Predicate {
        self
    }
}

/// Keep `predicates` concrete Predicates out of our public API.
/// [`predicates_core::Predicate`] used by [`IntoCodePredicate`] for code.
///
/// # Example
///
/// ```rust,no_run
/// use assert_cmd::prelude::*;
///
/// use std::process::Command;
///
/// Command::cargo_bin("bin_fixture")
///     .unwrap()
///     .env("exit", "42")
///     .assert()
///     .code(42);
/// ```
#[derive(Debug)]
pub struct EqCodePredicate(predicates::ord::EqPredicate<i32>);

impl EqCodePredicate {
    pub(crate) fn new(value: i32) -> Self {
        let pred = predicates::ord::eq(value);
        EqCodePredicate(pred)
    }
}

impl predicates_core::reflection::PredicateReflection for EqCodePredicate {
    fn parameters<'a>(
        &'a self,
    ) -> Box<dyn Iterator<Item = predicates_core::reflection::Parameter<'a>> + 'a> {
        self.0.parameters()
    }

    /// Nested `Predicate`s of the current `Predicate`.
    fn children<'a>(
        &'a self,
    ) -> Box<dyn Iterator<Item = predicates_core::reflection::Child<'a>> + 'a> {
        self.0.children()
    }
}

impl predicates_core::Predicate<i32> for EqCodePredicate {
    fn eval(&self, item: &i32) -> bool {
        self.0.eval(item)
    }

    fn find_case<'a>(
        &'a self,
        expected: bool,
        variable: &i32,
    ) -> Option<predicates_core::reflection::Case<'a>> {
        self.0.find_case(expected, variable)
    }
}

impl fmt::Display for EqCodePredicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl IntoCodePredicate<EqCodePredicate> for i32 {
    type Predicate = EqCodePredicate;

    fn into_code(self) -> Self::Predicate {
        Self::Predicate::new(self)
    }
}

/// Keep `predicates` concrete Predicates out of our public API.
/// [`predicates_core::Predicate`] used by [`IntoCodePredicate`] for iterables of codes.
///
/// # Example
///
/// ```rust,no_run
/// use assert_cmd::prelude::*;
///
/// use std::process::Command;
///
/// Command::cargo_bin("bin_fixture")
///     .unwrap()
///     .env("exit", "42")
///     .assert()
///     .code(&[2, 42] as &[i32]);
/// ```
#[derive(Debug)]
pub struct InCodePredicate(predicates::iter::InPredicate<i32>);

impl InCodePredicate {
    pub(crate) fn new<I: IntoIterator<Item = i32>>(value: I) -> Self {
        let pred = predicates::iter::in_iter(value);
        InCodePredicate(pred)
    }
}

impl predicates_core::reflection::PredicateReflection for InCodePredicate {
    fn parameters<'a>(
        &'a self,
    ) -> Box<dyn Iterator<Item = predicates_core::reflection::Parameter<'a>> + 'a> {
        self.0.parameters()
    }

    /// Nested `Predicate`s of the current `Predicate`.
    fn children<'a>(
        &'a self,
    ) -> Box<dyn Iterator<Item = predicates_core::reflection::Child<'a>> + 'a> {
        self.0.children()
    }
}

impl predicates_core::Predicate<i32> for InCodePredicate {
    fn eval(&self, item: &i32) -> bool {
        self.0.eval(item)
    }

    fn find_case<'a>(
        &'a self,
        expected: bool,
        variable: &i32,
    ) -> Option<predicates_core::reflection::Case<'a>> {
        self.0.find_case(expected, variable)
    }
}

impl fmt::Display for InCodePredicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl IntoCodePredicate<InCodePredicate> for Vec<i32> {
    type Predicate = InCodePredicate;

    fn into_code(self) -> Self::Predicate {
        Self::Predicate::new(self)
    }
}

impl IntoCodePredicate<InCodePredicate> for &'static [i32] {
    type Predicate = InCodePredicate;

    fn into_code(self) -> Self::Predicate {
        Self::Predicate::new(self.iter().cloned())
    }
}

/// Used by [`Assert::stdout`] and [`Assert::stderr`] to convert Self
/// into the needed [`predicates_core::Predicate<[u8]>`].
///
/// # Examples
///
/// ```rust,no_run
/// use assert_cmd::prelude::*;
///
/// use std::process::Command;
/// use predicates::prelude::*;
///
/// Command::cargo_bin("bin_fixture")
///     .unwrap()
///     .env("stdout", "hello")
///     .env("stderr", "world")
///     .assert()
///     .stdout(predicate::str::diff("hello\n").from_utf8());
///
/// // which can be shortened to:
/// Command::cargo_bin("bin_fixture")
///     .unwrap()
///     .env("stdout", "hello")
///     .env("stderr", "world")
///     .assert()
///     .stdout("hello\n");
/// ```
pub trait IntoOutputPredicate<P>
where
    P: predicates_core::Predicate<[u8]>,
{
    /// The type of the predicate being returned.
    type Predicate;

    /// Convert to a predicate for testing a path.
    fn into_output(self) -> P;
}

impl<P> IntoOutputPredicate<P> for P
where
    P: predicates_core::Predicate<[u8]>,
{
    type Predicate = P;

    fn into_output(self) -> Self::Predicate {
        self
    }
}

/// Keep `predicates` concrete Predicates out of our public API.
/// [`predicates_core::Predicate`] used by [`IntoOutputPredicate`] for bytes.
///
/// # Example
///
/// ```rust,no_run
/// use assert_cmd::prelude::*;
///
/// use std::process::Command;
///
/// Command::cargo_bin("bin_fixture")
///     .unwrap()
///     .env("stdout", "hello")
///     .env("stderr", "world")
///     .assert()
///     .stderr(b"world\n" as &[u8]);
/// ```
#[derive(Debug)]
pub struct BytesContentOutputPredicate(Cow<'static, [u8]>);

impl BytesContentOutputPredicate {
    pub(crate) fn new(value: &'static [u8]) -> Self {
        BytesContentOutputPredicate(Cow::from(value))
    }

    pub(crate) fn from_vec(value: Vec<u8>) -> Self {
        BytesContentOutputPredicate(Cow::from(value))
    }
}

impl predicates_core::reflection::PredicateReflection for BytesContentOutputPredicate {}

impl predicates_core::Predicate<[u8]> for BytesContentOutputPredicate {
    fn eval(&self, item: &[u8]) -> bool {
        self.0.as_ref() == item
    }

    fn find_case(
        &self,
        expected: bool,
        variable: &[u8],
    ) -> Option<predicates_core::reflection::Case<'_>> {
        let actual = self.eval(variable);
        if expected == actual {
            Some(predicates_core::reflection::Case::new(Some(self), actual))
        } else {
            None
        }
    }
}

impl fmt::Display for BytesContentOutputPredicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        predicates::ord::eq(self.0.as_ref()).fmt(f)
    }
}

impl IntoOutputPredicate<BytesContentOutputPredicate> for Vec<u8> {
    type Predicate = BytesContentOutputPredicate;

    fn into_output(self) -> Self::Predicate {
        Self::Predicate::from_vec(self)
    }
}

impl IntoOutputPredicate<BytesContentOutputPredicate> for &'static [u8] {
    type Predicate = BytesContentOutputPredicate;

    fn into_output(self) -> Self::Predicate {
        Self::Predicate::new(self)
    }
}

/// Keep `predicates` concrete Predicates out of our public API.
/// [`predicates_core::Predicate`] used by [`IntoOutputPredicate`] for [`str`].
///
/// # Example
///
/// ```rust,no_run
/// use assert_cmd::prelude::*;
///
/// use std::process::Command;
///
/// Command::cargo_bin("bin_fixture")
///     .unwrap()
///     .env("stdout", "hello")
///     .env("stderr", "world")
///     .assert()
///     .stderr("world\n");
/// ```
///
/// [`str`]: https://doc.rust-lang.org/std/primitive.str.html
#[derive(Debug, Clone)]
pub struct StrContentOutputPredicate(
    predicates::str::Utf8Predicate<predicates::str::DifferencePredicate>,
);

impl StrContentOutputPredicate {
    pub(crate) fn from_str(value: &'static str) -> Self {
        let pred = predicates::str::diff(value).from_utf8();
        StrContentOutputPredicate(pred)
    }

    pub(crate) fn from_string(value: String) -> Self {
        let pred = predicates::str::diff(value).from_utf8();
        StrContentOutputPredicate(pred)
    }
}

impl predicates_core::reflection::PredicateReflection for StrContentOutputPredicate {
    fn parameters<'a>(
        &'a self,
    ) -> Box<dyn Iterator<Item = predicates_core::reflection::Parameter<'a>> + 'a> {
        self.0.parameters()
    }

    /// Nested `Predicate`s of the current `Predicate`.
    fn children<'a>(
        &'a self,
    ) -> Box<dyn Iterator<Item = predicates_core::reflection::Child<'a>> + 'a> {
        self.0.children()
    }
}

impl predicates_core::Predicate<[u8]> for StrContentOutputPredicate {
    fn eval(&self, item: &[u8]) -> bool {
        self.0.eval(item)
    }

    fn find_case<'a>(
        &'a self,
        expected: bool,
        variable: &[u8],
    ) -> Option<predicates_core::reflection::Case<'a>> {
        self.0.find_case(expected, variable)
    }
}

impl fmt::Display for StrContentOutputPredicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl IntoOutputPredicate<StrContentOutputPredicate> for String {
    type Predicate = StrContentOutputPredicate;

    fn into_output(self) -> Self::Predicate {
        Self::Predicate::from_string(self)
    }
}

impl IntoOutputPredicate<StrContentOutputPredicate> for &'static str {
    type Predicate = StrContentOutputPredicate;

    fn into_output(self) -> Self::Predicate {
        Self::Predicate::from_str(self)
    }
}

// Keep `predicates` concrete Predicates out of our public API.
/// [`predicates_core::Predicate`] used by [`IntoOutputPredicate`] for
/// [`Predicate<str>`][predicates_core::Predicate].
///
/// # Example
///
/// ```rust,no_run
/// use assert_cmd::prelude::*;
///
/// use std::process::Command;
/// use predicates::prelude::*;
///
/// Command::cargo_bin("bin_fixture")
///     .unwrap()
///     .env("stdout", "hello")
///     .env("stderr", "world")
///     .assert()
///     .stderr(predicate::str::diff("world\n"));
/// ```
#[derive(Debug, Clone)]
pub struct StrOutputPredicate<P: predicates_core::Predicate<str>>(
    predicates::str::Utf8Predicate<P>,
);

impl<P> StrOutputPredicate<P>
where
    P: predicates_core::Predicate<str>,
{
    pub(crate) fn new(pred: P) -> Self {
        let pred = pred.from_utf8();
        StrOutputPredicate(pred)
    }
}

impl<P> predicates_core::reflection::PredicateReflection for StrOutputPredicate<P>
where
    P: predicates_core::Predicate<str>,
{
    fn parameters<'a>(
        &'a self,
    ) -> Box<dyn Iterator<Item = predicates_core::reflection::Parameter<'a>> + 'a> {
        self.0.parameters()
    }

    /// Nested `Predicate`s of the current `Predicate`.
    fn children<'a>(
        &'a self,
    ) -> Box<dyn Iterator<Item = predicates_core::reflection::Child<'a>> + 'a> {
        self.0.children()
    }
}

impl<P> predicates_core::Predicate<[u8]> for StrOutputPredicate<P>
where
    P: predicates_core::Predicate<str>,
{
    fn eval(&self, item: &[u8]) -> bool {
        self.0.eval(item)
    }

    fn find_case<'a>(
        &'a self,
        expected: bool,
        variable: &[u8],
    ) -> Option<predicates_core::reflection::Case<'a>> {
        self.0.find_case(expected, variable)
    }
}

impl<P> fmt::Display for StrOutputPredicate<P>
where
    P: predicates_core::Predicate<str>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<P> IntoOutputPredicate<StrOutputPredicate<P>> for P
where
    P: predicates_core::Predicate<str>,
{
    type Predicate = StrOutputPredicate<P>;

    fn into_output(self) -> Self::Predicate {
        Self::Predicate::new(self)
    }
}

/// [`Assert`] represented as a [`Result`].
///
/// Produced by the `try_` variants the [`Assert`] methods.
///
/// # Example
///
/// ```rust
/// use assert_cmd::prelude::*;
///
/// use std::process::Command;
///
/// let result = Command::new("echo")
///     .assert()
///     .try_success();
/// assert!(result.is_ok());
/// ```
///
/// [`Result`]: std::result::Result
pub type AssertResult = Result<Assert, AssertError>;

/// [`Assert`] error (see [`AssertResult`]).
#[derive(Debug)]
pub struct AssertError {
    assert: Assert,
    reason: AssertReason,
}

#[derive(Debug)]
enum AssertReason {
    UnexpectedFailure { actual_code: Option<i32> },
    UnexpectedSuccess,
    UnexpectedCompletion,
    CommandInterrupted,
    UnexpectedReturnCode { case_tree: CaseTree },
    UnexpectedStdout { case_tree: CaseTree },
    UnexpectedStderr { case_tree: CaseTree },
}

impl AssertError {
    #[track_caller]
    fn panic<T>(self) -> T {
        panic!("{}", self)
    }

    /// Returns the [`Assert`] wrapped into the [`Result`] produced by
    /// the `try_` variants of the [`Assert`] methods.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use assert_cmd::prelude::*;
    ///
    /// use std::process::Command;
    /// use predicates::prelude::*;
    ///
    /// let result = Command::new("echo")
    ///     .assert();
    ///
    /// match result.try_success() {
    ///         Ok(assert) => {
    ///             assert.stdout(predicate::eq(b"Success\n" as &[u8]));
    ///         }
    ///         Err(err) => {
    ///            err.assert().stdout(predicate::eq(b"Err but some specific output you might want to check\n" as &[u8]));
    ///         }
    ///     }
    /// ```
    pub fn assert(self) -> Assert {
        self.assert
    }
}

impl Error for AssertError {}

impl fmt::Display for AssertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.reason {
            AssertReason::UnexpectedFailure { actual_code } => writeln!(
                f,
                "Unexpected failure.\ncode={}\nstderr=```{}```",
                actual_code
                    .map(|actual_code| actual_code.to_string())
                    .unwrap_or_else(|| "<interrupted>".to_owned()),
                DebugBytes::new(&self.assert.output.stderr),
            ),
            AssertReason::UnexpectedSuccess => {
                writeln!(f, "Unexpected success")
            }
            AssertReason::UnexpectedCompletion => {
                writeln!(f, "Unexpected completion")
            }
            AssertReason::CommandInterrupted => {
                writeln!(f, "Command interrupted")
            }
            AssertReason::UnexpectedReturnCode { case_tree } => {
                writeln!(f, "Unexpected return code, failed {case_tree}")
            }
            AssertReason::UnexpectedStdout { case_tree } => {
                writeln!(f, "Unexpected stdout, failed {case_tree}")
            }
            AssertReason::UnexpectedStderr { case_tree } => {
                writeln!(f, "Unexpected stderr, failed {case_tree}")
            }
        }?;
        write!(f, "{}", self.assert)
    }
}

struct CaseTree(predicates_tree::CaseTree);

impl fmt::Display for CaseTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <predicates_tree::CaseTree as fmt::Display>::fmt(&self.0, f)
    }
}

// Work around `Debug` not being implemented for `predicates_tree::CaseTree`.
impl fmt::Debug for CaseTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <predicates_tree::CaseTree as fmt::Display>::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use predicates::prelude::*;

    // Since IntoCodePredicate exists solely for conversion, test it under that scenario to ensure
    // it works as expected.
    fn convert_code<I, P>(pred: I) -> P
    where
        I: IntoCodePredicate<P>,
        P: Predicate<i32>,
    {
        pred.into_code()
    }

    #[test]
    fn into_code_from_pred() {
        let pred = convert_code(predicate::eq(10));
        assert!(pred.eval(&10));
    }

    #[test]
    fn into_code_from_i32() {
        let pred = convert_code(10);
        assert!(pred.eval(&10));
    }

    #[test]
    fn into_code_from_vec() {
        let pred = convert_code(vec![3, 10]);
        assert!(pred.eval(&10));
    }

    #[test]
    fn into_code_from_array() {
        let pred = convert_code(&[3, 10] as &[i32]);
        assert!(pred.eval(&10));
    }

    // Since IntoOutputPredicate exists solely for conversion, test it under that scenario to ensure
    // it works as expected.
    fn convert_output<I, P>(pred: I) -> P
    where
        I: IntoOutputPredicate<P>,
        P: Predicate<[u8]>,
    {
        pred.into_output()
    }

    #[test]
    fn into_output_from_pred() {
        let pred = convert_output(predicate::eq(b"Hello" as &[u8]));
        assert!(pred.eval(b"Hello" as &[u8]));
    }

    #[test]
    fn into_output_from_bytes() {
        let pred = convert_output(b"Hello" as &[u8]);
        assert!(pred.eval(b"Hello" as &[u8]));
    }

    #[test]
    fn into_output_from_vec() {
        let pred = convert_output(vec![b'H', b'e', b'l', b'l', b'o']);
        assert!(pred.eval(b"Hello" as &[u8]));
    }

    #[test]
    fn into_output_from_str() {
        let pred = convert_output("Hello");
        assert!(pred.eval(b"Hello" as &[u8]));
    }
}
