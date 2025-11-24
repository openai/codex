mod r#impl;

/// Extension trait providing additional methods for `Option`.
pub trait OptionExt<T> {
    /// Returns `true` if the option is a [`Some`] value containing the given value.
    ///
    /// # Examples
    ///
    /// ```
    /// use option_ext::OptionExt;
    ///
    /// let x: Option<u32> = Some(2);
    /// assert_eq!(x.contains(&2), true);
    ///
    /// let x: Option<u32> = Some(3);
    /// assert_eq!(x.contains(&2), false);
    ///
    /// let x: Option<u32> = None;
    /// assert_eq!(x.contains(&2), false);
    /// ```
    #[must_use]
    fn contains<U>(&self, x: &U) -> bool where U: PartialEq<T>;

    /// Returns the result from applying the function to the contained value if the option is [`Some`],
    /// or returns provided default result if the option is [`None`].
    ///
    /// The `f` argument of `map_or2` is only evaluated  if the option is [`Some`].
    /// The default argument of `map_or2` is always evaluated – even if the option is [`Some`].
    /// Use [`map_or_else2`] to avoid this.
    ///
    /// [`map_or_else2`]: OptionExt::map_or_else2
    ///
    /// # Examples
    ///
    /// ```
    /// use option_ext::OptionExt;
    ///
    /// let x = Some("bar");
    /// assert_eq!(x.map_or2(|v| v.len(), 42), 3);
    ///
    /// let x: Option<&str> = None;
    /// assert_eq!(x.map_or2(|v| v.len(), 42), 42);
    /// ```
    #[must_use]
    fn map_or2<U, F: FnOnce(T) -> U>(self, f: F, default: U) -> U;

    /// Returns the result from applying the function to the contained value if the option is [`Some`],
    /// or returns the result from evaluating the provided default function if the option is [`None`].
    ///
    /// The `f` argument of `map_or_else2` is only evaluated  if the option is [`Some`].
    /// The default argument of `map_or_else2` is only evaluated if the option is [`None`].
    /// Use [`map_or2`] to always evaluate the default argument.
    ///
    /// [`map_or2`]: OptionExt::map_or2
    ///
    /// # Examples
    ///
    /// ```
    /// use option_ext::OptionExt;
    ///
    /// let k = 23;
    ///
    /// let x = Some("bar");
    /// assert_eq!(x.map_or_else2(|v| v.len(), || 2 * k), 3);
    ///
    /// let x: Option<&str> = None;
    /// assert_eq!(x.map_or_else2(|v| v.len(), || 2 * k), 46);
    /// ```
    #[must_use]
    fn map_or_else2<U, F: FnOnce(T) -> U, D: FnOnce() -> U>(self, f: F, default: D) -> U;
}
