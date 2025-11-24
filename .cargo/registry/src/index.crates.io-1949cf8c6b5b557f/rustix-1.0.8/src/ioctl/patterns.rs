//! Implements typical patterns for `ioctl` usage.

use super::{Ioctl, IoctlOutput, Opcode};

use crate::backend::c;
use crate::io::Result;

use core::ptr::addr_of_mut;
use core::{fmt, mem};

/// Implements an `ioctl` with no real arguments.
///
/// To compute a value for the `OPCODE` argument, see the functions in the
/// [`opcode`] module.
///
/// [`opcode`]: crate::ioctl::opcode
pub struct NoArg<const OPCODE: Opcode> {}

impl<const OPCODE: Opcode> fmt::Debug for NoArg<OPCODE> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("NoArg").field(&OPCODE).finish()
    }
}

impl<const OPCODE: Opcode> NoArg<OPCODE> {
    /// Create a new no-argument `ioctl` object.
    ///
    /// # Safety
    ///
    ///  - `OPCODE` must provide a valid opcode.
    #[inline]
    pub const unsafe fn new() -> Self {
        Self {}
    }
}

unsafe impl<const OPCODE: Opcode> Ioctl for NoArg<OPCODE> {
    type Output = ();

    const IS_MUTATING: bool = false;

    fn opcode(&self) -> self::Opcode {
        OPCODE
    }

    fn as_ptr(&mut self) -> *mut c::c_void {
        core::ptr::null_mut()
    }

    unsafe fn output_from_ptr(_: IoctlOutput, _: *mut c::c_void) -> Result<Self::Output> {
        Ok(())
    }
}

/// Implements the traditional “getter” pattern for `ioctl`s.
///
/// Some `ioctl`s just read data into the userspace. As this is a popular
/// pattern, this structure implements it.
///
/// To compute a value for the `OPCODE` argument, see the functions in the
/// [`opcode`] module.
///
/// [`opcode`]: crate::ioctl::opcode
pub struct Getter<const OPCODE: Opcode, Output> {
    /// The output data.
    output: mem::MaybeUninit<Output>,
}

impl<const OPCODE: Opcode, Output> fmt::Debug for Getter<OPCODE, Output> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Getter").field(&OPCODE).finish()
    }
}

impl<const OPCODE: Opcode, Output> Getter<OPCODE, Output> {
    /// Create a new getter-style `ioctl` object.
    ///
    /// # Safety
    ///
    ///  - `OPCODE` must provide a valid opcode.
    ///  - For this opcode, `Output` must be the type that the kernel expects
    ///    to write into.
    #[inline]
    pub const unsafe fn new() -> Self {
        Self {
            output: mem::MaybeUninit::uninit(),
        }
    }
}

unsafe impl<const OPCODE: Opcode, Output> Ioctl for Getter<OPCODE, Output> {
    type Output = Output;

    const IS_MUTATING: bool = true;

    fn opcode(&self) -> self::Opcode {
        OPCODE
    }

    fn as_ptr(&mut self) -> *mut c::c_void {
        self.output.as_mut_ptr().cast()
    }

    unsafe fn output_from_ptr(_: IoctlOutput, ptr: *mut c::c_void) -> Result<Self::Output> {
        Ok(ptr.cast::<Output>().read())
    }
}

/// Implements the pattern for `ioctl`s where a pointer argument is given to
/// the `ioctl`.
///
/// The opcode must be read-only.
///
/// To compute a value for the `OPCODE` argument, see the functions in the
/// [`opcode`] module.
///
/// [`opcode`]: crate::ioctl::opcode
pub struct Setter<const OPCODE: Opcode, Input> {
    /// The input data.
    input: Input,
}

impl<const OPCODE: Opcode, Input: fmt::Debug> fmt::Debug for Setter<OPCODE, Input> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Setter")
            .field(&OPCODE)
            .field(&self.input)
            .finish()
    }
}

