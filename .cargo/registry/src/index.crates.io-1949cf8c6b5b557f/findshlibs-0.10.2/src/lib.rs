//! # `findshlibs`
//!
//! Find the set of shared libraries currently loaded in this process with a
//! cross platform API.
//!
//! The API entry point is the `TargetSharedLibrary` type and its
//! `SharedLibrary::each` trait method implementation.
//!
//! ## Example
//!
//! Here is an example program that prints out each shared library that is
//! loaded in the process and the addresses where each of its segments are
//! mapped into memory.
//!
//! ```
//! extern crate findshlibs;
//! use findshlibs::{Segment, SharedLibrary, TargetSharedLibrary};
//!
//! fn main() {
//!     TargetSharedLibrary::each(|shlib| {
//!         println!("{}", shlib.name().to_string_lossy());
//!
//!         for seg in shlib.segments() {
//!             println!("    {}: segment {}",
//!                      seg.actual_virtual_memory_address(shlib),
//!                      seg.name());
//!         }
//!     });
//! }
//! ```
//!
//! ## Supported OSes
//!
//! These are the OSes that `findshlibs` currently supports:
//!
//! * Linux
//! * macOS
//! * Windows
//! * Android
//! * iOS
//!
//! If a platform is not supported then a fallback implementation is used that
//! does nothing.  To see if your platform does something at runtime the
//! `TARGET_SUPPORTED` constant can be used.
//!
//! Is your OS missing here? Send us a pull request!
//!
//! ## Addresses
//!
//! Shared libraries' addresses can be confusing. They are loaded somewhere in
//! physical memory, but we generally don't care about physical memory
//! addresses, because only the OS can see that address and in userspace we can
//! only access memory through its virtual memory address. But even "virtual
//! memory address" is ambiguous because it isn't clear whether this is the
//! address before or after the loader maps the shared library into memory and
//! performs relocation.
//!
//! To clarify between these different kinds of addresses, we borrow some
//! terminology from [LUL][]:
//!
//! > * **SVMA** ("Stated Virtual Memory Address"): this is an address of a
//! >   symbol (etc) as it is stated in the symbol table, or other
//! >   metadata, of an object.  Such values are typically small and
//! >   start from zero or thereabouts, unless the object has been
//! >   prelinked.
//! >
//! > * **AVMA** ("Actual Virtual Memory Address"): this is the address of a
//! >   symbol (etc) in a running process, that is, once the associated
//! >   object has been mapped into a process.  Such values are typically
//! >   much larger than SVMAs, since objects can get mapped arbitrarily
//! >   far along the address space.
//! >
//! > * **"Bias"**: the difference between AVMA and SVMA for a given symbol
//! >   (specifically, AVMA - SVMA).  The bias is always an integral
//! >   number of pages.  Once we know the bias for a given object's
//! >   text section (for example), we can compute the AVMAs of all of
//! >   its text symbols by adding the bias to their SVMAs.
//!
//! [LUL]: https://searchfox.org/mozilla-central/rev/13148faaa91a1c823a7d68563d9995480e714979/tools/profiler/lul/LulMain.h#17-51
//!
//! ## Names and IDs
//!
//! `findshlibs` also gives access to module names and IDs.  Since this is also
//! not consistent across operating systems the following general rules apply:
//!
//! > * `id` refers to the ID of the object file itself.  This is generally
//! >   available on all platforms however it might still not be compiled into
//! >   the binary in all case.  For instance on Linux the `gnu.build-id` note
//! >   needs to be compiled in (which Rust does automatically).
//! > * `debug_id` refers to the ID of the debug file.  This only plays a role
//! >   on Windows where the executable and the debug file (PDB) have a different
//! >   ID.
//! > * `name` is the name of the executable.  On most operating systems (and
//! >   all systems implemented currently) this is not just the name but in fact
//! >   the entire path to the executable.
//! > * `debug_name` is the name of the debug file if known.  This is again
//! >   the case on windows where this will be the path to the PDB file.
#![deny(missing_docs)]

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod macos;

#[cfg(any(
    target_os = "linux",
    all(target_os = "android", feature = "dl_iterate_phdr")
))]
pub mod linux;

#[cfg(target_os = "windows")]
pub mod windows;

use std::ffi::OsStr;
use std::fmt::{self, Debug};
use std::usize;

pub mod unsupported;

#[cfg(any(
    target_os = "linux",
    all(target_os = "android", feature = "dl_iterate_phdr")
))]
use crate::linux as native_mod;

#[cfg(any(target_os = "macos", target_os = "ios"))]
use crate::macos as native_mod;

#[cfg(target_os = "windows")]
use crate::windows as native_mod;

#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "linux",
    all(target_os = "android", feature = "dl_iterate_phdr"),
    target_os = "windows"
)))]
use unsupported as native_mod;

/// The [`SharedLibrary` trait](./trait.SharedLibrary.html)
/// implementation for the target operating system.
pub type TargetSharedLibrary<'a> = native_mod::SharedLibrary<'a>;

