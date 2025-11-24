use super::Header;

use fnv::FnvHasher;
use http::header;
use http::method::Method;

use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::{cmp, mem};

/// HPACK encoder table
#[derive(Debug)]
pub struct Table {
    mask: usize,
    indices: Vec<Option<Pos>>,
    slots: VecDeque<Slot>,
    inserted: usize,
    // Size is in bytes
    size: usize,
    max_size: usize,
}

#[derive(Debug)]
pub enum Index {
    // The header is already fully indexed
    Indexed(usize, Header),

    // The name is indexed, but not the value
    Name(usize, Header),

    // The full header has been inserted into the table.
    Inserted(usize),

    // Only the value has been inserted (hpack table idx, slots idx)
    InsertedValue(usize, usize),

    // The header is not indexed by this table
    NotIndexed(Header),
}

#[derive(Debug)]
struct Slot {
    hash: HashValue,
    header: Header,
    next: Option<usize>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct Pos {
    index: usize,
    hash: HashValue,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct HashValue(usize);

const MAX_SIZE: usize = 1 << 16;
const DYN_OFFSET: usize = 62;

macro_rules! probe_loop {
    ($probe_var: ident < $len: expr, $body: expr) => {
        debug_assert!($len > 0);
        loop {
            if $probe_var < $len {
                $body
                $probe_var += 1;
            } else {
                $probe_var = 0;
            }
        }
    };
}

impl Table {
    pub fn new(max_size: usize, capacity: usize) -> Table {
        if capacity == 0 {
            Table {
                mask: 0,
                indices: vec![],
                slots: VecDeque::new(),
                inserted: 0,
                size: 0,
                max_size,
            }
        } else {
            let capacity = cmp::max(to_raw_capacity(capacity).next_power_of_two(), 8);

            Table {
                mask: capacity.wrapping_sub(1),
                indices: vec![None; capacity],
                slots: VecDeque::with_capacity(usable_capacity(capacity)),
                inserted: 0,
                size: 0,
                max_size,
            }
        }
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        usable_capacity(self.indices.len())
    }

    pub fn max_size(&self) -> usize {
        self.max_size
    }

    /// Gets the header stored in the table
    pub fn resolve<'a>(&'a self, index: &'a Index) -> &'a Header {
        use self::Index::*;

        match *index {
            Indexed(_, ref h) => h,
            Name(_, ref h) => h,
            Inserted(idx) => &self.slots[idx].header,
            InsertedValue(_, idx) => &self.slots[idx].header,
            NotIndexed(ref h) => h,
        }
    }

    pub fn resolve_idx(&self, index: &Index) -> usize {
        use self::Index::*;

        match *index {
            Indexed(idx, ..) => idx,
            Name(idx, ..) => idx,
            Inserted(idx) => idx + DYN_OFFSET,
            InsertedValue(_name_idx, slot_idx) => slot_idx + DYN_OFFSET,
            NotIndexed(_) => panic!("cannot resolve index"),
        }
    }

    /// Index the header in the HPACK table.
    pub fn index(&mut self, header: Header) -> Index {
        // Check the static table
        let statik = index_static(&header);

        // Don't index certain headers. This logic is borrowed from nghttp2.
        if header.skip_value_index() {
            // Right now, if this is true, the header name is always in the
            // static table. At some point in the future, this might not be true
            // and this logic will need to be updated.
            debug_assert!(statik.is_some(), "skip_value_index requires a static name",);
            return Index::new(statik, header);
        }

        // If the header is already indexed by the static table, return that
        if let Some((n, true)) = statik {
            return Index::Indexed(n, header);
        }

        // Don't index large headers
        if header.len() * 4 > self.max_size * 3 {
            return Index::new(statik, header);
        }

        self.index_dynamic(header, statik)
    }

    fn index_dynamic(&mut self, header: Header, statik: Option<(usize, bool)>) -> Index {
        debug_assert!(self.assert_valid_state("one"));

        if header.len() + self.size < self.max_size || !header.is_sensitive() {
            // Only grow internal storage if needed
            self.reserve_one();
        }

        if self.indices.is_empty() {
            // If `indices` is not empty, then it is impossible for all
            // `indices` entries to be `Some`. So, we only need to check for the
            // empty case.
            return Index::new(statik, header);
        }

        let hash = hash_header(&header);

        let desired_pos = desired_pos(self.mask, hash);
        let mut probe = desired_pos;
        let mut dist = 0;

        // Start at the ideal position, checking all slots
        probe_loop!(probe < self.indices.len(), {
            if let Some(pos) = self.indices[probe] {
                // The slot is already occupied, but check if it has a lower
                // displacement.
                let their_dist = probe_distance(self.mask, pos.hash, probe);

                let slot_idx = pos.index.wrapping_add(self.inserted);

                if their_dist < dist {
                    // Index robinhood
                    return self.index_vacant(header, hash, dist, probe, statik);
                } else if pos.hash == hash && self.slots[slot_idx].header.name() == header.name() {
                    // Matching name, check values
                    return self.index_occupied(header, hash, pos.index, statik.map(|(n, _)| n));
                }
            } else {
                return self.index_vacant(header, hash, dist, probe, statik);
            }

            dist += 1;
        });
    }

    fn index_occupied(
        &mut self,
        header: Header,
        hash: HashValue,
        mut index: usize,
        statik: Option<usize>,
    ) -> Index {
        debug_assert!(self.assert_valid_state("top"));

        // There already is a match for the given header name. Check if a value
        // matches. The header will also only be inserted if the table is not at
        // capacity.
        loop {
            // Compute the real index into the VecDeque
            let real_idx = index.wrapping_add(self.inserted);

            if self.slots[real_idx].header.value_eq(&header) {
                // We have a full match!
                return Index::Indexed(real_idx + DYN_OFFSET, header);
            }

            if let Some(next) = self.slots[real_idx].next {
                index = next;
                continue;
            }

            if header.is_sensitive() {
                // Should we assert this?
                // debug_assert!(statik.is_none());
                return Index::Name(real_idx + DYN_OFFSET, header);
            }

            self.update_size(header.len(), Some(index));

            // Insert the new header
            self.insert(header, hash);

            // Recompute real_idx as it just changed.
            let new_real_idx = index.wrapping_add(self.inserted);

            // The previous node in the linked list may have gotten evicted
            // while making room for this header.
            if new_real_idx < self.slots.len() {
                let idx = 0usize.wrapping_sub(self.inserted);

                self.slots[new_real_idx].next = Some(idx);
            }

            debug_assert!(self.assert_valid_state("bottom"));

            // Even if the previous header was evicted, we can still reference
            // it when inserting the new one...
            return if let Some(n) = statik {
                // If name is in static table, use it instead
                Index::InsertedValue(n, 0)
            } else {
                Index::InsertedValue(real_idx + DYN_OFFSET, 0)
            };
        }
    }

    fn index_vacant(
        &mut self,
        header: Header,
        hash: HashValue,
        mut dist: usize,
        mut probe: usize,
        statik: Option<(usize, bool)>,
    ) -> Index {
        if header.is_sensitive() {
            return Index::new(statik, header);
        }

        debug_assert!(self.assert_valid_state("top"));
        debug_assert!(dist == 0 || self.indices[probe.wrapping_sub(1) & self.mask].is_some());

        // Passing in `usize::MAX` for prev_idx since there is no previous
        // header in this case.
        if self.update_size(header.len(), None) {
            while dist != 0 {
                let back = probe.wrapping_sub(1) & self.mask;

                if let Some(pos) = self.indices[back] {
                    let their_dist = probe_distance(self.mask, pos.hash, back);

                    if their_dist < (dist - 1) {
                        probe = back;
                        dist -= 1;
                    } else {
                        break;
                    }
                } else {
                    probe = back;
                    dist -= 1;
                }
            }
        }

        debug_assert!(self.assert_valid_state("after update"));

        self.insert(header, hash);

        let pos_idx = 0usize.wrapping_sub(self.inserted);

        let prev = mem::replace(
            &mut self.indices[probe],
            Some(Pos {
                index: pos_idx,
                hash,
            }),
        );

        if let Some(mut prev) = prev {
            // Shift forward
            let mut probe = probe + 1;

            probe_loop!(probe < self.indices.len(), {
                let pos = &mut self.indices[probe];

                prev = match mem::replace(pos, Some(prev)) {
                    Some(p) => p,
                    None => break,
                };
            });
        }

        debug_assert!(self.assert_valid_state("bottom"));

        if let Some((n, _)) = statik {
            Index::InsertedValue(n, 0)
        } else {
            Index::Inserted(0)
        }
    }

    fn insert(&mut self, header: Header, hash: HashValue) {
        self.inserted = self.inserted.wrapping_add(1);

        self.slots.push_front(Slot {
            hash,
            header,
            next: None,
        });
    }

    pub fn resize(&mut self, size: usize) {
        self.max_size = size;

        if size == 0 {
            self.size = 0;

            for i in &mut self.indices {
                *i = None;
            }

            self.slots.clear();
            self.inserted = 0;
        } else {
            self.converge(None);
        }
    }

    fn update_size(&mut self, len: usize, prev_idx: Option<usize>) -> bool {
        self.size += len;
        self.converge(prev_idx)
    }

    fn converge(&mut self, prev_idx: Option<usize>) -> bool {
        let mut ret = false;

        while self.size > self.max_size {
            ret = true;
            self.evict(prev_idx);
        }

        ret
    }

    fn evict(&mut self, prev_idx: Option<usize>) {
        let pos_idx = (self.slots.len() - 1).wrapping_sub(self.inserted);

        debug_assert!(!self.slots.is_empty());
        debug_assert!(self.assert_valid_state("one"));

        // Remove the header
        let slot = self.slots.pop_back().unwrap();
        let mut probe = desired_pos(self.mask, slot.hash);

        // Update the size
        self.size -= slot.header.len();

        debug_assert_eq!(
            self.indices
                .iter()
                .filter_map(|p| *p)
                .filter(|p| p.index == pos_idx)
                .count(),
            1
        );

        // Find the associated position
        probe_loop!(probe < self.indices.len(), {
            debug_assert!(self.indices[probe].is_some());

            let mut pos = self.indices[probe].unwrap();

            if pos.index == pos_idx {
                if let Some(idx) = slot.next {
                    pos.index = idx;
                    self.indices[probe] = Some(pos);
                } else if Some(pos.index) == prev_idx {
                    pos.index = 0usize.wrapping_sub(self.inserted + 1);
                    self.indices[probe] = Some(pos);
                } else {
                    self.indices[probe] = None;
                    self.remove_phase_two(probe);
                }

                break;
            }
        });

        debug_assert!(self.assert_valid_state("two"));
    }

    // Shifts all indices that were displaced by the header that has just been
    // removed.
    fn remove_phase_two(&mut self, probe: usize) {
        let mut last_probe = probe;
        let mut probe = probe + 1;

        probe_loop!(probe < self.indices.len(), {
            if let Some(pos) = self.indices[probe] {
                if probe_distance(self.mask, pos.hash, probe) > 0 {
                    self.indices[last_probe] = self.indices[probe].take();
                } else {
                    break;
                }
            } else {
                break;
            }

            last_probe = probe;
        });

        debug_assert!(self.assert_valid_state("two"));
    }

    fn reserve_one(&mut self) {
        let len = self.slots.len();

        if len == self.capacity() {
            if len == 0 {
                let new_raw_cap = 8;
                self.mask = 8 - 1;
                self.indices = vec![None; new_raw_cap];
            } else {
                let raw_cap = self.indices.len();
                self.grow(raw_cap << 1);
            }
        }
    }

    #[inline]
    fn grow(&mut self, new_raw_cap: usize) {
        // This path can never be reached when handling the first allocation in
        // the map.

        debug_assert!(self.assert_valid_state("top"));

        // find first ideally placed element -- start of cluster
        let mut first_ideal = 0;

        for (i, pos) in self.indices.iter().enumerate() {
            if let Some(pos) = *pos {
                if 0 == probe_distance(self.mask, pos.hash, i) {
                    first_ideal = i;
                    break;
                }
            }
        }

        // visit the entries in an order where we can simply reinsert them
        // into self.indices without any bucket stealing.
        let old_indices = mem::replace(&mut self.indices, vec![None; new_raw_cap]);
        self.mask = new_raw_cap.wrapping_sub(1);

        for &pos in &old_indices[first_ideal..] {
            self.reinsert_entry_in_order(pos);
        }

        for &pos in &old_indices[..first_ideal] {
            self.reinsert_entry_in_order(pos);
        }

        debug_assert!(self.assert_valid_state("bottom"));
    }

    fn reinsert_entry_in_order(&mut self, pos: Option<Pos>) {
        if let Some(pos) = pos {
            // Find first empty bucket and insert there
            let mut probe = desired_pos(self.mask, pos.hash);

            probe_loop!(probe < self.indices.len(), {
                if self.indices[probe].is_none() {
                    // empty bucket, insert here
                    self.indices[probe] = Some(pos);
                    return;
                }

                debug_assert!({
                    let them = self.indices[probe].unwrap();
                    let their_distance = probe_distance(self.mask, them.hash, probe);
                    let our_distance = probe_distance(self.mask, pos.hash, probe);

                    their_distance >= our_distance
                });
            });
        }
    }

    #[cfg(not(test))]
    fn assert_valid_state(&self, _: &'static str) -> bool {
        true
    }

    #[cfg(test)]
    fn assert_valid_state(&self, _msg: &'static str) -> bool {
        /*
            // Checks that the internal map structure is valid
            //
            // Ensure all hash codes in indices match the associated slot
            for pos in &self.indices {
                if let Some(pos) = *pos {
                    let real_idx = pos.index.wrapping_add(self.inserted);

                    if real_idx.wrapping_add(1) != 0 {
                        assert!(real_idx < self.slots.len(),
                                "out of index; real={}; len={}, msg={}",
                                real_idx, self.slots.len(), msg);

                        assert_eq!(pos.hash, self.slots[real_idx].hash,
                                   "index hash does not match slot; msg={}", msg);
                    }
                }
            }

            // Every index is only available once
            for i in 0..self.indices.len() {
                if self.indices[i].is_none() {
                    continue;
                }

                for j in i+1..self.indices.len() {
                    assert_ne!(self.indices[i], self.indices[j],
                                "duplicate indices; msg={}", msg);
                }
            }

            for (index, slot) in self.slots.iter().enumerate() {
                let mut indexed = None;

                // First, see if the slot is indexed
                for (i, pos) in self.indices.iter().enumerate() {
                    if let Some(pos) = *pos {
                        let real_idx = pos.index.wrapping_add(self.inserted);
                        if real_idx == index {
                            indexed = Some(i);
                            // Already know that there is no dup, so break
                            break;
                        }
                    }
                }

                if let Some(actual) = indexed {
                    // Ensure that it is accessible..
                    let desired = desired_pos(self.mask, slot.hash);
                    let mut probe = desired;
                    let mut dist = 0;

                    probe_loop!(probe < self.indices.len(), {
                        assert!(self.indices[probe].is_some(),
                                "unexpected empty slot; probe={}; hash={:?}; msg={}",
                                probe, slot.hash, msg);

                        let pos = self.indices[probe].unwrap();

                        let their_dist = probe_distance(self.mask, pos.hash, probe);
                        let real_idx = pos.index.wrapping_add(self.inserted);

                        if real_idx == index {
                            break;
                        }

                        assert!(dist <= their_dist,
                                "could not find entry; actual={}; desired={}" +
                                "probe={}, dist={}; their_dist={}; index={}; msg={}",
                                actual, desired, probe, dist, their_dist,
                                index.wrapping_sub(self.inserted), msg);

                        dist += 1;
                    });
                } else {
                    // There is exactly one next link
                    let cnt = self.slots.iter().map(|s| s.next)
                        .filter(|n| *n == Some(index.wrapping_sub(self.inserted)))
                        .count();

                    assert_eq!(1, cnt, "more than one node pointing here; msg={}", msg);
                }
            }
        */

        // TODO: Ensure linked lists are correct: no cycles, etc...

        true
    }
}

#[cfg(test)]
impl Table {
    /// Returns the number of headers in the table
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Returns the table size
    pub fn size(&self) -> usize {
        self.size
    }
}

impl Index {
    fn new(v: Option<(usize, bool)>, e: Header) -> Index {
        match v {
            None => Index::NotIndexed(e),
            Some((n, true)) => Index::Indexed(n, e),
            Some((n, false)) => Index::Name(n, e),
        }
    }
}

#[inline]
fn usable_capacity(cap: usize) -> usize {
    cap - cap / 4
}

#[inline]
fn to_raw_capacity(n: usize) -> usize {
    n + n / 3
}

#[inline]
fn desired_pos(mask: usize, hash: HashValue) -> usize {
    hash.0 & mask
}

#[inline]
fn probe_distance(mask: usize, hash: HashValue, current: usize) -> usize {
    current.wrapping_sub(desired_pos(mask, hash)) & mask
}

fn hash_header(header: &Header) -> HashValue {
    const MASK: u64 = (MAX_SIZE as u64) - 1;

    let mut h = FnvHasher::default();
    header.name().hash(&mut h);
    HashValue((h.finish() & MASK) as usize)
}

/// Checks the static table for the header. If found, returns the index and a
/// boolean representing if the value matched as well.
fn index_static(header: &Header) -> Option<(usize, bool)> {
    match *header {
        Header::Field {
            ref name,
            ref value,
        } => match *name {
            header::ACCEPT_CHARSET => Some((15, false)),
            header::ACCEPT_ENCODING => {
                if value == "gzip, deflate" {
                    Some((16, true))
                } else {
                    Some((16, false))
                }
            }
            header::ACCEPT_LANGUAGE => Some((17, false)),
            header::ACCEPT_RANGES => Some((18, false)),
            header::ACCEPT => Some((19, false)),
            header::ACCESS_CONTROL_ALLOW_ORIGIN => Some((20, false)),
            header::AGE => Some((21, false)),
            header::ALLOW => Some((22, false)),
            header::AUTHORIZATION => Some((23, false)),
            header::CACHE_CONTROL => Some((24, false)),
            header::CONTENT_DISPOSITION => Some((25, false)),
            header::CONTENT_ENCODING => Some((26, false)),
            header::CONTENT_LANGUAGE => Some((27, false)),
            header::CONTENT_LENGTH => Some((28, false)),
            header::CONTENT_LOCATION => Some((29, false)),
            header::CONTENT_RANGE => Some((30, false)),
            header::CONTENT_TYPE => Some((31, false)),
            header::COOKIE => Some((32, false)),
            header::DATE => Some((33, false)),
            header::ETAG => Some((34, false)),
            header::EXPECT => Some((35, false)),
            header::EXPIRES => Some((36, false)),
            header::FROM => Some((37, false)),
            header::HOST => Some((38, false)),
            header::IF_MATCH => Some((39, false)),
            header::IF_MODIFIED_SINCE => Some((40, false)),
            header::IF_NONE_MATCH => Some((41, false)),
            header::IF_RANGE => Some((42, false)),
            header::IF_UNMODIFIED_SINCE => Some((43, false)),
            header::LAST_MODIFIED => Some((44, false)),
            header::LINK => Some((45, false)),
            header::LOCATION => Some((46, false)),
            header::MAX_FORWARDS => Some((47, false)),
            header::PROXY_AUTHENTICATE => Some((48, false)),
            header::PROXY_AUTHORIZATION => Some((49, false)),
            header::RANGE => Some((50, false)),
            header::REFERER => Some((51, false)),
            header::REFRESH => Some((52, false)),
            header::RETRY_AFTER => Some((53, false)),
            header::SERVER => Some((54, false)),
            header::SET_COOKIE => Some((55, false)),
            header::STRICT_TRANSPORT_SECURITY => Some((56, false)),
            header::TRANSFER_ENCODING => Some((57, false)),
            header::USER_AGENT => Some((58, false)),
            header::VARY => Some((59, false)),
            header::VIA => Some((60, false)),
            header::WWW_AUTHENTICATE => Some((61, false)),
            _ => None,
        },
        Header::Authority(_) => Some((1, false)),
        Header::Method(ref v) => match *v {
            Method::GET => Some((2, true)),
            Method::POST => Some((3, true)),
            _ => Some((2, false)),
        },
        Header::Scheme(ref v) => match &**v {
            "http" => Some((6, true)),
            "https" => Some((7, true)),
            _ => Some((6, false)),
        },
        Header::Path(ref v) => match &**v {
            "/" => Some((4, true)),
            "/index.html" => Some((5, true)),
            _ => Some((4, false)),
        },
        Header::Protocol(..) => None,
        Header::Status(ref v) => match u16::from(*v) {
            200 => Some((8, true)),
            204 => Some((9, true)),
            206 => Some((10, true)),
            304 => Some((11, true)),
            400 => Some((12, true)),
            404 => Some((13, true)),
            500 => Some((14, true)),
            _ => Some((8, false)),
        },
    }
}