impl<const OPCODE: Opcode, Input> Setter<OPCODE, Input> {
    /// Create a new pointer setter-style `ioctl` object.
    ///
    /// # Safety
    ///
    ///  - `OPCODE` must provide a valid opcode.
    ///  - For this opcode, `Input` must be the type that the kernel expects to
    ///    get.
    #[inline]
    pub const unsafe fn new(input: Input) -> Self {
        Self { input }
    }
}

unsafe impl<const OPCODE: Opcode, Input> Ioctl for Setter<OPCODE, Input> {
    type Output = ();

    const IS_MUTATING: bool = false;

    fn opcode(&self) -> self::Opcode {
        OPCODE
    }

    fn as_ptr(&mut self) -> *mut c::c_void {
        addr_of_mut!(self.input).cast::<c::c_void>()
    }

    unsafe fn output_from_ptr(_: IoctlOutput, _: *mut c::c_void) -> Result<Self::Output> {
        Ok(())
    }
}

/// Implements an “updater” pattern for `ioctl`s.
///
/// The ioctl takes a reference to a struct that it reads its input from,
/// then writes output to the same struct.
///
/// To compute a value for the `OPCODE` argument, see the functions in the
/// [`opcode`] module.
///
/// [`opcode`]: crate::ioctl::opcode
pub struct Updater<'a, const OPCODE: Opcode, Value> {
    /// Reference to input/output data.
    value: &'a mut Value,
}

impl<'a, const OPCODE: Opcode, Value> Updater<'a, OPCODE, Value> {
    /// Create a new pointer updater-style `ioctl` object.
    ///
    /// # Safety
    ///
    ///  - `OPCODE` must provide a valid opcode.
    ///  - For this opcode, `Value` must be the type that the kernel expects to
    ///    get.
    #[inline]
    pub unsafe fn new(value: &'a mut Value) -> Self {
        Self { value }
    }
}

unsafe impl<'a, const OPCODE: Opcode, T> Ioctl for Updater<'a, OPCODE, T> {
    type Output = ();

    const IS_MUTATING: bool = true;

    fn opcode(&self) -> self::Opcode {
        OPCODE
    }

    fn as_ptr(&mut self) -> *mut c::c_void {
        (self.value as *mut T).cast()
    }

    unsafe fn output_from_ptr(_output: IoctlOutput, _ptr: *mut c::c_void) -> Result<()> {
        Ok(())
    }
}

/// Implements an `ioctl` that passes an integer into the `ioctl`.
///
/// To compute a value for the `OPCODE` argument, see the functions in the
/// [`opcode`] module.
///
/// [`opcode`]: crate::ioctl::opcode
pub struct IntegerSetter<const OPCODE: Opcode> {
    /// The value to pass in.
    ///
    /// For strict provenance preservation, this is a pointer.
    value: *mut c::c_void,
}

impl<const OPCODE: Opcode> IntegerSetter<OPCODE> {
    /// Create a new integer `Ioctl` helper containing a `usize`.
    ///
    /// # Safety
    ///
    ///  - `OPCODE` must provide a valid opcode.
    ///  - For this opcode, it must expect an integer.
    ///  - The integer is in the valid range for this opcode.
    #[inline]
    pub const unsafe fn new_usize(value: usize) -> Self {
        Self { value: value as _ }
    }

    /// Create a new integer `Ioctl` helper containing a `*mut c_void`.
    ///
    /// # Safety
    ///
    ///  - `OPCODE` must provide a valid opcode.
    ///  - For this opcode, it must expect an integer.
    ///  - The integer is in the valid range for this opcode.
    #[inline]
    pub const unsafe fn new_pointer(value: *mut c::c_void) -> Self {
        Self { value }
    }
}

unsafe impl<const OPCODE: Opcode> Ioctl for IntegerSetter<OPCODE> {
    type Output = ();

    const IS_MUTATING: bool = false;

    fn opcode(&self) -> self::Opcode {
        OPCODE
    }

    fn as_ptr(&mut self) -> *mut c::c_void {
        self.value
    }

    unsafe fn output_from_ptr(
        _out: IoctlOutput,
        _extract_output: *mut c::c_void,
    ) -> Result<Self::Output> {
        Ok(())
    }
}
