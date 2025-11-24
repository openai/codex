//! Server-side implementation of a Wayland protocol backend using `libwayland`

use std::{
    ffi::{CStr, CString},
    os::raw::{c_int, c_void},
    os::unix::{
        io::{BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd},
        net::UnixStream,
    },
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, Weak,
    },
};

use crate::protocol::{
    check_for_signature, same_interface, AllowNull, Argument, ArgumentType, Interface, Message,
    ObjectInfo, ANONYMOUS_INTERFACE,
};
use scoped_tls::scoped_thread_local;
use smallvec::SmallVec;

use wayland_sys::{common::*, ffi_dispatch, server::*};

use super::{free_arrays, server::*, RUST_MANAGED};

#[allow(unused_imports)]
pub use crate::types::server::{Credentials, DisconnectReason, GlobalInfo, InitError, InvalidId};

scoped_thread_local! {
    // scoped_tls does not allow unsafe_op_in_unsafe_fn internally
    #[allow(unsafe_op_in_unsafe_fn)]
    static HANDLE: (Arc<Mutex<dyn ErasedState + Send>>, *mut c_void)
}

type PendingDestructor<D> = (Arc<dyn ObjectData<D>>, ClientId, ObjectId);

// Pointer is &mut Vec<PendingDestructor<D>>
scoped_thread_local! {
    // scoped_tls does not allow unsafe_op_in_unsafe_fn internally
    #[allow(unsafe_op_in_unsafe_fn)]
    static PENDING_DESTRUCTORS: *mut c_void
}

/// An id of an object on a wayland server.
#[derive(Clone)]
pub struct InnerObjectId {
    id: u32,
    ptr: *mut wl_resource,
    alive: Arc<AtomicBool>,
    interface: &'static Interface,
}

unsafe impl Send for InnerObjectId {}
unsafe impl Sync for InnerObjectId {}

impl std::cmp::PartialEq for InnerObjectId {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.alive, &other.alive)
    }
}

impl std::cmp::Eq for InnerObjectId {}

impl std::hash::Hash for InnerObjectId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.ptr.hash(state);
        (&*self.alive as *const AtomicBool).hash(state);
    }
}

impl std::fmt::Display for InnerObjectId {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.interface.name, self.id)
    }
}

impl std::fmt::Debug for InnerObjectId {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ObjectId({self})")
    }
}

impl InnerObjectId {
    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }

    pub fn interface(&self) -> &'static Interface {
        self.interface
    }

    pub fn same_client_as(&self, other: &Self) -> bool {
        if !self.alive.load(Ordering::Acquire) || !other.alive.load(Ordering::Acquire) {
            return false;
        }
        let my_client_ptr =
            unsafe { ffi_dispatch!(wayland_server_handle(), wl_resource_get_client, self.ptr) };
        let other_client_ptr =
            unsafe { ffi_dispatch!(wayland_server_handle(), wl_resource_get_client, other.ptr) };
        my_client_ptr == other_client_ptr
    }

    pub fn protocol_id(&self) -> u32 {
        self.id
    }

    pub unsafe fn from_ptr(
        interface: Option<&'static Interface>,
        ptr: *mut wl_resource,
    ) -> Result<InnerObjectId, InvalidId> {
        let id = ffi_dispatch!(wayland_server_handle(), wl_resource_get_id, ptr);

        // This is a horrible back to work around the fact that it is not possible to check if the
        // resource implementation is RUST_MANAGED without knowing the interface of that object...
        let is_rust_managed = {
            // wl_resource_instance_of uses wl_interface_equals, which only checks that the interface name
            // is correct, so we create a temporary dummy wl_interface with the correct name so that the
            // check only actually verifies that the object is RUST_MANAGED
            let iface_name = ffi_dispatch!(wayland_server_handle(), wl_resource_get_class, ptr);
            let dummy_iface = wl_interface {
                name: iface_name,
                version: 0,
                request_count: 0,
                event_count: 0,
                requests: std::ptr::null(),
                events: std::ptr::null(),
            };
            ffi_dispatch!(
                wayland_server_handle(),
                wl_resource_instance_of,
                ptr,
                &dummy_iface,
                &RUST_MANAGED as *const u8 as *const _
            ) != 0
        };

        if is_rust_managed {
            // Using () instead of the type parameter here is safe, because:
            // 1) ResourceUserData is #[repr(C)], so its layout does not depend on D
            // 2) we are only accessing the field `.alive`, which type is independent of D
            //
            let udata = ffi_dispatch!(wayland_server_handle(), wl_resource_get_user_data, ptr)
                as *mut ResourceUserData<()>;
            let alive = unsafe { (*udata).alive.clone() };
            let udata_iface = unsafe { (*udata).interface };
            if let Some(iface) = interface {
                if !same_interface(iface, udata_iface) {
                    return Err(InvalidId);
                }
            }
            Ok(InnerObjectId { id, ptr, alive, interface: udata_iface })
        } else if let Some(interface) = interface {
            let iface_c_ptr =
                interface.c_ptr.expect("[wayland-backend-sys] Cannot use Interface without c_ptr!");
            // Safety: the provided pointer must be a valid wayland object
            let ptr_iface_name = unsafe {
                CStr::from_ptr(ffi_dispatch!(wayland_server_handle(), wl_resource_get_class, ptr))
            };
            // Safety: the code generated by wayland-scanner is valid
            let provided_iface_name = unsafe { CStr::from_ptr(iface_c_ptr.name) };
            if ptr_iface_name != provided_iface_name {
                return Err(InvalidId);
            }
            // On external resource, we use the wl_resource_add_destroy_listener mechanism to track their liveness
            let listener = ffi_dispatch!(
                wayland_server_handle(),
                wl_resource_get_destroy_listener,
                ptr,
                external_resource_destroy_notify
            );
            let alive = if listener.is_null() {
                // The listener needs to be initialized
                let alive = Arc::new(AtomicBool::new(true));
                let data = Box::into_raw(Box::new(ExternalResourceData { alive: alive.clone() }));
                let listener = signal::rust_listener_create(external_resource_destroy_notify);
                // Safety: we just created listener and client_data, they are valid
                unsafe {
                    signal::rust_listener_set_user_data(listener, data as *mut c_void);
                }
                ffi_dispatch!(
                    wayland_server_handle(),
                    wl_resource_add_destroy_listener,
                    ptr,
                    listener
                );
                alive
            } else {
                unsafe {
                    let data =
                        signal::rust_listener_get_user_data(listener) as *mut ExternalResourceData;
                    (*data).alive.clone()
                }
            };
            Ok(InnerObjectId { id, ptr, alive, interface })
        } else {
            Err(InvalidId)
        }
    }

    pub fn as_ptr(&self) -> *mut wl_resource {
        if self.alive.load(Ordering::Acquire) {
            self.ptr
        } else {
            std::ptr::null_mut()
        }
    }
}

struct ExternalResourceData {
    alive: Arc<AtomicBool>,
}

