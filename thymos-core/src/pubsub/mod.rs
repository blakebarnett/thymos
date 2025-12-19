//! Pub/Sub system for agent coordination
//!
//! This module provides a unified pub/sub abstraction that supports:
//! - Local-only pub/sub using AutoAgents runtime (fast, in-process)
//! - Distributed pub/sub using SurrealDB live queries (multi-process, persistent)
//! - Hybrid mode combining both for best performance and persistence

mod builder;
mod error;
mod local;
mod message;
mod traits;

#[cfg(feature = "pubsub-distributed")]
mod distributed;

#[cfg(feature = "pubsub-distributed")]
mod hybrid;

pub use builder::{PubSubBuilder, PubSubInstance, PubSubMode};
pub use error::PubSubError;
pub use local::LocalPubSub;
pub use message::PubSubMessage;
pub use traits::{PubSub, PubSubBackend, SubscriptionHandle};

#[cfg(feature = "pubsub-distributed")]
pub use distributed::DistributedPubSub;

#[cfg(feature = "pubsub-distributed")]
pub use hybrid::HybridPubSub;

