use std::{
    ffi::{c_int, c_uchar},
    mem, ptr,
};

#[cfg(unix)]
#[path = "unix.rs"]
mod imp;

#[cfg(windows)]
#[path = "windows.rs"]
mod imp;

pub(crate) use imp::Aligned;

/// Helper to encode a series of control messages (native "cmsgs") to a buffer for use in `sendmsg`
//  like API.
///
/// The operation must be "finished" for the native msghdr to be usable, either by calling `finish`
/// explicitly or by dropping the `Encoder`.
pub(crate) struct Encoder<'a, M: MsgHdr> {
    hdr: &'a mut M,
    cmsg: Option<&'a mut M::ControlMessage>,
    len: usize,
}

impl<'a, M: MsgHdr> Encoder<'a, M> {
    /// # Safety
    /// - `hdr` must contain a suitably aligned pointer to a big enough buffer to hold control messages
    ///   bytes. All bytes of this buffer can be safely written.
    /// - The `Encoder` must be dropped before `hdr` is passed to a system call, and must not be leaked.
    pub(crate) unsafe fn new(hdr: &'a mut M) -> Self {
        Self {
            cmsg: hdr.cmsg_first_hdr().as_mut(),
            hdr,
            len: 0,
        }
    }

    /// Append a control message to the buffer.
    ///
    /// # Panics
    /// - If insufficient buffer space remains.
    /// - If `T` has stricter alignment requirements than `M::ControlMessage`
    pub(crate) fn push<T: Copy>(&mut self, level: c_int, ty: c_int, value: T) {
        assert!(mem::align_of::<T>() <= mem::align_of::<M::ControlMessage>());
        let space = M::ControlMessage::cmsg_space(mem::size_of_val(&value));
        assert!(
            self.hdr.control_len() >= self.len + space,
            "control message buffer too small. Required: {}, Available: {}",
            self.len + space,
            self.hdr.control_len()
        );
        let cmsg = self.cmsg.take().expect("no control buffer space remaining");
        cmsg.set(
            level,
            ty,
            M::ControlMessage::cmsg_len(mem::size_of_val(&value)),
        );
        unsafe {
            ptr::write(cmsg.cmsg_data() as *const T as *mut T, value);
        }
        self.len += space;
        self.cmsg = unsafe { self.hdr.cmsg_nxt_hdr(cmsg).as_mut() };
    }

    /// Finishes appending control messages to the buffer
    pub(crate) fn finish(self) {
        // Delegates to the `Drop` impl
    }
}

// Statically guarantees that the encoding operation is "finished" before the control buffer is read
// by `sendmsg` like API.
impl<M: MsgHdr> Drop for Encoder<'_, M> {
    fn drop(&mut self) {
        self.hdr.set_control_len(self.len as _);
    }
}

/// # Safety
///
/// `cmsg` must refer to a native cmsg containing a payload of type `T`
pub(crate) unsafe fn decode<T: Copy, C: CMsgHdr>(cmsg: &impl CMsgHdr) -> T {
    assert!(mem::align_of::<T>() <= mem::align_of::<C>());
    debug_assert_eq!(cmsg.len(), C::cmsg_len(mem::size_of::<T>()));
    ptr::read(cmsg.cmsg_data() as *const T)
}

pub(crate) struct Iter<'a, M: MsgHdr> {
    hdr: &'a M,
    cmsg: Option<&'a M::ControlMessage>,
}

impl<'a, M: MsgHdr> Iter<'a, M> {
    /// # Safety
    ///
    /// `hdr` must hold a pointer to memory outliving `'a` which can be soundly read for the
    /// lifetime of the constructed `Iter` and contains a buffer of native cmsgs, i.e. is aligned
    //  for native `cmsghdr`, is fully initialized, and has correct internal links.
    pub(crate) unsafe fn new(hdr: &'a M) -> Self {
        Self {
            hdr,
            cmsg: hdr.cmsg_first_hdr().as_ref(),
        }
    }
}

impl<'a, M: MsgHdr> Iterator for Iter<'a, M> {
    type Item = &'a M::ControlMessage;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.cmsg.take()?;
        self.cmsg = unsafe { self.hdr.cmsg_nxt_hdr(current).as_ref() };

        #[cfg(apple_fast)]
        {
            // On MacOS < 14 CMSG_NXTHDR might continuously return a zeroed cmsg. In
            // such case, return `None` instead, thus indicating the end of
            // the cmsghdr chain.
            if current.len() < mem::size_of::<M::ControlMessage>() {
                return None;
            }
        }

        Some(current)
    }
}

// Helper traits for native types for control messages
pub(crate) trait MsgHdr {
    type ControlMessage: CMsgHdr;

    fn cmsg_first_hdr(&self) -> *mut Self::ControlMessage;

    fn cmsg_nxt_hdr(&self, cmsg: &Self::ControlMessage) -> *mut Self::ControlMessage;

    /// Sets the number of control messages added to this `struct msghdr`.
    ///
    /// Note that this is a destructive operation and should only be done as a finalisation
    /// step.
    fn set_control_len(&mut self, len: usize);

    fn control_len(&self) -> usize;
}

pub(crate) trait CMsgHdr {
    fn cmsg_len(length: usize) -> usize;

    fn cmsg_space(length: usize) -> usize;

    fn cmsg_data(&self) -> *mut c_uchar;

    fn set(&mut self, level: c_int, ty: c_int, len: usize);

    fn len(&self) -> usize;
}
