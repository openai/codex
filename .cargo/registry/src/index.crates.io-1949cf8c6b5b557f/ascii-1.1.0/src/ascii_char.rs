use core::cmp::Ordering;
use core::mem;
use core::{char, fmt};
#[cfg(feature = "std")]
use std::error::Error;

#[allow(non_camel_case_types)]
/// An ASCII character. It wraps a `u8`, with the highest bit always zero.
#[derive(Clone, PartialEq, PartialOrd, Ord, Eq, Hash, Copy)]
#[repr(u8)]
pub enum AsciiChar {
    /// `'\0'`
    Null = 0,
    /// [Start Of Heading](http://en.wikipedia.org/wiki/Start_of_Heading)
    SOH = 1,
    /// [Start Of teXt](http://en.wikipedia.org/wiki/Start_of_Text)
    SOX = 2,
    /// [End of TeXt](http://en.wikipedia.org/wiki/End-of-Text_character)
    ETX = 3,
    /// [End Of Transmission](http://en.wikipedia.org/wiki/End-of-Transmission_character)
    EOT = 4,
    /// [Enquiry](http://en.wikipedia.org/wiki/Enquiry_character)
    ENQ = 5,
    /// [Acknowledgement](http://en.wikipedia.org/wiki/Acknowledge_character)
    ACK = 6,
    /// [bell / alarm / audible](http://en.wikipedia.org/wiki/Bell_character)
    ///
    /// `'\a'` is not recognized by Rust.
    Bell = 7,
    /// [Backspace](http://en.wikipedia.org/wiki/Backspace)
    ///
    /// `'\b'` is not recognized by Rust.
    BackSpace = 8,
    /// `'\t'`
    Tab = 9,
    /// `'\n'`
    LineFeed = 10,
    /// [Vertical tab](http://en.wikipedia.org/wiki/Vertical_Tab)
    ///
    /// `'\v'` is not recognized by Rust.
    VT = 11,
    /// [Form Feed](http://en.wikipedia.org/wiki/Form_Feed)
    ///
    /// `'\f'` is not recognized by Rust.
    FF = 12,
    /// `'\r'`
    CarriageReturn = 13,
    /// [Shift In](http://en.wikipedia.org/wiki/Shift_Out_and_Shift_In_characters)
    SI = 14,
    /// [Shift Out](http://en.wikipedia.org/wiki/Shift_Out_and_Shift_In_characters)
    SO = 15,
    /// [Data Link Escape](http://en.wikipedia.org/wiki/Data_Link_Escape)
    DLE = 16,
    /// [Device control 1, often XON](http://en.wikipedia.org/wiki/Device_Control_1)
    DC1 = 17,
    /// Device control 2
    DC2 = 18,
    /// Device control 3, Often XOFF
    DC3 = 19,
    /// Device control 4
    DC4 = 20,
    /// [Negative AcKnowledgement](http://en.wikipedia.org/wiki/Negative-acknowledge_character)
    NAK = 21,
    /// [Synchronous idle](http://en.wikipedia.org/wiki/Synchronous_Idle)
    SYN = 22,
    /// [End of Transmission Block](http://en.wikipedia.org/wiki/End-of-Transmission-Block_character)
    ETB = 23,
    /// [Cancel](http://en.wikipedia.org/wiki/Cancel_character)
    CAN = 24,
    /// [End of Medium](http://en.wikipedia.org/wiki/End_of_Medium)
    EM = 25,
    /// [Substitute](http://en.wikipedia.org/wiki/Substitute_character)
    SUB = 26,
    /// [Escape](http://en.wikipedia.org/wiki/Escape_character)
    ///
    /// `'\e'` is not recognized by Rust.
    ESC = 27,
    /// [File Separator](http://en.wikipedia.org/wiki/File_separator)
    FS = 28,
    /// [Group Separator](http://en.wikipedia.org/wiki/Group_separator)
    GS = 29,
    /// [Record Separator](http://en.wikipedia.org/wiki/Record_separator)
    RS = 30,
    /// [Unit Separator](http://en.wikipedia.org/wiki/Unit_separator)
    US = 31,
    /// `' '`
    Space = 32,
    /// `'!'`
    Exclamation = 33,
    /// `'"'`
    Quotation = 34,
    /// `'#'`
    Hash = 35,
    /// `'$'`
    Dollar = 36,
    /// `'%'`
    Percent = 37,
    /// `'&'`
    Ampersand = 38,
    /// `'\''`
    Apostrophe = 39,
    /// `'('`
    ParenOpen = 40,
    /// `')'`
    ParenClose = 41,
    /// `'*'`
    Asterisk = 42,
    /// `'+'`
    Plus = 43,
    /// `','`
    Comma = 44,
    /// `'-'`
    Minus = 45,
    /// `'.'`
    Dot = 46,
    /// `'/'`
    Slash = 47,
    /// `'0'`
    _0 = 48,
    /// `'1'`
    _1 = 49,
    /// `'2'`
    _2 = 50,
    /// `'3'`
    _3 = 51,
    /// `'4'`
    _4 = 52,
    /// `'5'`
    _5 = 53,
    /// `'6'`
    _6 = 54,
    /// `'7'`
    _7 = 55,
    /// `'8'`
    _8 = 56,
    /// `'9'`
    _9 = 57,
    /// `':'`
    Colon = 58,
    /// `';'`
    Semicolon = 59,
    /// `'<'`
    LessThan = 60,
    /// `'='`
    Equal = 61,
    /// `'>'`
    GreaterThan = 62,
    /// `'?'`
    Question = 63,
    /// `'@'`
    At = 64,
    /// `'A'`
    A = 65,
    /// `'B'`
    B = 66,
    /// `'C'`
    C = 67,
    /// `'D'`
    D = 68,
    /// `'E'`
    E = 69,
    /// `'F'`
    F = 70,
    /// `'G'`
    G = 71,
    /// `'H'`
    H = 72,
    /// `'I'`
    I = 73,
    /// `'J'`
    J = 74,
    /// `'K'`
    K = 75,
    /// `'L'`
    L = 76,
    /// `'M'`
    M = 77,
    /// `'N'`
    N = 78,
    /// `'O'`
    O = 79,
    /// `'P'`
    P = 80,
    /// `'Q'`
    Q = 81,
    /// `'R'`
    R = 82,
    /// `'S'`
    S = 83,
    /// `'T'`
    T = 84,
    /// `'U'`
    U = 85,
    /// `'V'`
    V = 86,
    /// `'W'`
    W = 87,
    /// `'X'`
    X = 88,
    /// `'Y'`
    Y = 89,
    /// `'Z'`
    Z = 90,
    /// `'['`
    BracketOpen = 91,
    /// `'\'`
    BackSlash = 92,
    /// `']'`
    BracketClose = 93,
    /// `'^'`
    Caret = 94,
    /// `'_'`
    UnderScore = 95,
    /// `'`'`
    Grave = 96,
    /// `'a'`
    a = 97,
    /// `'b'`
    b = 98,
    /// `'c'`
    c = 99,
    /// `'d'`
    d = 100,
    /// `'e'`
    e = 101,
    /// `'f'`
    f = 102,
    /// `'g'`
    g = 103,
    /// `'h'`
    h = 104,
    /// `'i'`
    i = 105,
    /// `'j'`
    j = 106,
    /// `'k'`
    k = 107,
    /// `'l'`
    l = 108,
    /// `'m'`
    m = 109,
    /// `'n'`
    n = 110,
    /// `'o'`
    o = 111,
    /// `'p'`
    p = 112,
    /// `'q'`
    q = 113,
    /// `'r'`
    r = 114,
    /// `'s'`
    s = 115,
    /// `'t'`
    t = 116,
    /// `'u'`
    u = 117,
    /// `'v'`
    v = 118,
    /// `'w'`
    w = 119,
    /// `'x'`
    x = 120,
    /// `'y'`
    y = 121,
    /// `'z'`
    z = 122,
    /// `'{'`
    CurlyBraceOpen = 123,
    /// `'|'`
    VerticalBar = 124,
    /// `'}'`
    CurlyBraceClose = 125,
    /// `'~'`
    Tilde = 126,
    /// [Delete](http://en.wikipedia.org/wiki/Delete_character)
    DEL = 127,
}

