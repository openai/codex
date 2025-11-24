pub mod debouncer_full;
pub mod debouncer_mini;
pub mod event;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_impl() {
        macro_rules! assert_debug_impl {
            ($t:ty) => {{
                trait NeedsDebug: std::fmt::Debug {}
                impl NeedsDebug for $t {}
            }};
        }

        assert_debug_impl!(event::AccessKind);
        assert_debug_impl!(event::AccessMode);
        assert_debug_impl!(event::CreateKind);
        assert_debug_impl!(event::DataChange);
        assert_debug_impl!(event::EventAttributes);
        assert_debug_impl!(event::Flag);
        assert_debug_impl!(event::MetadataKind);
        assert_debug_impl!(event::ModifyKind);
        assert_debug_impl!(event::RemoveKind);
        assert_debug_impl!(event::RenameMode);
        assert_debug_impl!(event::Event);
        assert_debug_impl!(event::EventKind);
        assert_debug_impl!(debouncer_mini::DebouncedEvent);
        assert_debug_impl!(debouncer_mini::DebouncedEventKind);
        assert_debug_impl!(debouncer_full::DebouncedEvent);
    }
}
