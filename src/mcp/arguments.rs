//! Per-session arguments: which toolsets the agent has enabled. Carried in
//! the session's request arguments and deserialized when a session is minted.

use serde::{Deserialize, Serialize};

/// The toolsets enabled for a session. Every field is required — there is no
/// default; the caller must specify each toolset explicitly.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Arguments {
    pub tasks: bool,
    pub multi: bool,
    pub python: bool,
    pub objectiveai: bool,
}