impl AsciiChar {
    /// Constructs an ASCII character from a `u8`, `char` or other character type.
    ///
    /// # Errors
    /// Returns `Err(())` if the character can't be ASCII encoded.
    ///
    /// # Example
    /// ```
    /// # use ascii::AsciiChar;
    /// let a = AsciiChar::from_ascii('g').unwrap();
    /// assert_eq!(a.as_char(), 'g');
    /// ```
    #[inline]
    pub fn from_ascii<C: ToAsciiChar>(ch: C) -> Result<Self, ToAsciiCharError> {
        ch.to_ascii_char()
    }

    /// Create an `AsciiChar` from a `char`, panicking if it's not ASCII.
    ///
    /// This function is intended for creating `AsciiChar` values from
    /// hardcoded known-good character literals such as `'K'`, `'-'` or `'\0'`,
    /// and for use in `const` contexts.
    /// Use [`from_ascii()`](#method.from_ascii) instead when you're not
    /// certain the character is ASCII.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ascii::AsciiChar;
    /// assert_eq!(AsciiChar::new('@'), AsciiChar::At);
    /// assert_eq!(AsciiChar::new('C').as_char(), 'C');
    /// ```
    ///
    /// In a constant:
    /// ```
    /// # use ascii::AsciiChar;
    /// const SPLIT_ON: AsciiChar = AsciiChar::new(',');
    /// ```
    ///
    /// This will not compile:
    /// ```compile_fail
    /// # use ascii::AsciiChar;
    /// const BAD: AsciiChar = AsciiChar::new('Ø');
    /// ```
    ///
    /// # Panics
    ///
    /// This function will panic if passed a non-ASCII character.
    ///
    /// The panic message might not be the most descriptive due to the
    /// current limitations of `const fn`.
    #[must_use]
    pub const fn new(ch: char) -> AsciiChar {
        // It's restricted to this function, and without it
        // we'd need to specify `AsciiChar::` or `Self::` 128 times.
        #[allow(clippy::enum_glob_use)]
        use AsciiChar::*;

        #[rustfmt::skip]
        const ALL: [AsciiChar; 128] = [
            Null, SOH, SOX, ETX, EOT, ENQ, ACK, Bell,
            BackSpace, Tab, LineFeed, VT, FF, CarriageReturn, SI, SO,
            DLE, DC1, DC2, DC3, DC4, NAK, SYN, ETB,
            CAN, EM, SUB, ESC, FS, GS, RS, US,
            Space, Exclamation, Quotation, Hash, Dollar, Percent, Ampersand, Apostrophe,
            ParenOpen, ParenClose, Asterisk, Plus, Comma, Minus, Dot, Slash,
            _0, _1, _2, _3, _4, _5, _6, _7,
            _8, _9, Colon, Semicolon, LessThan, Equal, GreaterThan, Question,
            At, A, B, C, D, E, F, G,
            H, I, J, K, L, M, N, O,
            P, Q, R, S, T, U, V, W,
            X, Y, Z, BracketOpen, BackSlash, BracketClose, Caret, UnderScore,
            Grave, a, b, c, d, e, f, g,
            h, i, j, k, l, m, n, o,
            p, q, r, s, t, u, v, w,
            x, y, z, CurlyBraceOpen, VerticalBar, CurlyBraceClose, Tilde, DEL,
		];

        // We want to slice here and detect `const_err` from rustc if the slice is invalid
        #[allow(clippy::indexing_slicing)]
        ALL[ch as usize]
    }

