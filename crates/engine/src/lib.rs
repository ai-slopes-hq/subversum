//! Audio engine: command/telemetry protocol, `Engine`, and a probe world.
pub mod engine;
pub mod patch;
pub mod peaks;
pub mod probe;
pub mod protocol;

pub use engine::Engine;
pub use probe::ProbeWorld;
pub use protocol::*;
