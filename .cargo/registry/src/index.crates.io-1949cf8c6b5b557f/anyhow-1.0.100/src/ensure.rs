use crate::Error;
use alloc::string::String;
use core::fmt::{self, Debug, Write};
use core::mem::MaybeUninit;
use core::ptr;
use core::slice;
use core::str;

#[doc(hidden)]
pub trait BothDebug {
    fn __dispatch_ensure(self, msg: &'static str) -> Error;
}

impl<A, B> BothDebug for (A, B)
where
    A: Debug,
    B: Debug,
{
    fn __dispatch_ensure(self, msg: &'static str) -> Error {
        render(msg, &self.0, &self.1)
    }
}

#[doc(hidden)]
pub trait NotBothDebug {
    fn __dispatch_ensure(self, msg: &'static str) -> Error;
}

impl<A, B> NotBothDebug for &(A, B) {
    fn __dispatch_ensure(self, msg: &'static str) -> Error {
        Error::msg(msg)
    }
}

struct Buf {
    bytes: [MaybeUninit<u8>; 40],
    written: usize,
}

impl Buf {
    fn new() -> Self {
        Buf {
            bytes: [MaybeUninit::uninit(); 40],
            written: 0,
        }
    }

    fn as_str(&self) -> &str {
        unsafe {
            str::from_utf8_unchecked(slice::from_raw_parts(
                self.bytes.as_ptr().cast::<u8>(),
                self.written,
            ))
        }
    }
}

impl Write for Buf {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if s.bytes().any(|b| b == b' ' || b == b'\n') {
            return Err(fmt::Error);
        }

        let remaining = self.bytes.len() - self.written;
        if s.len() > remaining {
            return Err(fmt::Error);
        }

