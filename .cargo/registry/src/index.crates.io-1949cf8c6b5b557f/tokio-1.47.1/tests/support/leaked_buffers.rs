/// Can create buffers of arbitrary lifetime.
/// Frees created buffers when dropped.
///
/// This struct is of course unsafe and the fact that
/// it must outlive the created slices has to be ensured by
/// the programmer.
///
/// Used at certain test scenarios as a safer version of
/// Vec::leak, to satisfy the address sanitizer.
pub struct LeakedBuffers {
    leaked_vecs: Vec<Box<[u8]>>,
}

impl LeakedBuffers {
    pub fn new() -> Self {
        Self {
            leaked_vecs: vec![],
        }
    }
    pub unsafe fn create<'a>(&mut self, size: usize) -> &'a mut [u8] {
        let new_mem = vec![0u8; size].into_boxed_slice();
        self.leaked_vecs.push(new_mem);
        let new_mem = self.leaked_vecs.last_mut().unwrap();
        std::slice::from_raw_parts_mut(new_mem.as_mut_ptr(), new_mem.len())
    }
}