    /// Constructs an ASCII character from a `u8`, `char` or other character
    /// type without any checks.
    ///
    /// # Safety
    ///
    /// This function is very unsafe as it can create invalid enum
    /// discriminants, which instantly creates undefined behavior.
    /// (`let _ = AsciiChar::from_ascii_unchecked(200);` alone is UB).
    ///
    /// The undefined behavior is not just theoretical either:
    /// For example, `[0; 128][AsciiChar::from_ascii_unchecked(255) as u8 as usize] = 0`
    /// might not panic, creating a buffer overflow,
    /// and `Some(AsciiChar::from_ascii_unchecked(128))` might be `None`.
    #[inline]
    #[must_use]
    pub unsafe fn from_ascii_unchecked(ch: u8) -> Self {
        // SAFETY: Caller guarantees `ch` is within bounds of ascii.
        unsafe { ch.to_ascii_char_unchecked() }
    }

    /// Converts an ASCII character into a `u8`.
    #[inline]
    #[must_use]
    pub const fn as_byte(self) -> u8 {
        self as u8
    }

    /// Converts an ASCII character into a `char`.
    #[inline]
    #[must_use]
    pub const fn as_char(self) -> char {
        self as u8 as char
    }

    // the following methods are like ctype, and the implementation is inspired by musl.
    // The ascii_ methods take self by reference for maximum compatibility
    // with the corresponding methods on u8 and char.
    // It is bad for both usability and performance, but marking those
    // that doesn't have a non-ascii sibling #[inline] should
    // make the compiler optimize away the indirection.

    /// Turns uppercase into lowercase, but also modifies '@' and '<'..='_'
    #[must_use]
    const fn to_not_upper(self) -> u8 {
        self as u8 | 0b010_0000
    }

    /// Check if the character is a letter (a-z, A-Z)
    #[inline]
    #[must_use]
    pub const fn is_alphabetic(self) -> bool {
        (self.to_not_upper() >= b'a') & (self.to_not_upper() <= b'z')
    }

    /// Check if the character is a letter (a-z, A-Z).
    ///
    /// This method is identical to [`is_alphabetic()`](#method.is_alphabetic)
    #[inline]
    #[must_use]
    pub const fn is_ascii_alphabetic(&self) -> bool {
        self.is_alphabetic()
    }

    /// Check if the character is a digit in the given radix.
    ///
    /// If the radix is always 10 or 16,
    /// [`is_ascii_digit()`](#method.is_ascii_digit) and
    /// [`is_ascii_hexdigit()`](#method.is_ascii_hexdigit()) will be faster.
    ///
    /// # Panics
    ///
    /// Radixes greater than 36 are not supported and will result in a panic.
    #[must_use]
    pub fn is_digit(self, radix: u32) -> bool {
        match (self as u8, radix) {
            (b'0'..=b'9', 0..=36) => u32::from(self as u8 - b'0') < radix,
            (b'a'..=b'z', 11..=36) => u32::from(self as u8 - b'a') < radix - 10,
            (b'A'..=b'Z', 11..=36) => u32::from(self as u8 - b'A') < radix - 10,
            (_, 0..=36) => false,
            (_, _) => panic!("radixes greater than 36 are not supported"),
        }
    }

