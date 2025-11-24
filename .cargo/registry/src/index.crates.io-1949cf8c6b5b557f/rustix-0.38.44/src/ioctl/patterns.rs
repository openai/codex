//! Implements typical patterns for `ioctl` usage.

use super::{Ioctl, IoctlOutput, Opcode, RawOpcode};

use crate::backend::c;
use crate::io::Result;

use core::marker::PhantomData;
use core::ptr::addr_of_mut;
use core::{fmt, mem};

/// Implements an `ioctl` with no real arguments.
pub struct NoArg<Opcode> {
    /// The opcode.
    _opcode: PhantomData<Opcode>,
}

impl<Opcode: CompileTimeOpcode> fmt::Debug for NoArg<Opcode> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("NoArg").field(&Opcode::OPCODE).finish()
    }
}

impl<Opcode: CompileTimeOpcode> NoArg<Opcode> {
    /// Create a new no-argument `ioctl` object.
    ///
    /// # Safety
    ///
    /// - `Opcode` must provide a valid opcode.
    #[inline]
    pub unsafe fn new() -> Self {
        Self {
            _opcode: PhantomData,
        }
    }
}

unsafe impl<Opcode: CompileTimeOpcode> Ioctl for NoArg<Opcode> {
    type Output = ();

    const IS_MUTATING: bool = false;
    const OPCODE: self::Opcode = Opcode::OPCODE;

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
/// pattern this structure implements it.
pub struct Getter<Opcode, Output> {
    /// The output data.
    output: mem::MaybeUninit<Output>,

    /// The opcode.
    _opcode: PhantomData<Opcode>,
}

impl<Opcode: CompileTimeOpcode, Output> fmt::Debug for Getter<Opcode, Output> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Getter").field(&Opcode::OPCODE).finish()
    }
}

impl<Opcode: CompileTimeOpcode, Output> Getter<Opcode, Output> {
    /// Create a new getter-style `ioctl` object.
    ///
    /// # Safety
    ///
    /// - `Opcode` must provide a valid opcode.
    /// - For this opcode, `Output` must be the type that the kernel expects to
    ///   write into.
    #[inline]
    pub unsafe fn new() -> Self {
        Self {
            output: mem::MaybeUninit::uninit(),
            _opcode: PhantomData,
        }
    }
}

unsafe impl<Opcode: CompileTimeOpcode, Output> Ioctl for Getter<Opcode, Output> {
    type Output = Output;

    const IS_MUTATING: bool = true;
    const OPCODE: self::Opcode = Opcode::OPCODE;

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
pub struct Setter<Opcode, Input> {
    /// The input data.
    input: Input,

    /// The opcode.
    _opcode: PhantomData<Opcode>,
}

impl<Opcode: CompileTimeOpcode, Input: fmt::Debug> fmt::Debug for Setter<Opcode, Input> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Setter")
            .field(&Opcode::OPCODE)
            .field(&self.input)
            .finish()
    }
}

impl<Opcode: CompileTimeOpcode, Input> Setter<Opcode, Input> {
    /// Create a new pointer setter-style `ioctl` object.
    ///
    /// # Safety
    ///
    /// - `Opcode` must provide a valid opcode.
    /// - For this opcode, `Input` must be the type that the kernel expects to
    ///   get.
    #[inline]
    pub unsafe fn new(input: Input) -> Self {
        Self {
            input,
            _opcode: PhantomData,
        }
    }
}

unsafe impl<Opcode: CompileTimeOpcode, Input> Ioctl for Setter<Opcode, Input> {
    type Output = ();

    const IS_MUTATING: bool = false;
    const OPCODE: self::Opcode = Opcode::OPCODE;

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
pub struct Updater<'a, Opcode, Value> {
    /// Reference to input/output data.
    value: &'a mut Value,

    /// The opcode.
    _opcode: PhantomData<Opcode>,
}

impl<'a, Opcode: CompileTimeOpcode, Value> Updater<'a, Opcode, Value> {
    /// Create a new pointer updater-style `ioctl` object.
    ///
    /// # Safety
    ///
    /// - `Opcode` must provide a valid opcode.
    /// - For this opcode, `Value` must be the type that the kernel expects to
    ///   get.
    #[inline]
    pub unsafe fn new(value: &'a mut Value) -> Self {
        Self {
            value,
            _opcode: PhantomData,
        }
    }
}

unsafe impl<'a, Opcode: CompileTimeOpcode, T> Ioctl for Updater<'a, Opcode, T> {
    type Output = ();

