use alloc::sync::Arc;
use core::marker::PhantomData;
use core::ops::ControlFlow;

/// This struct contains the information needed to find split DWARF data
/// and to produce a `gimli::Dwarf<R>` for it.
pub struct SplitDwarfLoad<R> {
    /// The dwo id, for looking up in a DWARF package, or for
    /// verifying an unpacked dwo found on the file system
    pub dwo_id: gimli::DwoId,
    /// The compilation directory `path` is relative to.
    pub comp_dir: Option<R>,
    /// A path on the filesystem, relative to `comp_dir` to find this dwo.
    pub path: Option<R>,
    /// Once the split DWARF data is loaded, the loader is expected
    /// to call [make_dwo(parent)](gimli::read::Dwarf::make_dwo) before
    /// returning the data.
    pub parent: Arc<gimli::Dwarf<R>>,
}

/// Operations that consult debug information may require additional files
/// to be loaded if split DWARF is being used. This enum returns the result
/// of the operation in the `Output` variant, or information about the split
/// DWARF that is required and a continuation to invoke once it is available
/// in the `Load` variant.
///
/// This enum is intended to be used in a loop like so:
/// ```no_run
///   # use addr2line::*;
///   # use std::sync::Arc;
///   # let ctx: Context<gimli::EndianSlice<gimli::RunTimeEndian>> = todo!();
///   # let do_split_dwarf_load = |load: SplitDwarfLoad<gimli::EndianSlice<gimli::RunTimeEndian>>| -> Option<Arc<gimli::Dwarf<gimli::EndianSlice<gimli::RunTimeEndian>>>> { None };
///   const ADDRESS: u64 = 0xdeadbeef;
///   let mut r = ctx.find_frames(ADDRESS);
///   let result = loop {
///     match r {
///       LookupResult::Output(result) => break result,
///       LookupResult::Load { load, continuation } => {
///         let dwo = do_split_dwarf_load(load);
///         r = continuation.resume(dwo);
///       }
///     }
///   };
/// ```
pub enum LookupResult<L: LookupContinuation> {
    /// The lookup requires split DWARF data to be loaded.
    Load {
        /// The information needed to find the split DWARF data.
        load: SplitDwarfLoad<<L as LookupContinuation>::Buf>,
        /// The continuation to resume with the loaded split DWARF data.
        continuation: L,
    },
    /// The lookup has completed and produced an output.
    Output(<L as LookupContinuation>::Output),
}

/// This trait represents a partially complete operation that can be resumed
/// once a load of needed split DWARF data is completed or abandoned by the
/// API consumer.
pub trait LookupContinuation: Sized {
    /// The final output of this operation.
    type Output;
    /// The type of reader used.
    type Buf: gimli::Reader;

    /// Resumes the operation with the provided data.
    ///
    /// After the caller loads the split DWARF data required, call this
    /// method to resume the operation. The return value of this method
    /// indicates if the computation has completed or if further data is
    /// required.
    ///
    /// If the additional data cannot be located, or the caller does not
    /// support split DWARF, `resume(None)` can be used to continue the
    /// operation with the data that is available.
    fn resume(self, input: Option<Arc<gimli::Dwarf<Self::Buf>>>) -> LookupResult<Self>;
}

impl<L: LookupContinuation> LookupResult<L> {
    /// Callers that do not handle split DWARF can call `skip_all_loads`
    /// to fast-forward to the end result. This result is produced with
    /// the data that is available and may be less accurate than the
    /// the results that would be produced if the caller did properly
    /// support split DWARF.
    pub fn skip_all_loads(mut self) -> L::Output {
        loop {
            self = match self {
                LookupResult::Output(t) => return t,
                LookupResult::Load { continuation, .. } => continuation.resume(None),
            };
        }
    }

    pub(crate) fn map<T, F: FnOnce(L::Output) -> T>(
        self,
        f: F,
    ) -> LookupResult<MappedLookup<T, L, F>> {
        match self {
            LookupResult::Output(t) => LookupResult::Output(f(t)),
            LookupResult::Load { load, continuation } => LookupResult::Load {
                load,
                continuation: MappedLookup {
                    original: continuation,
                    mutator: f,
                },
            },
        }
    }

    pub(crate) fn unwrap(self) -> L::Output {
        match self {
            LookupResult::Output(t) => t,
            LookupResult::Load { .. } => unreachable!("Internal API misuse"),
        }
    }
}