unsafe extern "C" fn external_resource_destroy_notify(listener: *mut wl_listener, _: *mut c_void) {
    // Safety: if this function is invoked by libwayland its arguments must be valid
    let data = unsafe {
        Box::from_raw(signal::rust_listener_get_user_data(listener) as *mut ExternalResourceData)
    };
    unsafe {
        signal::rust_listener_destroy(listener);
    }
    data.alive.store(false, Ordering::Release);
}

/// An id of a client connected to the server.
#[derive(Debug, Clone)]
pub struct InnerClientId {
    ptr: *mut wl_client,
    alive: Arc<AtomicBool>,
}

unsafe impl Send for InnerClientId {}
unsafe impl Sync for InnerClientId {}

impl std::cmp::PartialEq for InnerClientId {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.alive, &other.alive)
    }
}

impl std::cmp::Eq for InnerClientId {}

impl std::hash::Hash for InnerClientId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (&*self.alive as *const AtomicBool).hash(state)
    }
}

/// The ID of a global
#[derive(Debug, Clone)]
pub struct InnerGlobalId {
    ptr: *mut wl_global,
    alive: Arc<AtomicBool>,
}

unsafe impl Send for InnerGlobalId {}
unsafe impl Sync for InnerGlobalId {}

impl std::cmp::PartialEq for InnerGlobalId {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.alive, &other.alive)
    }
}

impl std::cmp::Eq for InnerGlobalId {}

impl std::hash::Hash for InnerGlobalId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (&*self.alive as *const AtomicBool).hash(state)
    }
}

#[repr(C)]
struct ResourceUserData<D> {
    alive: Arc<AtomicBool>,
    data: Arc<dyn ObjectData<D>>,
    interface: &'static Interface,
}

struct ClientUserData {
    data: Arc<dyn ClientData>,
    alive: Arc<AtomicBool>,
}

struct GlobalUserData<D> {
    handler: Arc<dyn GlobalHandler<D>>,
    interface: &'static Interface,
    version: u32,
    disabled: bool,
    alive: Arc<AtomicBool>,
    ptr: *mut wl_global,
}

#[derive(Debug)]
pub struct State<D: 'static> {
    display: *mut wl_display,
    pending_destructors: Vec<PendingDestructor<D>>,
    timer_source: *mut wl_event_source,
    _data: std::marker::PhantomData<fn(&mut D)>,
    known_globals: Vec<InnerGlobalId>,
}

unsafe impl<D> Send for State<D> {}

#[derive(Debug)]
pub struct InnerBackend<D: 'static> {
    state: Arc<Mutex<State<D>>>,
    display_ptr: *mut wl_display,
}

unsafe impl<D> Send for InnerBackend<D> {}
unsafe impl<D> Sync for InnerBackend<D> {}

impl<D> InnerBackend<D> {
    pub fn new() -> Result<Self, InitError> {
        if !is_lib_available() {
            return Err(InitError::NoWaylandLib);
        }

        let display = unsafe { ffi_dispatch!(wayland_server_handle(), wl_display_create) };
        if display.is_null() {
            panic!("[wayland-backend-sys] libwayland reported an allocation failure.");
        }

        #[cfg(feature = "log")]
        unsafe {
            ffi_dispatch!(
                wayland_server_handle(),
                wl_log_set_handler_server,
                wl_log_trampoline_to_rust_server
            )
        };

        unsafe {
            ffi_dispatch!(
                wayland_server_handle(),
                wl_display_set_global_filter,
                display,
                global_filter::<D>,
                std::ptr::null_mut()
            );
        }

        // Insert a timer we can use to force wakeups of the libwayland inner event loop
        extern "C" fn timer_noop_cb(_: *mut c_void) -> i32 {
            0
        }

        let timer_source = unsafe {
            let evl = ffi_dispatch!(wayland_server_handle(), wl_display_get_event_loop, display);

            ffi_dispatch!(
                wayland_server_handle(),
                wl_event_loop_add_timer,
                evl,
                timer_noop_cb,
                std::ptr::null_mut()
            )
        };

        Ok(Self {
            state: Arc::new(Mutex::new(State {
                display,
                pending_destructors: Vec::new(),
                timer_source,
                _data: std::marker::PhantomData,
                known_globals: Vec::new(),
            })),
            display_ptr: display,
        })
    }

    pub fn flush(&mut self, client: Option<ClientId>) -> std::io::Result<()> {
        self.state.lock().unwrap().flush(client)
    }

    pub fn handle(&self) -> Handle {
        Handle { handle: InnerHandle { state: self.state.clone() as Arc<_> } }
    }

    pub fn poll_fd(&self) -> BorrowedFd<'_> {
        unsafe {
            let evl_ptr =
                ffi_dispatch!(wayland_server_handle(), wl_display_get_event_loop, self.display_ptr);
            BorrowedFd::borrow_raw(ffi_dispatch!(
                wayland_server_handle(),
                wl_event_loop_get_fd,
                evl_ptr
            ))
        }
    }

    pub fn dispatch_client(
        &mut self,
        data: &mut D,
        _client_id: InnerClientId,
    ) -> std::io::Result<usize> {
        self.dispatch_all_clients(data)
    }

    pub fn dispatch_all_clients(&mut self, data: &mut D) -> std::io::Result<usize> {
        let state = self.state.clone() as Arc<Mutex<dyn ErasedState + Send>>;
        let display = self.display_ptr;
        let ret = HANDLE.set(&(state, data as *mut _ as *mut c_void), || unsafe {
            let evl_ptr =
                ffi_dispatch!(wayland_server_handle(), wl_display_get_event_loop, display);
            ffi_dispatch!(wayland_server_handle(), wl_event_loop_dispatch, evl_ptr, 0)
        });

        let pending_destructors =
            std::mem::take(&mut self.state.lock().unwrap().pending_destructors);
        for (object, client_id, object_id) in pending_destructors {
            let handle = self.handle();
            object.clone().destroyed(&handle, data, client_id, object_id);
        }

        if ret < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(ret as usize)
        }
    }
}

impl<D> Drop for State<D> {
    fn drop(&mut self) {
        // wl_display_destroy_clients may result in the destruction of some wayland objects. Pending
        // destructors are queued up inside the PENDING_DESTRUCTORS scoped global. We need to set the scoped
        // global in order for destructors to be queued up properly.
        PENDING_DESTRUCTORS.set(&(&mut self.pending_destructors as *mut _ as *mut _), || unsafe {
            ffi_dispatch!(wayland_server_handle(), wl_display_destroy_clients, self.display);
        });

        let known_globals = std::mem::take(&mut self.known_globals);
        for global in known_globals {
            unsafe {
                let _ = Box::from_raw(ffi_dispatch!(
                    wayland_server_handle(),
                    wl_global_get_user_data,
                    global.ptr
                ) as *mut GlobalUserData<D>);
                ffi_dispatch!(wayland_server_handle(), wl_global_destroy, global.ptr);
            }
        }

        unsafe {
            ffi_dispatch!(wayland_server_handle(), wl_display_destroy, self.display);
        }
    }
}

#[derive(Clone)]
pub struct InnerHandle {
    state: Arc<Mutex<dyn ErasedState + Send>>,
}

