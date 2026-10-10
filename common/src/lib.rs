//! Shared vocabulary between server and client: the wire protocol and the
//! deterministic world generator.
//!
//! Both sides generate the *same* world from the same seed, so the initial
//! transfer is just the seed. Everything after that is a delta.

pub mod protocol;
pub mod world;

pub use protocol::*;
