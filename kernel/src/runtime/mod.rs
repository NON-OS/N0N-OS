//! NØNOS Runtime Subsystem
//!
//! Manages the zero-state execution environment for capsules and kernel services.

pub mod zerostate;
pub mod capsule;
pub mod isolation;

pub use zerostate::{init_zerostate, track_active_sandbox, ZeroStateConfig};
pub use capsule::{CapsuleRuntime, CapsuleId};
pub use isolation::{IsolationBoundary, SecurityPerimeter};

/// Initialize the entire runtime subsystem
pub fn init() {
    zerostate::init_zerostate();
    capsule::init_capsule_runtime();
    isolation::init_boundaries();
    
    log::info!("[RUNTIME] Zero-state runtime initialized");
}
