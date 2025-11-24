use crate::{tags::IfdPointer, TiffError, TiffFormatError};
use std::collections::HashMap;

/// The IFD structure of a TIFF file should be limited to a forest of entries, and a tree when we
/// only consider having a primary IFD and its children. There is up to one primary child of a node
/// that is the `next` entry in an IFD itself. Since we offer iteration over this chain we want to
/// detect when a new edge would introduce a cycle, such that callers can be guaranteed to
/// terminate iteration at some point even in malicious images.
///
/// However, we are not necessarily visiting the IFDs in their topological order.
///
/// Since we only consider one child, we run union find to assign each pointer to a known chain.
/// Then, when we insert a new edge we check if they belong to the same component.
#[derive(Default, Debug)]
pub struct IfdCycles {
    /// The root of each component union.
    component_union: HashMap<ComponentId, ComponentId>,
    links: HashMap<IfdPointer, u64>,
    chains: HashMap<IfdPointer, ComponentId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct ComponentId(u64);

impl IfdCycles {
    pub fn new() -> Self {
        IfdCycles::default()
    }

    pub fn insert_next(
        &mut self,
        from: IfdPointer,
        to: Option<IfdPointer>,
    ) -> Result<bool, TiffError> {
        let to_offset = to.map_or(0, |p| p.0);

        // For some reason we got
        match self.links.get(&from) {
            Some(existing) if *existing == to_offset => return Ok(false),
            // We got here twice with two different reads of the same IFD. That is .. unusual and
            // we interpret it as a cycle.
            Some(_) => return Err(TiffError::FormatError(TiffFormatError::CycleInOffsets)),
            None => self.links.insert(from, to_offset),
        };

        self.ensure_node(from);

        if let Some(to) = to {
            self.ensure_node(to);

            let parent = self.nominal_component(from);
            let child = self.nominal_component(to);

            if parent == child {
                return Err(TiffError::FormatError(TiffFormatError::CycleInOffsets));
            }

            self.component_union.insert(child, parent);
        }

        Ok(true)
    }

    fn ensure_node(&mut self, ifd: IfdPointer) {
        self.chains.entry(ifd).or_insert_with(|| {
            let id = ComponentId(self.component_union.len() as u64);
            self.component_union.insert(id, id);
            id
        });
    }

    fn nominal_component(&mut self, node: IfdPointer) -> ComponentId {
        let id = self.chains[&node];

        let nomimal = {
            let mut iter = id;

            loop {
                let parent = self.component_union[&iter];

                if parent == iter {
                    break parent;
                }

                iter = parent;
            }
        };

        if nomimal != id {
            // Compress.
            let mut iter = id;

            loop {
                let parent = self.component_union[&iter];

                if parent == iter {
                    break;
                }

                self.component_union.insert(iter, nomimal);
                iter = parent;
            }
        }

        nomimal
    }
}

#[test]
fn cycles_are_detected() {
    let mut cycles = IfdCycles::new();

    cycles
        .insert_next(IfdPointer(0x20), Some(IfdPointer(0x800)))
        .expect("non-existing link is valid");

    cycles
        .insert_next(IfdPointer(0x800), Some(IfdPointer(0x20)))
        .expect_err("cycle must be detected");
}

#[test]
fn reflective_cycle() {
    let mut cycles = IfdCycles::new();

    cycles
        .insert_next(IfdPointer(0x20), Some(IfdPointer(0x20)))
        .expect_err("self-referential cycle must be detected");
}

#[test]
fn late_cycle() {
    let mut cycles = IfdCycles::new();

    cycles
        .insert_next(IfdPointer(0x20), Some(IfdPointer(0x40)))
        .expect("non-existing link is valid");

    cycles
        .insert_next(IfdPointer(0x60), Some(IfdPointer(0x80)))
        .expect("non-existing link is valid");
    cycles
        .insert_next(IfdPointer(0x80), Some(IfdPointer(0x20)))
        .expect("non-existing link is valid");

    cycles
        .insert_next(IfdPointer(0x40), Some(IfdPointer(0x60)))
        .expect_err("non-existing link is valid");
}

#[test]
fn odd_cycle() {
    let mut cycles = IfdCycles::new();

    cycles
        .insert_next(IfdPointer(0x20), Some(IfdPointer(0x40)))
        .expect("non-existing link is valid");

    cycles
        .insert_next(IfdPointer(0x60), Some(IfdPointer(0x80)))
        .expect("non-existing link is valid");
    cycles
        .insert_next(IfdPointer(0x80), Some(IfdPointer(0x20)))
        .expect("non-existing link is valid");

    cycles
        .insert_next(IfdPointer(0x40), Some(IfdPointer(0x80)))
        .expect_err("non-existing link is valid");
}
