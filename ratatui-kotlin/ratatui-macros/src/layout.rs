/// Creates a single constraint.
///
/// If creating an array of constraints, you probably want to use
/// [`constraints!`] instead.
///
/// # Examples
///
/// ```
/// # use ratatui_core::layout::Constraint;
/// use ratatui_macros::constraint;
/// assert_eq!(constraint!(>= 3 + 4), Constraint::Min(7));
/// assert_eq!(constraint!(<= 3 + 4), Constraint::Max(7));
/// assert_eq!(constraint!(== 1 / 3), Constraint::Ratio(1, 3));
/// assert_eq!(constraint!(== 3), Constraint::Length(3));
/// assert_eq!(constraint!(== 10 %), Constraint::Percentage(10));
/// assert_eq!(constraint!(*= 1), Constraint::Fill(1));
/// ```
///
/// [`constraints!`]: crate::constraints
#[macro_export]
macro_rules! constraint {
    (== $token:tt %) => {
        $crate::ratatui_core::layout::Constraint::Percentage($token)
    };
    (>= $expr:expr) => {
        $crate::ratatui_core::layout::Constraint::Min($expr)
    };
    (<= $expr:expr) => {
        $crate::ratatui_core::layout::Constraint::Max($expr)
    };
    (== $num:tt / $denom:tt) => {
        $crate::ratatui_core::layout::Constraint::Ratio($num as u32, $denom as u32)
    };
    (== $expr:expr) => {
        $crate::ratatui_core::layout::Constraint::Length($expr)
    };
    (*= $expr:expr) => {
        $crate::ratatui_core::layout::Constraint::Fill($expr)
    };
}