pub(crate) struct SimpleLookup<T, R, F>
where
    F: FnOnce(Option<Arc<gimli::Dwarf<R>>>) -> T,
    R: gimli::Reader,
{
    f: F,
    phantom: PhantomData<(T, R)>,
}

impl<T, R, F> SimpleLookup<T, R, F>
where
    F: FnOnce(Option<Arc<gimli::Dwarf<R>>>) -> T,
    R: gimli::Reader,
{
    pub(crate) fn new_complete(t: F::Output) -> LookupResult<SimpleLookup<T, R, F>> {
        LookupResult::Output(t)
    }

    pub(crate) fn new_needs_load(
        load: SplitDwarfLoad<R>,
        f: F,
    ) -> LookupResult<SimpleLookup<T, R, F>> {
        LookupResult::Load {
            load,
            continuation: SimpleLookup {
                f,
                phantom: PhantomData,
            },
        }
    }
}

impl<T, R, F> LookupContinuation for SimpleLookup<T, R, F>
where
    F: FnOnce(Option<Arc<gimli::Dwarf<R>>>) -> T,
    R: gimli::Reader,
{
    type Output = T;
    type Buf = R;

    fn resume(self, v: Option<Arc<gimli::Dwarf<Self::Buf>>>) -> LookupResult<Self> {
        LookupResult::Output((self.f)(v))
    }
}

pub(crate) struct MappedLookup<T, L, F>
where
    L: LookupContinuation,
    F: FnOnce(L::Output) -> T,
{
    original: L,
    mutator: F,
}

impl<T, L, F> LookupContinuation for MappedLookup<T, L, F>
where
    L: LookupContinuation,
    F: FnOnce(L::Output) -> T,
{
    type Output = T;
    type Buf = L::Buf;

    fn resume(self, v: Option<Arc<gimli::Dwarf<Self::Buf>>>) -> LookupResult<Self> {
        match self.original.resume(v) {
            LookupResult::Output(t) => LookupResult::Output((self.mutator)(t)),
            LookupResult::Load { load, continuation } => LookupResult::Load {
                load,
                continuation: MappedLookup {
                    original: continuation,
                    mutator: self.mutator,
                },
            },
        }
    }
}

/// Some functions (e.g. `find_frames`) require considering multiple
/// compilation units, each of which might require their own split DWARF
/// lookup (and thus produce a continuation).
///
/// We store the underlying continuation here as well as a mutator function
/// that will either a) decide that the result of this continuation is
/// what is needed and mutate it to the final result or b) produce another
/// `LookupResult`. `new_lookup` will in turn eagerly drive any non-continuation
/// `LookupResult` with successive invocations of the mutator, until a new
/// continuation or a final result is produced. And finally, the impl of
/// `LookupContinuation::resume` will call `new_lookup` each time the
/// computation is resumed.
pub(crate) struct LoopingLookup<T, L, F>
where
    L: LookupContinuation,
    F: FnMut(L::Output) -> ControlFlow<T, LookupResult<L>>,
{
    continuation: L,
    mutator: F,
}

impl<T, L, F> LoopingLookup<T, L, F>
where
    L: LookupContinuation,
    F: FnMut(L::Output) -> ControlFlow<T, LookupResult<L>>,
{
    pub(crate) fn new_complete(t: T) -> LookupResult<Self> {
        LookupResult::Output(t)
    }

    pub(crate) fn new_lookup(mut r: LookupResult<L>, mut mutator: F) -> LookupResult<Self> {
        // Drive the loop eagerly so that we only ever have to represent one state
        // (the r == ControlFlow::Continue state) in LoopingLookup.
        loop {
            match r {
                LookupResult::Output(l) => match mutator(l) {
                    ControlFlow::Break(t) => return LookupResult::Output(t),
                    ControlFlow::Continue(r2) => {
                        r = r2;
                    }
                },
                LookupResult::Load { load, continuation } => {
                    return LookupResult::Load {
                        load,
                        continuation: LoopingLookup {
                            continuation,
                            mutator,
                        },
                    };
                }
            }
        }
    }
}

impl<T, L, F> LookupContinuation for LoopingLookup<T, L, F>
where
    L: LookupContinuation,
    F: FnMut(L::Output) -> ControlFlow<T, LookupResult<L>>,
{
    type Output = T;
    type Buf = L::Buf;

    fn resume(self, v: Option<Arc<gimli::Dwarf<Self::Buf>>>) -> LookupResult<Self> {
        let r = self.continuation.resume(v);
        LoopingLookup::new_lookup(r, self.mutator)
    }
}