/// An indicator if this platform is supported.
pub const TARGET_SUPPORTED: bool = cfg!(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "linux",
    all(target_os = "android", feature = "dl_iterate_phdr"),
    target_os = "windows"
));

macro_rules! simple_newtypes {
    (
        $(
            $(#[$attr:meta])*
            type $name:ident = $oldty:ty
            where
                default = $default:expr ,
                display = $format:expr ;
        )*
    ) => {
        $(
            $(#[$attr])*
            #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
            pub struct $name(pub $oldty);

            impl Default for $name {
                #[inline]
                fn default() -> Self {
                    $name( $default )
                }
            }

            impl From<$oldty> for $name {
                fn from(x: $oldty) -> $name {
                    $name(x)
                }
            }

            impl From<$name> for $oldty {
                fn from($name(x): $name) -> $oldty {
                    x
                }
            }

            impl fmt::Display for $name {
                fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    write!(f, $format, self.0)
                }
            }
        )*
    }
}

simple_newtypes! {
    /// Stated virtual memory address.
    ///
    /// See the module documentation for details.
    type Svma = usize
    where
        default = 0,
        display = "{:#x}";

    /// Actual virtual memory address.
    ///
    /// See the module documentation for details.
    type Avma = usize
    where
        default = 0,
        display = "{:#x}";

    /// Virtual memory bias.
    ///
    /// See the module documentation for details.
    type Bias = usize
    where
        default = 0,
        display = "{:#x}";
}

/// A mapped segment in a shared library.
#[allow(clippy::len_without_is_empty)]
pub trait Segment: Sized + Debug {
    /// The associated shared library type for this segment.
    type SharedLibrary: SharedLibrary<Segment = Self>;

    /// Get this segment's name.
    fn name(&self) -> &str;

    /// Returns `true` if this is a code segment.
    #[inline]
    fn is_code(&self) -> bool {
        false
    }

    /// Returns `true` if this is a segment loaded into memory.
    #[inline]
    fn is_load(&self) -> bool {
        self.is_code()
    }

    /// Get this segment's stated virtual address of this segment.
    ///
    /// This is the virtual memory address without the bias applied. See the
    /// module documentation for details.
    fn stated_virtual_memory_address(&self) -> Svma;

    /// Get the length of this segment in memory (in bytes).
    fn len(&self) -> usize;

    // Provided methods.

    /// Get this segment's actual virtual memory address.
    ///
    /// This is the virtual memory address with the bias applied. See the module
    /// documentation for details.
    #[inline]
    fn actual_virtual_memory_address(&self, shlib: &Self::SharedLibrary) -> Avma {
        let svma = self.stated_virtual_memory_address();
        let bias = shlib.virtual_memory_bias();
        Avma(svma.0 + bias.0)
    }

    /// Does this segment contain the given address?
    #[inline]
    fn contains_svma(&self, address: Svma) -> bool {
        let start = self.stated_virtual_memory_address().0;
        let end = start + self.len();
        let address = address.0;
        start <= address && address < end
    }

    /// Does this segment contain the given address?
    #[inline]
    fn contains_avma(&self, shlib: &Self::SharedLibrary, address: Avma) -> bool {
        let start = self.actual_virtual_memory_address(shlib).0;
        let end = start + self.len();
        let address = address.0;
        start <= address && address < end
    }
}

/// Represents an ID for a shared library.
#[derive(PartialEq, Eq, Hash)]
pub enum SharedLibraryId {
    /// A UUID (used on mac)
    Uuid([u8; 16]),
    /// A GNU build ID
    GnuBuildId(Vec<u8>),
    /// The PE timestamp and size
    PeSignature(u32, u32),
    /// A PDB GUID and age,
    PdbSignature([u8; 16], u32),
}

impl SharedLibraryId {
    /// Returns the raw bytes of the shared library ID.
    pub fn as_bytes(&self) -> &[u8] {
        match *self {
            SharedLibraryId::Uuid(ref bytes) => &*bytes,
            SharedLibraryId::GnuBuildId(ref bytes) => bytes,
            SharedLibraryId::PeSignature(_, _) => &[][..],
            SharedLibraryId::PdbSignature(ref bytes, _) => &*bytes,
        }
    }
}

impl fmt::Display for SharedLibraryId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            SharedLibraryId::Uuid(ref bytes) => {
                for (idx, byte) in bytes.iter().enumerate() {
                    if idx == 4 || idx == 6 || idx == 8 || idx == 10 {
                        write!(f, "-")?;
                    }
                    write!(f, "{:02x}", byte)?;
                }
            }
            SharedLibraryId::GnuBuildId(ref bytes) => {
                for byte in bytes {
                    write!(f, "{:02x}", byte)?;
                }
            }
            SharedLibraryId::PeSignature(timestamp, size_of_image) => {
                write!(f, "{:08X}{:x}", timestamp, size_of_image)?;
            }
            SharedLibraryId::PdbSignature(ref bytes, age) => {
                for (idx, byte) in bytes.iter().enumerate() {
                    if idx == 4 || idx == 6 || idx == 8 || idx == 10 {
                        write!(f, "-")?;
                    }
                    write!(f, "{:02X}", byte)?;
                }
                write!(f, "{:x}", age)?;
            }
        }
        Ok(())
    }
}