impl std::fmt::Debug for InnerHandle {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_struct("InnerHandle[sys]").finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct WeakInnerHandle {
    pub(crate) state: Weak<Mutex<dyn ErasedState + Send>>,
}

impl std::fmt::Debug for WeakInnerHandle {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_struct("WeakInnerHandle[sys]").finish_non_exhaustive()
    }
}

impl WeakInnerHandle {
    pub fn upgrade(&self) -> Option<InnerHandle> {
        self.state.upgrade().map(|state| InnerHandle { state })
    }
}

impl InnerHandle {
    pub fn downgrade(&self) -> WeakInnerHandle {
        WeakInnerHandle { state: Arc::downgrade(&self.state) }
    }

    pub fn object_info(&self, id: InnerObjectId) -> Result<ObjectInfo, InvalidId> {
        self.state.lock().unwrap().object_info(id)
    }

    pub fn insert_client(
        &self,
        stream: UnixStream,
        data: Arc<dyn ClientData>,
    ) -> std::io::Result<InnerClientId> {
        self.state.lock().unwrap().insert_client(stream, data)
    }

    pub fn get_client(&self, id: InnerObjectId) -> Result<ClientId, InvalidId> {
        self.state.lock().unwrap().get_client(id)
    }

    pub fn get_client_data(&self, id: InnerClientId) -> Result<Arc<dyn ClientData>, InvalidId> {
        self.state.lock().unwrap().get_client_data(id)
    }

    pub fn get_client_credentials(&self, id: InnerClientId) -> Result<Credentials, InvalidId> {
        self.state.lock().unwrap().get_client_credentials(id)
    }

    pub fn with_all_clients(&self, mut f: impl FnMut(ClientId)) {
        self.state.lock().unwrap().with_all_clients(&mut f)
    }

    pub fn with_all_objects_for(
        &self,
        client_id: InnerClientId,
        mut f: impl FnMut(ObjectId),
    ) -> Result<(), InvalidId> {
        self.state.lock().unwrap().with_all_objects_for(client_id, &mut f)
    }

    pub fn object_for_protocol_id(
        &self,
        client_id: InnerClientId,
        interface: &'static Interface,
        protocol_id: u32,
    ) -> Result<ObjectId, InvalidId> {
        self.state.lock().unwrap().object_for_protocol_id(client_id, interface, protocol_id)
    }

    pub fn create_object<D: 'static>(
        &self,
        client: InnerClientId,
        interface: &'static Interface,
        version: u32,
        data: Arc<dyn ObjectData<D>>,
    ) -> Result<ObjectId, InvalidId> {
        let mut state = self.state.lock().unwrap();
        // Keep this guard alive while the code is run to protect the C state
        let _state = (&mut *state as &mut dyn ErasedState)
            .downcast_mut::<State<D>>()
            .expect("Wrong type parameter passed to Handle::create_object().");

        if !client.alive.load(Ordering::Acquire) {
            return Err(InvalidId);
        }

        let interface_ptr =
            interface.c_ptr.expect("Interface without c_ptr are unsupported by the sys backend.");

        let resource = unsafe {
            ffi_dispatch!(
                wayland_server_handle(),
                wl_resource_create,
                client.ptr,
                interface_ptr,
                version as i32,
                0
            )
        };

        Ok(ObjectId { id: unsafe { init_resource(resource, interface, Some(data)).0 } })
    }

    pub fn null_id() -> ObjectId {
        ObjectId {
            id: InnerObjectId {
                ptr: std::ptr::null_mut(),
                id: 0,
                alive: Arc::new(AtomicBool::new(false)),
                interface: &ANONYMOUS_INTERFACE,
            },
        }
    }

    pub fn send_event(&self, msg: Message<ObjectId, RawFd>) -> Result<(), InvalidId> {
        self.state.lock().unwrap().send_event(msg)
    }

    pub fn get_object_data<D: 'static>(
        &self,
        id: InnerObjectId,
    ) -> Result<Arc<dyn ObjectData<D>>, InvalidId> {
        let mut state = self.state.lock().unwrap();
        // Keep this guard alive while the code is run to protect the C state
        let _state = (&mut *state as &mut dyn ErasedState)
            .downcast_mut::<State<D>>()
            .expect("Wrong type parameter passed to Handle::get_object_data().");

        if !id.alive.load(Ordering::Acquire) {
            return Err(InvalidId);
        }

        let iface_c_ptr =
            id.interface.c_ptr.expect("[wayland-backend-sys] Cannot use Interface without c_ptr!");
        let is_managed = unsafe {
            ffi_dispatch!(
                wayland_server_handle(),
                wl_resource_instance_of,
                id.ptr,
                iface_c_ptr,
                &RUST_MANAGED as *const u8 as *const _
            ) != 0
        };
        if !is_managed {
            return Err(InvalidId);
        }

        let udata = unsafe {
            &*(ffi_dispatch!(wayland_server_handle(), wl_resource_get_user_data, id.ptr)
                as *mut ResourceUserData<D>)
        };

        Ok(udata.data.clone())
    }

    pub fn get_object_data_any(
        &self,
        id: InnerObjectId,
    ) -> Result<Arc<dyn std::any::Any + Send + Sync>, InvalidId> {
        self.state.lock().unwrap().get_object_data_any(id)
    }

    pub fn set_object_data<D: 'static>(
        &self,
        id: InnerObjectId,
        data: Arc<dyn ObjectData<D>>,
    ) -> Result<(), InvalidId> {
        let mut state = self.state.lock().unwrap();
        // Keep this guard alive while the code is run to protect the C state
        let _state = (&mut *state as &mut dyn ErasedState)
            .downcast_mut::<State<D>>()
            .expect("Wrong type parameter passed to Handle::set_object_data().");

        if !id.alive.load(Ordering::Acquire) {
            return Err(InvalidId);
        }

        let iface_c_ptr =
            id.interface.c_ptr.expect("[wayland-backend-sys] Cannot use Interface without c_ptr!");
        let is_managed = unsafe {
            ffi_dispatch!(
                wayland_server_handle(),
                wl_resource_instance_of,
                id.ptr,
                iface_c_ptr,
                &RUST_MANAGED as *const u8 as *const _
            ) != 0
        };
        if !is_managed {
            return Err(InvalidId);
        }

        let udata = unsafe {
            &mut *(ffi_dispatch!(wayland_server_handle(), wl_resource_get_user_data, id.ptr)
                as *mut ResourceUserData<D>)
        };

        udata.data = data;

        Ok(())
    }

    pub fn post_error(&self, object_id: InnerObjectId, error_code: u32, message: CString) {
        self.state.lock().unwrap().post_error(object_id, error_code, message)
    }

    pub fn kill_client(&self, client_id: InnerClientId, reason: DisconnectReason) {
        self.state.lock().unwrap().kill_client(client_id, reason)
    }

    pub fn create_global<D: 'static>(
        &self,
        interface: &'static Interface,
        version: u32,
        handler: Arc<dyn GlobalHandler<D>>,
    ) -> InnerGlobalId {
        let display = {
            let mut state = self.state.lock().unwrap();
            let state = (&mut *state as &mut dyn ErasedState)
                .downcast_mut::<State<D>>()
                .expect("Wrong type parameter passed to Handle::create_global().");
            state.display
        };

        let alive = Arc::new(AtomicBool::new(true));

        let interface_ptr =
            interface.c_ptr.expect("Interface without c_ptr are unsupported by the sys backend.");

        let udata = Box::into_raw(Box::new(GlobalUserData {
            handler,
            alive: alive.clone(),
            interface,
            version,
            disabled: false,
            ptr: std::ptr::null_mut(),
        }));

        let ret = HANDLE.set(&(self.state.clone(), std::ptr::null_mut()), || unsafe {
            ffi_dispatch!(
                wayland_server_handle(),
                wl_global_create,
                display,
                interface_ptr,
                version as i32,
                udata as *mut c_void,
                global_bind::<D>
            )
        });

        if ret.is_null() {
            // free the user data as global creation failed
            let _ = unsafe { Box::from_raw(udata) };
            panic!(
                "[wayland-backend-sys] Invalid global specification or memory allocation failure."
            );
        }

        unsafe {
            (*udata).ptr = ret;
        }

        let mut state = self.state.lock().unwrap();
        let state = (&mut *state as &mut dyn ErasedState)
            .downcast_mut::<State<D>>()
            .expect("Wrong type parameter passed to Handle::create_global().");

        let id = InnerGlobalId { ptr: ret, alive };
        state.known_globals.push(id.clone());
        id
    }

    pub fn disable_global<D: 'static>(&self, id: InnerGlobalId) {
        // check that `D` is correct
        {
            let mut state = self.state.lock().unwrap();
            let _state = (&mut *state as &mut dyn ErasedState)
                .downcast_mut::<State<D>>()
                .expect("Wrong type parameter passed to Handle::disable_global().");
        }

        if !id.alive.load(Ordering::Acquire) {
            return;
        }

        let udata = unsafe {
            &mut *(ffi_dispatch!(wayland_server_handle(), wl_global_get_user_data, id.ptr)
                as *mut GlobalUserData<D>)
        };

        // libwayland will abort if wl_global_remove is called more than once.
        // This means we do nothing if the global is already disabled
        if !udata.disabled {
            udata.disabled = true;

            // send the global_remove
            HANDLE.set(&(self.state.clone(), std::ptr::null_mut()), || unsafe {
                ffi_dispatch!(wayland_server_handle(), wl_global_remove, id.ptr);
            });
        }
    }

    pub fn remove_global<D: 'static>(&self, id: InnerGlobalId) {
        {
            let mut state = self.state.lock().unwrap();
            let state = (&mut *state as &mut dyn ErasedState)
                .downcast_mut::<State<D>>()
                .expect("Wrong type parameter passed to Handle::remove_global().");
            state.known_globals.retain(|g| g != &id);
        }

        if !id.alive.load(Ordering::Acquire) {
            return;
        }

        let udata = unsafe {
            Box::from_raw(ffi_dispatch!(wayland_server_handle(), wl_global_get_user_data, id.ptr)
                as *mut GlobalUserData<D>)
        };
        udata.alive.store(false, Ordering::Release);

        HANDLE.set(&(self.state.clone(), std::ptr::null_mut()), || unsafe {
            ffi_dispatch!(wayland_server_handle(), wl_global_destroy, id.ptr);
        });
    }

    pub fn global_info(&self, id: InnerGlobalId) -> Result<GlobalInfo, InvalidId> {
        self.state.lock().unwrap().global_info(id)
    }

    /// Returns the handler which manages the visibility and notifies when a client has bound the global.
    pub fn get_global_handler<D: 'static>(
        &self,
        id: InnerGlobalId,
    ) -> Result<Arc<dyn GlobalHandler<D>>, InvalidId> {
        let mut state = self.state.lock().unwrap();
        // Keep this guard alive while the code is run to protect the C state
        let _state = (&mut *state as &mut dyn ErasedState)
            .downcast_mut::<State<D>>()
            .expect("Wrong type parameter passed to Handle::set_object_data().");

        if !id.alive.load(Ordering::Acquire) {
            return Err(InvalidId);
        }

        let udata = unsafe {
            Box::from_raw(ffi_dispatch!(wayland_server_handle(), wl_global_get_user_data, id.ptr)
                as *mut GlobalUserData<D>)
        };
        Ok(udata.handler.clone())
    }

    pub fn flush(&mut self, client: Option<ClientId>) -> std::io::Result<()> {
        self.state.lock().unwrap().flush(client)
    }

    pub fn display_ptr(&self) -> *mut wl_display {
        self.state.lock().unwrap().display_ptr()
    }
}

