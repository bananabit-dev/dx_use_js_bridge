// Core re-exports that are always available
pub use serde::{Deserialize, Serialize};

// JS Bridge functionality - only available with js-bridge feature
#[cfg(feature = "js-bridge")]
pub mod js_bridge;

#[cfg(feature = "js-bridge")]
pub use js_bridge::*;