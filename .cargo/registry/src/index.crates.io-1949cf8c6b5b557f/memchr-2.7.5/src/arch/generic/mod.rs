/*!
This module defines "generic" routines that can be specialized to specific
architectures.

We don't expose this module primarily because it would require exposing all
of the internal infrastructure required to write these generic routines.
That infrastructure should be treated as an implementation detail so that
it is allowed to evolve. Instead, what we expose are architecture specific
instantiations of these generic implementations. The generic code just lets us
write the code once (usually).
*/

pub(crate) mod memchr;
pub(crate) mod packedpair;