pub(crate) trait ErasedState: downcast_rs::Downcast {
    fn object_info(&self, id: InnerObjectId) -> Result<ObjectInfo, InvalidId>;
    fn insert_client(
        &self,
        stream: UnixStream,
        data: Arc<dyn ClientData>,
    ) -> std::io::Result<InnerClientId>;
    fn get_client(&self, id: InnerObjectId) -> Result<ClientId, InvalidId>;
    fn get_client_credentials(&self, id: InnerClientId) -> Result<Credentials, InvalidId>;
    fn get_client_data(&self, id: InnerClientId) -> Result<Arc<dyn ClientData>, InvalidId>;
    fn with_all_clients(&self, f: &mut dyn FnMut(ClientId));
    fn with_all_objects_for(
        &self,
        client_id: InnerClientId,
        f: &mut dyn FnMut(ObjectId),
    ) -> Result<(), InvalidId>;
    fn object_for_protocol_id(
        &self,
        client_id: InnerClientId,
        interface: &'static Interface,
        protocol_id: u32,
    ) -> Result<ObjectId, InvalidId>;
    fn get_object_data_any(
        &self,
        id: InnerObjectId,
    ) -> Result<Arc<dyn std::any::Any + Send + Sync>, InvalidId>;
    fn send_event(&mut self, msg: Message<ObjectId, RawFd>) -> Result<(), InvalidId>;
    fn post_error(&mut self, object_id: InnerObjectId, error_code: u32, message: CString);
    fn kill_client(&mut self, client_id: InnerClientId, reason: DisconnectReason);
    fn global_info(&self, id: InnerGlobalId) -> Result<GlobalInfo, InvalidId>;
    fn is_known_global(&self, global_ptr: *const wl_global) -> bool;
    fn flush(&mut self, client: Option<ClientId>) -> std::io::Result<()>;
    fn display_ptr(&self) -> *mut wl_display;
}

downcast_rs::impl_downcast!(ErasedState);