    const IS_MUTATING: bool = true;
    const OPCODE: self::Opcode = Opcode::OPCODE;

    fn as_ptr(&mut self) -> *mut c::c_void {
        (self.value as *mut T).cast()
    }

    unsafe fn output_from_ptr(_output: IoctlOutput, _ptr: *mut c::c_void) -> Result<()> {
        Ok(())
    }
}

/// Implements an `ioctl` that passes an integer into the `ioctl`.
pub struct IntegerSetter<Opcode> {
    /// The value to pass in.
    value: usize,

    /// The opcode.
    _opcode: PhantomData<Opcode>,
}

impl<Opcode: CompileTimeOpcode> IntegerSetter<Opcode> {
    /// Create a new integer `Ioctl` helper.
    ///
    /// # Safety
    ///
    /// - `Opcode` must provide a valid opcode.
    /// - For this opcode, it must expect an integer.
    /// - The integer is in the valid range for this opcode.
    #[inline]
    pub unsafe fn new(value: usize) -> Self {
        Self {
            value,
            _opcode: PhantomData,
        }
    }
}

unsafe impl<Opcode: CompileTimeOpcode> Ioctl for IntegerSetter<Opcode> {
    type Output = ();

    const IS_MUTATING: bool = false;
    const OPCODE: self::Opcode = Opcode::OPCODE;

    fn as_ptr(&mut self) -> *mut c::c_void {
        // TODO: strict provenance
        self.value as *mut c::c_void
    }

    unsafe fn output_from_ptr(
        _out: IoctlOutput,
        _extract_output: *mut c::c_void,
    ) -> Result<Self::Output> {
        Ok(())
    }
}

/// Trait for something that provides an `ioctl` opcode at compile time.
pub trait CompileTimeOpcode {
    /// The opcode.
    const OPCODE: Opcode;
}

/// Provides a bad opcode at compile time.
pub struct BadOpcode<const OPCODE: RawOpcode>;

impl<const OPCODE: RawOpcode> CompileTimeOpcode for BadOpcode<OPCODE> {
    const OPCODE: Opcode = Opcode::old(OPCODE);
}

/// Provides a read code at compile time.
///
/// This corresponds to the C macro `_IOR(GROUP, NUM, Data)`.
#[cfg(any(linux_kernel, bsd))]
pub struct ReadOpcode<const GROUP: u8, const NUM: u8, Data>(Data);

#[cfg(any(linux_kernel, bsd))]
impl<const GROUP: u8, const NUM: u8, Data> CompileTimeOpcode for ReadOpcode<GROUP, NUM, Data> {
    const OPCODE: Opcode = Opcode::read::<Data>(GROUP, NUM);
}

/// Provides a write code at compile time.
///
/// This corresponds to the C macro `_IOW(GROUP, NUM, Data)`.
#[cfg(any(linux_kernel, bsd))]
pub struct WriteOpcode<const GROUP: u8, const NUM: u8, Data>(Data);

#[cfg(any(linux_kernel, bsd))]
impl<const GROUP: u8, const NUM: u8, Data> CompileTimeOpcode for WriteOpcode<GROUP, NUM, Data> {
    const OPCODE: Opcode = Opcode::write::<Data>(GROUP, NUM);
}

/// Provides a read/write code at compile time.
///
/// This corresponds to the C macro `_IOWR(GROUP, NUM, Data)`.
#[cfg(any(linux_kernel, bsd))]
pub struct ReadWriteOpcode<const GROUP: u8, const NUM: u8, Data>(Data);

#[cfg(any(linux_kernel, bsd))]
impl<const GROUP: u8, const NUM: u8, Data> CompileTimeOpcode for ReadWriteOpcode<GROUP, NUM, Data> {
    const OPCODE: Opcode = Opcode::read_write::<Data>(GROUP, NUM);
}

/// Provides a `None` code at compile time.
///
/// This corresponds to the C macro `_IO(GROUP, NUM)` when `Data` is zero
/// sized.
#[cfg(any(linux_kernel, bsd))]
pub struct NoneOpcode<const GROUP: u8, const NUM: u8, Data>(Data);

#[cfg(any(linux_kernel, bsd))]
impl<const GROUP: u8, const NUM: u8, Data> CompileTimeOpcode for NoneOpcode<GROUP, NUM, Data> {
    const OPCODE: Opcode = Opcode::none::<Data>(GROUP, NUM);
}
