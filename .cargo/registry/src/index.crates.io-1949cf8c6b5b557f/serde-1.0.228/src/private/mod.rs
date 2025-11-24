#[cfg(not(no_serde_derive))]
pub mod de;
#[cfg(not(no_serde_derive))]
pub mod ser;

pub use crate::lib::clone::Clone;
pub use crate::lib::convert::{From, Into, TryFrom};
pub use crate::lib::default::Default;
pub use crate::lib::fmt::{self, Formatter};
pub use crate::lib::marker::PhantomData;
pub use crate::lib::option::Option::{self, None, Some};
pub use crate::lib::ptr;
pub use crate::lib::result::Result::{self, Err, Ok};

pub use crate::serde_core_private::string::from_utf8_lossy;

#[cfg(any(feature = "alloc", feature = "std"))]
pub use crate::lib::{ToString, Vec};