    /// Check if the character is a number (0-9)
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiChar;
    /// assert_eq!(AsciiChar::new('0').is_ascii_digit(), true);
    /// assert_eq!(AsciiChar::new('9').is_ascii_digit(), true);
    /// assert_eq!(AsciiChar::new('a').is_ascii_digit(), false);
    /// assert_eq!(AsciiChar::new('A').is_ascii_digit(), false);
    /// assert_eq!(AsciiChar::new('/').is_ascii_digit(), false);
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_ascii_digit(&self) -> bool {
        (*self as u8 >= b'0') & (*self as u8 <= b'9')
    }

    /// Check if the character is a letter or number
    #[inline]
    #[must_use]
    pub const fn is_alphanumeric(self) -> bool {
        self.is_alphabetic() | self.is_ascii_digit()
    }

    /// Check if the character is a letter or number
    ///
    /// This method is identical to [`is_alphanumeric()`](#method.is_alphanumeric)
    #[inline]
    #[must_use]
    pub const fn is_ascii_alphanumeric(&self) -> bool {
        self.is_alphanumeric()
    }

    /// Check if the character is a space or horizontal tab
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiChar;
    /// assert!(AsciiChar::Space.is_ascii_blank());
    /// assert!(AsciiChar::Tab.is_ascii_blank());
    /// assert!(!AsciiChar::VT.is_ascii_blank());
    /// assert!(!AsciiChar::LineFeed.is_ascii_blank());
    /// assert!(!AsciiChar::CarriageReturn.is_ascii_blank());
    /// assert!(!AsciiChar::FF.is_ascii_blank());
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_ascii_blank(&self) -> bool {
        (*self as u8 == b' ') | (*self as u8 == b'\t')
    }

    /// Check if the character one of ' ', '\t', '\n', '\r',
    /// '\0xb' (vertical tab) or '\0xc' (form feed).
    #[inline]
    #[must_use]
    pub const fn is_whitespace(self) -> bool {
        let b = self as u8;
        self.is_ascii_blank() | (b == b'\n') | (b == b'\r') | (b == 0x0b) | (b == 0x0c)
    }

    /// Check if the character is a ' ', '\t', '\n', '\r' or '\0xc' (form feed).
    ///
    /// This method is NOT identical to `is_whitespace()`.
    #[inline]
    #[must_use]
    pub const fn is_ascii_whitespace(&self) -> bool {
        self.is_ascii_blank()
            | (*self as u8 == b'\n')
            | (*self as u8 == b'\r')
            | (*self as u8 == 0x0c/*form feed*/)
    }

    /// Check if the character is a control character
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiChar;
    /// assert_eq!(AsciiChar::new('\0').is_ascii_control(), true);
    /// assert_eq!(AsciiChar::new('n').is_ascii_control(), false);
    /// assert_eq!(AsciiChar::new(' ').is_ascii_control(), false);
    /// assert_eq!(AsciiChar::new('\n').is_ascii_control(), true);
    /// assert_eq!(AsciiChar::new('\t').is_ascii_control(), true);
    /// assert_eq!(AsciiChar::EOT.is_ascii_control(), true);
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_ascii_control(&self) -> bool {
        ((*self as u8) < b' ') | (*self as u8 == 127)
    }

    /// Checks if the character is printable (except space)
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiChar;
    /// assert_eq!(AsciiChar::new('n').is_ascii_graphic(), true);
    /// assert_eq!(AsciiChar::new(' ').is_ascii_graphic(), false);
    /// assert_eq!(AsciiChar::new('\n').is_ascii_graphic(), false);
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_ascii_graphic(&self) -> bool {
        self.as_byte().wrapping_sub(b' ' + 1) < 0x5E
    }

    /// Checks if the character is printable (including space)
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiChar;
    /// assert_eq!(AsciiChar::new('n').is_ascii_printable(), true);
    /// assert_eq!(AsciiChar::new(' ').is_ascii_printable(), true);
    /// assert_eq!(AsciiChar::new('\n').is_ascii_printable(), false);
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_ascii_printable(&self) -> bool {
        self.as_byte().wrapping_sub(b' ') < 0x5F
    }

    /// Checks if the character is alphabetic and lowercase (a-z).
    ///
    /// # Examples
    /// ```
    /// use ascii::AsciiChar;
    /// assert_eq!(AsciiChar::new('a').is_lowercase(), true);
    /// assert_eq!(AsciiChar::new('A').is_lowercase(), false);
    /// assert_eq!(AsciiChar::new('@').is_lowercase(), false);
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_lowercase(self) -> bool {
        self.as_byte().wrapping_sub(b'a') < 26
    }

    /// Checks if the character is alphabetic and lowercase (a-z).
    ///
    /// This method is identical to [`is_lowercase()`](#method.is_lowercase)
    #[inline]
    #[must_use]
    pub const fn is_ascii_lowercase(&self) -> bool {
        self.is_lowercase()
    }

