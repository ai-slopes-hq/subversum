//! Audio engine: command/telemetry protocol, `Engine`, and a probe world.
pub mod amber;
pub mod engine;
pub mod macros;
pub mod melody;
pub mod patch;
pub mod peaks;
pub mod probe;
pub mod protocol;

pub use engine::Engine;
pub use probe::ProbeWorld;
pub use protocol::*;