impl<D: 'static> ErasedState for State<D> {
    fn object_info(&self, id: InnerObjectId) -> Result<ObjectInfo, InvalidId> {
        if !id.alive.load(Ordering::Acquire) {
            return Err(InvalidId);
        }

        let version =
            unsafe { ffi_dispatch!(wayland_server_handle(), wl_resource_get_version, id.ptr) }
                as u32;

        Ok(ObjectInfo { id: id.id, version, interface: id.interface })
    }

    fn insert_client(
        &self,
        stream: UnixStream,
        data: Arc<dyn ClientData>,
    ) -> std::io::Result<InnerClientId> {
        let ret = unsafe {
            ffi_dispatch!(
                wayland_server_handle(),
                wl_client_create,
                self.display,
                stream.into_raw_fd()
            )
        };

        if ret.is_null() {
            return Err(std::io::Error::last_os_error());
        }

        Ok(unsafe { init_client(ret, data) })
    }

    fn get_client(&self, id: InnerObjectId) -> Result<ClientId, InvalidId> {
        if !id.alive.load(Ordering::Acquire) {
            return Err(InvalidId);
        }

        unsafe {
            let client_ptr = ffi_dispatch!(wayland_server_handle(), wl_resource_get_client, id.ptr);
            client_id_from_ptr(client_ptr).ok_or(InvalidId).map(|id| ClientId { id })
        }
    }

    fn get_client_data(&self, id: InnerClientId) -> Result<Arc<dyn ClientData>, InvalidId> {
        if !id.alive.load(Ordering::Acquire) {
            return Err(InvalidId);
        }

        let data = unsafe {
            match client_user_data(id.ptr) {
                Some(ptr) => &mut *ptr,
                None => return Err(InvalidId),
            }
        };

        Ok(data.data.clone())
    }

    fn get_client_credentials(&self, id: InnerClientId) -> Result<Credentials, InvalidId> {
        if !id.alive.load(Ordering::Acquire) {
            return Err(InvalidId);
        }

        let mut creds = Credentials { pid: 0, uid: 0, gid: 0 };

        unsafe {
            ffi_dispatch!(
                wayland_server_handle(),
                wl_client_get_credentials,
                id.ptr,
                &mut creds.pid,
                &mut creds.uid,
                &mut creds.gid
            );
        }

        Ok(creds)
    }

    fn with_all_clients(&self, f: &mut dyn FnMut(ClientId)) {
        let mut client_list = unsafe {
            ffi_dispatch!(wayland_server_handle(), wl_display_get_client_list, self.display)
        };
        unsafe {
            while (*client_list).next != client_list {
                let client =
                    ffi_dispatch!(wayland_server_handle(), wl_client_from_link, client_list);
                if let Some(id) = client_id_from_ptr(client) {
                    f(ClientId { id })
                }

                client_list = (*client_list).next;
            }
        }
    }

    fn with_all_objects_for(
        &self,
        client_id: InnerClientId,
        mut f: &mut dyn FnMut(ObjectId),
    ) -> Result<(), InvalidId> {
        if !client_id.alive.load(Ordering::Acquire) {
            return Err(InvalidId);
        }

        unsafe extern "C" fn iterator_func(
            resource: *mut wl_resource,
            user_data: *mut c_void,
        ) -> c_int {
            // the closure is passed by double-reference because we only have 1 pointer of user data
            let closure = unsafe { &mut *(user_data as *mut &mut dyn FnMut(ObjectId)) };

            // Only process objects that are RUST_MANAGED (we cannot guess the Interface for the others,
            // and thus cannot create the ObjectId).
            if let Ok(id) = unsafe { InnerObjectId::from_ptr(None, resource) } {
                closure(ObjectId { id })
            }

            // return WL_ITERATOR_CONTINUE
            1
        }

        unsafe {
            ffi_dispatch!(
                wayland_server_handle(),
                wl_client_for_each_resource,
                client_id.ptr,
                iterator_func,
                &mut f as *mut _ as *mut c_void,
            )
        }

        Ok(())
    }

    fn object_for_protocol_id(
        &self,
        client_id: InnerClientId,
        interface: &'static Interface,
        protocol_id: u32,
    ) -> Result<ObjectId, InvalidId> {
        if !client_id.alive.load(Ordering::Acquire) {
            return Err(InvalidId);
        }
        let resource = unsafe {
            ffi_dispatch!(wayland_server_handle(), wl_client_get_object, client_id.ptr, protocol_id)
        };
        if resource.is_null() {
            Err(InvalidId)
        } else {
            unsafe { ObjectId::from_ptr(interface, resource) }
        }
    }

    fn get_object_data_any(
        &self,
        id: InnerObjectId,
    ) -> Result<Arc<dyn std::any::Any + Send + Sync>, InvalidId> {
        if !id.alive.load(Ordering::Acquire) {
            return Err(InvalidId);
        }

        let iface_c_ptr =
            id.interface.c_ptr.expect("[wayland-backend-sys] Cannot use Interface without c_ptr!");
        let is_managed = unsafe {
            ffi_dispatch!(
                wayland_server_handle(),
                wl_resource_instance_of,
                id.ptr,
                iface_c_ptr,
                &RUST_MANAGED as *const u8 as *const _
            ) != 0
        };
        if !is_managed {
            return Err(InvalidId);
        }

        let udata = unsafe {
            &*(ffi_dispatch!(wayland_server_handle(), wl_resource_get_user_data, id.ptr)
                as *mut ResourceUserData<D>)
        };

        Ok(udata.data.clone().into_any_arc())
    }

    fn send_event(
        &mut self,
        Message { sender_id: ObjectId { id }, opcode, args }: Message<ObjectId, RawFd>,
    ) -> Result<(), InvalidId> {
        if !id.alive.load(Ordering::Acquire) || id.ptr.is_null() {
            return Err(InvalidId);
        }

        // check that the argument list is valid
        let message_desc = match id.interface.events.get(opcode as usize) {
            Some(msg) => msg,
            None => {
                panic!("Unknown opcode {} for object {}@{}.", opcode, id.interface.name, id.id);
            }
        };
        if !check_for_signature(message_desc.signature, &args) {
            panic!(
                "Unexpected signature for request {}@{}.{}: expected {:?}, got {:?}.",
                id.interface.name, id.id, message_desc.name, message_desc.signature, args
            );
        }

        let mut argument_list = SmallVec::<[wl_argument; 4]>::with_capacity(args.len());
        let mut arg_interfaces = message_desc.arg_interfaces.iter();
        for (i, arg) in args.iter().enumerate() {
            match *arg {
                Argument::Uint(u) => argument_list.push(wl_argument { u }),
                Argument::Int(i) => argument_list.push(wl_argument { i }),
                Argument::Fixed(f) => argument_list.push(wl_argument { f }),
                Argument::Fd(h) => argument_list.push(wl_argument { h }),
                Argument::Array(ref a) => {
                    let a = Box::new(wl_array {
                        size: a.len(),
                        alloc: a.len(),
                        data: a.as_ptr() as *mut _,
                    });
                    argument_list.push(wl_argument { a: Box::into_raw(a) })
                }
                Argument::Str(Some(ref s)) => argument_list.push(wl_argument { s: s.as_ptr() }),
                Argument::Str(None) => argument_list.push(wl_argument { s: std::ptr::null() }),
                Argument::Object(ref o) => {
                    let next_interface = arg_interfaces.next().unwrap();
                    if !o.id.ptr.is_null() {
                        if !o.id.alive.load(Ordering::Acquire) {
                            unsafe { free_arrays(message_desc.signature, &argument_list) };
                            return Err(InvalidId);
                        }
                        // check that the object belongs to the right client
                        if self.get_client(id.clone()).unwrap().id.ptr
                            != self.get_client(o.id.clone()).unwrap().id.ptr
                        {
                            panic!("Attempting to send an event with objects from wrong client.");
                        }
                        if !same_interface(next_interface, o.id.interface) {
                            panic!("Event {}@{}.{} expects an argument of interface {} but {} was provided instead.", id.interface.name, id.id, message_desc.name, next_interface.name, o.id.interface.name);
                        }
                    } else if !matches!(
                        message_desc.signature[i],
                        ArgumentType::Object(AllowNull::Yes)
                    ) {
                        panic!(
                            "Event {}@{}.{} expects an non-null object argument.",
                            id.interface.name, id.id, message_desc.name
                        );
                    }
                    argument_list.push(wl_argument { o: o.id.ptr as *const _ })
                }
                Argument::NewId(ref o) => {
                    if !o.id.ptr.is_null() {
                        if !id.alive.load(Ordering::Acquire) {
                            unsafe { free_arrays(message_desc.signature, &argument_list) };
                            return Err(InvalidId);
                        }
                        // check that the object belongs to the right client
                        if self.get_client(id.clone()).unwrap().id.ptr
                            != self.get_client(o.id.clone()).unwrap().id.ptr
                        {
                            panic!("Attempting to send an event with objects from wrong client.");
                        }
                        let child_interface = match message_desc.child_interface {
                            Some(iface) => iface,
                            None => panic!("Trying to send event {}@{}.{} which creates an object without specifying its interface, this is unsupported.", id.interface.name, id.id, message_desc.name),
                        };
                        if !same_interface(child_interface, o.id.interface) {
                            panic!("Event {}@{}.{} expects an argument of interface {} but {} was provided instead.", id.interface.name, id.id, message_desc.name, child_interface.name, o.id.interface.name);
                        }
                    } else if !matches!(message_desc.signature[i], ArgumentType::NewId) {
                        panic!(
                            "Event {}@{}.{} expects an non-null object argument.",
                            id.interface.name, id.id, message_desc.name
                        );
                    }
                    argument_list.push(wl_argument { o: o.id.ptr as *const _ })
                }
            }
        }

        unsafe {
            ffi_dispatch!(
                wayland_server_handle(),
                wl_resource_post_event_array,
                id.ptr,
                opcode as u32,
                argument_list.as_mut_ptr()
            );
        }

        unsafe {
            free_arrays(message_desc.signature, &argument_list);
        }

        if message_desc.is_destructor {
            // wl_resource_destroy invokes a destructor
            PENDING_DESTRUCTORS.set(
                &(&mut self.pending_destructors as *mut _ as *mut _),
                || unsafe {
                    ffi_dispatch!(wayland_server_handle(), wl_resource_destroy, id.ptr);
                },
            );
        }

        Ok(())
    }

    fn post_error(&mut self, id: InnerObjectId, error_code: u32, message: CString) {
        if !id.alive.load(Ordering::Acquire) {
            return;
        }

        // Safety: at this point we already checked that the pointer is valid
        let client =
            unsafe { ffi_dispatch!(wayland_server_handle(), wl_resource_get_client, id.ptr) };
        let client_id = unsafe { client_id_from_ptr(client) }.unwrap();
        // mark the client as dead
        client_id.alive.store(false, Ordering::Release);

        unsafe {
            ffi_dispatch!(
                wayland_server_handle(),
                wl_resource_post_error,
                id.ptr,
                error_code,
                message.as_ptr()
            )
        }
    }

    fn kill_client(&mut self, client_id: InnerClientId, reason: DisconnectReason) {
        if !client_id.alive.load(Ordering::Acquire) {
            return;
        }
        if let Some(udata) = unsafe { client_user_data(client_id.ptr) } {
            let udata = unsafe { &*udata };
            udata.alive.store(false, Ordering::Release);
            udata.data.disconnected(ClientId { id: client_id.clone() }, reason);
        }

        // wl_client_destroy invokes destructors
        PENDING_DESTRUCTORS.set(&(&mut self.pending_destructors as *mut _ as *mut _), || unsafe {
            ffi_dispatch!(wayland_server_handle(), wl_client_destroy, client_id.ptr);
        });
    }

    fn global_info(&self, id: InnerGlobalId) -> Result<GlobalInfo, InvalidId> {
        if !id.alive.load(Ordering::Acquire) {
            return Err(InvalidId);
        }
        let udata = unsafe {
            &*(ffi_dispatch!(wayland_server_handle(), wl_global_get_user_data, id.ptr)
                as *mut GlobalUserData<D>)
        };

        Ok(GlobalInfo {
            interface: udata.interface,
            version: udata.version,
            disabled: udata.disabled,
        })
    }

    fn is_known_global(&self, global_ptr: *const wl_global) -> bool {
        self.known_globals.iter().any(|ginfo| std::ptr::eq(ginfo.ptr, global_ptr))
    }

    fn flush(&mut self, client: Option<ClientId>) -> std::io::Result<()> {
        if let Some(ClientId { id: client_id }) = client {
            if client_id.alive.load(Ordering::Acquire) {
                unsafe { ffi_dispatch!(wayland_server_handle(), wl_client_flush, client_id.ptr) }
            }
        } else {
            // wl_display_flush_clients might invoke destructors
            PENDING_DESTRUCTORS.set(
                &(&mut self.pending_destructors as *mut _ as *mut _),
                || unsafe {
                    ffi_dispatch!(wayland_server_handle(), wl_display_flush_clients, self.display);
                },
            );
        }
        if !self.pending_destructors.is_empty() {
            // Arm the timer to trigger a wakeup of the inner event loop in 1ms, so that the user
            // is indicated to call dispatch_clients() and have the destructors run
            unsafe {
                ffi_dispatch!(
                    wayland_server_handle(),
                    wl_event_source_timer_update,
                    self.timer_source,
                    1
                )
            };
        }
        Ok(())
    }

    fn display_ptr(&self) -> *mut wl_display {
        self.display
    }
}

