//! Per-process context threaded as `&Context` through every command handler.
//! Holds the env-derived [`Config`](super::config::Config).

/// The env-derived runtime context, threaded through every command handler.
pub struct Context {
    /// The env-derived runtime config.
    pub config: super::config::Config,
}

impl Context {
    /// Build the context from the process environment. Loads the config,
    /// which panics on missing required vars (see [`super::config::load_config`]).
    pub fn new() -> Self {
        Self {
            config: super::config::load_config(),
        }
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}
