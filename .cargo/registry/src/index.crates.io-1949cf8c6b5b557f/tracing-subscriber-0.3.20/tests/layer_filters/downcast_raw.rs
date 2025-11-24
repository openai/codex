use tracing::Subscriber;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::prelude::*;
use tracing_subscriber::Layer;

#[test]
fn downcast_ref_to_inner_layer_and_filter() {
    // Test that a filtered layer gives downcast_ref access to
    // both the layer and the filter.

    struct WrappedLayer;

    impl<S> Layer<S> for WrappedLayer
    where
        S: Subscriber,
        S: for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
    {
    }

    let layer = WrappedLayer;
    let filter = Targets::new().with_default(tracing::Level::INFO);
    let registry = tracing_subscriber::registry().with(layer.with_filter(filter));
    let dispatch = tracing::dispatcher::Dispatch::new(registry);

    // The filter is available
    assert!(dispatch.downcast_ref::<Targets>().is_some());
    // The wrapped layer is available
    assert!(dispatch.downcast_ref::<WrappedLayer>().is_some());
}

#[test]
fn forward_downcast_raw_to_layer() {
    // Test that a filtered layer still gives its wrapped layer a chance to
    // return a custom struct from downcast_raw.
    // https://github.com/tokio-rs/tracing/issues/1618

    struct WrappedLayer {
        with_context: WithContext,
    }

    struct WithContext;

    impl<S> Layer<S> for WrappedLayer
    where
        S: Subscriber,
        S: for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
    {
        unsafe fn downcast_raw(&self, id: std::any::TypeId) -> Option<*const ()> {
            match id {
                id if id == std::any::TypeId::of::<Self>() => Some(self as *const _ as *const ()),
                id if id == std::any::TypeId::of::<WithContext>() => {
                    Some(&self.with_context as *const _ as *const ())
                }
                _ => None,
            }
        }
    }

    let layer = WrappedLayer {
        with_context: WithContext,
    };
    let filter = Targets::new().with_default(tracing::Level::INFO);
    let registry = tracing_subscriber::registry().with(layer.with_filter(filter));
    let dispatch = tracing::dispatcher::Dispatch::new(registry);

    // Types from a custom implementation of `downcast_raw` are available
    assert!(dispatch.downcast_ref::<WithContext>().is_some());
}