unsafe fn init_client(client: *mut wl_client, data: Arc<dyn ClientData>) -> InnerClientId {
    let alive = Arc::new(AtomicBool::new(true));
    let client_data = Box::into_raw(Box::new(ClientUserData { alive: alive.clone(), data }));

    let listener = signal::rust_listener_create(client_destroy_notify);
    // Safety: we just created listener and client_data, they are valid
    unsafe {
        signal::rust_listener_set_user_data(listener, client_data as *mut c_void);
    }

    ffi_dispatch!(wayland_server_handle(), wl_client_add_destroy_listener, client, listener);

    InnerClientId { ptr: client, alive }
}

unsafe fn client_id_from_ptr(client: *mut wl_client) -> Option<InnerClientId> {
    // Safety: the provided pointer is a valid and initialized wl_client for type parameter D
    unsafe {
        client_user_data(client)
            .map(|udata| InnerClientId { ptr: client, alive: (*udata).alive.clone() })
    }
}

unsafe fn client_user_data(client: *mut wl_client) -> Option<*mut ClientUserData> {
    if client.is_null() {
        return None;
    }
    let listener = ffi_dispatch!(
        wayland_server_handle(),
        wl_client_get_destroy_listener,
        client,
        client_destroy_notify
    );
    if !listener.is_null() {
        // Safety: the pointer we got must be valid if the client is still alive
        unsafe { Some(signal::rust_listener_get_user_data(listener) as *mut ClientUserData) }
    } else {
        None
    }
}