impl fmt::Debug for SharedLibraryId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match *self {
            SharedLibraryId::Uuid(..) => "Uuid",
            SharedLibraryId::GnuBuildId(..) => "GnuBuildId",
            SharedLibraryId::PeSignature(..) => "PeSignature",
            SharedLibraryId::PdbSignature(..) => "PdbSignature",
        };
        write!(f, "{}(\"{}\")", name, self)
    }
}

/// A trait representing a shared library that is loaded in this process.
#[allow(clippy::len_without_is_empty)]
pub trait SharedLibrary: Sized + Debug {
    /// The associated segment type for this shared library.
    type Segment: Segment<SharedLibrary = Self>;

    /// An iterator over a shared library's segments.
    type SegmentIter: Debug + Iterator<Item = Self::Segment>;

    /// Get the name of this shared library.
    fn name(&self) -> &OsStr;

    /// Get the name of the debug file with this shared library if there is one.
    fn debug_name(&self) -> Option<&OsStr> {
        None
    }

    /// Get the code-id of this shared library if available.
    fn id(&self) -> Option<SharedLibraryId>;

    /// Get the debug-id of this shared library if available.
    fn debug_id(&self) -> Option<SharedLibraryId> {
        self.id()
    }

    /// Returns the address of where the library is loaded into virtual
    /// memory.
    ///
    /// This address maps to the `Avma` of the first segment loaded into
    /// memory. Depending on the platform, this segment may not contain code.
    fn actual_load_addr(&self) -> Avma {
        self.segments()
            .find(|x| x.is_load())
            .map(|x| x.actual_virtual_memory_address(self))
            .unwrap_or(Avma(usize::MAX))
    }

    #[inline]
    #[doc(hidden)]
    #[deprecated(note = "use stated_load_address() instead")]
    fn load_addr(&self) -> Svma {
        self.stated_load_addr()
    }

    /// Returns the address of where the library prefers to be loaded into
    /// virtual memory.
    ///
    /// This address maps to the `Svma` of the first segment loaded into
    /// memory. Depending on the platform, this segment may not contain code.
    fn stated_load_addr(&self) -> Svma {
        self.segments()
            .find(|x| x.is_load())
            .map(|x| x.stated_virtual_memory_address())
            .unwrap_or(Svma(usize::MAX))
    }

    /// Returns the size of the image.
    ///
    /// This typically is the size of the executable code segment.  This is
    /// normally used by server side symbolication systems to determine when
    /// an IP no longer falls into an image.
    fn len(&self) -> usize {
        let end_address = self
            .segments()
            .filter(|x| x.is_load())
            .map(|x| x.actual_virtual_memory_address(self).0 + x.len())
            .max()
            .unwrap_or(usize::MAX);

        end_address - self.actual_load_addr().0
    }

    /// Iterate over this shared library's segments.
    fn segments(&self) -> Self::SegmentIter;

    /// Get the bias of this shared library.
    ///
    /// See the module documentation for details.
    fn virtual_memory_bias(&self) -> Bias;

    /// Given an AVMA within this shared library, convert it back to an SVMA by
    /// removing this shared library's bias.
    #[inline]
    fn avma_to_svma(&self, address: Avma) -> Svma {
        let bias = self.virtual_memory_bias();
        Svma(address.0 - bias.0)
    }

    /// Find all shared libraries in this process and invoke `f` with each one.
    fn each<F, C>(f: F)
    where
        F: FnMut(&Self) -> C,
        C: Into<IterationControl>;
}

/// Control whether iteration over shared libraries should continue or stop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IterationControl {
    /// Stop iteration.
    Break,
    /// Continue iteration.
    Continue,
}

impl From<()> for IterationControl {
    #[inline]
    fn from(_: ()) -> Self {
        IterationControl::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panic_in_each() {
        use std::panic;

        match panic::catch_unwind(|| {
            TargetSharedLibrary::each(|_| panic!("uh oh"));
        }) {
            Ok(()) => panic!("Expected a panic, but didn't get one"),
            Err(any) => {
                assert!(
                    any.is::<&'static str>(),
                    "panic value should be a &'static str"
                );
                assert_eq!(*any.downcast_ref::<&'static str>().unwrap(), "uh oh");
            }
        }
    }

    #[test]
    fn test_load_address_bias() {
        TargetSharedLibrary::each(|lib| {
            let svma = lib.stated_load_addr();
            let avma = lib.actual_load_addr();
            assert_eq!(lib.avma_to_svma(avma), svma);
        });
    }
}
