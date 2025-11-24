//! Generic client implementation.
//!
//! This module contains the low level components to build a gRPC client. It
//! provides a codec agnostic gRPC client dispatcher and a decorated tower
//! service trait.
//!
//! This client is generally used by some code generation tool to provide stubs
//! for the gRPC service. Thusly, they are a bit cumbersome to use by hand.
//!
//! ## Concurrent usage
//!
//! Upon using the your generated client, you will discover all the functions
//! corresponding to your rpc methods take `&mut self`, making concurrent
//! usage of the client difficult. The answer is simply to clone the client,
//! which is cheap as all client instances will share the same channel for
//! communication. For more details, see
//! [transport::Channel](../transport/struct.Channel.html#multiplexing-requests).

mod grpc;
mod service;

pub use self::grpc::Grpc;
pub use self::service::GrpcService;