unsafe extern "C" fn client_destroy_notify(listener: *mut wl_listener, client_ptr: *mut c_void) {
    // Safety: if this function is invoked by libwayland its arguments must be valid
    let data = unsafe {
        Box::from_raw(signal::rust_listener_get_user_data(listener) as *mut ClientUserData)
    };
    unsafe {
        signal::rust_listener_destroy(listener);
    }
    // only notify the killing if it was not already
    if data.alive.load(Ordering::Acquire) {
        data.alive.store(false, Ordering::Release);
        data.data.disconnected(
            ClientId {
                id: InnerClientId { ptr: client_ptr as *mut wl_client, alive: data.alive.clone() },
            },
            DisconnectReason::ConnectionClosed,
        );
    }
}

unsafe extern "C" fn global_bind<D: 'static>(
    client: *mut wl_client,
    data: *mut c_void,
    version: u32,
    id: u32,
) {
    // Safety: when this function is invoked, the data pointer provided by libwayland is the data we previously put there
    let global_udata = unsafe { &mut *(data as *mut GlobalUserData<D>) };

    let global_id = InnerGlobalId { alive: global_udata.alive.clone(), ptr: global_udata.ptr };

    // Safety: libwayland invoked us with a valid wl_client
    let client_id = match unsafe { client_id_from_ptr(client) } {
        Some(id) => id,
        None => return,
    };

    // this must be Some(), checked at creation of the global
    let interface_ptr = global_udata.interface.c_ptr.unwrap();

    HANDLE.with(|&(ref state_arc, data_ptr)| {
        // Safety: the data_ptr is a valid pointer that live outside code put there
        let data = unsafe { &mut *(data_ptr as *mut D) };
        // create the object
        let resource = ffi_dispatch!(
            wayland_server_handle(),
            wl_resource_create,
            client,
            interface_ptr,
            version as i32,
            id
        );
        // Safety: resource was just created, it must be valid
        let (object_id, udata) = unsafe { init_resource(resource, global_udata.interface, None) };
        let obj_data = global_udata.handler.clone().bind(
            &Handle { handle: InnerHandle { state: state_arc.clone() } },
            data,
            ClientId { id: client_id },
            GlobalId { id: global_id },
            ObjectId { id: object_id },
        );
        // Safety: udata was just created, it is valid
        unsafe { (*udata).data = obj_data };
    })
}

unsafe extern "C" fn global_filter<D: 'static>(
    client: *const wl_client,
    global: *const wl_global,
    _: *mut c_void,
) -> bool {
    if !HANDLE.is_set() {
        // This happens if we are invoked during the global shutdown
        // at this point, we really don't care what we send to clients,
        // the compositor is shutting down anyway so they're all going to be killed
        // thus return false, so that nothing is sent to anybody
        return false;
    }

    // Safety: skip processing globals that do not belong to us
    let is_known_global = HANDLE.with(|(state_arc, _)| {
        let guard = state_arc.lock().unwrap();
        guard.is_known_global(global)
    });
    if !is_known_global {
        return true;
    }

    // Safety: if we are invoked here, the client is a valid client initialized by us
    let client_udata = match unsafe { client_user_data(client as *mut _) } {
        Some(id) => unsafe { &*id },
        None => return false,
    };

    let client_id = InnerClientId { ptr: client as *mut _, alive: client_udata.alive.clone() };

    // Safety: if we are invoked here, the global is a global client initialized by us
    let global_udata = unsafe {
        &*(ffi_dispatch!(wayland_server_handle(), wl_global_get_user_data, global)
            as *mut GlobalUserData<D>)
    };

    let global_id =
        InnerGlobalId { ptr: global as *mut wl_global, alive: global_udata.alive.clone() };

    global_udata.handler.can_view(
        ClientId { id: client_id },
        &client_udata.data,
        GlobalId { id: global_id },
    )
}

unsafe fn init_resource<D: 'static>(
    resource: *mut wl_resource,
    interface: &'static Interface,
    data: Option<Arc<dyn ObjectData<D>>>,
) -> (InnerObjectId, *mut ResourceUserData<D>) {
    let alive = Arc::new(AtomicBool::new(true));
    let udata = Box::into_raw(Box::new(ResourceUserData {
        data: data.unwrap_or_else(|| Arc::new(UninitObjectData)),
        interface,
        alive: alive.clone(),
    }));
    let id = ffi_dispatch!(wayland_server_handle(), wl_resource_get_id, resource);

    ffi_dispatch!(
        wayland_server_handle(),
        wl_resource_set_dispatcher,
        resource,
        resource_dispatcher::<D>,
        &RUST_MANAGED as *const u8 as *const c_void,
        udata as *mut c_void,
        Some(resource_destructor::<D>)
    );

    (InnerObjectId { interface, alive, id, ptr: resource }, udata)
}