/// Creates an array of constraints.
///
/// See [`constraint!`] for more information.
///
/// If you want to solve the constraints, see
/// [`vertical!`] and [`horizontal!`] macros.
///
/// # Examples
///
/// ```rust
/// use ratatui_macros::constraints;
/// assert_eq!(constraints![==5, ==30%, >=3, <=1, ==1/2].len(), 5);
/// assert_eq!(constraints![==5; 5].len(), 5);
/// ```
///
/// ```rust
/// # use ratatui_core::layout::Constraint;
/// # use ratatui_macros::constraints;
/// assert_eq!(
///     constraints![==50, ==30%, >=3, <=1, ==1/2, *=1],
///     [
///         Constraint::Length(50),
///         Constraint::Percentage(30),
///         Constraint::Min(3),
///         Constraint::Max(1),
///         Constraint::Ratio(1, 2),
///         Constraint::Fill(1),
///     ]
/// )
/// ```
///
/// [`constraint!`]: crate::constraint
/// [`vertical!`]: crate::vertical
/// [`horizontal!`]: crate::horizontal
#[macro_export]
macro_rules! constraints {
    // Note: this implementation forgoes speed for the sake of simplicity. Adding variations of the
    // comma and semicolon rules for each constraint type would be faster, but would result in a lot
    // of duplicated code.

    // Cannot start the constraints macro with a ,
    ([ , $($rest:tt)* ] -> () []) => {
        compile_error!("No rules expected the token `,` while trying to match the end of the macro")
    };

    // Comma finishes a constraint element, so parse it and continue.
    // When a comma is encountered, it marks the end of a constraint element, so this rule is responsible
    // for parsing the constraint expression up to the comma and continuing the parsing process.
    // It accumulated the $partial contains a Constraint and is parsed using a separate $crate::constraint! macro.
    // The constraint is then appended to the list of parsed constraints.
    //
    // [ , $($rest:tt)* ]                     -> In the rule matcher, this pattern matches a comma followed
    //                                              by the rest of the tokens. The comma signals the end of
    //                                              the current constraint element.
    // ($($partial:tt)*)                      -> In the rule matcher, this contains the partial tokens
    //                                              accumulated so far for the current constraint element.
    // [$($parsed:tt)* ]                      -> This contains the constraints that have been successfully
    //                                              parsed so far.
    // $crate::constraint!($($partial)*)      -> This macro call parses and expands the accumulated
    //                                              partial tokens into a single Constraint expression.
    // [$($parsed)* $crate::constraint!(...)] -> Appends the newly parsed constraint to the list of
    //                                              already parsed constraints.
    ([ , $($rest:tt)* ] -> ($($partial:tt)*) [ $($parsed:tt)* ]) => {
        $crate::constraints!([$($rest)*] -> () [$($parsed)* $crate::constraint!($($partial)*) ,])
    };

    // Semicolon indicates that there's repetition. The trailing comma is required because the 'entrypoint'
    // rule adds a trailing comma.
    // This rule is triggered when a semicolon is encountered, indicating that there is repetition of
    // constraints. It handles the repetition logic by parsing the count and generating an array of
    // constraints using the $crate::constraint! macro.
    //
    // [ ; $count:expr , ]                  -> In the rule matcher, this pattern matches a semicolon
    //                                          followed by an expression representing the count, and a
    //                                          trailing comma.
    // ($($partial:tt)*)                    -> In the rule matcher, this contains the partial tokens
    //                                          accumulated so far for the current constraint element.
    //                                          This represents everything before the ;
    // []                                   -> There will be no existed parsed constraints when using ;
    // $crate::constraint!($($partial)*)    -> This macro call parses and expands the accumulated
    //                                          partial tokens into a single Constraint expression.
    // [$crate::constraint!(...) ; $count]  -> Generates an array of constraints by repeating the
    //                                          parsed constraint count number of times.
    ([ ; $count:expr , ] -> ($($partial:tt)*) []) => {
        [$crate::constraint!($($partial)*); $count]
    };

    // Pull the first token (which can't be a comma or semicolon) onto the accumulator.
    // if first token is a comma or semicolon, previous rules will match before this rule
    //
    // [ $head:tt $($rest:tt)* ]           -> In the rule matcher, this pulls a single `head` token
    //                                          out of the previous rest, and puts
    //                                          the remaining into `rest`
    // [ $($rest)* ]                       -> This is what is fed back into the `constraints!` macro
    //                                          as the first segment for the match rule
    //
    // ($($partial:tt)*)                   -> In the rule matcher, this contains previous partial
    //                                          tokens that will make up a `Constraint` expression
    // ($($partial)* $head)                -> This combines head with the previous partial tokens
    //                                          i.e. this is the accumulated tokens
    //
    // [ $($parsed:tt)* ]                  -> In the rule matcher, this contains all parsed exprs
    // [$($parsed)* ]                      -> These are passed on to the next match untouched.
    ([ $head:tt $($rest:tt)* ] -> ($($partial:tt)*) [ $($parsed:tt)* ]) => {
        $crate::constraints!([$($rest)*] -> ($($partial)* $head) [$($parsed)* ])
    };

    // This rule is triggered when there are no more input tokens to process. It signals the end of the
    // macro invocation and outputs the parsed constraints as a final array.
    ([$(,)?]  -> () [ $( $parsed:tt )* ]) => {
        [$($parsed)*]
    };

    // Entrypoint where there's no comma at the end.
    // We add a comma to make sure there's always a trailing comma.
    // Right-hand side will accumulate the actual `Constraint` literals.
    ($( $constraint:tt )+) => {
        $crate::constraints!([ $($constraint)+ , ] -> () [])
    };
}

/// Creates a vertical layout with specified constraints.
///
/// It accepts a series of constraints and applies them to create a vertical layout. The constraints
/// can include fixed sizes, minimum and maximum sizes, percentages, and ratios.
///
/// See [`constraint!`]  or [`constraints!`] for more information.
///
/// # Examples
///
/// ```
/// // Vertical layout with a fixed size and a percentage constraint
/// use ratatui_macros::vertical;
/// vertical![== 50, == 30%];
/// ```
#[macro_export]
macro_rules! vertical {
    ($( $constraint:tt )+) => {
        $crate::ratatui_core::layout::Layout::vertical($crate::constraints!( $($constraint)+ ))
    };
}

/// Creates a horizontal layout with specified constraints.
///
/// It takes a series of constraints and applies them to create a horizontal layout. The constraints
/// can include fixed sizes, minimum and maximum sizes, percentages, and ratios.
///
/// See [`constraint!`]  or [`constraints!`] for more information.
///
/// # Examples
///
/// ```
/// // Horizontal layout with a ratio constraint and a minimum size constraint
/// use ratatui_macros::horizontal;
/// horizontal![== 1/3, >= 100];
/// ```
#[macro_export]
macro_rules! horizontal {
    ($( $constraint:tt )+) => {
        $crate::ratatui_core::layout::Layout::horizontal($crate::constraints!( $($constraint)+ ))
    };
}