        unsafe {
            ptr::copy_nonoverlapping(
                s.as_ptr(),
                self.bytes.as_mut_ptr().add(self.written).cast::<u8>(),
                s.len(),
            );
        }
        self.written += s.len();
        Ok(())
    }
}

fn render(msg: &'static str, lhs: &dyn Debug, rhs: &dyn Debug) -> Error {
    let mut lhs_buf = Buf::new();
    if fmt::write(&mut lhs_buf, format_args!("{:?}", lhs)).is_ok() {
        let mut rhs_buf = Buf::new();
        if fmt::write(&mut rhs_buf, format_args!("{:?}", rhs)).is_ok() {
            let lhs_str = lhs_buf.as_str();
            let rhs_str = rhs_buf.as_str();
            // "{msg} ({lhs} vs {rhs})"
            let len = msg.len() + 2 + lhs_str.len() + 4 + rhs_str.len() + 1;
            let mut string = String::with_capacity(len);
            string.push_str(msg);
            string.push_str(" (");
            string.push_str(lhs_str);
            string.push_str(" vs ");
            string.push_str(rhs_str);
            string.push(')');
            return Error::msg(string);
        }
    }
    Error::msg(msg)
}

#[doc(hidden)]
#[macro_export]
macro_rules! __parse_ensure {
    (atom () $bail:tt $fuel:tt {($($rhs:tt)+) ($($lhs:tt)+) $op:tt} $dup:tt $(,)?) => {
        $crate::__fancy_ensure!($($lhs)+, $op, $($rhs)+)
    };

    // low precedence control flow constructs

    (0 $stack:tt ($($bail:tt)*) $fuel:tt $parse:tt $dup:tt return $($rest:tt)*) => {
        $crate::__fallback_ensure!($($bail)*)
    };

    (0 $stack:tt ($($bail:tt)*) $fuel:tt $parse:tt $dup:tt break $($rest:tt)*) => {
        $crate::__fallback_ensure!($($bail)*)
    };

    (0 $stack:tt ($($bail:tt)*) $fuel:tt $parse:tt $dup:tt continue $($rest:tt)*) => {
        $crate::__fallback_ensure!($($bail)*)
    };

    (0 $stack:tt ($($bail:tt)*) $fuel:tt $parse:tt $dup:tt yield $($rest:tt)*) => {
        $crate::__fallback_ensure!($($bail)*)
    };

    (0 $stack:tt ($($bail:tt)*) $fuel:tt $parse:tt $dup:tt move $($rest:tt)*) => {
        $crate::__fallback_ensure!($($bail)*)
    };

    // unary operators

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($deref:tt $($dup:tt)*) * $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $deref) $($parse)*} ($($rest)*) $($rest)*)
    };

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($not:tt $($dup:tt)*) ! $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $not) $($parse)*} ($($rest)*) $($rest)*)
    };

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($neg:tt $($dup:tt)*) - $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $neg) $($parse)*} ($($rest)*) $($rest)*)
    };

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($let:tt $($dup:tt)*) let $($rest:tt)*) => {
        $crate::__parse_ensure!(pat $stack $bail ($($fuel)*) {($($buf)* $let) $($parse)*} ($($rest)*) $($rest)*)
    };

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($lifetime:tt $colon:tt $($dup:tt)*) $label:lifetime : $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $lifetime $colon) $($parse)*} ($($rest)*) $($rest)*)
    };

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($and:tt $mut:tt $($dup:tt)*) &mut $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $and $mut) $($parse)*} ($($rest)*) $($rest)*)
    };

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($and:tt $raw:tt $mut:tt $($dup:tt)*) &raw mut $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $and $raw $mut) $($parse)*} ($($rest)*) $($rest)*)
    };

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($and:tt $raw:tt $const:tt $($dup:tt)*) &raw const $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $and $raw $const) $($parse)*} ($($rest)*) $($rest)*)
    };

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($and:tt $($dup:tt)*) & $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $and) $($parse)*} ($($rest)*) $($rest)*)
    };

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($andand:tt $mut:tt $($dup:tt)*) &&mut $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $andand $mut) $($parse)*} ($($rest)*) $($rest)*)
    };

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($andand:tt $raw:tt $mut:tt $($dup:tt)*) &&raw mut $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $andand $raw $mut) $($parse)*} ($($rest)*) $($rest)*)
    };

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($andand:tt $raw:tt $const:tt $($dup:tt)*) &&raw const $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $andand $raw $const) $($parse)*} ($($rest)*) $($rest)*)
    };

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($andand:tt $($dup:tt)*) && $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $andand) $($parse)*} ($($rest)*) $($rest)*)
    };

    // control flow constructs

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($if:tt $($dup:tt)*) if $($rest:tt)*) => {
        $crate::__parse_ensure!(0 (cond $stack) $bail ($($fuel)*) {($($buf)* $if) $($parse)*} ($($rest)*) $($rest)*)
    };

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($match:tt $($dup:tt)*) match $($rest:tt)*) => {
        $crate::__parse_ensure!(0 (cond $stack) $bail ($($fuel)*) {($($buf)* $match) $($parse)*} ($($rest)*) $($rest)*)
    };

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($while:tt $($dup:tt)*) while $($rest:tt)*) => {
        $crate::__parse_ensure!(0 (cond $stack) $bail ($($fuel)*) {($($buf)* $while) $($parse)*} ($($rest)*) $($rest)*)
    };

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($for:tt $($dup:tt)*) for $($rest:tt)*) => {
        $crate::__parse_ensure!(pat (cond $stack) $bail ($($fuel)*) {($($buf)* $for) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom (cond $stack:tt) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($brace:tt $($dup:tt)*) {$($block:tt)*} $($rest:tt)*) => {
        $crate::__parse_ensure!(cond $stack $bail ($($fuel)*) {($($buf)* $brace) $($parse)*} ($($rest)*) $($rest)*)
    };

    (cond $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($else:tt $if:tt $($dup:tt)*) else if $($rest:tt)*) => {
        $crate::__parse_ensure!(0 (cond $stack) $bail ($($fuel)*) {($($buf)* $else $if) $($parse)*} ($($rest)*) $($rest)*)
    };

    (cond $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($else:tt $brace:tt $($dup:tt)*) else {$($block:tt)*} $($rest:tt)*) => {
        $crate::__parse_ensure!(atom $stack $bail ($($fuel)*) {($($buf)* $else $brace) $($parse)*} ($($rest)*) $($rest)*)
    };

    (cond $stack:tt $bail:tt (~$($fuel:tt)*) $parse:tt $dup:tt $($rest:tt)*) => {
        $crate::__parse_ensure!(atom $stack $bail ($($fuel)*) $parse $dup $($rest)*)
    };

    // atomic expressions

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($paren:tt $($dup:tt)*) ($($content:tt)*) $($rest:tt)*) => {
        $crate::__parse_ensure!(atom $stack $bail ($($fuel)*) {($($buf)* $paren) $($parse)*} ($($rest)*) $($rest)*)
    };

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($bracket:tt $($dup:tt)*) [$($array:tt)*] $($rest:tt)*) => {
        $crate::__parse_ensure!(atom $stack $bail ($($fuel)*) {($($buf)* $bracket) $($parse)*} ($($rest)*) $($rest)*)
    };

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($brace:tt $($dup:tt)*) {$($block:tt)*} $($rest:tt)*) => {
        $crate::__parse_ensure!(atom $stack $bail ($($fuel)*) {($($buf)* $brace) $($parse)*} ($($rest)*) $($rest)*)
    };

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($loop:tt $block:tt $($dup:tt)*) loop {$($body:tt)*} $($rest:tt)*) => {
        $crate::__parse_ensure!(atom $stack $bail ($($fuel)*) {($($buf)* $loop $block) $($parse)*} ($($rest)*) $($rest)*)
    };

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($async:tt $block:tt $($dup:tt)*) async {$($body:tt)*} $($rest:tt)*) => {
        $crate::__parse_ensure!(atom $stack $bail ($($fuel)*) {($($buf)* $async $block) $($parse)*} ($($rest)*) $($rest)*)
    };

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($async:tt $move:tt $block:tt $($dup:tt)*) async move {$($body:tt)*} $($rest:tt)*) => {
        $crate::__parse_ensure!(atom $stack $bail ($($fuel)*) {($($buf)* $async $move $block) $($parse)*} ($($rest)*) $($rest)*)
    };

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($unsafe:tt $block:tt $($dup:tt)*) unsafe {$($body:tt)*} $($rest:tt)*) => {
        $crate::__parse_ensure!(atom $stack $bail ($($fuel)*) {($($buf)* $unsafe $block) $($parse)*} ($($rest)*) $($rest)*)
    };

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($const:tt $block:tt $($dup:tt)*) const {$($body:tt)*} $($rest:tt)*) => {
        // TODO: this is mostly useless due to https://github.com/rust-lang/rust/issues/86730
        $crate::__parse_ensure!(atom $stack $bail ($($fuel)*) {($($buf)* $const $block) $($parse)*} ($($rest)*) $($rest)*)
    };

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($literal:tt $($dup:tt)*) $lit:literal $($rest:tt)*) => {
        $crate::__parse_ensure!(atom $stack $bail ($($fuel)*) {($($buf)* $literal) $($parse)*} ($($rest)*) $($rest)*)
    };

    // path expressions

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($colons:tt $ident:tt $($dup:tt)*) :: $i:ident $($rest:tt)*) => {
        $crate::__parse_ensure!(epath (atom $stack) $bail ($($fuel)*) {($($buf)* $colons $ident) $($parse)*} ($($rest)*) $($rest)*)
    };

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($ident:tt $($dup:tt)*) $i:ident $($rest:tt)*) => {
        $crate::__parse_ensure!(epath (atom $stack) $bail ($($fuel)*) {($($buf)* $ident) $($parse)*} ($($rest)*) $($rest)*)
    };

    (0 $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($langle:tt $($dup:tt)*) < $($rest:tt)*) => {
        $crate::__parse_ensure!(type (qpath (epath (atom $stack))) $bail ($($fuel)*) {($($buf)* $langle) $($parse)*} ($($rest)*) $($rest)*)
    };

    (epath $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($colons:tt $langle:tt $($dup:tt)*) :: < $($rest:tt)*) => {
        $crate::__parse_ensure!(generic (epath $stack) $bail ($($fuel)*) {($($buf)* $colons $langle) $($parse)*} ($($rest)*) $($rest)*)
    };

    (epath $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($colons:tt $langle:tt $($dup:tt)*) :: << $($rest:tt)*) => {
        $crate::__parse_ensure!(type (qpath (tpath (arglist (epath $stack)))) $bail ($($fuel)*) {($($buf)* $colons $langle) $($parse)*} ($($rest)*) $($rest)*)
    };

    (epath $stack:tt ($($bail:tt)*) (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} $dup:tt :: <- - $($rest:tt)*) => {
        $crate::__fallback_ensure!($($bail)*)
    };

    (epath $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($colons:tt $larrow:tt $($dup:tt)*) :: <- $lit:literal $($rest:tt)*) => {
        $crate::__parse_ensure!(generic (epath $stack) $bail ($($fuel)*) {($($buf)* $colons $larrow) $($parse)*} ($($dup)*) $($dup)*)
    };

    (epath $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($colons:tt $ident:tt $($dup:tt)*) :: $i:ident $($rest:tt)*) => {
        $crate::__parse_ensure!(epath $stack $bail ($($fuel)*) {($($buf)* $colons $ident) $($parse)*} ($($rest)*) $($rest)*)
    };

    (epath ($pop:ident $stack:tt) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($bang:tt $args:tt $($dup:tt)*) ! ($($mac:tt)*) $($rest:tt)*) => {
        $crate::__parse_ensure!($pop $stack $bail ($($fuel)*) {($($buf)* $bang $args) $($parse)*} ($($rest)*) $($rest)*)
    };

    (epath ($pop:ident $stack:tt) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($bang:tt $args:tt $($dup:tt)*) ! [$($mac:tt)*] $($rest:tt)*) => {
        $crate::__parse_ensure!($pop $stack $bail ($($fuel)*) {($($buf)* $bang $args) $($parse)*} ($($rest)*) $($rest)*)
    };

    (epath ($pop:ident $stack:tt) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($bang:tt $args:tt $($dup:tt)*) ! {$($mac:tt)*} $($rest:tt)*) => {
        $crate::__parse_ensure!($pop $stack $bail ($($fuel)*) {($($buf)* $bang $args) $($parse)*} ($($rest)*) $($rest)*)
    };

    (epath (split ($pop:ident $stack:tt)) $bail:tt (~$($fuel:tt)*) $parse:tt $dup:tt $($rest:tt)*) => {
        $crate::__parse_ensure!($pop (split $stack) $bail ($($fuel)*) $parse $dup $($rest)*)
    };

    (epath ($pop:ident $stack:tt) $bail:tt (~$($fuel:tt)*) $parse:tt $dup:tt $($rest:tt)*) => {
        $crate::__parse_ensure!($pop $stack $bail ($($fuel)*) $parse $dup $($rest)*)
    };

    // trailer expressions

    (atom $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($paren:tt $($dup:tt)*) ($($call:tt)*) $($rest:tt)*) => {
        $crate::__parse_ensure!(atom $stack $bail ($($fuel)*) {($($buf)* $paren) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($bracket:tt $($dup:tt)*) [$($index:tt)*] $($rest:tt)*) => {
        $crate::__parse_ensure!(atom $stack $bail ($($fuel)*) {($($buf)* $bracket) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($brace:tt $($dup:tt)*) {$($init:tt)*} $($rest:tt)*) => {
        $crate::__parse_ensure!(atom $stack $bail ($($fuel)*) {($($buf)* $brace) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($question:tt $($dup:tt)*) ? $($rest:tt)*) => {
        $crate::__parse_ensure!(atom $stack $bail ($($fuel)*) {($($buf)* $question) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($dot:tt $ident:tt $colons:tt $langle:tt $($dup:tt)*) . $i:ident :: < $($rest:tt)*) => {
        $crate::__parse_ensure!(generic (atom $stack) $bail ($($fuel)*) {($($buf)* $dot $ident $colons $langle) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($dot:tt $ident:tt $colons:tt $langle:tt $($dup:tt)*) . $i:ident :: << $($rest:tt)*) => {
        $crate::__parse_ensure!(type (qpath (tpath (arglist (atom $stack)))) $bail ($($fuel)*) {($($buf)* $dot $ident $colons $langle) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom $stack:tt ($($bail:tt)*) (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} $dup:tt . $i:ident :: <- - $($rest:tt)*) => {
        $crate::__fallback_ensure!($($bail)*)
    };

    (atom $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($dot:tt $ident:tt $colons:tt $larrow:tt $($dup:tt)*) . $i:ident :: <- $lit:literal $($rest:tt)*) => {
        $crate::__parse_ensure!(generic (atom $stack) $bail ($($fuel)*) {($($buf)* $dot $ident $colons $larrow) $($parse)*} ($($dup)*) $($dup)*)
    };

    (atom $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($dot:tt $field:tt $($dup:tt)*) . $i:ident $($rest:tt)*) => {
        $crate::__parse_ensure!(atom $stack $bail ($($fuel)*) {($($buf)* $dot $field) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom $stack:tt ($($bail:tt)*) (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} $dup:tt . - $($rest:tt)*) => {
        $crate::__fallback_ensure!($($bail)*)
    };

    (atom $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($dot:tt $index:tt $($dup:tt)*) . $lit:literal $($rest:tt)*) => {
        $crate::__parse_ensure!(atom $stack $bail ($($fuel)*) {($($buf)* $dot $index) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($as:tt $($dup:tt)*) as $($rest:tt)*) => {
        $crate::__parse_ensure!(type (atom $stack) $bail ($($fuel)*) {($($buf)* $as) $($parse)*} ($($rest)*) $($rest)*)
    };

    // types

    (type ($pop:ident $stack:tt) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($bracket:tt $($dup:tt)*) [$($content:tt)*] $($rest:tt)*) => {
        $crate::__parse_ensure!($pop $stack $bail ($($fuel)*) {($($buf)* $bracket) $($parse)*} ($($rest)*) $($rest)*)
    };

    (type ($pop:ident $stack:tt) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($paren:tt $($dup:tt)*) ($($content:tt)*) $($rest:tt)*) => {
        $crate::__parse_ensure!($pop $stack $bail ($($fuel)*) {($($buf)* $paren) $($parse)*} ($($rest)*) $($rest)*)
    };

    (type $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($star:tt $const:tt $($dup:tt)*) *const $($rest:tt)*) => {
        $crate::__parse_ensure!(type $stack $bail ($($fuel)*) {($($buf)* $star $const) $($parse)*} ($($rest)*) $($rest)*)
    };

    (type $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($star:tt $mut:tt $($dup:tt)*) *mut $($rest:tt)*) => {
        $crate::__parse_ensure!(type $stack $bail ($($fuel)*) {($($buf)* $star $mut) $($parse)*} ($($rest)*) $($rest)*)
    };

    (type $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($and:tt $lifetime:tt $mut:tt $($dup:tt)*) & $l:lifetime mut $($rest:tt)*) => {
        $crate::__parse_ensure!(type $stack $bail ($($fuel)*) {($($buf)* $and $lifetime $mut) $($parse)*} ($($rest)*) $($rest)*)
    };

    (type $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($and:tt $mut:tt $($dup:tt)*) & mut $($rest:tt)*) => {
        $crate::__parse_ensure!(type $stack $bail ($($fuel)*) {($($buf)* $and $mut) $($parse)*} ($($rest)*) $($rest)*)
    };

    (type $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($and:tt $lifetime:tt $($dup:tt)*) & $l:lifetime $($rest:tt)*) => {
        $crate::__parse_ensure!(type $stack $bail ($($fuel)*) {($($buf)* $and $lifetime) $($parse)*} ($($rest)*) $($rest)*)
    };

    (type $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($and:tt $($dup:tt)*) & $($rest:tt)*) => {
        $crate::__parse_ensure!(type $stack $bail ($($fuel)*) {($($buf)* $and) $($parse)*} ($($rest)*) $($rest)*)
    };

    (type $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($and:tt $lifetime:tt $mut:tt $($dup:tt)*) && $l:lifetime mut $($rest:tt)*) => {
        $crate::__parse_ensure!(type $stack $bail ($($fuel)*) {($($buf)* $and $lifetime $mut) $($parse)*} ($($rest)*) $($rest)*)
    };

    (type $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($and:tt $mut:tt $($dup:tt)*) && mut $($rest:tt)*) => {
        $crate::__parse_ensure!(type $stack $bail ($($fuel)*) {($($buf)* $and $mut) $($parse)*} ($($rest)*) $($rest)*)
    };

    (type $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($and:tt $lifetime:tt $($dup:tt)*) && $l:lifetime $($rest:tt)*) => {
        $crate::__parse_ensure!(type $stack $bail ($($fuel)*) {($($buf)* $and $lifetime) $($parse)*} ($($rest)*) $($rest)*)
    };

    (type $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($and:tt $($dup:tt)*) && $($rest:tt)*) => {
        $crate::__parse_ensure!(type $stack $bail ($($fuel)*) {($($buf)* $and) $($parse)*} ($($rest)*) $($rest)*)
    };

    (type $stack:tt ($($bail:tt)*) (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} $dup:tt unsafe extern - $($rest:tt)*) => {
        $crate::__fallback_ensure!($($bail)*)
    };

    (type $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($unsafe:tt $(extern $($abi:literal)?)? fn $($dup:tt)*) unsafe $($rest:tt)*) => {
        $crate::__parse_ensure!(type $stack $bail ($($fuel)*) {($($buf)* $unsafe) $($parse)*} ($($rest)*) $($rest)*)
    };

    (type $stack:tt ($($bail:tt)*) (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} $dup:tt extern - $($rest:tt)*) => {
        $crate::__fallback_ensure!($($bail)*)
    };

    (type $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($extern:tt $abi:tt fn $($dup:tt)*) extern $lit:literal $($rest:tt)*) => {
        $crate::__parse_ensure!(type $stack $bail ($($fuel)*) {($($buf)* $extern $abi) $($parse)*} ($($rest)*) $($rest)*)
    };

    (type $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($extern:tt fn $($dup:tt)*) extern $($rest:tt)*) => {
        $crate::__parse_ensure!(type $stack $bail ($($fuel)*) {($($buf)* $extern) $($parse)*} ($($rest)*) $($rest)*)
    };

    (type $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($fn:tt $paren:tt $arrow:tt $($dup:tt)*) fn ($($args:tt)*) -> $($rest:tt)*) => {
        $crate::__parse_ensure!(type $stack $bail ($($fuel)*) {($($buf)* $fn $paren $arrow) $($parse)*} ($($rest)*) $($rest)*)
    };

    (type ($pop:ident $stack:tt) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($fn:tt $paren:tt $($dup:tt)*) fn ($($args:tt)*) $($rest:tt)*) => {
        $crate::__parse_ensure!($pop $stack $bail ($($fuel)*) {($($buf)* $fn $paren) $($parse)*} ($($rest)*) $($rest)*)
    };

    (type $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($impl:tt $($dup:tt)*) impl $($rest:tt)*) => {
        $crate::__parse_ensure!(type $stack $bail ($($fuel)*) {($($buf)* $impl) $($parse)*} ($($rest)*) $($rest)*)
    };

    (type $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($dyn:tt $($dup:tt)*) dyn $($rest:tt)*) => {
        $crate::__parse_ensure!(type $stack $bail ($($fuel)*) {($($buf)* $dyn) $($parse)*} ($($rest)*) $($rest)*)
    };

    (type ($pop:ident $stack:tt) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($wild:tt $($dup:tt)*) _ $($rest:tt)*) => {
        $crate::__parse_ensure!($pop $stack $bail ($($fuel)*) {($($buf)* $wild) $($parse)*} ($($rest)*) $($rest)*)
    };

    (type ($pop:ident $stack:tt) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($never:tt $($dup:tt)*) ! $($rest:tt)*) => {
        $crate::__parse_ensure!($pop $stack $bail ($($fuel)*) {($($buf)* $never) $($parse)*} ($($rest)*) $($rest)*)
    };

    (type $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($for:tt $langle:tt $($dup:tt)*) for < $($rest:tt)*) => {
        $crate::__parse_ensure!(generic (type $stack) $bail ($($fuel)*) {($($buf)* $for $langle) $($parse)*} ($($rest)*) $($rest)*)
    };

    // path types

    (type $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($colons:tt $ident:tt $($dup:tt)*) :: $i:ident $($rest:tt)*) => {
        $crate::__parse_ensure!(tpath $stack $bail ($($fuel)*) {($($buf)* $colons $ident) $($parse)*} ($($rest)*) $($rest)*)
    };

    (type $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($ident:tt $($dup:tt)*) $i:ident $($rest:tt)*) => {
        $crate::__parse_ensure!(tpath $stack $bail ($($fuel)*) {($($buf)* $ident) $($parse)*} ($($rest)*) $($rest)*)
    };

    (type $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($langle:tt $($dup:tt)*) < $($rest:tt)*) => {
        $crate::__parse_ensure!(type (qpath (tpath $stack)) $bail ($($fuel)*) {($($buf)* $langle) $($parse)*} ($($rest)*) $($rest)*)
    };

    (tpath $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($langle:tt $($dup:tt)*) < $($rest:tt)*) => {
        $crate::__parse_ensure!(generic (tpath $stack) $bail ($($fuel)*) {($($buf)* $langle) $($parse)*} ($($rest)*) $($rest)*)
    };

    (tpath $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($langle:tt $($dup:tt)*) << $($rest:tt)*) => {
        $crate::__parse_ensure!(type (qpath (tpath (arglist (tpath $stack)))) $bail ($($fuel)*) {($($buf)* $langle) $($parse)*} ($($rest)*) $($rest)*)
    };

    (tpath $stack:tt ($($bail:tt)*) (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} $dup:tt <- - $($rest:tt)*) => {
        $crate::__fallback_ensure!($($bail)*)
    };

    (tpath $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($larrow:tt $($dup:tt)*) <- $lit:literal $($rest:tt)*) => {
        $crate::__parse_ensure!(generic (tpath $stack) $bail ($($fuel)*) {($($buf)* $larrow) $($parse)*} ($($dup)*) $($dup)*)
    };

    (tpath $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($colons:tt $langle:tt $($dup:tt)*) :: < $($rest:tt)*) => {
        $crate::__parse_ensure!(generic (tpath $stack) $bail ($($fuel)*) {($($buf)* $colons $langle) $($parse)*} ($($rest)*) $($rest)*)
    };

    (tpath $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($colons:tt $langle:tt $($dup:tt)*) :: << $($rest:tt)*) => {
        $crate::__parse_ensure!(type (qpath (tpath (arglist (tpath $stack)))) $bail ($($fuel)*) {($($buf)* $colons $langle) $($parse)*} ($($rest)*) $($rest)*)
    };

    (tpath $stack:tt ($($bail:tt)*) (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} $dup:tt :: <- - $($rest:tt)*) => {
        $crate::__fallback_ensure!($($bail)*)
    };

    (tpath $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($colons:tt $larrow:tt $($dup:tt)*) :: <- $lit:literal $($rest:tt)*) => {
        $crate::__parse_ensure!(generic (tpath $stack) $bail ($($fuel)*) {($($buf)* $colons $larrow) $($parse)*} ($($dup)*) $($dup)*)
    };

    (tpath $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($colons:tt $ident:tt $($dup:tt)*) :: $i:ident $($rest:tt)*) => {
        $crate::__parse_ensure!(tpath $stack $bail ($($fuel)*) {($($buf)* $colons $ident) $($parse)*} ($($rest)*) $($rest)*)
    };

    (tpath $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($paren:tt $arrow:tt $($dup:tt)*) ($($args:tt)*) -> $($rest:tt)*) => {
        $crate::__parse_ensure!(type $stack $bail ($($fuel)*) {($($buf)* $paren $arrow) $($parse)*} ($($rest)*) $($rest)*)
    };

    (tpath $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($paren:tt $($dup:tt)*) ($($args:tt)*) $($rest:tt)*) => {
        $crate::__parse_ensure!(object $stack $bail ($($fuel)*) {($($buf)* $paren) $($parse)*} ($($rest)*) $($rest)*)
    };

    (tpath $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($colons:tt $paren:tt $arrow:tt $($dup:tt)*) :: ($($args:tt)*) -> $($rest:tt)*) => {
        $crate::__parse_ensure!(type $stack $bail ($($fuel)*) {($($buf)* $colons $paren $arrow) $($parse)*} ($($rest)*) $($rest)*)
    };

    (tpath $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($colons:tt $paren:tt $($dup:tt)*) :: ($($args:tt)*) $($rest:tt)*) => {
        $crate::__parse_ensure!(object $stack $bail ($($fuel)*) {($($buf)* $colons $paren) $($parse)*} ($($rest)*) $($rest)*)
    };

    (tpath ($pop:ident $stack:tt) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($bang:tt $args:tt $($dup:tt)*) ! ($($mac:tt)*) $($rest:tt)*) => {
        $crate::__parse_ensure!($pop $stack $bail ($($fuel)*) {($($buf)* $bang $args) $($parse)*} ($($rest)*) $($rest)*)
    };

    (tpath ($pop:ident $stack:tt) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($bang:tt $args:tt $($dup:tt)*) ! [$($mac:tt)*] $($rest:tt)*) => {
        $crate::__parse_ensure!($pop $stack $bail ($($fuel)*) {($($buf)* $bang $args) $($parse)*} ($($rest)*) $($rest)*)
    };

    (tpath ($pop:ident $stack:tt) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($bang:tt $args:tt $($dup:tt)*) ! {$($mac:tt)*} $($rest:tt)*) => {
        $crate::__parse_ensure!($pop $stack $bail ($($fuel)*) {($($buf)* $bang $args) $($parse)*} ($($rest)*) $($rest)*)
    };

    (tpath $stack:tt $bail:tt (~$($fuel:tt)*) $parse:tt $dup:tt $($rest:tt)*) => {
        $crate::__parse_ensure!(object $stack $bail ($($fuel)*) $parse $dup $($rest)*)
    };

    // qualified paths

    (qpath (split ($pop:ident $stack:tt)) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($rangle:tt $colons:tt $ident:tt $($dup:tt)*) >> :: $i:ident $($rest:tt)*) => {
        $crate::__parse_ensure!($pop $stack $bail ($($fuel)*) {($($buf)* $rangle $colons $ident) $($parse)*} ($($rest)*) $($rest)*)
    };

    (qpath ($pop:ident $stack:tt) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($rangle:tt $colons:tt $ident:tt $($dup:tt)*) > :: $i:ident $($rest:tt)*) => {
        $crate::__parse_ensure!($pop $stack $bail ($($fuel)*) {($($buf)* $rangle $colons $ident) $($parse)*} ($($rest)*) $($rest)*)
    };

    (qpath $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($as:tt $($dup:tt)*) as $($rest:tt)*) => {
        $crate::__parse_ensure!(type (qpath $stack) $bail ($($fuel)*) {($($buf)* $as) $($parse)*} ($($rest)*) $($rest)*)
    };

    // trait objects

    (object (arglist $stack:tt) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($plus:tt $colons:tt $ident:tt $($dup:tt)*) + :: $i:ident $($rest:tt)*) => {
        $crate::__parse_ensure!(tpath (arglist $stack) $bail ($($fuel)*) {($($buf)* $plus $colons $ident) $($parse)*} ($($rest)*) $($rest)*)
    };

    (object (arglist $stack:tt) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($plus:tt $ident:tt $($dup:tt)*) + $i:ident $($rest:tt)*) => {
        $crate::__parse_ensure!(tpath (arglist $stack) $bail ($($fuel)*) {($($buf)* $plus $ident) $($parse)*} ($($rest)*) $($rest)*)
    };

    (object (split ($pop:ident $stack:tt)) $bail:tt (~$($fuel:tt)*) $parse:tt $dup:tt $($rest:tt)*) => {
        $crate::__parse_ensure!($pop (split $stack) $bail ($($fuel)*) $parse $dup $($rest)*)
    };

    (object ($pop:ident $stack:tt) $bail:tt (~$($fuel:tt)*) $parse:tt $dup:tt $($rest:tt)*) => {
        $crate::__parse_ensure!($pop $stack $bail ($($fuel)*) $parse $dup $($rest)*)
    };

    // angle bracketed generic arguments

    (generic (split ($pop:ident $stack:tt)) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($rangle:tt $($dup:tt)*) >> $($rest:tt)*) => {
        $crate::__parse_ensure!($pop $stack $bail ($($fuel)*) {($($buf)* $rangle) $($parse)*} ($($rest)*) $($rest)*)
    };

    (generic ($pop:ident $stack:tt) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($rangle:tt $($dup:tt)*) > $($rest:tt)*) => {
        $crate::__parse_ensure!($pop $stack $bail ($($fuel)*) {($($buf)* $rangle) $($parse)*} ($($rest)*) $($rest)*)
    };

    (generic ($pop:ident $stack:tt) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($rangle:tt $($dup:tt)*) >> $($rest:tt)*) => {
        $crate::__parse_ensure!($pop (split $stack) $bail ($($fuel)*) {($($buf)*) $($parse)*} ($rangle $($rest)*) $rangle $($rest)*)
    };

    (generic $stack:tt ($($bail:tt)*) (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} $dup:tt - - $($rest:tt)*) => {
        $crate::__fallback_ensure!($($bail)*)
    };

    (generic $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($neg:tt $($dup:tt)*) - $lit:literal $($rest:tt)*) => {
        $crate::__parse_ensure!(generic $stack $bail ($($fuel)*) {($($buf)* $neg) $($parse)*} ($($dup)*) $($dup)*)
    };

    (generic $stack:tt ($($bail:tt)*) (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} $dup:tt - $($rest:tt)*) => {
        $crate::__fallback_ensure!($($bail)*)
    };

    (generic $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($literal:tt $($dup:tt)*) $lit:literal $($rest:tt)*) => {
        $crate::__parse_ensure!(arglist $stack $bail ($($fuel)*) {($($buf)* $literal) $($parse)*} ($($rest)*) $($rest)*)
    };

    (generic $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($brace:tt $($dup:tt)*) {$($block:tt)*} $($rest:tt)*) => {
        $crate::__parse_ensure!(arglist $stack $bail ($($fuel)*) {($($buf)* $brace) $($parse)*} ($($rest)*) $($rest)*)
    };

    (generic $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($lifetime:tt $($dup:tt)*) $l:lifetime $($rest:tt)*) => {
        $crate::__parse_ensure!(arglist $stack $bail ($($fuel)*) {($($buf)* $lifetime) $($parse)*} ($($rest)*) $($rest)*)
    };

    (generic $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($assoc:tt $eq:tt $($dup:tt)*) $ident:ident = $($rest:tt)*) => {
        $crate::__parse_ensure!(type (arglist $stack) $bail ($($fuel)*) {($($buf)* $assoc $eq) $($parse)*} ($($rest)*) $($rest)*)
    };

    (generic $stack:tt $bail:tt (~$($fuel:tt)*) $parse:tt $dup:tt $($rest:tt)*) => {
        $crate::__parse_ensure!(type (arglist $stack) $bail ($($fuel)*) $parse $dup $($rest)*)
    };

    (arglist $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($comma:tt $($dup:tt)*) , $($rest:tt)*) => {
        $crate::__parse_ensure!(generic $stack $bail ($($fuel)*) {($($buf)* $comma) $($parse)*} ($($rest)*) $($rest)*)
    };

    (arglist (split ($pop:ident $stack:tt)) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($rangle:tt $($dup:tt)*) >> $($rest:tt)*) => {
        $crate::__parse_ensure!($pop $stack $bail ($($fuel)*) {($($buf)*) $rangle $($parse)*} ($($rest)*) $($rest)*)
    };

    (arglist ($pop:ident $stack:tt) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($rangle:tt $($dup:tt)*) > $($rest:tt)*) => {
        $crate::__parse_ensure!($pop $stack $bail ($($fuel)*) {($($buf)* $rangle) $($parse)*} ($($rest)*) $($rest)*)
    };

    (arglist ($pop:ident $stack:tt) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($rangle:tt $($dup:tt)*) >> $($rest:tt)*) => {
        $crate::__parse_ensure!($pop (split $stack) $bail ($($fuel)*) {($($buf)*) $($parse)*} ($rangle $($rest)*) $rangle $($rest)*)
    };

    // patterns

    (pat $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($pipe:tt $($dup:tt)*) | $($rest:tt)*) => {
        $crate::__parse_ensure!(pat $stack $bail ($($fuel)*) {($($buf)* $pipe) $($parse)*} ($($rest)*) $($rest)*)
    };

    (pat $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($eq:tt $($dup:tt)*) = $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $eq) $($parse)*} ($($rest)*) $($rest)*)
    };

    (pat $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($in:tt $($dup:tt)*) in $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $in) $($parse)*} ($($rest)*) $($rest)*)
    };

    (pat $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($ref:tt $($dup:tt)*) ref $($rest:tt)*) => {
        $crate::__parse_ensure!(pat $stack $bail ($($fuel)*) {($($buf)* $ref) $($parse)*} ($($rest)*) $($rest)*)
    };

    (pat $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($mut:tt $($dup:tt)*) mut $($rest:tt)*) => {
        $crate::__parse_ensure!(pat $stack $bail ($($fuel)*) {($($buf)* $mut) $($parse)*} ($($rest)*) $($rest)*)
    };

    (pat $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($at:tt $($dup:tt)*) @ $($rest:tt)*) => {
        $crate::__parse_ensure!(pat $stack $bail ($($fuel)*) {($($buf)* $at) $($parse)*} ($($rest)*) $($rest)*)
    };

    (pat $stack:tt ($($bail:tt)*) (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} $dup:tt - - $($rest:tt)*) => {
        $crate::__fallback_ensure!($($bail)*)
    };

    (pat $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($neg:tt $($dup:tt)*) - $lit:literal $($rest:tt)*) => {
        $crate::__parse_ensure!(pat $stack $bail ($($fuel)*) {($($buf)* $neg) $($parse)*} ($($dup)*) $($dup)*)
    };

    (pat $stack:tt ($($bail:tt)*) (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} $dup:tt - $($rest:tt)*) => {
        $crate::__fallback_ensure!($($bail)*)
    };

    (pat $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($literal:tt $($dup:tt)*) $lit:literal $($rest:tt)*) => {
        $crate::__parse_ensure!(pat $stack $bail ($($fuel)*) {($($buf)* $literal) $($parse)*} ($($rest)*) $($rest)*)
    };

    (pat $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($range:tt $($dup:tt)*) .. $($rest:tt)*) => {
        $crate::__parse_ensure!(pat $stack $bail ($($fuel)*) {($($buf)* $range) $($parse)*} ($($rest)*) $($rest)*)
    };

    (pat $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($range:tt $($dup:tt)*) ..= $($rest:tt)*) => {
        $crate::__parse_ensure!(pat $stack $bail ($($fuel)*) {($($buf)* $range) $($parse)*} ($($rest)*) $($rest)*)
    };

    (pat $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($and:tt $($dup:tt)*) & $($rest:tt)*) => {
        $crate::__parse_ensure!(pat $stack $bail ($($fuel)*) {($($buf)* $and) $($parse)*} ($($rest)*) $($rest)*)
    };

    (pat $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($andand:tt $($dup:tt)*) && $($rest:tt)*) => {
        $crate::__parse_ensure!(pat $stack $bail ($($fuel)*) {($($buf)* $andand) $($parse)*} ($($rest)*) $($rest)*)
    };

    (pat $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($paren:tt $($dup:tt)*) ($($content:tt)*) $($rest:tt)*) => {
        $crate::__parse_ensure!(pat $stack $bail ($($fuel)*) {($($buf)* $paren) $($parse)*} ($($rest)*) $($rest)*)
    };

    (pat $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($bracket:tt $($dup:tt)*) [$($content:tt)*] $($rest:tt)*) => {
        $crate::__parse_ensure!(pat $stack $bail ($($fuel)*) {($($buf)* $bracket) $($parse)*} ($($rest)*) $($rest)*)
    };

    (pat $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($brace:tt $($dup:tt)*) {$($content:tt)*} $($rest:tt)*) => {
        $crate::__parse_ensure!(pat $stack $bail ($($fuel)*) {($($buf)* $brace) $($parse)*} ($($rest)*) $($rest)*)
    };

    (pat $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($wild:tt $($dup:tt)*) _ $($rest:tt)*) => {
        $crate::__parse_ensure!(pat $stack $bail ($($fuel)*) {($($buf)* $wild) $($parse)*} ($($rest)*) $($rest)*)
    };

    (pat $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($colons:tt $ident:tt $($dup:tt)*) :: $i:ident $($rest:tt)*) => {
        $crate::__parse_ensure!(epath (pat $stack) $bail ($($fuel)*) {($($buf)* $colons $ident) $($parse)*} ($($rest)*) $($rest)*)
    };

    (pat $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($ident:tt $($dup:tt)*) $i:ident $($rest:tt)*) => {
        $crate::__parse_ensure!(epath (pat $stack) $bail ($($fuel)*) {($($buf)* $ident) $($parse)*} ($($rest)*) $($rest)*)
    };

    (pat $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($langle:tt $($dup:tt)*) < $($rest:tt)*) => {
        $crate::__parse_ensure!(type (qpath (epath (pat $stack))) $bail ($($fuel)*) {($($buf)* $langle) $($parse)*} ($($rest)*) $($rest)*)
    };

    // comparison binary operators

    (atom () $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($eq:tt $($dup:tt)*) == $($rest:tt)*) => {
        $crate::__parse_ensure!(0 () $bail ($($fuel)*) {() $($parse)* ($($buf)*) $eq} ($($rest)*) $($rest)*)
    };

    (atom $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)+) $($parse:tt)*} ($eq:tt $($dup:tt)*) == $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $eq) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom () $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($le:tt $($dup:tt)*) <= $($rest:tt)*) => {
        $crate::__parse_ensure!(0 () $bail ($($fuel)*) {() $($parse)* ($($buf)*) $le} ($($rest)*) $($rest)*)
    };

    (atom $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)+) $($parse:tt)*} ($le:tt $($dup:tt)*) <= $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $le) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom () $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($lt:tt $($dup:tt)*) < $($rest:tt)*) => {
        $crate::__parse_ensure!(0 () $bail ($($fuel)*) {() $($parse)* ($($buf)*) $lt} ($($rest)*) $($rest)*)
    };

    (atom $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)+) $($parse:tt)*} ($lt:tt $($dup:tt)*) < $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $lt) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom () $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($ne:tt $($dup:tt)*) != $($rest:tt)*) => {
        $crate::__parse_ensure!(0 () $bail ($($fuel)*) {() $($parse)* ($($buf)*) $ne} ($($rest)*) $($rest)*)
    };

    (atom $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)+) $($parse:tt)*} ($ne:tt $($dup:tt)*) != $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $ne) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom () $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($ge:tt $($dup:tt)*) >= $($rest:tt)*) => {
        $crate::__parse_ensure!(0 () $bail ($($fuel)*) {() $($parse)* ($($buf)*) $ge} ($($rest)*) $($rest)*)
    };

    (atom $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)+) $($parse:tt)*} ($ge:tt $($dup:tt)*) >= $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $ge) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom (split ()) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} $dup:tt >> $($rest:tt)*) => {
        $crate::__parse_ensure!(0 () $bail ($($fuel)*) {() $($parse)* ($($buf)* > ) > } ($($rest)*) $($rest)*)
    };

    (atom () $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($gt:tt $($dup:tt)*) > $($rest:tt)*) => {
        $crate::__parse_ensure!(0 () $bail ($($fuel)*) {() $($parse)* ($($buf)*) $gt} ($($rest)*) $($rest)*)
    };

    (atom (split $stack:tt) $bail:tt (~$($fuel:tt)*) {($($buf:tt)+) $($parse:tt)*} ($rangle:tt $($dup:tt)*) >> $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $rangle) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)+) $($parse:tt)*} ($gt:tt $($dup:tt)*) > $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $gt) $($parse)*} ($($rest)*) $($rest)*)
    };

    // high precedence binary operators

    (atom $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($add:tt $($dup:tt)*) + $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $add) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($sub:tt $($dup:tt)*) - $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $sub) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($mul:tt $($dup:tt)*) * $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $mul) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($div:tt $($dup:tt)*) / $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $div) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($rem:tt $($dup:tt)*) % $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $rem) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($bitxor:tt $($dup:tt)*) ^ $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $bitxor) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($bitand:tt $($dup:tt)*) & $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $bitand) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($bitor:tt $($dup:tt)*) | $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $bitor) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($shl:tt $($dup:tt)*) << $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $shl) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom $stack:tt $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($shr:tt $($dup:tt)*) >> $($rest:tt)*) => {
        $crate::__parse_ensure!(0 $stack $bail ($($fuel)*) {($($buf)* $shr) $($parse)*} ($($rest)*) $($rest)*)
    };

    // low precedence binary operators

    (atom ($($stack:tt)+) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($and:tt $($dup:tt)*) && $($rest:tt)*) => {
        $crate::__parse_ensure!(0 ($($stack)*) $bail ($($fuel)*) {($($buf)* $and) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom ($($stack:tt)+) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($or:tt $($dup:tt)*) || $($rest:tt)*) => {
        $crate::__parse_ensure!(0 ($($stack)*) $bail ($($fuel)*) {($($buf)* $or) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom ($($stack:tt)+) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($assign:tt $($dup:tt)*) = $($rest:tt)*) => {
        $crate::__parse_ensure!(0 ($($stack)*) $bail ($($fuel)*) {($($buf)* $assign) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom ($($stack:tt)+) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($addeq:tt $($dup:tt)*) += $($rest:tt)*) => {
        $crate::__parse_ensure!(0 ($($stack)*) $bail ($($fuel)*) {($($buf)* $addeq) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom ($($stack:tt)+) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($subeq:tt $($dup:tt)*) -= $($rest:tt)*) => {
        $crate::__parse_ensure!(0 ($($stack)*) $bail ($($fuel)*) {($($buf)* $subeq) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom ($($stack:tt)+) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($muleq:tt $($dup:tt)*) *= $($rest:tt)*) => {
        $crate::__parse_ensure!(0 ($($stack)*) $bail ($($fuel)*) {($($buf)* $muleq) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom ($($stack:tt)+) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($diveq:tt $($dup:tt)*) /= $($rest:tt)*) => {
        $crate::__parse_ensure!(0 ($($stack)*) $bail ($($fuel)*) {($($buf)* $diveq) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom ($($stack:tt)+) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($remeq:tt $($dup:tt)*) %= $($rest:tt)*) => {
        $crate::__parse_ensure!(0 ($($stack)*) $bail ($($fuel)*) {($($buf)* $remeq) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom ($($stack:tt)+) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($bitxoreq:tt $($dup:tt)*) ^= $($rest:tt)*) => {
        $crate::__parse_ensure!(0 ($($stack)*) $bail ($($fuel)*) {($($buf)* $bitxoreq) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom ($($stack:tt)+) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($bitandeq:tt $($dup:tt)*) &= $($rest:tt)*) => {
        $crate::__parse_ensure!(0 ($($stack)*) $bail ($($fuel)*) {($($buf)* $bitandeq) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom ($($stack:tt)+) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($bitoreq:tt $($dup:tt)*) |= $($rest:tt)*) => {
        $crate::__parse_ensure!(0 ($($stack)*) $bail ($($fuel)*) {($($buf)* $bitoreq) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom ($($stack:tt)+) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($shleq:tt $($dup:tt)*) <<= $($rest:tt)*) => {
        $crate::__parse_ensure!(0 ($($stack)*) $bail ($($fuel)*) {($($buf)* $shleq) $($parse)*} ($($rest)*) $($rest)*)
    };

    (atom ($($stack:tt)+) $bail:tt (~$($fuel:tt)*) {($($buf:tt)*) $($parse:tt)*} ($shreq:tt $($dup:tt)*) >>= $($rest:tt)*) => {
        $crate::__parse_ensure!(0 ($($stack)*) $bail ($($fuel)*) {($($buf)* $shreq) $($parse)*} ($($rest)*) $($rest)*)
    };

    // unrecognized expression

    ($state:tt $stack:tt ($($bail:tt)*) $($rest:tt)*) => {
        $crate::__fallback_ensure!($($bail)*)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __fancy_ensure {
    ($lhs:expr, $op:tt, $rhs:expr) => {
        match (&$lhs, &$rhs) {
            (lhs, rhs) => {
                if !(lhs $op rhs) {
                    #[allow(unused_imports)]
                    use $crate::__private::{BothDebug, NotBothDebug};
                    return Err((lhs, rhs).__dispatch_ensure(
                        $crate::__private::concat!(
                            "Condition failed: `",
                            $crate::__private::stringify!($lhs),
                            " ",
                            $crate::__private::stringify!($op),
                            " ",
                            $crate::__private::stringify!($rhs),
                            "`",
                        ),
                    ));
                }
            }
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __fallback_ensure {
    ($cond:expr $(,)?) => {
        if $crate::__private::not($cond) {
            return $crate::__private::Err($crate::Error::msg(
                $crate::__private::concat!("Condition failed: `", $crate::__private::stringify!($cond), "`")
            ));
        }
    };
    ($cond:expr, $msg:literal $(,)?) => {
        if $crate::__private::not($cond) {
            return $crate::__private::Err($crate::__anyhow!($msg));
        }
    };
    ($cond:expr, $err:expr $(,)?) => {
        if $crate::__private::not($cond) {
            return $crate::__private::Err($crate::__anyhow!($err));
        }
    };
    ($cond:expr, $fmt:expr, $($arg:tt)*) => {
        if $crate::__private::not($cond) {
            return $crate::__private::Err($crate::__anyhow!($fmt, $($arg)*));
        }
    };
}
