use std::{
    ffi::{CStr, CString},
    sync::Arc,
};

use crate::protocol::{Argument, Interface};
use crate::types::server::{GlobalInfo, InvalidId};

use super::{
    client::{Client, ClientStore},
    ClientId, GlobalHandler, GlobalId, InnerGlobalId, InnerObjectId, ObjectId,
};

/*
    GlobalId.id is the global protocol name (starting at 1), hence
    we must subtract 1 to it before indexing the vec
*/

#[derive(Debug)]
struct Global<D: 'static> {
    id: InnerGlobalId,
    interface: &'static Interface,
    version: u32,
    handler: Arc<dyn GlobalHandler<D>>,
    disabled: bool,
}

#[derive(Debug)]

pub struct Registry<D: 'static> {
    globals: Vec<Option<Global<D>>>,
    known_registries: Vec<InnerObjectId>,
    last_serial: u32,
}

impl<D> Registry<D> {
    pub(crate) fn new() -> Self {
        Self { globals: Vec::new(), known_registries: Vec::new(), last_serial: 0 }
    }

    fn next_serial(&mut self) -> u32 {
        self.last_serial = self.last_serial.wrapping_add(1);
        self.last_serial
    }

    pub(crate) fn create_global(
        &mut self,
        interface: &'static Interface,
        version: u32,
        handler: Arc<dyn GlobalHandler<D>>,
        clients: &mut ClientStore<D>,
    ) -> InnerGlobalId {
        if version > interface.version {
            panic!(
                "Cannot create global {} version {}: maximum supported version is {}",
                interface.name, version, interface.version
            );
        }
        let serial = self.next_serial();
        let (id, place) = match self.globals.iter_mut().enumerate().find(|(_, g)| g.is_none()) {
            Some((id, place)) => (id, place),
            None => {
                self.globals.push(None);
                (self.globals.len() - 1, self.globals.last_mut().unwrap())
            }
        };

        let id = InnerGlobalId { id: id as u32 + 1, serial };

        *place = Some(Global { id: id.clone(), interface, version, handler, disabled: false });

        self.send_global_to_all(id.clone(), clients).unwrap();

        id
    }

    fn get_global(&self, id: InnerGlobalId) -> Result<&Global<D>, InvalidId> {
        self.globals
            .get(id.id as usize - 1)
            .and_then(|o| o.as_ref())
            .filter(|o| o.id == id)
            .ok_or(InvalidId)
    }

    pub(crate) fn get_info(&self, id: InnerGlobalId) -> Result<GlobalInfo, InvalidId> {
        let global = self.get_global(id)?;
        Ok(GlobalInfo {
            interface: global.interface,
            version: global.version,
            disabled: global.disabled,
        })
    }

    pub(crate) fn get_handler(
        &self,
        id: InnerGlobalId,
    ) -> Result<Arc<dyn GlobalHandler<D>>, InvalidId> {
        let global = self.get_global(id)?;
        Ok(global.handler.clone())
    }

    pub(crate) fn check_bind(
        &self,
        client: &Client<D>,
        name: u32,
        interface_name: &CStr,
        version: u32,
    ) -> Option<(&'static Interface, InnerGlobalId, Arc<dyn GlobalHandler<D>>)> {
        if name == 0 || version == 0 {
            return None;
        }
        let target_global = self.globals.get((name - 1) as usize).and_then(|o| o.as_ref())?;
        if target_global.interface.name.as_bytes() != interface_name.to_bytes() {
            return None;
        }
        if target_global.version < version {
            return None;
        }
        if !target_global.handler.can_view(
            ClientId { id: client.id.clone() },
            &client.data,
            GlobalId { id: target_global.id.clone() },
        ) {
            return None;
        }

        Some((target_global.interface, target_global.id.clone(), target_global.handler.clone()))
    }

    pub(crate) fn cleanup(&mut self, dead_clients: &[Client<D>]) {
        self.known_registries
            .retain(|obj_id| !dead_clients.iter().any(|cid| cid.id == obj_id.client_id))
    }

    pub(crate) fn disable_global(&mut self, id: InnerGlobalId, clients: &mut ClientStore<D>) {
        let global = match self.globals.get_mut(id.id as usize - 1) {
            Some(&mut Some(ref mut g)) if g.id == id => g,
            _ => return,
        };

        // Do nothing if the global is already disabled
        if !global.disabled {
            global.disabled = true;
            // send the global_remove
            for registry in self.known_registries.iter().cloned() {
                if let Ok(client) = clients.get_client_mut(registry.client_id.clone()) {
                    let _ =
                        send_global_remove_to(client, global, ObjectId { id: registry.clone() });
                }
            }
        }
    }

    pub(crate) fn remove_global(&mut self, id: InnerGlobalId, clients: &mut ClientStore<D>) {
        // disable the global if not already disabled
        self.disable_global(id.clone(), clients);
        // now remove it if the id is still valid
        if let Some(place) = self.globals.get_mut(id.id as usize - 1) {
            if place.as_ref().map(|g| g.id == id).unwrap_or(false) {
                *place = None;
            }
        }
    }

    pub(crate) fn new_registry(
        &mut self,
        registry: InnerObjectId,
        client: &mut Client<D>,
    ) -> Result<(), InvalidId> {
        self.send_all_globals_to(registry.clone(), client)?;
        self.known_registries.push(registry);
        Ok(())
    }

    pub(crate) fn send_all_globals_to(
        &self,
        registry: InnerObjectId,
        client: &mut Client<D>,
    ) -> Result<(), InvalidId> {
        for global in self.globals.iter().flat_map(|opt| opt.as_ref()) {
            if !global.disabled
                && global.handler.can_view(
                    ClientId { id: client.id.clone() },
                    &client.data,
                    GlobalId { id: global.id.clone() },
                )
            {
                // fail the whole send on error, there is no point in trying further on a failing client
                send_global_to(client, global, ObjectId { id: registry.clone() })?;
            }
        }
        Ok(())
    }

    pub(crate) fn send_global_to_all(
        &self,
        global_id: InnerGlobalId,
        clients: &mut ClientStore<D>,
    ) -> Result<(), InvalidId> {
        let global = self.get_global(global_id)?;
        if global.disabled {
            return Err(InvalidId);
        }
        for registry in self.known_registries.iter().cloned() {
            if let Ok(client) = clients.get_client_mut(registry.client_id.clone()) {
                if !global.disabled
                    && global.handler.can_view(
                        ClientId { id: client.id.clone() },
                        &client.data,
                        GlobalId { id: global.id.clone() },
                    )
                {
                    // don't fail the whole send for a single erroring client
                    let _ = send_global_to(client, global, ObjectId { id: registry.clone() });
                }
            }
        }
        Ok(())
    }
}

#[inline]
fn send_global_to<D>(
    client: &mut Client<D>,
    global: &Global<D>,
    registry: ObjectId,
) -> Result<(), InvalidId> {
    client.send_event(
        message!(
            registry,
            0, // wl_registry.global
            [
                Argument::Uint(global.id.id),
                Argument::Str(Some(Box::new(CString::new(global.interface.name).unwrap()))),
                Argument::Uint(global.version),
            ],
        ),
        // This is not a destructor event
        None,
    )
}

#[inline]
fn send_global_remove_to<D>(
    client: &mut Client<D>,
    global: &Global<D>,
    registry: ObjectId,
) -> Result<(), InvalidId> {
    client.send_event(
        message!(
            registry,
            1, // wl_registry.global_remove
            [Argument::Uint(global.id.id)],
        ),
        // This is not a destructor event
        None,
    )
}
