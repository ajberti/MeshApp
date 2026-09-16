//! Native binding boundary.
//!
//! The public UniFFI API will be added after the core protocol types settle.
//! Keeping this crate in the workspace now fixes the intended dependency
//! direction: Android/iOS -> mesh-bindings -> mesh-core.

pub use mesh_core::{CoreAction, CoreEvent, MeshConfig, MeshCore};