    /// Checks if the character is alphabetic and uppercase (A-Z).
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiChar;
    /// assert_eq!(AsciiChar::new('A').is_uppercase(), true);
    /// assert_eq!(AsciiChar::new('a').is_uppercase(), false);
    /// assert_eq!(AsciiChar::new('@').is_uppercase(), false);
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_uppercase(self) -> bool {
        self.as_byte().wrapping_sub(b'A') < 26
    }

    /// Checks if the character is alphabetic and uppercase (A-Z).
    ///
    /// This method is identical to [`is_uppercase()`](#method.is_uppercase)
    #[inline]
    #[must_use]
    pub const fn is_ascii_uppercase(&self) -> bool {
        self.is_uppercase()
    }

    /// Checks if the character is punctuation
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiChar;
    /// assert_eq!(AsciiChar::new('n').is_ascii_punctuation(), false);
    /// assert_eq!(AsciiChar::new(' ').is_ascii_punctuation(), false);
    /// assert_eq!(AsciiChar::new('_').is_ascii_punctuation(), true);
    /// assert_eq!(AsciiChar::new('~').is_ascii_punctuation(), true);
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_ascii_punctuation(&self) -> bool {
        self.is_ascii_graphic() & !self.is_alphanumeric()
    }

    /// Checks if the character is a valid hex digit
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiChar;
    /// assert_eq!(AsciiChar::new('5').is_ascii_hexdigit(), true);
    /// assert_eq!(AsciiChar::new('a').is_ascii_hexdigit(), true);
    /// assert_eq!(AsciiChar::new('F').is_ascii_hexdigit(), true);
    /// assert_eq!(AsciiChar::new('G').is_ascii_hexdigit(), false);
    /// assert_eq!(AsciiChar::new(' ').is_ascii_hexdigit(), false);
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_ascii_hexdigit(&self) -> bool {
        self.is_ascii_digit() | ((*self as u8 | 0x20_u8).wrapping_sub(b'a') < 6)
    }

    /// Unicode has printable versions of the ASCII control codes, like '␛'.
    ///
    /// This function is identical with `.as_char()`
    /// for all values `.is_printable()` returns true for,
    /// but replaces the control codes with those unicodes printable versions.
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiChar;
    /// assert_eq!(AsciiChar::new('\0').as_printable_char(), '␀');
    /// assert_eq!(AsciiChar::new('\n').as_printable_char(), '␊');
    /// assert_eq!(AsciiChar::new(' ').as_printable_char(), ' ');
    /// assert_eq!(AsciiChar::new('p').as_printable_char(), 'p');
    /// ```
    #[must_use]
    pub fn as_printable_char(self) -> char {
        match self as u8 {
            // Non printable characters
            // SAFETY: From codepoint 0x2400 ('␀') to 0x241f (`␟`), there are characters representing
            //         the unprintable characters from 0x0 to 0x1f, ordered correctly.
            //         As `b` is guaranteed to be within 0x0 to 0x1f, the conversion represents a
            //         valid character.
            b @ 0x0..=0x1f => unsafe { char::from_u32_unchecked(u32::from('␀') + u32::from(b)) },

            // 0x7f (delete) has it's own character at codepoint 0x2420, not 0x247f, so it is special
            // cased to return it's character
            0x7f => '␡',

            // All other characters are printable, and per function contract use `Self::as_char`
            _ => self.as_char(),
        }
    }

    /// Replaces letters `a` to `z` with `A` to `Z`
    pub fn make_ascii_uppercase(&mut self) {
        *self = self.to_ascii_uppercase();
    }

    /// Replaces letters `A` to `Z` with `a` to `z`
    pub fn make_ascii_lowercase(&mut self) {
        *self = self.to_ascii_lowercase();
    }

    /// Maps letters a-z to A-Z and returns any other character unchanged.
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiChar;
    /// assert_eq!(AsciiChar::new('u').to_ascii_uppercase().as_char(), 'U');
    /// assert_eq!(AsciiChar::new('U').to_ascii_uppercase().as_char(), 'U');
    /// assert_eq!(AsciiChar::new('2').to_ascii_uppercase().as_char(), '2');
    /// assert_eq!(AsciiChar::new('=').to_ascii_uppercase().as_char(), '=');
    /// assert_eq!(AsciiChar::new('[').to_ascii_uppercase().as_char(), '[');
    /// ```
    #[inline]
    #[must_use]
    #[allow(clippy::indexing_slicing)] // We're sure it'll either access one or the other, as `bool` is either `0` or `1`
    pub const fn to_ascii_uppercase(&self) -> Self {
        [*self, AsciiChar::new((*self as u8 & 0b101_1111) as char)][self.is_lowercase() as usize]
    }

