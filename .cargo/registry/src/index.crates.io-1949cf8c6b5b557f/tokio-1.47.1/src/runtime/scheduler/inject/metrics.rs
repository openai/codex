use super::Inject;

impl<T: 'static> Inject<T> {
    pub(crate) fn len(&self) -> usize {
        self.shared.len()
    }
}
