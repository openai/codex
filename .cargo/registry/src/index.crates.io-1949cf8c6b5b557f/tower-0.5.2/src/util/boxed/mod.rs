mod layer;
mod layer_clone;
mod layer_clone_sync;
mod sync;
mod unsync;

#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::{
    layer::BoxLayer, layer_clone::BoxCloneServiceLayer, layer_clone_sync::BoxCloneSyncServiceLayer,
    sync::BoxService, unsync::UnsyncBoxService,
};