    /// Maps letters A-Z to a-z and returns any other character unchanged.
    ///
    /// # Examples
    /// ```
    /// # use ascii::AsciiChar;
    /// assert_eq!(AsciiChar::new('U').to_ascii_lowercase().as_char(), 'u');
    /// assert_eq!(AsciiChar::new('u').to_ascii_lowercase().as_char(), 'u');
    /// assert_eq!(AsciiChar::new('2').to_ascii_lowercase().as_char(), '2');
    /// assert_eq!(AsciiChar::new('^').to_ascii_lowercase().as_char(), '^');
    /// assert_eq!(AsciiChar::new('\x7f').to_ascii_lowercase().as_char(), '\x7f');
    /// ```
    #[inline]
    #[must_use]
    #[allow(clippy::indexing_slicing)] // We're sure it'll either access one or the other, as `bool` is either `0` or `1`
    pub const fn to_ascii_lowercase(&self) -> Self {
        [*self, AsciiChar::new(self.to_not_upper() as char)][self.is_uppercase() as usize]
    }

    /// Compares two characters case-insensitively.
    #[inline]
    #[must_use]
    pub const fn eq_ignore_ascii_case(&self, other: &Self) -> bool {
        (self.as_byte() == other.as_byte())
            | (self.is_alphabetic() & (self.to_not_upper() == other.to_not_upper()))
    }
}

impl fmt::Display for AsciiChar {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.as_char().fmt(f)
    }
}

impl fmt::Debug for AsciiChar {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.as_char().fmt(f)
    }
}

impl Default for AsciiChar {
    fn default() -> AsciiChar {
        AsciiChar::Null
    }
}

macro_rules! impl_into_partial_eq_ord {
    ($wider:ty, $to_wider:expr) => {
        impl From<AsciiChar> for $wider {
            #[inline]
            fn from(a: AsciiChar) -> $wider {
                $to_wider(a)
            }
        }
        impl PartialEq<$wider> for AsciiChar {
            #[inline]
            fn eq(&self, rhs: &$wider) -> bool {
                $to_wider(*self) == *rhs
            }
        }
        impl PartialEq<AsciiChar> for $wider {
            #[inline]
            fn eq(&self, rhs: &AsciiChar) -> bool {
                *self == $to_wider(*rhs)
            }
        }
        impl PartialOrd<$wider> for AsciiChar {
            #[inline]
            fn partial_cmp(&self, rhs: &$wider) -> Option<Ordering> {
                $to_wider(*self).partial_cmp(rhs)
            }
        }
        impl PartialOrd<AsciiChar> for $wider {
            #[inline]
            fn partial_cmp(&self, rhs: &AsciiChar) -> Option<Ordering> {
                self.partial_cmp(&$to_wider(*rhs))
            }
        }
    };
}
impl_into_partial_eq_ord! {u8, AsciiChar::as_byte}
impl_into_partial_eq_ord! {char, AsciiChar::as_char}

/// Error returned by `ToAsciiChar`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ToAsciiCharError(());

const ERRORMSG_CHAR: &str = "not an ASCII character";

#[cfg(not(feature = "std"))]
impl ToAsciiCharError {
    /// Returns a description for this error, like `std::error::Error::description`.
    #[inline]
    #[must_use]
    #[allow(clippy::unused_self)]
    pub const fn description(&self) -> &'static str {
        ERRORMSG_CHAR
    }
}

impl fmt::Debug for ToAsciiCharError {
    fn fmt(&self, fmtr: &mut fmt::Formatter) -> fmt::Result {
        write!(fmtr, "{}", ERRORMSG_CHAR)
    }
}

impl fmt::Display for ToAsciiCharError {
    fn fmt(&self, fmtr: &mut fmt::Formatter) -> fmt::Result {
        write!(fmtr, "{}", ERRORMSG_CHAR)
    }
}

#[cfg(feature = "std")]
impl Error for ToAsciiCharError {
    #[inline]
    fn description(&self) -> &'static str {
        ERRORMSG_CHAR
    }
}

/// Convert `char`, `u8` and other character types to `AsciiChar`.
pub trait ToAsciiChar {
    /// Convert to `AsciiChar`.
    ///
    /// # Errors
    /// If `self` is outside the valid ascii range, this returns `Err`
    fn to_ascii_char(self) -> Result<AsciiChar, ToAsciiCharError>;

    /// Convert to `AsciiChar` without checking that it is an ASCII character.
    ///
    /// # Safety
    /// Calling this function with a value outside of the ascii range, `0x0` to `0x7f` inclusive,
    /// is undefined behavior.
    // TODO: Make sure this is the contract we want to express in this function.
    //       It is ambigous if numbers such as `0xffffff20_u32` are valid ascii characters,
    //       as this function returns `Ascii::Space` due to the cast to `u8`, even though
    //       `to_ascii_char` returns `Err()`.
    unsafe fn to_ascii_char_unchecked(self) -> AsciiChar;
}

impl ToAsciiChar for AsciiChar {
    #[inline]
    fn to_ascii_char(self) -> Result<AsciiChar, ToAsciiCharError> {
        Ok(self)
    }