unsafe extern "C" fn resource_dispatcher<D: 'static>(
    _: *const c_void,
    resource: *mut c_void,
    opcode: u32,
    _: *const wl_message,
    args: *const wl_argument,
) -> i32 {
    let resource = resource as *mut wl_resource;
    let udata_ptr = ffi_dispatch!(wayland_server_handle(), wl_resource_get_user_data, resource)
        as *mut ResourceUserData<D>;
    // Safety: if we are invoked, the resource is valid and rust-managed, so its user data is valid
    let udata = unsafe { &mut *udata_ptr };
    let client = ffi_dispatch!(wayland_server_handle(), wl_resource_get_client, resource);
    let resource_id = ffi_dispatch!(wayland_server_handle(), wl_resource_get_id, resource);
    let version = ffi_dispatch!(wayland_server_handle(), wl_resource_get_version, resource);
    let interface = udata.interface;
    let message_desc = match interface.requests.get(opcode as usize) {
        Some(desc) => desc,
        None => {
            crate::log_error!("Unknown event opcode {} for interface {}.", opcode, interface.name);
            return -1;
        }
    };

    let mut parsed_args =
        SmallVec::<[Argument<ObjectId, OwnedFd>; 4]>::with_capacity(message_desc.signature.len());
    let mut arg_interfaces = message_desc.arg_interfaces.iter().copied();
    let mut created = None;
    // Safety (args deference): the args array provided by libwayland is well-formed
    for (i, typ) in message_desc.signature.iter().enumerate() {
        match typ {
            ArgumentType::Uint => parsed_args.push(Argument::Uint(unsafe { (*args.add(i)).u })),
            ArgumentType::Int => parsed_args.push(Argument::Int(unsafe { (*args.add(i)).i })),
            ArgumentType::Fixed => parsed_args.push(Argument::Fixed(unsafe { (*args.add(i)).f })),
            ArgumentType::Fd => {
                parsed_args.push(Argument::Fd(unsafe { OwnedFd::from_raw_fd((*args.add(i)).h) }))
            }
            ArgumentType::Array => {
                let array = unsafe { &*((*args.add(i)).a) };
                // Safety: the wl_array provided by libwayland is valid
                let content =
                    unsafe { std::slice::from_raw_parts(array.data as *mut u8, array.size) };
                parsed_args.push(Argument::Array(Box::new(content.into())));
            }
            ArgumentType::Str(_) => {
                let ptr = unsafe { (*args.add(i)).s };
                // Safety: the c-string provided by libwayland is valid
                if !ptr.is_null() {
                    let cstr = unsafe { std::ffi::CStr::from_ptr(ptr) };
                    parsed_args.push(Argument::Str(Some(Box::new(cstr.into()))));
                } else {
                    parsed_args.push(Argument::Str(None));
                }
            }
            ArgumentType::Object(_) => {
                let obj = unsafe { (*args.add(i)).o as *mut wl_resource };
                let next_interface = arg_interfaces.next().unwrap_or(&ANONYMOUS_INTERFACE);
                let id = if !obj.is_null() {
                    ObjectId {
                        id: unsafe { InnerObjectId::from_ptr(Some(next_interface), obj) }
                            .expect("Received an invalid ID when parsing a message."),
                    }
                } else {
                    // libwayland-server.so checks nulls for us
                    InnerHandle::null_id()
                };
                parsed_args.push(Argument::Object(id))
            }
            ArgumentType::NewId => {
                let new_id = unsafe { (*args.add(i)).n };
                // create the object
                if new_id != 0 {
                    let child_interface = match message_desc.child_interface {
                        Some(iface) => iface,
                        None => panic!("Received request {}@{}.{} which creates an object without specifying its interface, this is unsupported.", udata.interface.name, resource_id, message_desc.name),
                    };
                    // create the object
                    let resource = ffi_dispatch!(
                        wayland_server_handle(),
                        wl_resource_create,
                        client,
                        child_interface.c_ptr.unwrap(),
                        version,
                        new_id
                    );
                    // Safety: the resource has just been created and is valid
                    let (child_id, child_data_ptr) =
                        unsafe { init_resource::<D>(resource, child_interface, None) };
                    created = Some((child_id.clone(), child_data_ptr));
                    parsed_args.push(Argument::NewId(ObjectId { id: child_id }));
                } else {
                    parsed_args.push(Argument::NewId(InnerHandle::null_id()))
                }
            }
        }
    }

    let object_id = ObjectId {
        id: InnerObjectId {
            ptr: resource,
            id: resource_id,
            interface: udata.interface,
            alive: udata.alive.clone(),
        },
    };

    // Safety: the client ptr is valid and provided by libwayland
    let client_id = unsafe { client_id_from_ptr(client) }.unwrap();

    let ret = HANDLE.with(|&(ref state_arc, data_ptr)| {
        // Safety: the data pointer has been set by outside code and is valid
        let data = unsafe { &mut *(data_ptr as *mut D) };
        udata.data.clone().request(
            &Handle { handle: InnerHandle { state: state_arc.clone() } },
            data,
            ClientId { id: client_id.clone() },
            Message { sender_id: object_id.clone(), opcode: opcode as u16, args: parsed_args },
        )
    });

    if message_desc.is_destructor {
        ffi_dispatch!(wayland_server_handle(), wl_resource_destroy, resource);
    }

    match (created, ret) {
        (Some((_, child_udata_ptr)), Some(child_data)) => unsafe {
            (*child_udata_ptr).data = child_data;
        },
        (Some((child_id, _)), None) => {
            // Accept a missing object data if a protocol error occurred (and the object is already dead)
            if client_id.alive.load(Ordering::Acquire) {
                panic!("Callback creating object {child_id} did not provide any object data.");
            }
        }
        (None, Some(_)) => {
            panic!("An object data was returned from a callback not creating any object");
        }
        (None, None) => {}
    }

    0
}

unsafe extern "C" fn resource_destructor<D: 'static>(resource: *mut wl_resource) {
    // Safety: if this destructor is called resource is valid and initialized by us
    let udata = unsafe {
        Box::from_raw(ffi_dispatch!(wayland_server_handle(), wl_resource_get_user_data, resource)
            as *mut ResourceUserData<D>)
    };
    let id = ffi_dispatch!(wayland_server_handle(), wl_resource_get_id, resource);
    let client = ffi_dispatch!(wayland_server_handle(), wl_resource_get_client, resource);
    // if this destructor is invoked during cleanup, the client ptr is no longer valid and it'll return None
    let client_id = unsafe { client_id_from_ptr(client) }.unwrap_or(InnerClientId {
        ptr: std::ptr::null_mut(),
        alive: Arc::new(AtomicBool::new(false)),
    });
    udata.alive.store(false, Ordering::Release);
    let object_id =
        InnerObjectId { interface: udata.interface, ptr: resource, alive: udata.alive.clone(), id };
    // Due to reentrancy, it is possible that both HANDLE and PENDING_DESTRUCTORS are set at the same time
    // If this is the case, PENDING_DESTRUCTORS should have priority
    if !PENDING_DESTRUCTORS.is_set() {
        HANDLE.with(|&(ref state_arc, data_ptr)| {
            // Safety: the data pointer have been set by outside code and are valid
            let data = unsafe { &mut *(data_ptr as *mut D) };
            udata.data.destroyed(
                &Handle { handle: InnerHandle { state: state_arc.clone() } },
                data,
                ClientId { id: client_id },
                ObjectId { id: object_id },
            );
        });
    } else {
        PENDING_DESTRUCTORS.with(|&pending_ptr| {
            // Safety: the pending pointer have been set by outside code and are valid
            let pending = unsafe { &mut *(pending_ptr as *mut Vec<PendingDestructor<D>>) };
            pending.push((
                udata.data.clone(),
                ClientId { id: client_id },
                ObjectId { id: object_id },
            ));
        })
    }
}

#[cfg(feature = "log")]
extern "C" {
    fn wl_log_trampoline_to_rust_server(fmt: *const std::os::raw::c_char, list: *const c_void);
}

struct UninitObjectData;

impl<D> ObjectData<D> for UninitObjectData {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn request(
        self: Arc<Self>,
        _: &Handle,
        _: &mut D,
        _: ClientId,
        msg: Message<ObjectId, OwnedFd>,
    ) -> Option<Arc<dyn ObjectData<D>>> {
        panic!("Received a message on an uninitialized object: {msg:?}");
    }

    #[cfg_attr(unstable_coverage, coverage(off))]
    fn destroyed(self: Arc<Self>, _: &Handle, _: &mut D, _: ClientId, _: ObjectId) {}

    #[cfg_attr(unstable_coverage, coverage(off))]
    fn debug(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UninitObjectData").finish()
    }
}
