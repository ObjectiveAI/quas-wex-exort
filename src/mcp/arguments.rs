//! Per-session arguments: which toolsets the agent has enabled. Carried in
//! the `x-objectiveai-arguments` request header as serialized JSON.

use rmcp::model::Extensions;
use serde::{Deserialize, Deserializer, Serialize};

/// The request header carrying the serialized [`Arguments`].
pub const HEADER: &str = "x-objectiveai-arguments";

/// The toolsets enabled for a session. Every field is required — there is no
/// default; the caller must specify each toolset explicitly.
///
/// The host sends the values as JSON **strings** (`{"tasks":"true",…}`), so the
/// bools deserialize leniently from either a real bool or a `"true"`/`"false"`
/// string.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Arguments {
    #[serde(deserialize_with = "de_bool")]
    pub tasks: bool,
    #[serde(deserialize_with = "de_bool")]
    pub multi: bool,
    #[serde(deserialize_with = "de_bool")]
    pub python: bool,
    #[serde(deserialize_with = "de_bool")]
    pub objectiveai: bool,
}

/// Accept a JSON bool or a `"true"`/`"false"` (or `"1"`/`"0"`) string.
fn de_bool<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum BoolOrStr {
        Bool(bool),
        Str(String),
    }
    match BoolOrStr::deserialize(d)? {
        BoolOrStr::Bool(b) => Ok(b),
        BoolOrStr::Str(s) => match s.trim().to_ascii_lowercase().as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" | "" => Ok(false),
            other => Err(serde::de::Error::custom(format!("invalid boolean: {other:?}"))),
        },
    }
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