    #[inline]
    unsafe fn to_ascii_char_unchecked(self) -> AsciiChar {
        self
    }
}

impl ToAsciiChar for u8 {
    #[inline]
    fn to_ascii_char(self) -> Result<AsciiChar, ToAsciiCharError> {
        u32::from(self).to_ascii_char()
    }
    #[inline]
    unsafe fn to_ascii_char_unchecked(self) -> AsciiChar {
        // SAFETY: Caller guarantees `self` is within bounds of the enum
        //         variants, so this cast successfully produces a valid ascii
        //         variant
        unsafe { mem::transmute::<u8, AsciiChar>(self) }
    }
}

// Note: Casts to `u8` here does not cause problems, as the negative
//       range is mapped outside of ascii bounds and we don't mind losing
//       the sign, as long as negative numbers are mapped outside ascii range.
#[allow(clippy::cast_sign_loss)]
impl ToAsciiChar for i8 {
    #[inline]
    fn to_ascii_char(self) -> Result<AsciiChar, ToAsciiCharError> {
        u32::from(self as u8).to_ascii_char()
    }
    #[inline]
    unsafe fn to_ascii_char_unchecked(self) -> AsciiChar {
        // SAFETY: Caller guarantees `self` is within bounds of the enum
        //         variants, so this cast successfully produces a valid ascii
        //         variant
        unsafe { mem::transmute::<u8, AsciiChar>(self as u8) }
    }
}

impl ToAsciiChar for char {
    #[inline]
    fn to_ascii_char(self) -> Result<AsciiChar, ToAsciiCharError> {
        u32::from(self).to_ascii_char()
    }
    #[inline]
    unsafe fn to_ascii_char_unchecked(self) -> AsciiChar {
        // SAFETY: Caller guarantees we're within ascii range.
        unsafe { u32::from(self).to_ascii_char_unchecked() }
    }
}

impl ToAsciiChar for u32 {
    fn to_ascii_char(self) -> Result<AsciiChar, ToAsciiCharError> {
        match self {
            // SAFETY: We're within the valid ascii range in this branch.
            0x0..=0x7f => Ok(unsafe { self.to_ascii_char_unchecked() }),
            _ => Err(ToAsciiCharError(())),
        }
    }

    #[inline]
    unsafe fn to_ascii_char_unchecked(self) -> AsciiChar {
        // Note: This cast discards the top bytes, this may cause problems, see
        //       the TODO on this method's documentation in the trait.
        // SAFETY: Caller guarantees we're within ascii range.
        #[allow(clippy::cast_possible_truncation)] // We want to truncate it
        unsafe {
            (self as u8).to_ascii_char_unchecked()
        }
    }
}

