//! Per-process context threaded as `&Context` through every command handler.
//! Holds the env-derived [`Config`](super::config::Config).

/// The env-derived runtime context, threaded through every command handler.
pub struct Context {
    /// The env-derived runtime config.
    pub config: super::config::Config,
}
