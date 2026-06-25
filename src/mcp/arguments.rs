//! Per-session arguments: which toolsets the agent has enabled. Carried in
//! the `x-objectiveai-arguments` request header as serialized JSON.

use rmcp::model::Extensions;
use serde::{Deserialize, Serialize};

/// The request header carrying the serialized [`Arguments`].
pub const HEADER: &str = "x-objectiveai-arguments";

/// The toolsets enabled for a session. Every field is required — there is no
/// default; the caller must specify each toolset explicitly.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Arguments {
    pub tasks: bool,
    pub multi: bool,
    pub python: bool,
    pub objectiveai: bool,
}

impl Arguments {
    /// Extract and deserialize the [`Arguments`] from the
    /// `x-objectiveai-arguments` header on the rmcp request extensions (the
    /// streamable-HTTP transport injects [`http::request::Parts`]). Returns
    /// `None` if the header is absent, non-UTF-8, or not valid `Arguments` JSON.
    pub fn extract(extensions: &Extensions) -> Option<Self> {
        let parts = extensions.get::<http::request::Parts>()?;
        let raw = parts.headers.get(HEADER)?.to_str().ok()?;
        serde_json::from_str(raw).ok()
    }
}