impl ToAsciiChar for u16 {
    fn to_ascii_char(self) -> Result<AsciiChar, ToAsciiCharError> {
        u32::from(self).to_ascii_char()
    }
    #[inline]
    unsafe fn to_ascii_char_unchecked(self) -> AsciiChar {
        // Note: This cast discards the top bytes, this may cause problems, see
        //       the TODO on this method's documentation in the trait.
        // SAFETY: Caller guarantees we're within ascii range.
        #[allow(clippy::cast_possible_truncation)] // We want to truncate it
        unsafe {
            (self as u8).to_ascii_char_unchecked()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AsciiChar, ToAsciiChar, ToAsciiCharError};

    #[test]
    fn to_ascii_char() {
        fn generic<C: ToAsciiChar>(ch: C) -> Result<AsciiChar, ToAsciiCharError> {
            ch.to_ascii_char()
        }
        assert_eq!(generic(AsciiChar::A), Ok(AsciiChar::A));
        assert_eq!(generic(b'A'), Ok(AsciiChar::A));
        assert_eq!(generic('A'), Ok(AsciiChar::A));
        assert!(generic(200_u16).is_err());
        assert!(generic('λ').is_err());
    }

    #[test]
    fn as_byte_and_char() {
        assert_eq!(AsciiChar::A.as_byte(), b'A');
        assert_eq!(AsciiChar::A.as_char(), 'A');
    }

    #[test]
    fn new_array_is_correct() {
        for byte in 0..128_u8 {
            assert_eq!(AsciiChar::new(byte as char).as_byte(), byte);
        }
    }

    #[test]
    fn is_all() {
        #![allow(clippy::is_digit_ascii_radix)] // testing it
        for byte in 0..128_u8 {
            let ch = byte as char;
            let ascii = AsciiChar::new(ch);
            assert_eq!(ascii.is_alphabetic(), ch.is_alphabetic());
            assert_eq!(ascii.is_ascii_alphabetic(), ch.is_ascii_alphabetic());
            assert_eq!(ascii.is_alphanumeric(), ch.is_alphanumeric());
            assert_eq!(ascii.is_ascii_alphanumeric(), ch.is_ascii_alphanumeric());
            assert_eq!(ascii.is_digit(8), ch.is_digit(8), "is_digit(8) {:?}", ch);
            assert_eq!(ascii.is_digit(10), ch.is_digit(10), "is_digit(10) {:?}", ch);
            assert_eq!(ascii.is_digit(16), ch.is_digit(16), "is_digit(16) {:?}", ch);
            assert_eq!(ascii.is_digit(36), ch.is_digit(36), "is_digit(36) {:?}", ch);
            assert_eq!(ascii.is_ascii_digit(), ch.is_ascii_digit());
            assert_eq!(ascii.is_ascii_hexdigit(), ch.is_ascii_hexdigit());
            assert_eq!(ascii.is_ascii_control(), ch.is_ascii_control());
            assert_eq!(ascii.is_ascii_graphic(), ch.is_ascii_graphic());
            assert_eq!(ascii.is_ascii_punctuation(), ch.is_ascii_punctuation());
            assert_eq!(
                ascii.is_whitespace(),
                ch.is_whitespace(),
                "{:?} ({:#04x})",
                ch,
                byte
            );
            assert_eq!(
                ascii.is_ascii_whitespace(),
                ch.is_ascii_whitespace(),
                "{:?} ({:#04x})",
                ch,
                byte
            );
            assert_eq!(ascii.is_uppercase(), ch.is_uppercase());
            assert_eq!(ascii.is_ascii_uppercase(), ch.is_ascii_uppercase());
            assert_eq!(ascii.is_lowercase(), ch.is_lowercase());
            assert_eq!(ascii.is_ascii_lowercase(), ch.is_ascii_lowercase());
            assert_eq!(ascii.to_ascii_uppercase(), ch.to_ascii_uppercase());
            assert_eq!(ascii.to_ascii_lowercase(), ch.to_ascii_lowercase());
        }
    }

    #[test]
    fn is_digit_strange_radixes() {
        assert_eq!(AsciiChar::_0.is_digit(0), '0'.is_digit(0));
        assert_eq!(AsciiChar::_0.is_digit(1), '0'.is_digit(1));
        assert_eq!(AsciiChar::_5.is_digit(5), '5'.is_digit(5));
        assert_eq!(AsciiChar::z.is_digit(35), 'z'.is_digit(35));
    }

    #[test]
    #[should_panic]
    fn is_digit_bad_radix() {
        let _ = AsciiChar::_7.is_digit(37);
    }

    #[test]
    fn cmp_wider() {
        assert_eq!(AsciiChar::A, 'A');
        assert_eq!(b'b', AsciiChar::b);
        assert!(AsciiChar::a < 'z');
    }

    #[test]
    fn ascii_case() {
        assert_eq!(AsciiChar::At.to_ascii_lowercase(), AsciiChar::At);
        assert_eq!(AsciiChar::At.to_ascii_uppercase(), AsciiChar::At);
        assert_eq!(AsciiChar::A.to_ascii_lowercase(), AsciiChar::a);
        assert_eq!(AsciiChar::A.to_ascii_uppercase(), AsciiChar::A);
        assert_eq!(AsciiChar::a.to_ascii_lowercase(), AsciiChar::a);
        assert_eq!(AsciiChar::a.to_ascii_uppercase(), AsciiChar::A);

        let mut mutable = (AsciiChar::A, AsciiChar::a);
        mutable.0.make_ascii_lowercase();
        mutable.1.make_ascii_uppercase();
        assert_eq!(mutable.0, AsciiChar::a);
        assert_eq!(mutable.1, AsciiChar::A);

        assert!(AsciiChar::LineFeed.eq_ignore_ascii_case(&AsciiChar::LineFeed));
        assert!(!AsciiChar::LineFeed.eq_ignore_ascii_case(&AsciiChar::CarriageReturn));
        assert!(AsciiChar::z.eq_ignore_ascii_case(&AsciiChar::Z));
        assert!(AsciiChar::Z.eq_ignore_ascii_case(&AsciiChar::z));
        assert!(AsciiChar::A.eq_ignore_ascii_case(&AsciiChar::a));
        assert!(!AsciiChar::K.eq_ignore_ascii_case(&AsciiChar::C));
        assert!(!AsciiChar::Z.eq_ignore_ascii_case(&AsciiChar::DEL));
        assert!(!AsciiChar::BracketOpen.eq_ignore_ascii_case(&AsciiChar::CurlyBraceOpen));
        assert!(!AsciiChar::Grave.eq_ignore_ascii_case(&AsciiChar::At));
        assert!(!AsciiChar::Grave.eq_ignore_ascii_case(&AsciiChar::DEL));
    }

    #[test]
    #[cfg(feature = "std")]
    fn fmt_ascii() {
        assert_eq!(format!("{}", AsciiChar::t), "t");
        assert_eq!(format!("{:?}", AsciiChar::t), "'t'");
        assert_eq!(format!("{}", AsciiChar::LineFeed), "\n");
        assert_eq!(format!("{:?}", AsciiChar::LineFeed), "'\\n'");
    }
}
